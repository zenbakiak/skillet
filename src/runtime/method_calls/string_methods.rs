use crate::ast::Expr;
use crate::error::Error;
use crate::runtime::evaluator::{eval_arg, EvaluationContext};
use crate::types::Value;

/// Handle string method calls
pub fn exec_string_method(
    name: &str,
    recv: &Value,
    args_expr: &[Expr],
    ctx: &dyn EvaluationContext,
) -> Result<Value, Error> {
    let recv_string = match recv {
        Value::String(s) => s.clone(),
        _ => return Err(Error::new("Method called on non-string", None)),
    };

    let lname = name.to_lowercase();

    match lname.as_str() {
        // Characters, as `LEN` and `LENGTH` count them (ADR-0034 §5).
        "length" | "len" => Ok(Value::Number(recv_string.chars().count() as f64)),

        "upper" | "upcase" => Ok(Value::String(crate::limits::case_mapped(
            &recv_string,
            true,
        )?)),

        "lower" | "downcase" => Ok(Value::String(crate::limits::case_mapped(
            &recv_string,
            false,
        )?)),

        "trim" => Ok(Value::String(recv_string.trim().to_string())),

        "reverse" => Ok(Value::String(recv_string.chars().rev().collect())),

        "includes" | "contains" => {
            if args_expr.is_empty() {
                return Err(Error::new("includes method expects 1 argument", None));
            }
            let substr_val = eval_arg(&args_expr[0], ctx)?;
            let substr = match substr_val {
                Value::String(s) => s,
                _ => return Err(Error::new("includes method expects string argument", None)),
            };
            Ok(Value::Boolean(recv_string.contains(&substr)))
        }

        "startswith" | "starts_with" => {
            if args_expr.is_empty() {
                return Err(Error::new("starts_with method expects 1 argument", None));
            }
            let prefix_val = eval_arg(&args_expr[0], ctx)?;
            let prefix = match prefix_val {
                Value::String(s) => s,
                _ => {
                    return Err(Error::new(
                        "starts_with method expects string argument",
                        None,
                    ))
                }
            };
            Ok(Value::Boolean(recv_string.starts_with(&prefix)))
        }

        "endswith" | "ends_with" => {
            if args_expr.is_empty() {
                return Err(Error::new("ends_with method expects 1 argument", None));
            }
            let suffix_val = eval_arg(&args_expr[0], ctx)?;
            let suffix = match suffix_val {
                Value::String(s) => s,
                _ => return Err(Error::new("ends_with method expects string argument", None)),
            };
            Ok(Value::Boolean(recv_string.ends_with(&suffix)))
        }

        "split" => {
            if args_expr.is_empty() {
                return Err(Error::new("split method expects 1 argument", None));
            }
            let delimiter_val = eval_arg(&args_expr[0], ctx)?;
            let delimiter = match delimiter_val {
                Value::String(s) => s,
                _ => return Err(Error::new("split method expects string argument", None)),
            };
            crate::runtime::string::split_charged(recv_string.split(delimiter.as_str()))
        }

        "replace" => {
            if args_expr.len() < 2 {
                return Err(Error::new("replace method expects 2 arguments", None));
            }
            let from_val = eval_arg(&args_expr[0], ctx)?;
            let to_val = eval_arg(&args_expr[1], ctx)?;
            let (from, to) = match (from_val, to_val) {
                (Value::String(f), Value::String(t)) => (f, t),
                _ => return Err(Error::new("replace method expects string arguments", None)),
            };
            crate::limits::charge_replace(&recv_string, &from, &to)?;
            Ok(Value::String(recv_string.replace(&from, &to)))
        }

        "substring" | "substr" => {
            if args_expr.is_empty() {
                return Err(Error::new(
                    "substring method expects at least 1 argument",
                    None,
                ));
            }
            let start_val = eval_arg(&args_expr[0], ctx)?;
            let start = match start_val {
                Value::Number(n) => n as usize,
                _ => return Err(Error::new("substring start must be number", None)),
            };

            let chars: Vec<char> = recv_string.chars().collect();
            let result = if args_expr.len() >= 2 {
                let len_val = eval_arg(&args_expr[1], ctx)?;
                let len = match len_val {
                    Value::Number(n) => n as usize,
                    _ => return Err(Error::new("substring length must be number", None)),
                };
                chars
                    .get(
                        start
                            ..start
                                .min(chars.len())
                                .saturating_add(len.min(chars.len() - start.min(chars.len()))),
                    )
                    .unwrap_or(&[])
                    .iter()
                    .collect()
            } else {
                chars.get(start..).unwrap_or(&[]).iter().collect()
            };
            Ok(Value::String(result))
        }

        _ => Err(Error::new(format!("Unknown string method: {}", name), None)),
    }
}
