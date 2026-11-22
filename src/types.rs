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
#[allow(deprecated)] // the derives match `Currency` until it is removed in 0.8
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
    /// Deprecated: Skillet has one number type (ADR-0033). Nothing the language evaluates
    /// produces it: `x::Currency` rounds to 4 decimals and returns a [`Value::Number`]. A
    /// `Currency` passed in from Rust (a variable, a custom function result) is read as the
    /// `Number` of the same amount, and results never contain one. Removed in 0.8.0.
    #[deprecated(
        since = "0.7.0",
        note = "Skillet has one number type (ADR-0033); use Value::Number"
    )]
    Currency(f64),
    /// Seconds since the Unix epoch.
    DateTime(i64),
    /// A JSON object, kept as JSON text (nested objects from JSON variables, object literals).
    ///
    /// Deprecated (ADR-0030 step 2): in 0.8.0 (BL-011) objects become a structured
    /// `Value::Object` and this variant is removed. It still works unchanged in 0.7: objects
    /// that Skillet evaluates or reads from JSON are `Json`, and you can build and match it. To
    /// build an object without naming the variant, use [`crate::json_to_value`].
    ///
    /// Naming it is a deprecation warning, so a crate that denies warnings stops compiling:
    ///
    /// ```compile_fail
    /// #![deny(deprecated)]
    /// let _ = skillet::Value::Json(r#"{"a":1}"#.to_string());
    /// ```
    #[deprecated(
        since = "0.7.0",
        note = "JSON objects become the structured Value::Object in 0.8.0 (ADR-0030, BL-011) \
                and this variant is removed; build objects with skillet::json_to_value"
    )]
    Json(String),
}

impl Value {
    /// The number, for a `Number` (and a deprecated `Currency`, read as a Number, ADR-0033).
    #[allow(deprecated)]
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Number(n) | Value::Currency(n) => Some(*n),
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

/// Whether `v` is, or holds in an array at any depth, a deprecated `Value::Currency`.
#[allow(deprecated)]
pub(crate) fn holds_currency(v: &Value) -> bool {
    match v {
        Value::Currency(_) => true,
        Value::Array(items) => any_currency(items),
        _ => false,
    }
}

/// [`holds_currency`] for a slice. A flat loop over the tags; it recurses only into arrays, so a
/// large array of numbers costs one pass over memory.
#[allow(deprecated)]
fn any_currency(items: &[Value]) -> bool {
    let mut nested = false;
    for item in items {
        match item {
            Value::Currency(_) => return true,
            Value::Array(_) => nested = true,
            _ => {}
        }
    }
    nested
        && items
            .iter()
            .any(|v| matches!(v, Value::Array(a) if any_currency(a)))
}

/// `v` with every `Value::Currency` (in arrays too) replaced by the `Number` of the same amount
/// (ADR-0033 §5). Values that hold no Currency are returned as they are.
#[allow(deprecated)]
pub(crate) fn currency_to_number(v: Value) -> Value {
    match v {
        Value::Currency(n) => Value::Number(n),
        Value::Array(items) if any_currency(&items) => {
            Value::Array(items.into_iter().map(currency_to_number).collect())
        }
        other => other,
    }
}
