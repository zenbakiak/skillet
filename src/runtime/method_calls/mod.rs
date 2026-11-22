//! Method calls module - refactored into smaller, focused sub-modules

pub mod array_methods;
pub mod conversion_methods;
pub mod lambda_methods;
pub mod predicates;
pub mod string_methods;

use crate::ast::Expr;
use crate::custom::FunctionRegistry;
use crate::error::Error;
use crate::runtime::evaluator::{eval_arg, EmptyContext, EvaluationContext, VariableContext};
use crate::types::Value;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub use array_methods::exec_array_method;
pub use conversion_methods::exec_conversion_method;
pub use lambda_methods::{exec_filter, exec_find, exec_map, exec_reduce};
pub use predicates::exec_predicate;
pub use string_methods::exec_string_method;

/// Method dispatch over a copy-free variable view (kept for callers outside the evaluator).
pub fn exec_method(
    name: &str,
    predicate: bool,
    recv: &Value,
    args_expr: &[Expr],
    base_vars: Option<&HashMap<String, Value>>,
) -> Result<Value, Error> {
    match base_vars {
        Some(vars) => exec_method_in(
            name,
            predicate,
            recv,
            args_expr,
            &VariableContext::new(vars),
        ),
        None => exec_method_in(name, predicate, recv, args_expr, &EmptyContext),
    }
}

/// Method dispatch with custom functions visible inside lambda bodies.
pub fn exec_method_with_custom(
    name: &str,
    predicate: bool,
    recv: &Value,
    args_expr: &[Expr],
    base_vars: Option<&HashMap<String, Value>>,
    custom_registry: &Arc<RwLock<FunctionRegistry>>,
) -> Result<Value, Error> {
    let empty = HashMap::new();
    let ctx = VariableContext::with_custom(base_vars.unwrap_or(&empty), custom_registry);
    exec_method_in(name, predicate, recv, args_expr, &ctx)
}

/// Method dispatch used by the evaluator. Arguments and lambda bodies are evaluated against
/// `ctx` without copying it; both lambda bodies and plain arguments see `ctx`'s custom
/// registry (ADR-0016).
#[allow(deprecated)] // `Value::Currency` is read as a Number (ADR-0033)
pub(crate) fn exec_method_in(
    name: &str,
    predicate: bool,
    recv: &Value,
    args_expr: &[Expr],
    ctx: &dyn EvaluationContext,
) -> Result<Value, Error> {
    if predicate {
        return exec_predicate(name, recv);
    }

    // Compute lowercase once for all dispatch checks
    let lname = name.to_lowercase();

    // Check for conversion methods first (available on all types)
    match lname.as_str() {
        "to_s" | "to_string" | "to_i" | "to_int" | "to_f" | "to_float" | "to_a" | "to_array"
        | "to_json" | "to_bool" | "to_boolean" => {
            return exec_conversion_method(name, recv);
        }
        _ => {}
    }

    // Dispatch to appropriate method handler based on receiver type
    match recv {
        Value::String(_) => exec_string_method(name, recv, args_expr, ctx),
        Value::Array(_) => {
            // Check for higher-order functions first
            match lname.as_str() {
                "filter" => exec_filter(recv, args_expr, ctx),
                "map" => exec_map(recv, args_expr, ctx),
                "find" => exec_find(recv, args_expr, ctx),
                "reduce" => exec_reduce(recv, args_expr, ctx),
                _ => exec_array_method(name, recv, args_expr, ctx),
            }
        }
        Value::Number(_) | Value::Currency(_) => exec_number_method(name, recv, args_expr, ctx),
        Value::Json(_) => exec_json_method(name, recv, args_expr, ctx),
        _ => Err(Error::new(
            format!("No methods available for {:?} type", recv),
            None,
        )),
    }
}

/// Handle number method calls
#[allow(deprecated)] // `Value::Currency` is read as a Number (ADR-0033)
fn exec_number_method(
    name: &str,
    recv: &Value,
    args_expr: &[Expr],
    ctx: &dyn EvaluationContext,
) -> Result<Value, Error> {
    let num = match recv {
        Value::Number(n) | Value::Currency(n) => *n,
        _ => return Err(Error::new("Method called on non-number", None)),
    };

    let lname = name.to_lowercase();

    match lname.as_str() {
        "abs" => Ok(Value::Number(num.abs())),
        "ceil" | "ceiling" => Ok(Value::Number(num.ceil())),
        "floor" => Ok(Value::Number(num.floor())),
        // The same function as `ROUND` (BL-088): half away from zero on the 15-digit form.
        "round" => {
            let round = crate::runtime::arithmetic::round;
            match args_expr {
                [] => round("round", &[Value::Number(num)]),
                [digits] => round("round", &[Value::Number(num), eval_arg(digits, ctx)?]),
                _ => Err(Error::new("round expects ([num_digits])", None)),
            }
        }
        "sqrt" => {
            if num < 0.0 {
                Err(Error::new(
                    "Cannot take square root of negative number",
                    None,
                ))
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
            let min_val = eval_arg(&args_expr[0], ctx)?;
            let max_val = eval_arg(&args_expr[1], ctx)?;

            let min = match min_val {
                Value::Number(n) | Value::Currency(n) => n,
                _ => return Err(Error::new("between min must be a number", None)),
            };
            let max = match max_val {
                Value::Number(n) | Value::Currency(n) => n,
                _ => return Err(Error::new("between max must be a number", None)),
            };

            Ok(Value::Boolean(num >= min && num <= max))
        }
        _ => Err(Error::new(format!("Unknown number method: {}", name), None)),
    }
}

/// Handle JSON object method calls
fn exec_json_method(
    name: &str,
    recv: &Value,
    args_expr: &[Expr],
    ctx: &dyn EvaluationContext,
) -> Result<Value, Error> {
    let json_str = match recv {
        Value::Json(s) => s,
        _ => return Err(Error::new("Method called on non-JSON", None)),
    };

    let lname = name.to_lowercase();

    match lname.as_str() {
        "keys" => {
            crate::limits::charge_json_parse(json_str)?;
            let parsed: serde_json::Value = serde_json::from_str(json_str)
                .map_err(|e| Error::new(format!("Invalid JSON: {}", e), None))?;

            if let serde_json::Value::Object(obj) = parsed {
                let keys: Vec<Value> = obj.keys().map(|k| Value::String(k.clone())).collect();
                Ok(Value::Array(keys))
            } else {
                Err(Error::new("keys() method requires JSON object", None))
            }
        }

        "values" => {
            crate::limits::charge_json_parse(json_str)?;
            let parsed: serde_json::Value = serde_json::from_str(json_str)
                .map_err(|e| Error::new(format!("Invalid JSON: {}", e), None))?;

            if let serde_json::Value::Object(obj) = parsed {
                let values: Result<Vec<Value>, Error> = obj
                    .values()
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
            let key_val = eval_arg(&args_expr[0], ctx)?;
            let key = match key_val {
                Value::String(s) => s,
                _ => return Err(Error::new("has_key method expects string argument", None)),
            };

            crate::limits::charge_json_parse(json_str)?;
            let parsed: serde_json::Value = serde_json::from_str(json_str)
                .map_err(|e| Error::new(format!("Invalid JSON: {}", e), None))?;

            if let serde_json::Value::Object(obj) = parsed {
                Ok(Value::Boolean(obj.contains_key(&key)))
            } else {
                Ok(Value::Boolean(false))
            }
        }

        "dig" => {
            // :json_obj.dig([path...], [default])
            if args_expr.is_empty() {
                return Err(Error::new(
                    "dig(path_array, [default]) expects at least 1 argument",
                    None,
                ));
            }
            let path_val = eval_arg(&args_expr[0], ctx)?;
            let default_val = if args_expr.len() > 1 {
                Some(eval_arg(&args_expr[1], ctx)?)
            } else {
                None
            };

            let path_vals = match path_val {
                Value::Array(a) => a,
                _ => {
                    return Err(Error::new(
                        "dig expects first argument to be an array path",
                        None,
                    ))
                }
            };

            crate::limits::charge_json_parse(json_str)?;
            let parsed: serde_json::Value = serde_json::from_str(json_str)
                .map_err(|e| Error::new(format!("Invalid JSON: {}", e), None))?;
            let mut cur = &parsed;
            let mut ok = true;
            for seg in &path_vals {
                match seg {
                    Value::String(key) => {
                        if let serde_json::Value::Object(map) = cur {
                            if let Some(next) = map.get(key) {
                                cur = next;
                            } else {
                                ok = false;
                                break;
                            }
                        } else {
                            ok = false;
                            break;
                        }
                    }
                    Value::Number(n) => {
                        if let serde_json::Value::Array(arr) = cur {
                            let idx = if n.is_finite() {
                                n.floor() as isize
                            } else {
                                -1
                            };
                            if idx >= 0 && (idx as usize) < arr.len() {
                                cur = &arr[idx as usize];
                            } else {
                                ok = false;
                                break;
                            }
                        } else {
                            ok = false;
                            break;
                        }
                    }
                    _ => {
                        ok = false;
                        break;
                    }
                }
            }

            if ok {
                crate::json_to_value(cur.clone())
            } else if let Some(defv) = default_val {
                Ok(defv)
            } else {
                Ok(Value::Null)
            }
        }

        _ => Err(Error::new(format!("Unknown JSON method: {}", name), None)),
    }
}
