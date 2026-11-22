//! Comparison operators (ADR-0023).
//!
//! No comparison converts between types: `1 == "1"` and `TRUE == 1` are `false`, and ordering
//! values of different types is an error. Conversion happens only in arithmetic.
//!
//! - `==` / `!=`: a value equals only a value of the same type with the same value. Text is
//!   case-sensitive. Arrays and JSON objects compare element by element with this rule.
//!   DateTimes compare by timestamp. `NULL` equals only `NULL` (ADR-0013).
//! - `<`, `<=`, `>`, `>=` order two numbers, two currency values, two booleans
//!   (`FALSE < TRUE`), two texts (case-sensitive, by Unicode code point) or two DateTimes.
//!   Anything else is an error.

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
            BinaryOp::Eq => x == y,
            BinaryOp::Ne => x != y,
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
        BinaryOp::Eq => equal(a, b)?,
        BinaryOp::Ne => !equal(a, b)?,
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

/// The amount of a Number or a Currency; `None` for every other value.
fn amount(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) | Value::Currency(n) => Some(*n),
        _ => None,
    }
}

/// `==`: same type, same value. Number and Currency compare by amount (ADR-0027 Amendment 1).
fn equal(a: &Value, b: &Value) -> Result<bool, Error> {
    if let (Some(x), Some(y)) = (amount(a), amount(b)) {
        return Ok(x == y);
    }
    Ok(match (a, b) {
        (Value::Null, Value::Null) => true,
        (Value::String(x), Value::String(y)) => x == y,
        (Value::Boolean(x), Value::Boolean(y)) => x == y,
        (Value::DateTime(x), Value::DateTime(y)) => x == y,
        (Value::Array(x), Value::Array(y)) => arrays_eq(x, y)?,
        (Value::Json(x), Value::Json(y)) => json_eq(x, y)?,
        _ => false,
    })
}

fn arrays_eq(x: &[Value], y: &[Value]) -> Result<bool, Error> {
    if x.len() != y.len() {
        return Ok(false);
    }
    for (a, b) in x.iter().zip(y) {
        limits::tick()?;
        if !equal(a, b)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Two JSON objects (kept as text, ADR-0004): parse both and compare the trees, so key order
/// and number spelling (`1` vs `1.0`) do not matter. Parsing is charged to the budget first.
fn json_eq(x: &str, y: &str) -> Result<bool, Error> {
    if x == y {
        return Ok(true);
    }
    limits::charge_json_parse(x)?;
    limits::charge_json_parse(y)?;
    let parse = |s: &str| {
        serde_json::from_str::<serde_json::Value>(s)
            .map_err(|e| Error::new(format!("Invalid JSON: {}", e), None))
    };
    json_tree_eq(&parse(x)?, &parse(y)?)
}

fn json_tree_eq(a: &serde_json::Value, b: &serde_json::Value) -> Result<bool, Error> {
    use serde_json::Value as J;
    limits::tick()?;
    match (a, b) {
        (J::Array(x), J::Array(y)) => {
            if x.len() != y.len() {
                return Ok(false);
            }
            for (p, q) in x.iter().zip(y) {
                if !json_tree_eq(p, q)? {
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
                    Some(q) if json_tree_eq(p, q)? => {}
                    _ => return Ok(false),
                }
            }
            Ok(true)
        }
        // Numbers compare as `f64`, like everywhere else in Skillet.
        (J::Number(x), J::Number(y)) => Ok(x.as_f64() == y.as_f64()),
        _ => Ok(a == b),
    }
}

/// `<`, `<=`, `>`, `>=` on two values of the same orderable type (Number and Currency are one).
fn order(a: &Value, b: &Value) -> Result<Ordering, Error> {
    if let (Some(x), Some(y)) = (amount(a), amount(b)) {
        return x.partial_cmp(&y).ok_or_else(incompatible);
    }
    match (a, b) {
        (Value::String(x), Value::String(y)) => Ok(x.cmp(y)),
        (Value::Boolean(x), Value::Boolean(y)) => Ok(x.cmp(y)),
        (Value::DateTime(x), Value::DateTime(y)) => Ok(x.cmp(y)),
        _ => Err(incompatible()),
    }
}
