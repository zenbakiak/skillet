use crate::ast::Expr;
use crate::error::Error;
use crate::runtime::evaluator::{eval_arg, EvaluationContext};
use crate::types::Value;
use std::collections::BTreeSet;

/// Handle array method calls
#[allow(deprecated)] // `Value::Currency` is read as a Number (ADR-0033)
pub fn exec_array_method(
    name: &str,
    recv: &Value,
    args_expr: &[Expr],
    ctx: &dyn EvaluationContext,
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
            for (i, val) in recv_array.iter().enumerate() {
                crate::limits::tick_item(i)?;
                let key = format!("{:?}", val); // Use debug representation as key
                if seen.insert(key) {
                    unique_vals.push(val.clone());
                }
            }
            Ok(Value::Array(unique_vals))
        }

        "sort" => {
            let desc = if !args_expr.is_empty() {
                let order_val = eval_arg(&args_expr[0], ctx)?;
                match order_val {
                    Value::String(s) => s.to_uppercase() == "DESC",
                    _ => false,
                }
            } else {
                false
            };

            sort_method(recv_array, desc)
        }

        "sum" => {
            let mut total = 0.0;
            for val in recv_array {
                match val {
                    Value::Number(n) | Value::Currency(n) => total += n,
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
                    Value::Number(n) | Value::Currency(n) => total += n,
                    _ => return Err(Error::new("avg method expects numeric array", None)),
                }
            }
            Ok(Value::Number(total / recv_array.len() as f64))
        }

        "min" => extreme_method(recv_array, "min", f64::min, i64::min),

        "max" => extreme_method(recv_array, "max", f64::max, i64::max),

        "join" => {
            let separator = if !args_expr.is_empty() {
                let sep_val = eval_arg(&args_expr[0], ctx)?;
                match sep_val {
                    Value::String(s) => s,
                    _ => ",".to_string(),
                }
            } else {
                ",".to_string()
            };

            // Each item's text form, as `JOIN` writes it (ADR-0034 §3).
            crate::runtime::text::join(
                recv_array,
                &separator,
                "join method cannot convert value to string",
            )
            .map(Value::String)
        }

        "contains" | "includes" => {
            if args_expr.is_empty() {
                return Err(Error::new("contains method expects 1 argument", None));
            }
            let search_val = eval_arg(&args_expr[0], ctx)?;

            let found = recv_array.contains(&search_val);
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

        "merge" => {
            // Estimate capacity: receiver + all arguments
            let mut capacity = recv_array.len();
            for arg_expr in args_expr {
                let arg_val = eval_arg(arg_expr, ctx)?;
                capacity += match &arg_val {
                    Value::Array(items) => items.len(),
                    _ => 1,
                };
            }

            let mut result = Vec::with_capacity(capacity);
            result.extend_from_slice(recv_array);

            for arg_expr in args_expr {
                let arg_val = eval_arg(arg_expr, ctx)?;

                match arg_val {
                    Value::Array(items) => result.extend(items),
                    other => result.push(other),
                }
            }

            Ok(Value::Array(result))
        }

        _ => Err(Error::new(format!("Unknown array method: {}", name), None)),
    }
}

/// `.sort([order])`: an array of only numbers or of only dates (ADR-0031 amendment 2, B5),
/// ascending or, with `"DESC"`, descending. Equal dates keep their order.
#[allow(deprecated)] // `Value::Currency` is read as a Number (ADR-0033)
fn sort_method(items: &[Value], desc: bool) -> Result<Value, Error> {
    let wrong = || Error::new("sort expects an array of numbers or of dates", None);
    if let Some(Value::DateTime(_)) = items.first() {
        let mut dates = Vec::with_capacity(items.len());
        for val in items {
            crate::limits::tick_item(dates.len())?;
            match val {
                Value::DateTime(t) => dates.push(*t),
                _ => return Err(wrong()),
            }
        }
        if desc {
            dates.sort_by(|a, b| b.cmp(a));
        } else {
            dates.sort();
        }
        return Ok(Value::Array(
            dates.into_iter().map(Value::DateTime).collect(),
        ));
    }
    let mut nums = Vec::with_capacity(items.len());
    for val in items {
        crate::limits::tick_item(nums.len())?;
        match val {
            Value::Number(n) | Value::Currency(n) => nums.push(*n),
            _ => return Err(wrong()),
        }
    }
    crate::runtime::utils::sort_numbers(&mut nums, desc)?;
    Ok(Value::Array(nums.into_iter().map(Value::Number).collect()))
}

/// `.min()`/`.max()`: the smallest/largest of an array of only numbers, or the earliest/latest
/// of an array of only dates (ADR-0031 amendment 2, B5); `NULL` for an empty array.
#[allow(deprecated)] // `Value::Currency` is read as a Number (ADR-0033)
fn extreme_method(
    items: &[Value],
    name: &str,
    pick: impl Fn(f64, f64) -> f64,
    pick_date: impl Fn(i64, i64) -> i64,
) -> Result<Value, Error> {
    let wrong = || {
        Error::new(
            format!("{name} method expects an array of numbers or of dates"),
            None,
        )
    };
    match items.first() {
        None => Ok(Value::Null),
        Some(Value::DateTime(first)) => {
            let mut best = *first;
            for val in items {
                match val {
                    Value::DateTime(t) => best = pick_date(best, *t),
                    _ => return Err(wrong()),
                }
            }
            Ok(Value::DateTime(best))
        }
        Some(_) => {
            let mut best: Option<f64> = None;
            for val in items {
                match val {
                    Value::Number(n) | Value::Currency(n) => {
                        best = Some(best.map_or(*n, |c| pick(*n, c)))
                    }
                    _ => return Err(wrong()),
                }
            }
            Ok(Value::Number(best.unwrap_or(0.0)))
        }
    }
}
