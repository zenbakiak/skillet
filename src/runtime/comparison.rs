//! Comparison operators (ADR-0023).
//!
//! No comparison converts between types: `1 == "1"` and `TRUE == 1` are `false`, and ordering
//! values of different types is an error. Conversion happens only in arithmetic.
//!
//! - `==` / `!=`: like Excel's `=`, text compares case-insensitively (`"a" == "A"`). Every
//!   other value equals only a value of the same type with the same value. Arrays and JSON
//!   objects compare element by element with this rule. `NULL` equals only `NULL` (ADR-0013).
//! - `===` / `!==`: the same, but text is case-sensitive.
//! - `<`, `<=`, `>`, `>=` order two numbers, two currency values, two booleans
//!   (`FALSE < TRUE`), two texts (case-insensitively) or two DateTimes. Anything else is an
//!   error.

use crate::ast::BinaryOp;
use crate::error::Error;
use crate::limits;
use crate::types::Value;
use std::cmp::Ordering;

/// Evaluate a comparison operator. Two numbers take the inline fast path.
#[inline]
pub(crate) fn compare(op: BinaryOp, a: &Value, b: &Value) -> Result<Value, Error> {
    if let (Value::Number(x), Value::Number(y)) = (a, b) {
        return Ok(Value::Boolean(match op {
            BinaryOp::Eq | BinaryOp::StrictEq => x == y,
            BinaryOp::Ne | BinaryOp::StrictNe => x != y,
            BinaryOp::Lt => x < y,
            BinaryOp::Le => x <= y,
            BinaryOp::Gt => x > y,
            BinaryOp::Ge => x >= y,
            _ => return Err(not_a_comparison()),
        }));
    }
    compare_values(op, a, b).map(Value::Boolean)
}

#[inline(never)]
fn compare_values(op: BinaryOp, a: &Value, b: &Value) -> Result<bool, Error> {
    Ok(match op {
        BinaryOp::Eq => fuzzy_eq(a, b)?,
        BinaryOp::Ne => !fuzzy_eq(a, b)?,
        BinaryOp::StrictEq => strict_eq(a, b)?,
        BinaryOp::StrictNe => !strict_eq(a, b)?,
        BinaryOp::Lt => order(a, b)? == Ordering::Less,
        BinaryOp::Le => order(a, b)? != Ordering::Greater,
        BinaryOp::Gt => order(a, b)? == Ordering::Greater,
        BinaryOp::Ge => order(a, b)? != Ordering::Less,
        _ => return Err(not_a_comparison()),
    })
}

fn not_a_comparison() -> Error {
    Error::new("Not a comparison operator", None)
}

fn incompatible() -> Error {
    Error::new("Comparison of incompatible types", None)
}

/// `==`: type-exact, but text ignores case.
fn fuzzy_eq(a: &Value, b: &Value) -> Result<bool, Error> {
    match (a, b) {
        (Value::String(x), Value::String(y)) => Ok(text_eq_ignore_case(x, y)),
        (Value::Array(x), Value::Array(y)) => arrays_eq(x, y, fuzzy_eq),
        (Value::Json(x), Value::Json(y)) => json_eq(x, y, false),
        _ => strict_eq(a, b),
    }
}

/// `===`: same type, same value, case-sensitive text. `Number` and `Currency` differ.
fn strict_eq(a: &Value, b: &Value) -> Result<bool, Error> {
    Ok(match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::Number(x), Value::Number(y)) | (Value::Currency(x), Value::Currency(y)) => x == y,
        (Value::String(x), Value::String(y)) => x == y,
        (Value::Boolean(x), Value::Boolean(y)) => x == y,
        (Value::DateTime(x), Value::DateTime(y)) => x == y,
        (Value::Array(x), Value::Array(y)) => arrays_eq(x, y, strict_eq)?,
        (Value::Json(x), Value::Json(y)) => json_eq(x, y, true)?,
        _ => false,
    })
}

fn arrays_eq(
    x: &[Value],
    y: &[Value],
    eq: fn(&Value, &Value) -> Result<bool, Error>,
) -> Result<bool, Error> {
    if x.len() != y.len() {
        return Ok(false);
    }
    for (a, b) in x.iter().zip(y) {
        limits::tick()?;
        if !eq(a, b)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Two JSON objects (kept as text, ADR-0004): parse both and compare the trees. Keys match
/// exactly; values use the fuzzy or strict rule. Parsing is charged to the budget first.
fn json_eq(x: &str, y: &str, strict: bool) -> Result<bool, Error> {
    if x == y {
        return Ok(true);
    }
    limits::charge_json_parse(x)?;
    limits::charge_json_parse(y)?;
    let parse = |s: &str| {
        serde_json::from_str::<serde_json::Value>(s)
            .map_err(|e| Error::new(format!("Invalid JSON: {}", e), None))
    };
    json_tree_eq(&parse(x)?, &parse(y)?, strict)
}

fn json_tree_eq(a: &serde_json::Value, b: &serde_json::Value, strict: bool) -> Result<bool, Error> {
    use serde_json::Value as J;
    limits::tick()?;
    match (a, b) {
        (J::Array(x), J::Array(y)) => {
            if x.len() != y.len() {
                return Ok(false);
            }
            for (p, q) in x.iter().zip(y) {
                if !json_tree_eq(p, q, strict)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        (J::Object(x), J::Object(y)) => {
            if x.len() != y.len() {
                return Ok(false);
            }
            for (k, p) in x {
                match y.get(k) {
                    Some(q) if json_tree_eq(p, q, strict)? => {}
                    _ => return Ok(false),
                }
            }
            Ok(true)
        }
        (J::Array(_) | J::Object(_), _) | (_, J::Array(_) | J::Object(_)) => Ok(false),
        _ => {
            let (p, q) = (json_scalar(a), json_scalar(b));
            if strict {
                strict_eq(&p, &q)
            } else {
                fuzzy_eq(&p, &q)
            }
        }
    }
}

/// A JSON leaf as a `Value` (numbers become `f64`, like everywhere else in Skillet).
fn json_scalar(v: &serde_json::Value) -> Value {
    match v {
        serde_json::Value::Bool(b) => Value::Boolean(*b),
        serde_json::Value::Number(n) => n.as_f64().map_or(Value::Null, Value::Number),
        serde_json::Value::String(s) => Value::String(s.clone()),
        _ => Value::Null,
    }
}

/// `<`, `<=`, `>`, `>=` on two values of the same orderable type.
fn order(a: &Value, b: &Value) -> Result<Ordering, Error> {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) | (Value::Currency(x), Value::Currency(y)) => {
            x.partial_cmp(y).ok_or_else(incompatible)
        }
        (Value::String(x), Value::String(y)) => Ok(text_cmp_ignore_case(x, y)),
        (Value::Boolean(x), Value::Boolean(y)) => Ok(x.cmp(y)),
        (Value::DateTime(x), Value::DateTime(y)) => Ok(x.cmp(y)),
        _ => Err(incompatible()),
    }
}

/// Case-insensitive text equality. Equal bytes short-circuit (a `memcmp`), then an ASCII prefix
/// is compared byte by byte; only from the first non-ASCII character on are characters
/// lowercased (`char::to_lowercase`, so `"ÉCOLE" == "école"`).
fn text_eq_ignore_case(x: &str, y: &str) -> bool {
    x == y || text_cmp_ignore_case(x, y) == Ordering::Equal
}

fn text_cmp_ignore_case(x: &str, y: &str) -> Ordering {
    if x == y {
        return Ordering::Equal;
    }
    let (xb, yb) = (x.as_bytes(), y.as_bytes());
    let mut i = 0;
    while i < xb.len() && i < yb.len() && xb[i].is_ascii() && yb[i].is_ascii() {
        match xb[i].to_ascii_lowercase().cmp(&yb[i].to_ascii_lowercase()) {
            Ordering::Equal => i += 1,
            unequal => return unequal,
        }
    }
    // Every byte before `i` is ASCII, so `i` is a character boundary in both strings.
    x[i..]
        .chars()
        .flat_map(char::to_lowercase)
        .cmp(y[i..].chars().flat_map(char::to_lowercase))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn case_insensitive_text_handles_non_ascii() {
        assert!(text_eq_ignore_case("ÉCOLE", "école"));
        assert!(text_eq_ignore_case("abÉ", "ABé"));
        assert!(!text_eq_ignore_case("ab", "abc"));
        assert_eq!(text_cmp_ignore_case("Äb", "äa"), Ordering::Greater);
        assert_eq!(text_cmp_ignore_case("apple", "Banana"), Ordering::Less);
        assert_eq!(text_cmp_ignore_case("ab", "AB"), Ordering::Equal);
        assert_eq!(text_cmp_ignore_case("ab", "ABC"), Ordering::Less);
    }
}
