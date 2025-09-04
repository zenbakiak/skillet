use crate::error::Error;
use crate::runtime::utils::is_blank;
use crate::types::Value;

pub fn exec_string(name: &str, args: &[Value]) -> Result<Value, Error> {
    match name {
        "LEFT" => {
            // LEFT(String, [NumberOfCharacters]) -> default 1 character if omitted
            if args.is_empty() {
                return Err(Error::new("LEFT expects string, [num_chars]", None));
            }
            let s = match args.get(0) {
                Some(Value::String(st)) => st,
                _ => return Err(Error::new("LEFT expects string as first argument", None)),
            };
            let n = match args.get(1) {
                Some(Value::Number(n)) => *n,
                Some(_) => return Err(Error::new("LEFT expects number as second argument", None)),
                None => 1.0,
            };
            let take = if n.is_finite() && n > 0.0 { n as usize } else { 0usize };
            let chars: Vec<char> = s.chars().collect();
            let end = take.min(chars.len());
            Ok(Value::String(chars[0..end].iter().collect()))
        }
        "RIGHT" => {
            // RIGHT(String, [NumberOfCharacters]) -> default 1 character if omitted
            if args.is_empty() {
                return Err(Error::new("RIGHT expects string, [num_chars]", None));
            }
            let s = match args.get(0) {
                Some(Value::String(st)) => st,
                _ => return Err(Error::new("RIGHT expects string as first argument", None)),
            };
            let n = match args.get(1) {
                Some(Value::Number(n)) => *n,
                Some(_) => return Err(Error::new("RIGHT expects number as second argument", None)),
                None => 1.0,
            };
            let take = if n.is_finite() && n > 0.0 { n as usize } else { 0usize };
            let chars: Vec<char> = s.chars().collect();
            let len = chars.len();
            let start = len.saturating_sub(take).min(len);
            Ok(Value::String(chars[start..len].iter().collect()))
        }
        "MID" => {
            // MID(String, StartingPosition [, NumberOfCharacters])
            // StartingPosition is 1-based (Excel-like). If NumberOfCharacters omitted, go to end.
            if args.len() < 2 {
                return Err(Error::new(
                    "MID expects string, start, [num_chars]",
                    None,
                ));
            }
            let s = match args.get(0) {
                Some(Value::String(st)) => st,
                _ => return Err(Error::new("MID expects string as first argument", None)),
            };
            let start_num = match args.get(1) {
                Some(Value::Number(n)) => *n,
                _ => return Err(Error::new("MID expects number as second argument", None)),
            };
            let len_opt = match args.get(2) {
                Some(Value::Number(n)) => Some(*n),
                Some(_) => return Err(Error::new("MID expects number as third argument", None)),
                None => None,
            };

            let chars: Vec<char> = s.chars().collect();
            let total = chars.len();
            // Excel-like: 1-based start; clamp below 1 to 1
            let start_index = if start_num.is_finite() {
                let s1 = if start_num < 1.0 { 1.0 } else { start_num.floor() } as usize;
                s1.saturating_sub(1).min(total)
            } else {
                0usize
            };
            let end_index = if let Some(n) = len_opt {
                let take = if n.is_finite() && n > 0.0 { n as usize } else { 0usize };
                start_index.saturating_add(take).min(total)
            } else {
                total
            };
            if start_index >= total || start_index >= end_index {
                Ok(Value::String(String::new()))
            } else {
                Ok(Value::String(chars[start_index..end_index].iter().collect()))
            }
        }
        "LENGTH" => match args.get(0) {
            Some(Value::Array(items)) => Ok(Value::Number(items.len() as f64)),
            Some(Value::String(s)) => Ok(Value::Number(s.chars().count() as f64)),
            Some(Value::Null) => Ok(Value::Number(0.0)),
            Some(_) | None => Err(Error::new("LENGTH expects array or string", None)),
        },
        "CONCAT" => {
            let mut out = String::new();
            fn push_val(s: &mut String, v: &Value) -> Result<(), Error> {
                match v {
                    Value::String(st) => {
                        s.push_str(st);
                        Ok(())
                    }
                    Value::Number(n) => {
                        s.push_str(&n.to_string());
                        Ok(())
                    }
                    Value::Array(arr) => {
                        for it in arr {
                            push_val(s, it)?;
                        }
                        Ok(())
                    }
                    Value::Boolean(b) => {
                        s.push_str(if *b { "TRUE" } else { "FALSE" });
                        Ok(())
                    }
                    Value::Null => Ok(()),
                    Value::Currency(_) => Ok(()),
                    Value::DateTime(_) => Ok(()),
                    Value::Json(_) => Ok(()),
                }
            }
            for a in args {
                if let Value::Null = a { /* skip */
                } else {
                    push_val(&mut out, a)?;
                }
            }
            Ok(Value::String(out))
        }
        "UPPER" => match args.get(0) {
            Some(Value::String(s)) => Ok(Value::String(s.to_uppercase())),
            _ => Err(Error::new("UPPER expects string", None)),
        },
        "LOWER" => match args.get(0) {
            Some(Value::String(s)) => Ok(Value::String(s.to_lowercase())),
            _ => Err(Error::new("LOWER expects string", None)),
        },
        "TRIM" => match args.get(0) {
            Some(Value::String(s)) => Ok(Value::String(s.trim().to_string())),
            _ => Err(Error::new("TRIM expects string", None)),
        },
        "SUBSTRING" => {
            if args.len() < 2 {
                return Err(Error::new(
                    "SUBSTRING expects string, start, [length]",
                    None,
                ));
            }
            let string = match args.get(0) {
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

            let chars: Vec<char> = string.chars().collect();
            let string_len = chars.len();

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
                let substring: String = chars[start..end].iter().collect();
                Ok(Value::String(substring))
            }
        }
        "SPLIT" => match (args.get(0), args.get(1)) {
            (Some(Value::String(s)), Some(Value::String(sep))) => Ok(Value::Array(
                s.split(sep).map(|p| Value::String(p.to_string())).collect(),
            )),
            (Some(Value::String(s)), None) => Ok(Value::Array(
                s.split(',')
                    .map(|p| Value::String(p.trim().to_string()))
                    .collect(),
            )),
            _ => Err(Error::new("SPLIT expects string, [separator]", None)),
        },
        "REPLACE" => match (args.get(0), args.get(1), args.get(2)) {
            (Some(Value::String(s)), Some(Value::String(from)), Some(Value::String(to))) => {
                Ok(Value::String(s.replace(from, to)))
            }
            _ => Err(Error::new("REPLACE expects string, search, replace", None)),
        },
        "REVERSE" => match args.get(0) {
            Some(Value::String(s)) => Ok(Value::String(s.chars().rev().collect())),
            _ => Err(Error::new("REVERSE expects string", None)),
        },
        "ISBLANK" => {
            let v = args.get(0).cloned().unwrap_or(Value::Null);
            Ok(Value::Boolean(is_blank(&v)))
        }
        "ISNUMBER" => {
            let v = args.get(0).cloned().unwrap_or(Value::Null);
            Ok(Value::Boolean(matches!(
                v,
                Value::Number(_) | Value::Currency(_)
            )))
        }
        "ISTEXT" => {
            let v = args.get(0).cloned().unwrap_or(Value::Null);
            Ok(Value::Boolean(matches!(v, Value::String(_))))
        }
        _ => Err(Error::new(
            format!("Unknown string function: {}", name),
            None,
        )),
    }
}
