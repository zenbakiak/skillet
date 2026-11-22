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
///
/// `Value` is `#[non_exhaustive]` (since 0.7.0): later releases add variants (`Object` in 0.8.0,
/// ADR-0030) without a breaking change, so a `match` outside this crate needs a `_` arm:
///
/// ```rust
/// use skillet::Value;
///
/// fn type_name(v: &Value) -> &'static str {
///     match v {
///         Value::Number(_) => "Number",
///         Value::String(_) => "String",
///         Value::Boolean(_) => "Boolean",
///         Value::Array(_) => "Array",
///         Value::DateTime(_) => "DateTime",
///         Value::Null => "Null",
///         _ => "Other",
///     }
/// }
/// assert_eq!(type_name(&Value::Null), "Null");
/// ```
///
/// Without it, the match does not compile (error E0004, "non-exhaustive patterns: `_` not
/// covered"), even when it names every variant:
///
/// ```compile_fail,E0004
/// # #![allow(deprecated)]
/// use skillet::Value;
///
/// fn type_name(v: &Value) -> &'static str {
///     match v {
///         Value::Number(_) | Value::Currency(_) => "Number",
///         Value::String(_) => "String",
///         Value::Boolean(_) => "Boolean",
///         Value::Array(_) => "Array",
///         Value::DateTime(_) => "DateTime",
///         Value::Null => "Null",
///         Value::Json(_) => "Json",
///     }
/// }
/// ```
#[derive(Debug, Clone, PartialEq)]
#[allow(deprecated)] // the derives match `Currency` until it is removed in 0.8
#[non_exhaustive]
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
    /// build an object without naming the variant, use [`Value::from_json_text`] (or
    /// [`crate::json_to_value`] for an already parsed `serde_json::Value`); to read one, use
    /// [`Value::to_json_text`]. Both keep their signatures in 0.8.0.
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
                and this variant is removed; build objects with Value::from_json_text"
    )]
    Json(String),
}

/// Building and reading values without naming a variant (ADR-0030 §3). These keep their
/// signatures in 0.8.0, when objects become `Value::Object` and strings and arrays are shared.
impl Value {
    /// Parse JSON text into a value, exactly as the `evaluate_with_json*` entry points read each
    /// variable: numbers become `f64` [`Value::Number`]s, strings [`Value::String`], arrays
    /// [`Value::Array`], and objects the deprecated [`Value::Json`] (until 0.8.0), holding the
    /// object's JSON text with its keys sorted.
    ///
    /// A number outside an object is an `f64`, so an integer beyond 2^53 is rounded; inside an
    /// object it keeps its digits (the object is held as text). Invalid JSON, or nesting deeper
    /// than 128, is an `Invalid JSON: …` error.
    ///
    /// ```rust
    /// use skillet::Value;
    ///
    /// assert_eq!(Value::from_json_text("[1, \"a\", true, null]").unwrap(), Value::from(vec![
    ///     Value::from(1.0),
    ///     Value::from("a"),
    ///     Value::from(true),
    ///     Value::Null,
    /// ]));
    ///
    /// let obj = Value::from_json_text(r#"{"b": 2, "a": 1}"#).unwrap();
    /// assert_eq!(obj.to_json_text().unwrap(), r#"{"a":1,"b":2}"#);
    ///
    /// let err = Value::from_json_text("{oops}").unwrap_err();
    /// assert!(err.message.starts_with("Invalid JSON: "));
    /// ```
    pub fn from_json_text(text: &str) -> Result<Value, crate::Error> {
        crate::json_to_value(parse_json(text)?)
    }

    /// The value's JSON text: what `.to_json()` writes (ADR-0034 §4, ADR-0030 decision d).
    /// Integral numbers below 2^53 print as integers (`5`), other numbers as `serde_json` prints
    /// an `f64` (`1.5`, `1e+21`); a DateTime is its text in quotes; an object is its JSON with
    /// keys sorted. One difference: `NULL.to_json()` is `{}` (documented method behaviour, kept),
    /// while this returns `null`, so the text always parses back with [`Value::from_json_text`].
    ///
    /// Errors: a non-finite number (`NaN`, infinity: only a value built in Rust can hold one), a
    /// `Json` value whose text is not JSON, or, when called inside an evaluation (for example
    /// from a [`crate::CustomFunction`]), a text larger than the remaining budget. Outside an
    /// evaluation nothing is charged.
    ///
    /// ```rust
    /// use skillet::{evaluate, Value};
    ///
    /// let v = evaluate("[5, 1.5, 'a\"b', NULL]").unwrap();
    /// assert_eq!(v.to_json_text().unwrap(), r#"[5,1.5,"a\"b",null]"#);
    /// assert!(Value::from(f64::NAN).to_json_text().is_err());
    /// ```
    pub fn to_json_text(&self) -> Result<String, crate::Error> {
        crate::runtime::text::json_text(self)
    }

    /// The text of a [`Value::String`]; `None` for every other value.
    ///
    /// ```rust
    /// use skillet::Value;
    ///
    /// assert_eq!(Value::from("abc").as_str(), Some("abc"));
    /// assert_eq!(Value::from(1.0).as_str(), None);
    /// ```
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    /// The items of a [`Value::Array`]; `None` for every other value. A slice, so the call keeps
    /// compiling when 0.8.0 changes the array's storage.
    ///
    /// ```rust
    /// use skillet::{evaluate, Value};
    ///
    /// let v = evaluate("[1, 2, 3]").unwrap();
    /// assert_eq!(v.as_array().map(<[Value]>::len), Some(3));
    /// assert_eq!(Value::Null.as_array(), None);
    /// ```
    pub fn as_array(&self) -> Option<&[Value]> {
        match self {
            Value::Array(items) => Some(items),
            _ => None,
        }
    }
}

/// Parse JSON text into a `serde_json` tree; the error is `Invalid JSON: …` (shared by
/// [`Value::from_json_text`] and the `evaluate_with_json*` entry points).
pub(crate) fn parse_json(text: &str) -> Result<serde_json::Value, crate::Error> {
    serde_json::from_str(text).map_err(|e| crate::Error::new(format!("Invalid JSON: {}", e), None))
}

/// ```rust
/// use skillet::Value;
///
/// assert_eq!(Value::from(String::from("a")), Value::String("a".to_string()));
/// ```
impl From<String> for Value {
    fn from(s: String) -> Value {
        Value::String(s)
    }
}

/// ```rust
/// use skillet::Value;
///
/// assert_eq!(Value::from("a"), Value::String("a".to_string()));
/// ```
impl From<&str> for Value {
    fn from(s: &str) -> Value {
        Value::String(s.to_owned())
    }
}

/// ```rust
/// use skillet::Value;
///
/// let v = Value::from(vec![Value::from(1.0), Value::Null]);
/// assert_eq!(v, Value::Array(vec![Value::Number(1.0), Value::Null]));
/// ```
impl From<Vec<Value>> for Value {
    fn from(items: Vec<Value>) -> Value {
        Value::Array(items)
    }
}

/// ```rust
/// use skillet::Value;
///
/// assert_eq!(Value::from(2.5), Value::Number(2.5));
/// ```
impl From<f64> for Value {
    fn from(n: f64) -> Value {
        Value::Number(n)
    }
}

/// ```rust
/// use skillet::Value;
///
/// assert_eq!(Value::from(true), Value::Boolean(true));
/// ```
impl From<bool> for Value {
    fn from(b: bool) -> Value {
        Value::Boolean(b)
    }
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
