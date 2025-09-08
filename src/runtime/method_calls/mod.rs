//! Method calls module - refactored into smaller, focused sub-modules

pub mod predicates;
pub mod string_methods;
pub mod array_methods;
pub mod lambda_methods;
pub mod conversion_methods;

use crate::ast::Expr;
use crate::custom::FunctionRegistry;
use crate::error::Error;
use crate::types::Value;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub use predicates::exec_predicate;
pub use string_methods::exec_string_method;
pub use array_methods::exec_array_method;
pub use lambda_methods::{exec_filter, exec_map, exec_find, exec_reduce};
pub use conversion_methods::exec_conversion_method;

/// Main method dispatch function with improved architecture
pub fn exec_method(
    name: &str,
    predicate: bool,
    recv: &Value,
    args_expr: &[Expr],
    base_vars: Option<&HashMap<String, Value>>,
) -> Result<Value, Error> {
    if predicate {
        return exec_predicate(name, recv);
    }
    
    // Check for conversion methods first (available on all types)
    let lname = name.to_lowercase();
    match lname.as_str() {
        "to_s" | "to_string" | "to_i" | "to_int" | "to_f" | "to_float" | 
        "to_a" | "to_array" | "to_json" | "to_bool" | "to_boolean" => {
            return exec_conversion_method(name, recv);
        }
        _ => {}
    }
    
    // Dispatch to appropriate method handler based on receiver type
    match recv {
        Value::String(_) => exec_string_method(name, recv, args_expr, base_vars),
        Value::Array(_) => {
            // Check for higher-order functions first
            match name.to_lowercase().as_str() {
                "filter" => exec_filter(recv, args_expr, base_vars),
                "map" => exec_map(recv, args_expr, base_vars),
                "find" => exec_find(recv, args_expr, base_vars),
                "reduce" => exec_reduce(recv, args_expr, base_vars),
                _ => exec_array_method(name, recv, args_expr, base_vars),
            }
        }
        Value::Number(_) => exec_number_method(name, recv, args_expr, base_vars),
        Value::Json(_) => exec_json_method(name, recv, args_expr, base_vars),
        _ => Err(Error::new(
            format!("No methods available for {:?} type", recv),
            None,
        )),
    }
}

/// Main method dispatch function with custom function support
pub fn exec_method_with_custom(
    name: &str,
    predicate: bool,
    recv: &Value,
    args_expr: &[Expr],
    base_vars: Option<&HashMap<String, Value>>,
    custom_registry: &Arc<RwLock<FunctionRegistry>>,
) -> Result<Value, Error> {
    if predicate {
        return exec_predicate(name, recv);
    }
    
    // Check for conversion methods first (available on all types)
    let lname = name.to_lowercase();
    match lname.as_str() {
        "to_s" | "to_string" | "to_i" | "to_int" | "to_f" | "to_float" | 
        "to_a" | "to_array" | "to_json" | "to_bool" | "to_boolean" => {
            return exec_conversion_method(name, recv);
        }
        _ => {}
    }
    
    // First try method dispatch
    let method_result = match recv {
        Value::String(_) => exec_string_method(name, recv, args_expr, base_vars),
        Value::Array(_) => {
            // Check for higher-order functions first
            match name.to_lowercase().as_str() {
                "filter" => lambda_methods::exec_filter_with_custom(recv, args_expr, base_vars, custom_registry),
                "map" => lambda_methods::exec_map_with_custom(recv, args_expr, base_vars, custom_registry),
                "find" => lambda_methods::exec_find_with_custom(recv, args_expr, base_vars, custom_registry),
                "reduce" => lambda_methods::exec_reduce_with_custom(recv, args_expr, base_vars, custom_registry),
                _ => exec_array_method(name, recv, args_expr, base_vars),
            }
        }
        Value::Number(_) => exec_number_method(name, recv, args_expr, base_vars),
        Value::Json(_) => exec_json_method(name, recv, args_expr, base_vars),
        _ => Err(Error::new(
            format!("No methods available for {:?} type", recv),
            None,
        )),
    };
    
    method_result
}

/// Handle number method calls
fn exec_number_method(
    name: &str,
    recv: &Value,
    args_expr: &[Expr],
    base_vars: Option<&HashMap<String, Value>>,
) -> Result<Value, Error> {
    let num = match recv {
        Value::Number(n) => *n,
        Value::Currency(c) => *c,
        _ => return Err(Error::new("Method called on non-number", None)),
    };
    
    let lname = name.to_lowercase();
    
    match lname.as_str() {
        "abs" => Ok(Value::Number(num.abs())),
        "ceil" | "ceiling" => Ok(Value::Number(num.ceil())),
        "floor" => Ok(Value::Number(num.floor())),
        "round" => {
            if args_expr.is_empty() {
                Ok(Value::Number(num.round()))
            } else {
                use crate::runtime::evaluation::{eval, eval_with_vars};
                let precision_val = if let Some(vars) = base_vars {
                    eval_with_vars(&args_expr[0], vars)?
                } else {
                    eval(&args_expr[0])?
                };
                let precision = match precision_val {
                    Value::Number(p) => p as i32,
                    _ => return Err(Error::new("round precision must be number", None)),
                };
                
                if precision == 0 {
                    Ok(Value::Number(num.round()))
                } else {
                    let multiplier = 10f64.powi(precision);
                    Ok(Value::Number((num * multiplier).round() / multiplier))
                }
            }
        }
        "sqrt" => {
            if num < 0.0 {
                Err(Error::new("Cannot take square root of negative number", None))
            } else {
                Ok(Value::Number(num.sqrt()))
            }
        }
        "sin" => Ok(Value::Number(num.sin())),
        "cos" => Ok(Value::Number(num.cos())),
        "tan" => Ok(Value::Number(num.tan())),
        "int" => Ok(Value::Number(num.trunc())),
        "between" => {
            if args_expr.len() != 2 {
                return Err(Error::new("between expects 2 arguments: min, max", None));
            }
            
            use crate::runtime::evaluation::{eval, eval_with_vars};
            let min_val = if let Some(vars) = base_vars {
                eval_with_vars(&args_expr[0], vars)?
            } else {
                eval(&args_expr[0])?
            };
            let max_val = if let Some(vars) = base_vars {
                eval_with_vars(&args_expr[1], vars)?
            } else {
                eval(&args_expr[1])?
            };
            
            let min = match min_val {
                Value::Number(n) => n,
                Value::Currency(c) => c,
                _ => return Err(Error::new("between min must be a number", None)),
            };
            let max = match max_val {
                Value::Number(n) => n,
                Value::Currency(c) => c,
                _ => return Err(Error::new("between max must be a number", None)),
            };
            
            Ok(Value::Boolean(num >= min && num <= max))
        }
        _ => Err(Error::new(
            format!("Unknown number method: {}", name),
            None,
        )),
    }
}

/// Handle JSON object method calls
fn exec_json_method(
    name: &str,
    recv: &Value,
    args_expr: &[Expr],
    base_vars: Option<&HashMap<String, Value>>,
) -> Result<Value, Error> {
    let json_str = match recv {
        Value::Json(s) => s,
        _ => return Err(Error::new("Method called on non-JSON", None)),
    };
    
    let lname = name.to_lowercase();
    
    match lname.as_str() {
        "keys" => {
            let parsed: serde_json::Value = serde_json::from_str(json_str)
                .map_err(|e| Error::new(format!("Invalid JSON: {}", e), None))?;
            
            if let serde_json::Value::Object(obj) = parsed {
                let keys: Vec<Value> = obj.keys()
                    .map(|k| Value::String(k.clone()))
                    .collect();
                Ok(Value::Array(keys))
            } else {
                Err(Error::new("keys() method requires JSON object", None))
            }
        }
        
        "values" => {
            let parsed: serde_json::Value = serde_json::from_str(json_str)
                .map_err(|e| Error::new(format!("Invalid JSON: {}", e), None))?;
            
            if let serde_json::Value::Object(obj) = parsed {
                let values: Result<Vec<Value>, Error> = obj.values()
                    .map(|v| crate::json_to_value(v.clone()))
                    .collect();
                Ok(Value::Array(values?))
            } else {
                Err(Error::new("values() method requires JSON object", None))
            }
        }
        
        "has_key" | "has" => {
            if args_expr.is_empty() {
                return Err(Error::new("has_key method expects 1 argument", None));
            }
            
            use crate::runtime::evaluation::{eval, eval_with_vars};
            let key_val = if let Some(vars) = base_vars {
                eval_with_vars(&args_expr[0], vars)?
            } else {
                eval(&args_expr[0])?
            };
            let key = match key_val {
                Value::String(s) => s,
                _ => return Err(Error::new("has_key method expects string argument", None)),
            };
            
            let parsed: serde_json::Value = serde_json::from_str(json_str)
                .map_err(|e| Error::new(format!("Invalid JSON: {}", e), None))?;
                
            if let serde_json::Value::Object(obj) = parsed {
                Ok(Value::Boolean(obj.contains_key(&key)))
            } else {
                Ok(Value::Boolean(false))
            }
        }
        
        _ => Err(Error::new(
            format!("Unknown JSON method: {}", name),
            None,
        )),
    }
}