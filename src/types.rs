/// The result of an evaluation, and the type of every variable.
///
/// ```rust
/// use skillet::{evaluate, Value};
///
/// assert_eq!(evaluate("1 + 1").unwrap(), Value::Number(2.0));
/// assert_eq!(evaluate("1 < 2").unwrap(), Value::Boolean(true));
/// assert_eq!(evaluate("NULL").unwrap(), Value::Null);
/// assert_eq!(evaluate("'a'.upper()").unwrap().as_number(), None);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// Every number is an `f64`, including integers.
    Number(f64),
    /// An ordered list of values.
    Array(Vec<Value>),
    /// `TRUE`/`FALSE` and comparison results.
    Boolean(bool),
    /// UTF-8 text.
    String(String),
    /// Missing value (`NULL`, or a null-safe access on nothing).
    Null,
    /// A number tagged as money (for example `12.5::Currency`).
    Currency(f64),
    /// Seconds since the Unix epoch.
    DateTime(i64),
    /// A JSON object, kept as JSON text (nested objects from JSON variables, object literals).
    Json(String),
}

impl Value {
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Boolean(b) => Some(*b),
            _ => None,
        }
    }
}
