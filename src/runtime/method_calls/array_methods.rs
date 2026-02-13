use crate::ast::Expr;
use crate::error::Error;
use crate::runtime::evaluation::{eval, eval_with_vars};
use crate::types::Value;
use std::collections::{BTreeSet, HashMap};

/// Handle array method calls
pub fn exec_array_method(
    name: &str,
    recv: &Value,
    args_expr: &[Expr],
    base_vars: Option<&HashMap<String, Value>>,
) -> Result<Value, Error> {
    let recv_array = match recv {
        Value::Array(a) => a,
        _ => return Err(Error::new("Method called on non-array", None)),
    };

    let lname = name.to_lowercase();

    match lname.as_str() {
        "length" | "len" | "count" => Ok(Value::Number(recv_array.len() as f64)),

        "first" => recv_array
            .first()
            .cloned()
            .map(Ok)
            .unwrap_or(Ok(Value::Null)),

        "last" => recv_array
            .last()
            .cloned()
            .map(Ok)
            .unwrap_or(Ok(Value::Null)),

        "reverse" => Ok(Value::Array(recv_array.iter().rev().cloned().collect())),

        "unique" => {
            let mut unique_vals = Vec::new();
            let mut seen = BTreeSet::new();
            for val in recv_array {
                let key = format!("{:?}", val); // Use debug representation as key
                if seen.insert(key) {
                    unique_vals.push(val.clone());
                }
            }
            Ok(Value::Array(unique_vals))
        }

        "sort" => {
            let desc = if !args_expr.is_empty() {
                let order_val = if let Some(vars) = base_vars {
                    eval_with_vars(&args_expr[0], vars)?
                } else {
                    eval(&args_expr[0])?
                };
                match order_val {
                    Value::String(s) => s.to_uppercase() == "DESC",
                    _ => false,
                }
            } else {
                false
            };

            let mut nums = Vec::with_capacity(recv_array.len());
            for val in recv_array {
                match val {
                    Value::Number(n) => nums.push(*n),
                    _ => return Err(Error::new("sort expects numeric array", None)),
                }
            }

            if desc {
                nums.sort_by(|a, b| b.partial_cmp(a).unwrap_or(std::cmp::Ordering::Equal));
            } else {
                nums.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            }
            Ok(Value::Array(nums.into_iter().map(Value::Number).collect()))
        }

        "sum" => {
            let mut total = 0.0;
            for val in recv_array {
                match val {
                    Value::Number(n) => total += n,
                    Value::Currency(c) => total += c,
                    _ => return Err(Error::new("sum method expects numeric array", None)),
                }
            }
            Ok(Value::Number(total))
        }

        "avg" | "average" => {
            if recv_array.is_empty() {
                return Ok(Value::Number(0.0));
            }
            let mut total = 0.0;
            for val in recv_array {
                match val {
                    Value::Number(n) => total += n,
                    Value::Currency(c) => total += c,
                    _ => return Err(Error::new("avg method expects numeric array", None)),
                }
            }
            Ok(Value::Number(total / recv_array.len() as f64))
        }

        "min" => {
            if recv_array.is_empty() {
                return Ok(Value::Null);
            }
            let mut min_val = None;
            for val in recv_array {
                match val {
                    Value::Number(n) => {
                        min_val = Some(match min_val {
                            None => *n,
                            Some(current) => n.min(current),
                        });
                    }
                    Value::Currency(c) => {
                        min_val = Some(match min_val {
                            None => *c,
                            Some(current) => c.min(current),
                        });
                    }
                    _ => return Err(Error::new("min method expects numeric array", None)),
                }
            }
            Ok(Value::Number(min_val.unwrap_or(0.0)))
        }

        "max" => {
            if recv_array.is_empty() {
                return Ok(Value::Null);
            }
            let mut max_val = None;
            for val in recv_array {
                match val {
                    Value::Number(n) => {
                        max_val = Some(match max_val {
                            None => *n,
                            Some(current) => n.max(current),
                        });
                    }
                    Value::Currency(c) => {
                        max_val = Some(match max_val {
                            None => *c,
                            Some(current) => c.max(current),
                        });
                    }
                    _ => return Err(Error::new("max method expects numeric array", None)),
                }
            }
            Ok(Value::Number(max_val.unwrap_or(0.0)))
        }

        "join" => {
            let separator = if !args_expr.is_empty() {
                let sep_val = if let Some(vars) = base_vars {
                    eval_with_vars(&args_expr[0], vars)?
                } else {
                    eval(&args_expr[0])?
                };
                match sep_val {
                    Value::String(s) => s,
                    _ => ",".to_string(),
                }
            } else {
                ",".to_string()
            };

            let string_vals: Result<Vec<String>, Error> = recv_array
                .iter()
                .map(|v| match v {
                    Value::String(s) => Ok(s.clone()),
                    Value::Number(n) => Ok(n.to_string()),
                    Value::Boolean(b) => Ok(b.to_string()),
                    _ => Err(Error::new("join method cannot convert value to string", None)),
                })
                .collect();

            Ok(Value::String(string_vals?.join(&separator)))
        }

        "contains" | "includes" => {
            if args_expr.is_empty() {
                return Err(Error::new("contains method expects 1 argument", None));
            }
            let search_val = if let Some(vars) = base_vars {
                eval_with_vars(&args_expr[0], vars)?
            } else {
                eval(&args_expr[0])?
            };

            let found = recv_array.iter().any(|v| *v == search_val);
            Ok(Value::Boolean(found))
        }

        "flatten" => {
            fn flatten_recursive(arr: &[Value]) -> Vec<Value> {
                let mut result = Vec::new();
                for val in arr {
                    match val {
                        Value::Array(inner) => result.extend(flatten_recursive(inner)),
                        other => result.push(other.clone()),
                    }
                }
                result
            }
            Ok(Value::Array(flatten_recursive(recv_array)))
        }

        "compact" => {
            let compacted: Vec<Value> = recv_array
                .iter()
                .filter(|v| !matches!(v, Value::Null))
                .cloned()
                .collect();
            Ok(Value::Array(compacted))
        }

        _ => Err(Error::new(
            format!("Unknown array method: {}", name),
            None,
        )),
    }
}
