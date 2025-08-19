pub mod ast;
pub mod custom;
pub mod error;
pub mod js_plugin;
pub mod lexer;
pub mod parser;
pub mod runtime;
pub mod types;

pub use ast::Expr;
pub use custom::{CustomFunction, FunctionRegistry};
pub use error::Error;
pub use js_plugin::{JavaScriptFunction, JSPluginLoader};
pub use types::Value;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use serde_json;

// Global function registry
lazy_static::lazy_static! {
    static ref GLOBAL_REGISTRY: Arc<RwLock<FunctionRegistry>> = Arc::new(RwLock::new(FunctionRegistry::new()));
}

/// Parse an arithmetic expression (optional leading '=') into an AST.
pub fn parse(input: &str) -> Result<Expr, Error> {
    // Allow optional leading '=' after whitespace
    let trimmed = input.trim_start();
    let input2: std::borrow::Cow<'_, str> = if let Some(rest) = trimmed.strip_prefix('=') { std::borrow::Cow::from(rest) } else { std::borrow::Cow::from(input) };
    let mut parser = parser::Parser::new(&input2);
    parser.parse()
}

/// Evaluate an arithmetic expression to f64.
pub fn evaluate(input: &str) -> Result<Value, Error> {
    let expr = parse(input)?;
    runtime::eval(&expr)
}

/// Evaluate with a map of numeric variables and built-in functions.
pub fn evaluate_with(input: &str, vars: &HashMap<String, Value>) -> Result<Value, Error> {
    let expr = parse(input)?;
    runtime::eval_with_vars(&expr, vars)
}

/// Evaluate with variables provided as JSON string.
/// JSON format: {"var1": "value1", "var2": 42, "var3": true}
/// Supports flat JSON structure with automatic type conversion.
pub fn evaluate_with_json(input: &str, json_vars: &str) -> Result<Value, Error> {
    let json_value: serde_json::Value = serde_json::from_str(json_vars)
        .map_err(|e| Error::new(format!("Invalid JSON: {}", e), None))?;
    
    let vars = match json_value {
        serde_json::Value::Object(map) => {
            let mut result = HashMap::new();
            for (key, value) in map {
                let skillet_value = json_to_value(value)?;
                result.insert(key, skillet_value);
            }
            result
        }
        _ => return Err(Error::new("JSON must be an object with key-value pairs", None)),
    };
    
    evaluate_with(input, &vars)
}

/// Convert serde_json::Value to skillet::Value with type inference
fn json_to_value(json: serde_json::Value) -> Result<Value, Error> {
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

/// Register a custom function globally
pub fn register_function(function: Box<dyn CustomFunction>) -> Result<(), Error> {
    let mut registry = GLOBAL_REGISTRY.write()
        .map_err(|_| Error::new("Failed to acquire registry lock", None))?;
    registry.register(function)
}

/// Unregister a custom function by name
pub fn unregister_function(name: &str) -> bool {
    if let Ok(mut registry) = GLOBAL_REGISTRY.write() {
        registry.unregister(name)
    } else {
        false
    }
}

/// List all registered custom functions
pub fn list_custom_functions() -> Vec<String> {
    if let Ok(registry) = GLOBAL_REGISTRY.read() {
        registry.list_functions().iter().map(|s| s.to_string()).collect()
    } else {
        Vec::new()
    }
}

/// Check if a custom function is registered
pub fn has_custom_function(name: &str) -> bool {
    if let Ok(registry) = GLOBAL_REGISTRY.read() {
        registry.has_function(name)
    } else {
        false
    }
}

/// Evaluate with custom functions support
pub fn evaluate_with_custom(input: &str, vars: &HashMap<String, Value>) -> Result<Value, Error> {
    let expr = parse(input)?;
    runtime::eval_with_vars_and_custom(&expr, vars, &GLOBAL_REGISTRY)
}

/// Evaluate with JSON and custom functions support
pub fn evaluate_with_json_custom(input: &str, json_vars: &str) -> Result<Value, Error> {
    let json_value: serde_json::Value = serde_json::from_str(json_vars)
        .map_err(|e| Error::new(format!("Invalid JSON: {}", e), None))?;
    
    let vars = match json_value {
        serde_json::Value::Object(map) => {
            let mut result = HashMap::new();
            for (key, value) in map {
                let skillet_value = json_to_value(value)?;
                result.insert(key, skillet_value);
            }
            result
        }
        _ => return Err(Error::new("JSON must be an object with key-value pairs", None)),
    };
    
    evaluate_with_custom(input, &vars)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approxv(v: Value, b: f64) -> bool { matches!(v, Value::Number(a) if (a - b).abs() < 1e-9) }

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
