use crate::error::Error;
use crate::limits;
use crate::runtime::utils::is_blank;
use crate::types::Value;
use std::borrow::Cow;

/// Get the byte offset corresponding to a character index, without collecting into Vec<char>.
#[inline]
fn char_byte_offset(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(i, _)| i)
        .unwrap_or(s.len())
}

/// Count the number of characters without collecting into Vec<char>.
#[inline]
fn char_count(s: &str) -> usize {
    s.chars().count()
}

/// Write the `CONCAT` (and `&`) of `args` to `w`. Excel turns every value into text: strings as
/// is, numbers and currency in Skillet's number format (`1.5::Currency` → `1.5`), booleans as
/// `TRUE`/`FALSE`, a DateTime as its `::String` text (Unix seconds, the way Excel writes a date's
/// serial number), arrays flattened. NULL (a blank cell) and JSON objects add nothing.
fn concat_into(w: &mut impl std::fmt::Write, args: &[Value]) {
    for v in args {
        let _ = match v {
            Value::String(st) => w.write_str(st),
            Value::Number(n) | Value::Currency(n) => write!(w, "{}", n),
            Value::DateTime(ts) => write!(w, "{}", ts),
            Value::Array(arr) => {
                concat_into(w, arr);
                Ok(())
            }
            Value::Boolean(b) => w.write_str(if *b { "TRUE" } else { "FALSE" }),
            Value::Null | Value::Json(_) => Ok(()),
        };
    }
}

/// A scalar as Excel's text functions see it: text as is, numbers (and currency) in Skillet's
/// number format, booleans as `TRUE`/`FALSE`, NULL (an empty cell) as `""`. Arrays, dates and
/// JSON objects have no text form here (`None`).
fn excel_text(v: &Value) -> Option<Cow<'_, str>> {
    match v {
        Value::String(s) => Some(Cow::Borrowed(s)),
        Value::Number(n) | Value::Currency(n) => Some(Cow::Owned(n.to_string())),
        Value::Boolean(b) => Some(Cow::Borrowed(if *b { "TRUE" } else { "FALSE" })),
        Value::Null => Some(Cow::Borrowed("")),
        Value::Array(_) | Value::DateTime(_) | Value::Json(_) => None,
    }
}

/// Flatten `v` into `out` as text, for `TEXTJOIN`.
fn textjoin_parts<'v>(v: &'v Value, out: &mut Vec<Cow<'v, str>>) -> Result<(), Error> {
    match v {
        Value::Array(items) => items.iter().try_for_each(|it| textjoin_parts(it, out)),
        _ => {
            let text = excel_text(v).ok_or_else(|| {
                Error::new("TEXTJOIN expects text, numbers, booleans or arrays", None)
            })?;
            limits::tick_item(out.len())?;
            out.push(text);
            Ok(())
        }
    }
}

/// Excel `TEXTJOIN(delimiter, ignore_empty, text1, ...)`. Arrays are flattened. With
/// `ignore_empty` TRUE, empty strings and NULLs are skipped. An array delimiter is used in turn
/// between items, as in Excel. The output is charged before it is built (ADR-0011).
fn textjoin(args: &[Value]) -> Result<Value, Error> {
    let [delimiter, ignore_empty, texts @ ..] = args else {
        return Err(Error::new(
            "TEXTJOIN expects delimiter, ignore_empty, text1, [text2], ...",
            None,
        ));
    };
    if texts.is_empty() {
        return Err(Error::new(
            "TEXTJOIN expects delimiter, ignore_empty, text1, [text2], ...",
            None,
        ));
    }
    let mut delimiters = Vec::new();
    textjoin_parts(delimiter, &mut delimiters)?;
    if delimiters.is_empty() {
        delimiters.push(Cow::Borrowed(""));
    }
    let ignore_empty = match ignore_empty {
        Value::Boolean(b) => *b,
        Value::Number(n) => *n != 0.0,
        _ => {
            return Err(Error::new(
                "TEXTJOIN ignore_empty must be TRUE or FALSE",
                None,
            ))
        }
    };
    let mut parts = Vec::new();
    for t in texts {
        textjoin_parts(t, &mut parts)?;
    }
    if ignore_empty {
        parts.retain(|p| !p.is_empty());
    }
    let seps = parts.len().saturating_sub(1);
    let total = parts
        .iter()
        .map(|p| p.len())
        .chain(delimiters.iter().cycle().take(seps).map(|d| d.len()))
        .fold(0usize, usize::saturating_add);
    limits::charge_string(total)?;
    let mut out = String::with_capacity(total);
    for (i, part) in parts.iter().enumerate() {
        limits::tick_item(i)?;
        if i > 0 {
            out.push_str(&delimiters[(i - 1) % delimiters.len()]);
        }
        out.push_str(part);
    }
    Ok(Value::String(out))
}

/// Excel `FIND(find_text, within_text, [start_num])`: the 1-based character position of the
/// first case-sensitive match at or after `start_num` (default 1; truncated). Not found, or
/// `start_num` below 1 or past the end of `within_text`, is an error (Excel `#VALUE!`). An empty
/// `find_text` matches at `start_num`. `FIND(array, lambda)` is the evaluator's array search.
pub(crate) fn find_text(args: &[Value]) -> Result<Value, Error> {
    let (needle, haystack) = match args {
        [Value::String(n), h] | [Value::String(n), h, _] => (n, h),
        _ => {
            return Err(Error::new(
                "FIND expects find_text, within_text, [start_num]",
                None,
            ))
        }
    };
    let haystack =
        excel_text(haystack).ok_or_else(|| Error::new("FIND expects within_text as text", None))?;
    let start = match args.get(2) {
        None | Some(Value::Null) => 1.0,
        Some(Value::Number(n)) => n.trunc(),
        Some(_) => return Err(Error::new("FIND start_num must be a number", None)),
    };
    let len = char_count(&haystack);
    // NaN fails both comparisons, so it is rejected too.
    if !(start >= 1.0 && start <= len as f64) {
        return Err(Error::new(
            "FIND start_num must be between 1 and the length of within_text",
            None,
        ));
    }
    let from = char_byte_offset(&haystack, start as usize - 1);
    match haystack[from..].find(needle.as_str()) {
        Some(byte) => {
            let pos = start as usize + char_count(&haystack[from..from + byte]);
            Ok(Value::Number(pos as f64))
        }
        None => Err(Error::new("FIND: find_text not found in within_text", None)),
    }
}

/// Collect split parts, charging `48 + part.len` for each one while collecting (ADR-0011).
pub(crate) fn split_charged<'s>(parts: impl Iterator<Item = &'s str>) -> Result<Value, Error> {
    let mut out = Vec::new();
    for p in parts {
        limits::charge_string(p.len())?;
        out.push(Value::String(p.to_string()));
    }
    Ok(Value::Array(out))
}

pub fn exec_string(name: &str, args: &[Value]) -> Result<Value, Error> {
    match name {
        "SUBSTITUTE" => substitute(args),
        "SUBSTITUTEM" => {
            // SUBSTITUTEM(text, substr, replacement) - replace all occurrences (alias of SUBSTITUTE)
            match (args.first(), args.get(1), args.get(2)) {
                (
                    Some(Value::String(text)),
                    Some(Value::String(substr)),
                    Some(Value::String(repl)),
                ) => {
                    limits::charge_replace(text, substr, repl)?;
                    Ok(Value::String(text.replace(substr, repl)))
                }
                _ => Err(Error::new(
                    "SUBSTITUTEM expects (text: string, substr: string, replacement: string)",
                    None,
                )),
            }
        }
        "LEFT" | "RIGHT" => left_right(name, args),
        "MID" => mid(args),
        "LENGTH" => match args.first() {
            Some(Value::Array(items)) => Ok(Value::Number(items.len() as f64)),
            Some(Value::String(s)) => Ok(Value::Number(s.chars().count() as f64)),
            Some(Value::Null) => Ok(Value::Number(0.0)),
            Some(_) | None => Err(Error::new("LENGTH expects array or string", None)),
        },
        "CONCAT" => {
            // Count the exact output length first, then build it (ADR-0011 amendment 2).
            limits::charge_formatted(|c| concat_into(c, args))?;
            let mut out = String::new();
            concat_into(&mut out, args);
            Ok(Value::String(out))
        }
        "UPPER" => match args.first() {
            Some(Value::String(s)) => Ok(Value::String(limits::case_mapped(s, true)?)),
            _ => Err(Error::new("UPPER expects string", None)),
        },
        "LOWER" => match args.first() {
            Some(Value::String(s)) => Ok(Value::String(limits::case_mapped(s, false)?)),
            _ => Err(Error::new("LOWER expects string", None)),
        },
        // Excel TRIM: drop leading and trailing spaces and collapse inner runs of spaces to one.
        // Only the space character (U+0020) counts, as in Excel. The `.trim()` method keeps
        // Ruby semantics (all whitespace at both ends, inner runs kept).
        "TRIM" => match args.first() {
            Some(Value::String(s)) => Ok(Value::String(
                s.split(' ')
                    .filter(|w| !w.is_empty())
                    .collect::<Vec<_>>()
                    .join(" "),
            )),
            _ => Err(Error::new("TRIM expects string", None)),
        },
        "SUBSTRING" => {
            if args.len() < 2 {
                return Err(Error::new(
                    "SUBSTRING expects string, start, [length]",
                    None,
                ));
            }
            let string = match args.first() {
                Some(Value::String(s)) => s,
                _ => {
                    return Err(Error::new(
                        "SUBSTRING expects string as first argument",
                        None,
                    ))
                }
            };
            let start = match args.get(1) {
                Some(Value::Number(n)) => *n as usize,
                _ => {
                    return Err(Error::new(
                        "SUBSTRING expects number as second argument",
                        None,
                    ))
                }
            };

            let string_len = char_count(string);

            let end = if let Some(Value::Number(len)) = args.get(2) {
                let length = *len as usize;
                start.saturating_add(length).min(string_len)
            } else {
                string_len
            };

            let start = start.min(string_len);
            let end = end.max(start);

            if start >= string_len {
                Ok(Value::String(String::new()))
            } else {
                let byte_start = char_byte_offset(string, start);
                let byte_end = char_byte_offset(string, end);
                Ok(Value::String(string[byte_start..byte_end].to_string()))
            }
        }
        "SPLIT" => match (args.first(), args.get(1)) {
            (Some(Value::String(s)), Some(Value::String(sep))) => {
                split_charged(s.split(sep.as_str()))
            }
            (Some(Value::String(s)), None) => split_charged(s.split(',').map(str::trim)),
            _ => Err(Error::new("SPLIT expects string, [separator]", None)),
        },
        "REPLACE" => replace(args),
        "ISBLANK" => match args.first() {
            Some(v) => Ok(Value::Boolean(is_blank(v))),
            None => Ok(Value::Boolean(true)),
        },
        "ISNUMBER" => Ok(Value::Boolean(matches!(
            args.first(),
            Some(Value::Number(_) | Value::Currency(_))
        ))),
        "ISTEXT" => Ok(Value::Boolean(matches!(
            args.first(),
            Some(Value::String(_))
        ))),
        "INCLUDES" => {
            // INCLUDES(string, substring) -> boolean
            if args.len() != 2 {
                return Err(Error::new("INCLUDES expects string, substring", None));
            }
            match (args.first(), args.get(1)) {
                (Some(Value::String(s)), Some(Value::String(substring))) => {
                    Ok(Value::Boolean(s.contains(substring)))
                }
                (Some(Value::String(_)), Some(_)) => Err(Error::new(
                    "INCLUDES expects string as second argument",
                    None,
                )),
                (Some(_), Some(_)) => Err(Error::new(
                    "INCLUDES expects string as first argument",
                    None,
                )),
                _ => Err(Error::new("INCLUDES expects string, substring", None)),
            }
        }
        "LEN" => match args {
            [v] => {
                let text = excel_text(v).ok_or_else(|| Error::new("LEN expects text", None))?;
                Ok(Value::Number(char_count(&text) as f64))
            }
            _ => Err(Error::new("LEN expects 1 argument: text", None)),
        },
        "TEXTJOIN" => textjoin(args),
        _ => Err(Error::new(
            format!("Unknown string function: {}", name),
            None,
        )),
    }
}

/// Excel's `num_chars`: a number of characters, truncated. Negative or not a finite number is
/// Excel's `#VALUE!`. A length past the end of the text is clamped by the caller.
fn count_arg(func: &str, v: &Value) -> Result<usize, Error> {
    match v {
        Value::Number(n) if n.is_finite() && *n >= 0.0 => Ok(n.trunc() as usize),
        Value::Number(_) => Err(Error::new(
            format!("{func} num_chars must not be negative (#VALUE!)"),
            None,
        )),
        _ => Err(Error::new(
            format!("{func} expects number as num_chars"),
            None,
        )),
    }
}

/// Excel's 1-based `start_num`, truncated, as a 0-based index. Below 1 is Excel's `#VALUE!`.
fn start_arg(func: &str, v: &Value) -> Result<usize, Error> {
    match v {
        Value::Number(n) if n.is_finite() && n.trunc() >= 1.0 => Ok(n.trunc() as usize - 1),
        Value::Number(_) => Err(Error::new(
            format!("{func} start_num must be at least 1 (#VALUE!)"),
            None,
        )),
        _ => Err(Error::new(
            format!("{func} expects number as start_num"),
            None,
        )),
    }
}

/// The first argument of a text function, which must be a string (excel-compat X-9).
fn text_arg<'a>(func: &str, args: &'a [Value]) -> Result<&'a str, Error> {
    match args.first() {
        Some(Value::String(s)) => Ok(s),
        _ => Err(Error::new(
            format!("{func} expects string as first argument"),
            None,
        )),
    }
}

/// Excel `LEFT(text, [num_chars])` / `RIGHT(text, [num_chars])`: `num_chars` defaults to 1.
fn left_right(name: &str, args: &[Value]) -> Result<Value, Error> {
    if args.is_empty() {
        return Err(Error::new(
            format!("{name} expects string, [num_chars]"),
            None,
        ));
    }
    let s = text_arg(name, args)?;
    let take = match args.get(1) {
        Some(v) => count_arg(name, v)?,
        None => 1,
    };
    let len = char_count(s);
    let (from, to) = if name == "LEFT" {
        (0, take.min(len))
    } else {
        (len.saturating_sub(take), len)
    };
    Ok(Value::String(
        s[char_byte_offset(s, from)..char_byte_offset(s, to)].to_string(),
    ))
}

/// Excel `MID(text, start_num, num_chars)`. A start past the end gives `""`; a length past the
/// end is clamped. Leaving out `num_chars` (to the end) is a Skillet extension.
fn mid(args: &[Value]) -> Result<Value, Error> {
    let [_, start, rest @ ..] = args else {
        return Err(Error::new("MID expects string, start, [num_chars]", None));
    };
    let s = text_arg("MID", args)?;
    let start = start_arg("MID", start)?;
    let total = char_count(s);
    let end = match rest.first() {
        Some(v) => start.saturating_add(count_arg("MID", v)?).min(total),
        None => total,
    };
    if start >= end {
        return Ok(Value::String(String::new()));
    }
    Ok(Value::String(
        s[char_byte_offset(s, start)..char_byte_offset(s, end)].to_string(),
    ))
}

/// Excel `REPLACE(old_text, start_num, num_chars, new_text)`. A start past the end appends
/// `new_text`; `num_chars` past the end is clamped.
fn replace(args: &[Value]) -> Result<Value, Error> {
    let [_, start, count, new_text] = args else {
        return Err(Error::new(
            "REPLACE expects (old_text: string, start_num: number, num_chars: number, new_text: string)",
            None,
        ));
    };
    let old_text = text_arg("REPLACE", args)?;
    let start = start_arg("REPLACE", start)?;
    let count = count_arg("REPLACE", count)?;
    let Value::String(new_text) = new_text else {
        return Err(Error::new(
            "REPLACE expects string as fourth argument",
            None,
        ));
    };
    let len = char_count(old_text);
    let start = start.min(len);
    let end = start.saturating_add(count).min(len);
    let byte_start = char_byte_offset(old_text, start);
    let byte_end = char_byte_offset(old_text, end);
    let mut out = String::with_capacity(old_text.len() + new_text.len());
    out.push_str(&old_text[..byte_start]);
    out.push_str(new_text);
    out.push_str(&old_text[byte_end..]);
    Ok(Value::String(out))
}

/// Excel `SUBSTITUTE(text, old_text, new_text, [instance_num])`: replace every occurrence, or
/// only the `instance_num`-th (1-based, truncated). An empty `old_text` leaves the text unchanged.
fn substitute(args: &[Value]) -> Result<Value, Error> {
    let (text, old, new) = match (args.first(), args.get(1), args.get(2), args.len()) {
        (Some(Value::String(t)), Some(Value::String(o)), Some(Value::String(n)), 3 | 4) => {
            (t, o, n)
        }
        _ => {
            return Err(Error::new(
                "SUBSTITUTE expects (text: string, old_text: string, new_text: string, [instance_num: number])",
                None,
            ))
        }
    };
    let instance = match args.get(3) {
        None => None,
        Some(Value::Number(n)) if *n >= 1.0 => Some(n.trunc()),
        Some(_) => {
            return Err(Error::new(
                "SUBSTITUTE instance_num must be a number >= 1 (#VALUE!)",
                None,
            ))
        }
    };
    if old.is_empty() {
        return Ok(Value::String(text.clone()));
    }
    match instance {
        None => {
            limits::charge_replace(text, old, new)?;
            Ok(Value::String(text.replace(old.as_str(), new)))
        }
        Some(k) => {
            let hit = text
                .match_indices(old.as_str())
                .nth((k as usize).saturating_sub(1));
            Ok(Value::String(match hit {
                Some((at, _)) => {
                    let mut out = String::with_capacity(text.len() + new.len());
                    out.push_str(&text[..at]);
                    out.push_str(new);
                    out.push_str(&text[at + old.len()..]);
                    out
                }
                None => text.clone(),
            }))
        }
    }
}
