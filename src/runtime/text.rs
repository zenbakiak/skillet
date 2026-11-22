//! The text form of values (ADR-0034): one form per type, written here and nowhere else.
//!
//! Every surface that turns a value into text uses these writers: `::String`, `.to_s()`,
//! `CONCAT`, `&`, `TEXTJOIN`, `JOIN`, `.join()`, `LEN` and `FIND`. Results that can be large
//! (arrays, joins, concatenations) are counted first and charged to the evaluation budget
//! before they are built (ADR-0011 amendment 2).

use crate::error::Error;
use crate::limits::{self, ByteCounter};
use crate::runtime::datetime::{datetime_text_len, write_datetime};
use crate::types::Value;
use std::fmt::{self, Write};
use std::ops::Deref;

/// 2^53: integral numbers below it are exact in an `f64` and print as integers.
const EXACT_INT: f64 = 9_007_199_254_740_992.0;

/// Significant digits of a number's text, as in Excel.
const SIG_DIGITS: usize = 15;

/// Write a Number's text (ADR-0034 §2): `0` for both zeros, exact integers below 2^53, else
/// 15 significant digits, plain from 1e-7 up to 1e15 and `d.dddE±XX` outside that.
/// NaN and infinities (host values only) keep Rust's text.
pub(crate) fn write_number(w: &mut impl Write, n: f64) -> fmt::Result {
    if n == 0.0 {
        return w.write_char('0');
    }
    if !n.is_finite() {
        return write!(w, "{n}");
    }
    if n.fract() == 0.0 && n.abs() < EXACT_INT {
        return write!(w, "{}", n as i64);
    }
    if n < 0.0 {
        w.write_char('-')?;
    }
    let digits = Digits::of(n.abs());
    digits.write(w)
}

/// The decimal digits (no leading or trailing zeros) and exponent of a positive finite number
/// rounded to [`SIG_DIGITS`]: the value is `d.ddd × 10^exp`.
struct Digits {
    buf: [u8; SIG_DIGITS],
    len: usize,
    exp: i32,
}

impl Digits {
    fn of(n: f64) -> Digits {
        // Fast path: when the shortest round-trip text has at most 15 digits it is also the
        // correctly rounded 15-digit text (the rounding interval of an f64 is far narrower than
        // the spacing of 15-digit decimals), so the slower fixed-precision format is skipped.
        // Subnormals have fewer significant bits, so they always take the exact path.
        if n >= f64::MIN_POSITIVE {
            let mut text = StackText::new();
            let _ = write!(text, "{n:e}");
            if let Some(d) = Digits::parse(text.as_str()) {
                return d;
            }
        }
        let mut text = StackText::new();
        let _ = write!(text, "{n:.14e}");
        // `{:.14e}` writes exactly 15 digits, so this parse cannot fail; `1` is a safe default.
        Digits::parse(text.as_str()).unwrap_or(Digits {
            buf: [b'1'; SIG_DIGITS],
            len: 1,
            exp: 0,
        })
    }

    /// Read Rust's `{:e}` text (`1.2345e-7`). `None` when it has more than 15 digits.
    fn parse(text: &str) -> Option<Digits> {
        let (mantissa, exp) = text.split_once('e')?;
        let mut d = Digits {
            buf: [b'0'; SIG_DIGITS],
            len: 0,
            exp: exp.parse().ok()?,
        };
        for b in mantissa.bytes().filter(u8::is_ascii_digit) {
            *d.buf.get_mut(d.len)? = b;
            d.len += 1;
        }
        while d.len > 1 && d.buf[d.len - 1] == b'0' {
            d.len -= 1;
        }
        Some(d)
    }

    fn write(&self, w: &mut impl Write) -> fmt::Result {
        let digits = ascii(&self.buf[..self.len]);
        let e = self.exp;
        if !(-7..15).contains(&e) {
            let (first, rest) = digits.split_at(1);
            w.write_str(first)?;
            if !rest.is_empty() {
                w.write_char('.')?;
                w.write_str(rest)?;
            }
            let sign = if e < 0 { '-' } else { '+' };
            return write!(w, "E{sign}{:02}", e.unsigned_abs());
        }
        if e < 0 {
            w.write_str("0.")?;
            zeros(w, (-e - 1) as usize)?;
            return w.write_str(digits);
        }
        let int_len = e as usize + 1;
        if digits.len() <= int_len {
            w.write_str(digits)?;
            zeros(w, int_len - digits.len())
        } else {
            let (int, frac) = digits.split_at(int_len);
            w.write_str(int)?;
            w.write_char('.')?;
            w.write_str(frac)
        }
    }
}

fn zeros(w: &mut impl Write, n: usize) -> fmt::Result {
    (0..n).try_for_each(|_| w.write_char('0'))
}

/// The bytes the digit writers produce are ASCII digits, so this never falls back.
fn ascii(bytes: &[u8]) -> &str {
    std::str::from_utf8(bytes).unwrap_or("")
}

/// A fixed buffer for short texts (numbers, dates): no heap allocation. A write that does not
/// fit fails, and [`scalar_text`] falls back to a `String`.
pub(crate) struct StackText {
    buf: [u8; 40],
    len: usize,
}

impl StackText {
    fn new() -> StackText {
        StackText {
            buf: [0; 40],
            len: 0,
        }
    }

    fn as_str(&self) -> &str {
        // Only whole `&str`s are copied in, so the bytes are valid UTF-8.
        std::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }
}

impl Write for StackText {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let end = self.len + s.len();
        let slot = self.buf.get_mut(self.len..end).ok_or(fmt::Error)?;
        slot.copy_from_slice(s.as_bytes());
        self.len = end;
        Ok(())
    }
}

/// The text of a scalar, borrowed when the value holds it and inline otherwise.
pub(crate) enum ScalarText<'a> {
    Borrowed(&'a str),
    Inline(StackText),
    Owned(String),
}

impl Deref for ScalarText<'_> {
    type Target = str;
    fn deref(&self) -> &str {
        match self {
            ScalarText::Borrowed(s) => s,
            ScalarText::Inline(t) => t.as_str(),
            ScalarText::Owned(s) => s,
        }
    }
}

impl<'a> ScalarText<'a> {
    pub(crate) fn into_cow(self) -> std::borrow::Cow<'a, str> {
        match self {
            ScalarText::Borrowed(s) => std::borrow::Cow::Borrowed(s),
            ScalarText::Inline(t) => std::borrow::Cow::Owned(t.as_str().to_owned()),
            ScalarText::Owned(s) => std::borrow::Cow::Owned(s),
        }
    }
}

/// The text of a non-array value (ADR-0034 §1). `None` for an array, whose text is its JSON
/// text and can fail ([`to_text`]).
#[allow(deprecated)] // `Value::Currency` is read as a Number (ADR-0033)
pub(crate) fn scalar_text(v: &Value) -> Option<ScalarText<'_>> {
    Some(match v {
        Value::String(s) | Value::Json(s) => ScalarText::Borrowed(s),
        Value::Boolean(b) => ScalarText::Borrowed(bool_text(*b)),
        Value::Null => ScalarText::Borrowed(""),
        Value::Array(_) => return None,
        Value::Number(_) | Value::Currency(_) | Value::DateTime(_) => {
            let mut t = StackText::new();
            if write_scalar(&mut t, v).is_ok() {
                ScalarText::Inline(t)
            } else {
                let mut s = String::new();
                let _ = write_scalar(&mut s, v);
                ScalarText::Owned(s)
            }
        }
    })
}

fn bool_text(b: bool) -> &'static str {
    if b {
        "TRUE"
    } else {
        "FALSE"
    }
}

/// Write the text of a non-array value; an array writes nothing (callers flatten or reject it).
#[allow(deprecated)] // `Value::Currency` is read as a Number (ADR-0033)
fn write_scalar(w: &mut impl Write, v: &Value) -> fmt::Result {
    match v {
        Value::String(s) | Value::Json(s) => w.write_str(s),
        Value::Number(n) | Value::Currency(n) => write_number(w, *n),
        Value::Boolean(b) => w.write_str(bool_text(*b)),
        Value::DateTime(ts) => write_datetime(w, *ts),
        Value::Null | Value::Array(_) => Ok(()),
    }
}

/// Length in bytes of a non-array value's text, without building it when it is known.
#[allow(deprecated)] // `Value::Currency` is read as a Number (ADR-0033)
fn scalar_len(v: &Value) -> usize {
    match v {
        Value::String(s) | Value::Json(s) => s.len(),
        Value::Boolean(b) => bool_text(*b).len(),
        Value::DateTime(ts) => datetime_text_len(*ts),
        Value::Null | Value::Array(_) => 0,
        Value::Number(_) | Value::Currency(_) => {
            let mut c = ByteCounter(0);
            let _ = write_scalar(&mut c, v);
            c.0 as usize
        }
    }
}

/// The text form of any value (`::String`, `.to_s()`). An array's JSON text is charged before
/// it is built; a scalar's text is at most as long as the value it comes from plus a few bytes.
pub(crate) fn to_text(v: &Value) -> Result<String, Error> {
    match v {
        Value::Array(items) => {
            let mut counter = ByteCounter(0);
            write_json_array(&mut counter, items, true)?;
            let len = usize::try_from(counter.0).unwrap_or(usize::MAX);
            limits::charge_string(len)?;
            let mut out = String::with_capacity(len);
            write_json_array(&mut out, items, false)?;
            Ok(out)
        }
        other => Ok(scalar_text(other).as_deref().unwrap_or("").to_owned()),
    }
}

/// `CONCAT` of `args` (and `a & b`): each value's text, arrays flattened item by item, NULL
/// adding nothing (ADR-0034 §3). Counted, charged, then built.
pub(crate) fn concat<'a>(args: impl Iterator<Item = &'a Value> + Clone) -> Result<String, Error> {
    let mut counter = ByteCounter(0);
    args.clone().for_each(|v| write_flat(&mut counter, v));
    let len = usize::try_from(counter.0).unwrap_or(usize::MAX);
    limits::charge_string(len)?;
    let mut out = String::with_capacity(len);
    args.for_each(|v| write_flat(&mut out, v));
    Ok(out)
}

fn write_flat(w: &mut impl Write, v: &Value) {
    match v {
        Value::Array(items) => items.iter().for_each(|it| write_flat(w, it)),
        other => {
            let _ = write_scalar(w, other);
        }
    }
}

/// `JOIN(items, sep)` and `items.join(sep)`: each item's text between separators. A nested
/// array is an error with `nested_error`. Counted, charged, then built.
pub(crate) fn join(items: &[Value], sep: &str, nested_error: &str) -> Result<String, Error> {
    let mut len = sep.len().saturating_mul(items.len().saturating_sub(1));
    for (i, it) in items.iter().enumerate() {
        limits::tick_item(i)?;
        if let Value::Array(_) = it {
            return Err(Error::new(nested_error, None));
        }
        len = len.saturating_add(scalar_len(it));
    }
    limits::charge_string(len)?;
    let mut out = String::with_capacity(len);
    for (i, it) in items.iter().enumerate() {
        if i > 0 {
            out.push_str(sep);
        }
        let _ = write_scalar(&mut out, it);
    }
    Ok(out)
}

/// The JSON text `.to_json()` writes for a value (ADR-0034 §4). Strings and arrays, whose text
/// can be far larger than the value (escapes), are charged before they are built.
pub(crate) fn json_text(v: &Value) -> Result<String, Error> {
    let mut counter = ByteCounter(0);
    if matches!(v, Value::String(_) | Value::Array(_)) {
        write_json(&mut counter, v, true)?;
        limits::charge_string(usize::try_from(counter.0).unwrap_or(usize::MAX))?;
    }
    let mut out = String::with_capacity(counter.0 as usize);
    write_json(&mut out, v, counter.0 == 0)?;
    Ok(out)
}

fn write_json_array(w: &mut impl Write, items: &[Value], charge: bool) -> Result<(), Error> {
    let _ = w.write_char('[');
    for (i, it) in items.iter().enumerate() {
        if i > 0 {
            let _ = w.write_char(',');
        }
        write_json(w, it, charge)?;
    }
    let _ = w.write_char(']');
    Ok(())
}

/// Write one value as JSON. `charge` charges the parse of an object's text (done on the
/// counting pass only, so the build pass does not charge it twice).
#[allow(deprecated)] // `Value::Currency` is read as a Number (ADR-0033)
fn write_json(w: &mut impl Write, v: &Value, charge: bool) -> Result<(), Error> {
    let _ = match v {
        Value::Null => w.write_str("null"),
        Value::Boolean(b) => w.write_str(if *b { "true" } else { "false" }),
        Value::Number(n) | Value::Currency(n) => return write_json_number(w, *n),
        Value::String(s) => write_json_str(w, s),
        Value::DateTime(ts) => {
            let _ = w.write_char('"');
            let _ = write_datetime(w, *ts);
            w.write_char('"')
        }
        Value::Array(items) => return write_json_array(w, items, charge),
        Value::Json(s) => {
            if charge {
                limits::charge_json_parse(s)?;
            }
            let parsed: serde_json::Value = serde_json::from_str(s)
                .map_err(|e| Error::new(format!("Invalid JSON: {}", e), None))?;
            let text = serde_json::to_string(&parsed)
                .map_err(|e| Error::new(format!("Failed to convert to JSON: {}", e), None))?;
            w.write_str(&text)
        }
    };
    Ok(())
}

/// A number as JSON (ADR-0030 decision d): integer digits for an integral `n` with
/// `|n| < 2^53` (not `-0`), else `serde_json`'s shortest round-trip text.
fn write_json_number(w: &mut impl Write, n: f64) -> Result<(), Error> {
    if n.fract() == 0.0 && n.abs() < EXACT_INT && !(n == 0.0 && n.is_sign_negative()) {
        let _ = write!(w, "{}", n as i64);
        return Ok(());
    }
    let num = serde_json::Number::from_f64(n)
        .ok_or_else(|| Error::new("Invalid number for JSON", None))?;
    let _ = write!(w, "{num}");
    Ok(())
}

/// A string as JSON, escaped as `serde_json` escapes it: `"` and `\`, the short escapes
/// `\b \f \n \r \t`, other control characters as `\u00xx`.
fn write_json_str(w: &mut impl Write, s: &str) -> fmt::Result {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    w.write_char('"')?;
    let mut start = 0;
    for (i, b) in s.bytes().enumerate() {
        let short = match b {
            b'"' => "\\\"",
            b'\\' => "\\\\",
            b'\n' => "\\n",
            b'\r' => "\\r",
            b'\t' => "\\t",
            0x08 => "\\b",
            0x0c => "\\f",
            0x00..=0x1f => "",
            _ => continue,
        };
        w.write_str(&s[start..i])?;
        if short.is_empty() {
            let hex = [HEX[usize::from(b >> 4)], HEX[usize::from(b & 0xf)]];
            w.write_str("\\u00")?;
            w.write_str(ascii(&hex))?;
        } else {
            w.write_str(short)?;
        }
        start = i + 1;
    }
    w.write_str(&s[start..])?;
    w.write_char('"')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn num(n: f64) -> String {
        let mut s = String::new();
        write_number(&mut s, n).unwrap();
        s
    }

    #[test]
    fn numbers_follow_adr_0034() {
        let cases: &[(f64, &str)] = &[
            (0.0, "0"),
            (-0.0, "0"),
            (5.0, "5"),
            (-42.0, "-42"),
            (0.1 + 0.2, "0.3"),
            (1.0 / 3.0, "0.333333333333333"),
            (2f64.sqrt(), "1.4142135623731"),
            (-1.5, "-1.5"),
            (123456.789, "123456.789"),
            (1e15, "1000000000000000"),
            (1234567890123456.0, "1234567890123456"),
            (1e21, "1E+21"),
            (1e100, "1E+100"),
            (12345678901234567890.0, "1.23456789012346E+19"),
            (1e-7, "0.0000001"),
            (1.5e-8, "1.5E-08"),
            (4.94065645841247e-324, "4.94065645841247E-324"),
            (9007199254740992.0, "9.00719925474099E+15"),
            (100000000000000.01, "100000000000000"),
            (999999999999999.9, "1E+15"),
            (f64::NAN, "NaN"),
            (f64::INFINITY, "inf"),
            (f64::NEG_INFINITY, "-inf"),
        ];
        for (n, want) in cases {
            assert_eq!(num(*n), *want, "{n:e}");
        }
    }

    /// The shortest-digits fast path agrees with rounding to 15 digits on random bit patterns.
    #[test]
    fn fast_path_matches_fixed_precision() {
        let mut x: u64 = 0x9E37_79B9_7F4A_7C15;
        for _ in 0..200_000 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let n = f64::from_bits(x).abs();
            if !n.is_finite() || n < f64::MIN_POSITIVE {
                continue;
            }
            let fast = Digits::of(n);
            let slow = Digits::parse(&format!("{n:.14e}")).unwrap();
            assert_eq!(
                (&fast.buf[..fast.len], fast.exp),
                (&slow.buf[..slow.len], slow.exp),
                "{n:e}"
            );
        }
    }

    #[test]
    fn json_strings_match_serde_json() {
        let all: String = (0u8..0x80)
            .map(char::from)
            .chain("héllo €𝄞".chars())
            .collect();
        for s in [all.as_str(), "", "a\"b\\c", "\u{7f}"] {
            let mut out = String::new();
            write_json_str(&mut out, s).unwrap();
            assert_eq!(out, serde_json::to_string(s).unwrap());
        }
    }

    #[test]
    fn json_numbers() {
        let text = |n: f64| {
            let mut s = String::new();
            write_json_number(&mut s, n).unwrap();
            s
        };
        assert_eq!(text(5.0), "5");
        assert_eq!(text(1.5), "1.5");
        assert_eq!(text(-0.0), "-0.0");
        assert_eq!(text(0.1 + 0.2), "0.30000000000000004");
        assert_eq!(text(1e21), "1e+21");
        assert!(write_json_number(&mut String::new(), f64::NAN).is_err());
    }
}
