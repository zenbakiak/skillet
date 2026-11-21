//! Skillet: an embeddable expression language with Excel-style functions, Ruby-style methods,
//! null-safe navigation, JSONPath and optional JavaScript plugins.
//!
//! Formulas are parsed into an [`Expr`] tree and evaluated to a [`Value`]. A leading `=` is
//! optional. Variables use the `:name` sigil, builtins are UPPERCASE and methods are lowercase.
//!
//! ```rust
//! use std::collections::HashMap;
//! use skillet::{evaluate, evaluate_with, Value};
//!
//! assert_eq!(evaluate("=SUM(1, 2, 3) * 2").unwrap(), Value::Number(12.0));
//!
//! let mut vars = HashMap::new();
//! vars.insert("nums".to_string(), Value::Array(vec![
//!     Value::Number(1.0), Value::Number(5.0), Value::Number(9.0),
//! ]));
//! assert_eq!(evaluate_with(":nums.filter(:x > 2).sum()", &vars).unwrap(), Value::Number(14.0));
//! ```
//!
//! Entry points, from simplest to most capable:
//!
//! | Function | Variables | Custom functions | Assignments (`:a := …; …`) |
//! |---|---|---|---|
//! | [`evaluate`] | none | no | no |
//! | [`evaluate_with`] | `HashMap` | no | no |
//! | [`evaluate_with_json`] | JSON object | no | no |
//! | [`evaluate_with_custom`] | `HashMap` | global registry | no |
//! | [`evaluate_with_json_custom`] | JSON object | global registry | no |
//! | [`evaluate_with_assignments`] | `HashMap` | no | yes |
//! | [`evaluate_with_assignments_and_context`] | `HashMap` | no | yes, returns the variables |
//!
//! Each of [`evaluate`], [`evaluate_with`], [`evaluate_with_custom`],
//! [`evaluate_with_assignments`] and [`evaluate_with_assignments_and_context`] has an
//! `evaluate_parsed*` twin ([`evaluate_parsed`], [`evaluate_parsed_with`], …) that takes the
//! [`Expr`] returned by [`parse`], so a formula used many times is parsed once.
//!
//! Every evaluation runs under the limits in [`MAX_NESTING_DEPTH`] and [`MAX_EVAL_BYTES`]; use
//! [`with_limits`] to tighten them or add a deadline. See `docs/guide/limits.md`.

/// Abstract syntax tree produced by [`parse`].
pub mod ast;
/// The [`CustomFunction`] trait and the [`FunctionRegistry`] that holds custom functions.
pub mod custom;
/// The [`Error`] type returned by parsing and evaluation.
pub mod error;
/// JavaScript plugins run by QuickJS (`plugins` feature, on by default).
#[cfg(feature = "plugins")]
pub mod js_plugin;
/// Tokenizer used by the parser.
pub mod lexer;
mod limits;
/// Recursive-descent parser; most callers use [`parse`] instead.
pub mod parser;
/// The evaluator and the builtin function and method implementations.
pub mod runtime;
/// The [`Value`] type that every evaluation returns.
pub mod types;

pub use ast::Expr;
pub use custom::{CustomFunction, FunctionRegistry};
pub use error::Error;
#[cfg(feature = "plugins")]
pub use js_plugin::{JSPluginLoader, JavaScriptFunction};
pub use limits::{with_limits, EvalLimits, MAX_EVAL_BYTES, MAX_NESTING_DEPTH};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
pub use types::Value;

/// Sanitize JSON keys by replacing special characters with underscores
fn sanitize_json_key(key: &str) -> String {
    key.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Build the variable map for the `*_json*` entry points: the raw JSON is
/// exposed as `:arguments` (for `JQ`) and each top-level key becomes a
/// sanitized variable.
fn json_vars_to_map(json_vars: &str) -> Result<HashMap<String, Value>, Error> {
    let json_value: serde_json::Value = serde_json::from_str(json_vars)
        .map_err(|e| Error::new(format!("Invalid JSON: {}", e), None))?;
    let serde_json::Value::Object(map) = json_value else {
        return Err(Error::new(
            "JSON must be an object with key-value pairs",
            None,
        ));
    };

    let mut vars = HashMap::with_capacity(map.len() + 1);
    // Add the original JSON data for JQ function
    vars.insert("arguments".to_string(), Value::Json(json_vars.to_string()));
    for (key, value) in map {
        vars.insert(sanitize_json_key(&key), json_to_value(value)?);
    }
    Ok(vars)
}

// Global function registry
lazy_static::lazy_static! {
    static ref GLOBAL_REGISTRY: Arc<RwLock<FunctionRegistry>> = Arc::new(RwLock::new(FunctionRegistry::new()));
}

/// Parse a Skillet expression into an [`Expr`] tree without evaluating it.
///
/// Accepts the full language (operators, function and method calls, lambdas, variables,
/// assignments and `;` sequences, object and array literals, casts, …). Leading whitespace and
/// one leading `=` are ignored. Syntax errors and expressions nested deeper than
/// [`MAX_NESTING_DEPTH`] return an [`Error`], usually with the byte position of the problem.
///
/// ```rust
/// use skillet::{parse, Expr};
///
/// // Precedence: `*` binds tighter than `+`, so the root is the `+`.
/// let expr = parse("=2 + 3 * 4").unwrap();
/// assert!(matches!(expr, Expr::Binary(_, skillet::ast::BinaryOp::Add, _)));
///
/// assert_eq!(parse(":total").unwrap(), Expr::Variable("total".to_string()));
///
/// let err = parse("2 +").unwrap_err();
/// assert!(err.position.is_some());
/// ```
pub fn parse(input: &str) -> Result<Expr, Error> {
    // Allow optional leading '=' after whitespace
    let trimmed = input.trim_start();
    let input2: std::borrow::Cow<'_, str> = if let Some(rest) = trimmed.strip_prefix('=') {
        std::borrow::Cow::from(rest)
    } else {
        std::borrow::Cow::from(input)
    };
    let mut parser = parser::Parser::new(&input2);
    parser.parse()
}

/// Parse and evaluate an expression that uses no variables.
///
/// Returns the resulting [`Value`] (a number, string, array, …). A formula that reads a
/// variable fails with `Missing variable: :name`; use [`evaluate_with`] to pass variables.
///
/// ```rust
/// use skillet::{evaluate, Value};
///
/// assert_eq!(evaluate("=2 + 3 * 4").unwrap(), Value::Number(14.0));
/// assert_eq!(evaluate("UPPER('abc')").unwrap(), Value::String("ABC".to_string()));
/// assert_eq!(
///     evaluate("[3, 1, 2].sort()").unwrap(),
///     Value::Array(vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)])
/// );
/// assert_eq!(evaluate(":price").unwrap_err().message, "Missing variable: :price");
/// ```
pub fn evaluate(input: &str) -> Result<Value, Error> {
    evaluate_parsed(&parse(input)?)
}

/// Evaluate an expression with variables.
///
/// Keys are variable names without the `:` sigil; values can be any [`Value`], not only
/// numbers. Custom functions are not visible here; use [`evaluate_with_custom`] for those.
///
/// ```rust
/// use std::collections::HashMap;
/// use skillet::{evaluate_with, Value};
///
/// let mut vars = HashMap::new();
/// vars.insert("price".to_string(), Value::Number(2.5));
/// vars.insert("qty".to_string(), Value::Number(4.0));
/// vars.insert("name".to_string(), Value::String("ada".to_string()));
///
/// assert_eq!(evaluate_with(":price * :qty", &vars).unwrap(), Value::Number(10.0));
/// assert_eq!(evaluate_with(":name.upper()", &vars).unwrap(), Value::String("ADA".to_string()));
/// ```
pub fn evaluate_with(input: &str, vars: &HashMap<String, Value>) -> Result<Value, Error> {
    evaluate_parsed_with(&parse(input)?, vars)
}

/// Evaluate an expression with variables given as a JSON object.
///
/// Each top-level key becomes a variable, converted with [`json_to_value`] (nested objects
/// become [`Value::Json`] and support property access such as `:user.name`). Characters other
/// than letters, digits and `_` in a key are replaced by `_`, so `"first-name"` is read as
/// `:first_name`. The whole JSON text is also available as `:arguments`, for `JQ`.
///
/// The input must be a JSON object; anything else is an error.
///
/// ```rust
/// use skillet::{evaluate_with_json, Value};
///
/// let json = r#"{"first-name": "Ada", "user": {"age": 36}, "tags": ["a", "b"]}"#;
///
/// assert_eq!(evaluate_with_json(":first_name", json).unwrap(), Value::String("Ada".into()));
/// assert_eq!(evaluate_with_json(":user.age + 1", json).unwrap(), Value::Number(37.0));
/// assert_eq!(evaluate_with_json(":tags.length()", json).unwrap(), Value::Number(2.0));
/// assert_eq!(
///     evaluate_with_json("JQ(:arguments, '$.user.age')", json).unwrap(),
///     Value::Number(36.0)
/// );
/// assert_eq!(
///     evaluate_with_json("1", "[1, 2]").unwrap_err().message,
///     "JSON must be an object with key-value pairs"
/// );
/// ```
pub fn evaluate_with_json(input: &str, json_vars: &str) -> Result<Value, Error> {
    evaluate_with(input, &json_vars_to_map(json_vars)?)
}

/// Convert a `serde_json::Value` into a Skillet [`Value`].
///
/// `null`, booleans, numbers, strings and arrays map to [`Value::Null`], [`Value::Boolean`],
/// [`Value::Number`] (always `f64`), [`Value::String`] and [`Value::Array`]. Objects are kept as
/// JSON text in [`Value::Json`].
///
/// ```rust
/// use skillet::{json_to_value, Value};
///
/// let v = json_to_value(serde_json::json!([1, "a", true, null])).unwrap();
/// assert_eq!(v, Value::Array(vec![
///     Value::Number(1.0),
///     Value::String("a".to_string()),
///     Value::Boolean(true),
///     Value::Null,
/// ]));
///
/// let obj = json_to_value(serde_json::json!({"a": 1})).unwrap();
/// assert_eq!(obj, Value::Json(r#"{"a":1}"#.to_string()));
/// ```
pub fn json_to_value(json: serde_json::Value) -> Result<Value, Error> {
    match json {
        serde_json::Value::Null => Ok(Value::Null),
        serde_json::Value::Bool(b) => Ok(Value::Boolean(b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::Number(i as f64))
            } else if let Some(f) = n.as_f64() {
                Ok(Value::Number(f))
            } else {
                Err(Error::new("Invalid number in JSON", None))
            }
        }
        serde_json::Value::String(s) => Ok(Value::String(s)),
        serde_json::Value::Array(arr) => {
            let mut result = Vec::new();
            for item in arr {
                result.push(json_to_value(item)?);
            }
            Ok(Value::Array(result))
        }
        serde_json::Value::Object(_) => {
            // For nested objects, convert to JSON string
            let json_str = serde_json::to_string(&json)
                .map_err(|e| Error::new(format!("Failed to serialize JSON object: {}", e), None))?;
            Ok(Value::Json(json_str))
        }
    }
}

/// Register a custom function in the process-wide registry.
///
/// Names are case-insensitive and stored in uppercase; registering a name again replaces the
/// earlier function. The function is visible to [`evaluate_with_custom`] and
/// [`evaluate_with_json_custom`], not to [`evaluate`] or [`evaluate_with`]. Returns an error if
/// the name is empty or `min_args > max_args`.
///
/// ```rust
/// use skillet::{CustomFunction, Error, Value};
///
/// struct Double;
///
/// impl CustomFunction for Double {
///     fn name(&self) -> &str { "DOUBLE" }
///     fn min_args(&self) -> usize { 1 }
///     fn max_args(&self) -> Option<usize> { Some(1) }
///     fn execute(&self, args: Vec<Value>) -> Result<Value, Error> {
///         let n = args[0].as_number().ok_or_else(|| Error::new("DOUBLE expects a number", None))?;
///         Ok(Value::Number(n * 2.0))
///     }
/// }
///
/// skillet::register_function(Box::new(Double)).unwrap();
///
/// let vars = std::collections::HashMap::new();
/// assert_eq!(skillet::evaluate_with_custom("DOUBLE(21)", &vars).unwrap(), Value::Number(42.0));
/// assert!(skillet::evaluate("DOUBLE(21)").is_err()); // not visible without `_custom`
/// ```
pub fn register_function(function: Box<dyn CustomFunction>) -> Result<(), Error> {
    let mut registry = GLOBAL_REGISTRY
        .write()
        .map_err(|_| Error::new("Failed to acquire registry lock", None))?;
    registry.register(function)
}

/// Remove a custom function from the process-wide registry.
///
/// The name is case-insensitive. Returns `true` if a function was removed.
///
/// ```rust
/// use skillet::{CustomFunction, Error, Value};
///
/// struct Double;
///
/// impl CustomFunction for Double {
///     fn name(&self) -> &str { "DOUBLE" }
///     fn min_args(&self) -> usize { 1 }
///     fn max_args(&self) -> Option<usize> { Some(1) }
///     fn execute(&self, args: Vec<Value>) -> Result<Value, Error> {
///         let n = args[0].as_number().ok_or_else(|| Error::new("DOUBLE expects a number", None))?;
///         Ok(Value::Number(n * 2.0))
///     }
/// }
///
/// skillet::register_function(Box::new(Double)).unwrap();
/// assert!(skillet::unregister_function("double"));
/// assert!(!skillet::unregister_function("double")); // already gone
/// assert!(!skillet::has_custom_function("DOUBLE"));
/// ```
pub fn unregister_function(name: &str) -> bool {
    if let Ok(mut registry) = GLOBAL_REGISTRY.write() {
        registry.unregister(name)
    } else {
        false
    }
}

/// List the names of all functions in the process-wide registry, in uppercase.
///
/// The order is unspecified.
///
/// ```rust
/// use skillet::{CustomFunction, Error, Value};
///
/// struct Double;
///
/// impl CustomFunction for Double {
///     fn name(&self) -> &str { "DOUBLE" }
///     fn min_args(&self) -> usize { 1 }
///     fn max_args(&self) -> Option<usize> { Some(1) }
///     fn execute(&self, args: Vec<Value>) -> Result<Value, Error> {
///         let n = args[0].as_number().ok_or_else(|| Error::new("DOUBLE expects a number", None))?;
///         Ok(Value::Number(n * 2.0))
///     }
/// }
///
/// skillet::register_function(Box::new(Double)).unwrap();
/// assert!(skillet::list_custom_functions().contains(&"DOUBLE".to_string()));
/// ```
pub fn list_custom_functions() -> Vec<String> {
    if let Ok(registry) = GLOBAL_REGISTRY.read() {
        registry
            .list_functions()
            .iter()
            .map(|s| s.to_string())
            .collect()
    } else {
        Vec::new()
    }
}

/// Check whether a function is in the process-wide registry (case-insensitive).
///
/// Only custom functions count; builtins such as `SUM` return `false`.
///
/// ```rust
/// use skillet::{CustomFunction, Error, Value};
///
/// struct Double;
///
/// impl CustomFunction for Double {
///     fn name(&self) -> &str { "DOUBLE" }
///     fn min_args(&self) -> usize { 1 }
///     fn max_args(&self) -> Option<usize> { Some(1) }
///     fn execute(&self, args: Vec<Value>) -> Result<Value, Error> {
///         let n = args[0].as_number().ok_or_else(|| Error::new("DOUBLE expects a number", None))?;
///         Ok(Value::Number(n * 2.0))
///     }
/// }
///
/// assert!(!skillet::has_custom_function("DOUBLE"));
/// skillet::register_function(Box::new(Double)).unwrap();
/// assert!(skillet::has_custom_function("double"));
/// assert!(!skillet::has_custom_function("SUM"));
/// ```
pub fn has_custom_function(name: &str) -> bool {
    if let Ok(registry) = GLOBAL_REGISTRY.read() {
        registry.has_function(name)
    } else {
        false
    }
}

/// Evaluate an expression with variables and the custom functions in the process-wide
/// registry (see [`register_function`]).
///
/// A custom function with the same name as a builtin takes precedence over the builtin.
///
/// ```rust
/// use std::collections::HashMap;
/// use skillet::{CustomFunction, Error, Value};
///
/// struct Double;
///
/// impl CustomFunction for Double {
///     fn name(&self) -> &str { "DOUBLE" }
///     fn min_args(&self) -> usize { 1 }
///     fn max_args(&self) -> Option<usize> { Some(1) }
///     fn execute(&self, args: Vec<Value>) -> Result<Value, Error> {
///         let n = args[0].as_number().ok_or_else(|| Error::new("DOUBLE expects a number", None))?;
///         Ok(Value::Number(n * 2.0))
///     }
/// }
///
/// skillet::register_function(Box::new(Double)).unwrap();
///
/// let mut vars = HashMap::new();
/// vars.insert("n".to_string(), Value::Number(5.0));
/// assert_eq!(
///     skillet::evaluate_with_custom("DOUBLE(:n) + 1", &vars).unwrap(),
///     Value::Number(11.0)
/// );
/// ```
pub fn evaluate_with_custom(input: &str, vars: &HashMap<String, Value>) -> Result<Value, Error> {
    evaluate_parsed_with_custom(&parse(input)?, vars)
}

/// Evaluate an expression with JSON variables and the custom functions in the process-wide
/// registry.
///
/// JSON handling is the same as [`evaluate_with_json`] (key sanitizing, `:arguments`); function
/// lookup is the same as [`evaluate_with_custom`].
///
/// ```rust
/// use skillet::{CustomFunction, Error, Value};
///
/// struct Double;
///
/// impl CustomFunction for Double {
///     fn name(&self) -> &str { "DOUBLE" }
///     fn min_args(&self) -> usize { 1 }
///     fn max_args(&self) -> Option<usize> { Some(1) }
///     fn execute(&self, args: Vec<Value>) -> Result<Value, Error> {
///         let n = args[0].as_number().ok_or_else(|| Error::new("DOUBLE expects a number", None))?;
///         Ok(Value::Number(n * 2.0))
///     }
/// }
///
/// skillet::register_function(Box::new(Double)).unwrap();
/// assert_eq!(
///     skillet::evaluate_with_json_custom("DOUBLE(:order.total)", r#"{"order": {"total": 4}}"#)
///         .unwrap(),
///     Value::Number(8.0)
/// );
/// ```
pub fn evaluate_with_json_custom(input: &str, json_vars: &str) -> Result<Value, Error> {
    evaluate_with_custom(input, &json_vars_to_map(json_vars)?)
}

/// Evaluate an expression that may assign variables (`:name := value`) and chain steps with `;`.
///
/// Steps run left to right; later steps see earlier assignments, and the result is the value of
/// the last step. `vars` is not modified (the evaluator works on a copy).
///
/// ```rust
/// use std::collections::HashMap;
/// use skillet::{evaluate_with_assignments, Value};
///
/// let mut vars = HashMap::new();
/// vars.insert("base".to_string(), Value::Number(10.0));
///
/// let result = evaluate_with_assignments(":tax := :base * 0.5; :base + :tax", &vars).unwrap();
/// assert_eq!(result, Value::Number(15.0));
/// assert!(!vars.contains_key("tax"));
/// ```
pub fn evaluate_with_assignments(
    input: &str,
    vars: &HashMap<String, Value>,
) -> Result<Value, Error> {
    evaluate_parsed_with_assignments(&parse(input)?, vars)
}

/// Like [`evaluate_with_assignments`], but also return the variables after evaluation: the
/// input variables plus every variable the expression assigned.
///
/// ```rust
/// use std::collections::HashMap;
/// use skillet::{evaluate_with_assignments_and_context, Value};
///
/// let vars = HashMap::new();
/// let (result, out) =
///     evaluate_with_assignments_and_context(":a := 2; :b := :a * 3; :b + 1", &vars).unwrap();
///
/// assert_eq!(result, Value::Number(7.0));
/// assert_eq!(out.get("a"), Some(&Value::Number(2.0)));
/// assert_eq!(out.get("b"), Some(&Value::Number(6.0)));
/// ```
pub fn evaluate_with_assignments_and_context(
    input: &str,
    vars: &HashMap<String, Value>,
) -> Result<(Value, HashMap<String, Value>), Error> {
    evaluate_parsed_with_assignments_and_context(&parse(input)?, vars)
}

/// Evaluate an expression that [`parse`] already parsed, with no variables.
///
/// The `evaluate_parsed*` functions are the second half of the matching `evaluate*` function:
/// `evaluate(text)` is exactly `evaluate_parsed(&parse(text)?)`, with the same result and the
/// same errors. Parse once and evaluate many times when the same formula runs with different
/// variables; this saves the parse, which is most of the cost for short formulas and ~25 µs for
/// a 1.8 KB one (`benches/BASELINE.md`).
///
/// Limits are the same as for a text evaluation: [`parse`] rejects input nested deeper than
/// [`MAX_NESTING_DEPTH`], and each `evaluate_parsed*` call runs under the evaluation budget
/// ([`MAX_EVAL_BYTES`]) and any deadline set with [`with_limits`], counted from zero for each call
/// (unless the call runs inside an outer scope). Pass trees that came from [`parse`]: a tree built
/// by hand is not checked for depth, and a very deep one can overflow the stack.
///
/// ```rust
/// use skillet::{evaluate_parsed, parse, Value};
///
/// let expr = parse("=SUM(1, 2, 3) * 2").unwrap();
/// assert_eq!(evaluate_parsed(&expr).unwrap(), Value::Number(12.0));
/// assert_eq!(evaluate_parsed(&expr).unwrap(), Value::Number(12.0)); // reusable
/// ```
pub fn evaluate_parsed(expr: &Expr) -> Result<Value, Error> {
    runtime::evaluator::eval(expr)
}

/// Evaluate a parsed expression with variables: [`evaluate_with`] without the parse.
///
/// See [`evaluate_parsed`] for the limits that apply.
///
/// ```rust
/// use std::collections::HashMap;
/// use skillet::{evaluate_parsed_with, parse, Value};
///
/// let expr = parse(":price * :qty").unwrap();
/// for (price, total) in [(2.5, 10.0), (3.0, 12.0)] {
///     let mut vars = HashMap::new();
///     vars.insert("price".to_string(), Value::Number(price));
///     vars.insert("qty".to_string(), Value::Number(4.0));
///     assert_eq!(evaluate_parsed_with(&expr, &vars).unwrap(), Value::Number(total));
/// }
/// ```
pub fn evaluate_parsed_with(expr: &Expr, vars: &HashMap<String, Value>) -> Result<Value, Error> {
    runtime::evaluator::eval_with_vars(expr, vars)
}

/// Evaluate a parsed expression with variables and the process-wide custom functions:
/// [`evaluate_with_custom`] without the parse.
///
/// Functions are looked up when the expression is evaluated, not when it is parsed, so a
/// function registered after [`parse`] is visible. See [`evaluate_parsed`] for the limits.
///
/// ```rust
/// use std::collections::HashMap;
/// use skillet::{evaluate_parsed_with_custom, parse, CustomFunction, Error, Value};
///
/// struct Triple;
///
/// impl CustomFunction for Triple {
///     fn name(&self) -> &str { "TRIPLE" }
///     fn min_args(&self) -> usize { 1 }
///     fn max_args(&self) -> Option<usize> { Some(1) }
///     fn execute(&self, args: Vec<Value>) -> Result<Value, Error> {
///         let n = args[0].as_number().ok_or_else(|| Error::new("TRIPLE expects a number", None))?;
///         Ok(Value::Number(n * 3.0))
///     }
/// }
///
/// let expr = parse("TRIPLE(:n) + 1").unwrap();
/// skillet::register_function(Box::new(Triple)).unwrap();
///
/// let mut vars = HashMap::new();
/// vars.insert("n".to_string(), Value::Number(5.0));
/// assert_eq!(evaluate_parsed_with_custom(&expr, &vars).unwrap(), Value::Number(16.0));
/// ```
pub fn evaluate_parsed_with_custom(
    expr: &Expr,
    vars: &HashMap<String, Value>,
) -> Result<Value, Error> {
    runtime::evaluator::eval_with_vars_and_custom(expr, vars, &GLOBAL_REGISTRY)
}

/// Evaluate a parsed expression that may assign variables and chain steps with `;`:
/// [`evaluate_with_assignments`] without the parse.
///
/// `vars` is not modified. See [`evaluate_parsed`] for the limits.
///
/// ```rust
/// use std::collections::HashMap;
/// use skillet::{evaluate_parsed_with_assignments, parse, Value};
///
/// let expr = parse(":tax := :base * 0.5; :base + :tax").unwrap();
/// let mut vars = HashMap::new();
/// vars.insert("base".to_string(), Value::Number(10.0));
/// assert_eq!(evaluate_parsed_with_assignments(&expr, &vars).unwrap(), Value::Number(15.0));
/// ```
pub fn evaluate_parsed_with_assignments(
    expr: &Expr,
    vars: &HashMap<String, Value>,
) -> Result<Value, Error> {
    runtime::evaluator::eval_with_assignments(expr, vars)
}

/// Like [`evaluate_parsed_with_assignments`], but also return the variables after evaluation:
/// [`evaluate_with_assignments_and_context`] without the parse.
///
/// ```rust
/// use std::collections::HashMap;
/// use skillet::{evaluate_parsed_with_assignments_and_context, parse, Value};
///
/// let expr = parse(":a := 2; :b := :a * 3; :b + 1").unwrap();
/// let (result, out) =
///     evaluate_parsed_with_assignments_and_context(&expr, &HashMap::new()).unwrap();
/// assert_eq!(result, Value::Number(7.0));
/// assert_eq!(out.get("b"), Some(&Value::Number(6.0)));
/// ```
pub fn evaluate_parsed_with_assignments_and_context(
    expr: &Expr,
    vars: &HashMap<String, Value>,
) -> Result<(Value, HashMap<String, Value>), Error> {
    runtime::evaluator::eval_with_assignments_and_context(expr, vars)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approxv(v: Value, b: f64) -> bool {
        matches!(v, Value::Number(a) if (a - b).abs() < 1e-9)
    }

    #[test]
    fn test_basic_arithmetic() {
        assert!(approxv(evaluate("2 + 3 * 4").unwrap(), 14.0));
        assert!(approxv(evaluate("(2 + 3) * 4").unwrap(), 20.0));
        assert!(approxv(evaluate("2 ^ 3").unwrap(), 8.0));
        assert!(approxv(evaluate("2 ^ 3 ^ 2").unwrap(), 512.0));
        assert!(approxv(evaluate("-3 ^ 2").unwrap(), -9.0));
        assert!(approxv(evaluate("(-3) ^ 2").unwrap(), 9.0));
        assert!(approxv(evaluate("= 10 + 20 * 3").unwrap(), 70.0));
        assert!(approxv(evaluate("= (10 + 20) * 3").unwrap(), 90.0));
    }
}
