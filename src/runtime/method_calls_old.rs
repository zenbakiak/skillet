use crate::ast::Expr;
use crate::custom::FunctionRegistry;
use crate::error::Error;
use crate::runtime::evaluation::{eval, eval_with_vars, eval_with_vars_and_custom};
use crate::runtime::utils::is_blank;
use crate::types::Value;
use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, RwLock};

pub fn exec_method(
    name: &str,
    predicate: bool,
    recv: &Value,
    args_expr: &[Expr],
    base_vars: Option<&HashMap<String, Value>>,
) -> Result<Value, Error> {
    let lname = name.to_lowercase();
    if predicate {
        return match lname.as_str() {
            "positive" => Ok(Value::Boolean(
                recv.as_number().map(|n| n > 0.0).unwrap_or(false),
            )),
            "negative" => Ok(Value::Boolean(
                recv.as_number().map(|n| n < 0.0).unwrap_or(false),
            )),
            "zero" => Ok(Value::Boolean(
                recv.as_number().map(|n| n == 0.0).unwrap_or(false),
            )),
            "even" => Ok(Value::Boolean(
                recv.as_number()
                    .map(|n| (n as i64) % 2 == 0)
                    .unwrap_or(false),
            )),
            "odd" => Ok(Value::Boolean(
                recv.as_number()
                    .map(|n| (n as i64) % 2 != 0)
                    .unwrap_or(false),
            )),
            "numeric" => Ok(Value::Boolean(matches!(recv, Value::Number(_)))),
            "array" => Ok(Value::Boolean(matches!(recv, Value::Array(_)))),
            "nil" => Ok(Value::Boolean(matches!(recv, Value::Null))),
            "blank" => Ok(Value::Boolean(is_blank(recv))),
            "present" => Ok(Value::Boolean(!is_blank(recv))),
            _ => Err(Error::new(
                format!("Unknown predicate method: {}?", name),
                None,
            )),
        };
    }

    // Helper to evaluate argument expressions with spread handling
    let eval_args = |exprs: &[Expr]| -> Result<Vec<Value>, Error> {
        let mut out = Vec::new();
        for e in exprs {
            match e {
                Expr::Spread(inner) => {
                    let v = match base_vars {
                        Some(env) => eval_with_vars(inner, env)?,
                        None => eval(inner)?,
                    };
                    if let Value::Array(items) = v {
                        out.extend(items);
                    } else {
                        return Err(Error::new("Spread expects array", None));
                    }
                }
                _ => {
                    let v = match base_vars {
                        Some(env) => eval_with_vars(e, env)?,
                        None => eval(e)?,
                    };
                    out.push(v);
                }
            }
        }
        Ok(out)
    };

    match lname.as_str() {
        // Numeric transformations on receiver
        "abs" => Ok(Value::Number(
            recv.as_number()
                .ok_or_else(|| Error::new("abs expects number receiver", None))?
                .abs(),
        )),
        "round" => {
            let n = recv
                .as_number()
                .ok_or_else(|| Error::new("round expects number receiver", None))?;
            let a = eval_args(args_expr)?;
            let decimals = match a.get(0) {
                Some(Value::Number(d)) => *d as i32,
                _ => 0,
            };
            let factor = 10f64.powi(decimals.max(0));
            Ok(Value::Number((n * factor).round() / factor))
        }
        "floor" => Ok(Value::Number(
            recv.as_number()
                .ok_or_else(|| Error::new("floor expects number receiver", None))?
                .floor(),
        )),
        "ceil" => Ok(Value::Number(
            recv.as_number()
                .ok_or_else(|| Error::new("ceil expects number receiver", None))?
                .ceil(),
        )),
        "between" => {
            let value = recv.as_number()
                .ok_or_else(|| Error::new("between expects number receiver", None))?;
            let a = eval_args(args_expr)?;
            if a.len() != 2 {
                return Err(Error::new("between expects 2 arguments: min, max", None));
            }
            let min = a[0].as_number()
                .ok_or_else(|| Error::new("between min must be a number", None))?;
            let max = a[1].as_number()
                .ok_or_else(|| Error::new("between max must be a number", None))?;
            Ok(Value::Boolean(value >= min && value <= max))
        }

        // String transforms
        "upper" => match recv {
            Value::String(s) => Ok(Value::String(s.to_uppercase())),
            _ => Err(Error::new("upper expects string receiver", None)),
        },
        "lower" => match recv {
            Value::String(s) => Ok(Value::String(s.to_lowercase())),
            _ => Err(Error::new("lower expects string receiver", None)),
        },
        "trim" => match recv {
            Value::String(s) => Ok(Value::String(s.trim().to_string())),
            _ => Err(Error::new("trim expects string receiver", None)),
        },
        "reverse" => match recv {
            Value::String(s) => Ok(Value::String(s.chars().rev().collect())),
            Value::Array(items) => {
                let mut v = items.clone();
                v.reverse();
                Ok(Value::Array(v))
            }
            _ => Err(Error::new("reverse expects string or array receiver", None)),
        },
        "includes" => {
            // String includes method: "hello world".includes("world") -> true
            if let Value::String(s) = recv {
                let a = eval_args(args_expr)?;
                if let Some(Value::String(substring)) = a.get(0) {
                    Ok(Value::Boolean(s.contains(substring)))
                } else {
                    Err(Error::new("includes expects string argument", None))
                }
            } else {
                Err(Error::new("includes expects string receiver", None))
            }
        },

        // Array accessors / transforms
        "length" | "size" => match recv {
            Value::Array(items) => Ok(Value::Number(items.len() as f64)),
            Value::String(s) => Ok(Value::Number(s.chars().count() as f64)),
            Value::Null => Ok(Value::Number(0.0)),
            _ => Err(Error::new("length expects array or string receiver", None)),
        },
        "first" => match recv {
            Value::Array(items) => items
                .first()
                .cloned()
                .ok_or_else(|| Error::new("first on empty array", None)),
            _ => Err(Error::new("first expects array receiver", None)),
        },
        "last" => match recv {
            Value::Array(items) => items
                .last()
                .cloned()
                .ok_or_else(|| Error::new("last on empty array", None)),
            _ => Err(Error::new("last expects array receiver", None)),
        },
        "sort" => match recv {
            Value::Array(items) => {
                let mut nums: Vec<f64> = Vec::with_capacity(items.len());
                for it in items {
                    match it {
                        Value::Number(n) => nums.push(*n),
                        _ => return Err(Error::new("sort expects numeric array", None)),
                    }
                }
                nums.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                Ok(Value::Array(nums.into_iter().map(Value::Number).collect()))
            }
            _ => Err(Error::new("sort expects array receiver", None)),
        },
        "unique" => match recv {
            Value::Array(items) => {
                let mut set = BTreeSet::new();
                let mut out = Vec::new();
                for it in items {
                    match it {
                        Value::Number(n) => {
                            if set.insert((*n).to_bits()) {
                                out.push(Value::Number(*n));
                            }
                        }
                        _ => return Err(Error::new("unique expects numeric array", None)),
                    }
                }
                Ok(Value::Array(out))
            }
            _ => Err(Error::new("unique expects array receiver", None)),
        },
        "sum" => match recv {
            Value::Array(items) => {
                let mut acc = 0.0;
                for it in items {
                    if let Value::Number(n) = it {
                        acc += n;
                    } else {
                        return Err(Error::new("sum expects numeric array", None));
                    }
                }
                Ok(Value::Number(acc))
            }
            _ => Err(Error::new("sum expects array receiver", None)),
        },
        "avg" => match recv {
            Value::Array(items) => {
                let mut acc = 0.0;
                let mut count = 0usize;
                for it in items {
                    if let Value::Number(n) = it {
                        acc += n;
                        count += 1;
                    } else {
                        return Err(Error::new("avg expects numeric array", None));
                    }
                }
                Ok(Value::Number(if count == 0 {
                    0.0
                } else {
                    acc / count as f64
                }))
            }
            _ => Err(Error::new("avg expects array receiver", None)),
        },
        "min" => match recv {
            Value::Array(items) => {
                let mut cur: Option<f64> = None;
                for it in items {
                    if let Value::Number(n) = it {
                        cur = Some(cur.map_or(*n, |c| c.min(*n)));
                    } else {
                        return Err(Error::new("min expects numeric array", None));
                    }
                }
                Ok(Value::Number(cur.unwrap_or(0.0)))
            }
            _ => Err(Error::new("min expects array receiver", None)),
        },
        "max" => match recv {
            Value::Array(items) => {
                let mut cur: Option<f64> = None;
                for it in items {
                    if let Value::Number(n) = it {
                        cur = Some(cur.map_or(*n, |c| c.max(*n)));
                    } else {
                        return Err(Error::new("max expects numeric array", None));
                    }
                }
                Ok(Value::Number(cur.unwrap_or(0.0)))
            }
            _ => Err(Error::new("max expects array receiver", None)),
        },
        "filter" => match recv {
            Value::Array(items) => {
                let expr = args_expr
                    .get(0)
                    .cloned()
                    .ok_or_else(|| Error::new("filter expects an expression", None))?;
                // Optional param name as second arg
                let param_vals = eval_args(&args_expr[1..])?;
                let param_name = match param_vals.get(0) {
                    Some(Value::String(s)) => s.clone(),
                    _ => "x".to_string(),
                };
                let mut out = Vec::new();
                for it in items {
                    let mut env = HashMap::new();
                    env.insert(param_name.clone(), it.clone());
                    if let Some(base) = base_vars {
                        for (k, v) in base.iter() {
                            env.insert(k.clone(), v.clone());
                        }
                    }
                    let keep = match eval_with_vars(&expr, &env)? {
                        Value::Boolean(b) => b,
                        _ => false,
                    };
                    if keep {
                        out.push(it.clone());
                    }
                }
                Ok(Value::Array(out))
            }
            _ => Err(Error::new("filter expects array receiver", None)),
        },
        "find" => match recv {
            Value::Array(items) => {
                let expr = args_expr
                    .get(0)
                    .cloned()
                    .ok_or_else(|| Error::new("find expects an expression", None))?;
                // Optional param name as second arg
                let param_vals = eval_args(&args_expr[1..])?;
                let param_name = match param_vals.get(0) {
                    Some(Value::String(s)) => s.clone(),
                    _ => "x".to_string(),
                };
                for it in items {
                    let mut env = HashMap::new();
                    env.insert(param_name.clone(), it.clone());
                    if let Some(base) = base_vars {
                        for (k, v) in base.iter() {
                            env.insert(k.clone(), v.clone());
                        }
                    }
                    let matches = match eval_with_vars(&expr, &env)? {
                        Value::Boolean(b) => b,
                        _ => false,
                    };
                    if matches {
                        return Ok(it.clone());
                    }
                }
                Ok(Value::Null)
            }
            _ => Err(Error::new("find expects array receiver", None)),
        },
        "map" => match recv {
            Value::Array(items) => {
                let expr = args_expr
                    .get(0)
                    .cloned()
                    .ok_or_else(|| Error::new("map expects an expression", None))?;
                let param_vals = eval_args(&args_expr[1..])?;
                let param_name = match param_vals.get(0) {
                    Some(Value::String(s)) => s.clone(),
                    _ => "x".to_string(),
                };
                let mut out = Vec::new();
                for it in items {
                    let mut env = HashMap::new();
                    env.insert(param_name.clone(), it.clone());
                    if let Some(base) = base_vars {
                        for (k, v) in base.iter() {
                            env.insert(k.clone(), v.clone());
                        }
                    }
                    out.push(eval_with_vars(&expr, &env)?);
                }
                Ok(Value::Array(out))
            }
            _ => Err(Error::new("map expects array receiver", None)),
        },
        "reduce" => match recv {
            Value::Array(items) => {
                let expr = args_expr
                    .get(0)
                    .cloned()
                    .ok_or_else(|| Error::new("reduce expects expression and initial", None))?;
                let a = eval_args(&args_expr[1..])?;
                let mut acc = a
                    .get(0)
                    .cloned()
                    .ok_or_else(|| Error::new("reduce expects initial value", None))?;
                let val_param = match a.get(1) {
                    Some(Value::String(s)) => s.clone(),
                    _ => "x".to_string(),
                };
                let acc_param = match a.get(2) {
                    Some(Value::String(s)) => s.clone(),
                    _ => "acc".to_string(),
                };
                for it in items {
                    let mut env = HashMap::new();
                    env.insert(val_param.clone(), it.clone());
                    env.insert(acc_param.clone(), acc);
                    if let Some(base) = base_vars {
                        for (k, v) in base.iter() {
                            env.insert(k.clone(), v.clone());
                        }
                    }
                    acc = eval_with_vars(&expr, &env)?;
                }
                Ok(acc)
            }
            _ => Err(Error::new("reduce expects array receiver", None)),
        },
        "flatten" => match recv {
            Value::Array(items) => {
                fn flatten(v: &Value, out: &mut Vec<Value>) {
                    match v {
                        Value::Array(inner) => {
                            for it in inner {
                                flatten(it, out);
                            }
                        }
                        other => out.push(other.clone()),
                    }
                }
                let mut out = Vec::new();
                for it in items {
                    flatten(it, &mut out);
                }
                Ok(Value::Array(out))
            }
            _ => Err(Error::new("flatten expects array receiver", None)),
        },
        // compact implemented with Null support
        "compact" => match recv {
            Value::Array(items) => Ok(Value::Array(
                items
                    .iter()
                    .cloned()
                    .filter(|v| !matches!(v, Value::Null))
                    .collect(),
            )),
            _ => Err(Error::new("compact expects array receiver", None)),
        },
        
        // Type casting methods
        "to_s" => match recv {
            Value::String(s) => Ok(Value::String(s.clone())),
            Value::Number(n) => Ok(Value::String(n.to_string())),
            Value::Boolean(b) => Ok(Value::String(if *b { "TRUE".into() } else { "FALSE".into() })),
            Value::Null => Ok(Value::String(String::new())),
            Value::Array(items) => Ok(Value::String(format!("{:?}", items))),
            Value::Currency(n) => Ok(Value::String(format!("{:.4}", n))),
            Value::DateTime(ts) => Ok(Value::String(ts.to_string())),
            Value::Json(s) => Ok(Value::String(s.clone())),
        },
        "to_i" => match recv {
            Value::Number(n) => Ok(Value::Number((*n as i64) as f64)),
            Value::Currency(n) => Ok(Value::Number((*n as i64) as f64)),
            Value::String(s) => {
                let mut clean_s = String::new();
                let mut has_dot = false;
                for (i, c) in s.chars().enumerate() {
                    if i == 0 && (c == '-' || c == '+') {
                        clean_s.push(c);
                    } else if c.is_ascii_digit() {
                        clean_s.push(c);
                    } else if c == '.' && !has_dot {
                        clean_s.push(c);
                        has_dot = true;
                    } else {
                        break;
                    }
                }
                Ok(Value::Number(
                    clean_s.parse::<f64>()
                        .unwrap_or(0.0)
                        .trunc(),
                ))
            },
            Value::Boolean(b) => Ok(Value::Number(if *b { 1.0 } else { 0.0 })),
            Value::Null => Ok(Value::Number(0.0)),
            _ => Err(Error::new("Cannot cast to Integer", None)),
        },
        "to_number" => match recv {
            Value::Number(n) => Ok(Value::Number(*n)),
            Value::Currency(n) => Ok(Value::Number(*n)),
            Value::String(s) => Ok(Value::Number(
                s.parse::<f64>()
                    .map_err(|_| Error::new("Cannot cast String to Number", None))?,
            )),
            Value::Boolean(b) => Ok(Value::Number(if *b { 1.0 } else { 0.0 })),
            Value::Null => Ok(Value::Number(0.0)),
            _ => Err(Error::new("Cannot cast to Number", None)),
        },
        "to_currency" => match recv {
            Value::Currency(n) => Ok(Value::Currency(*n)),
            Value::Number(n) => Ok(Value::Currency(*n)),
            Value::String(s) => Ok(Value::Currency(
                s.parse::<f64>()
                    .map_err(|_| Error::new("Cannot cast String to Currency", None))?,
            )),
            Value::Boolean(b) => Ok(Value::Currency(if *b { 1.0 } else { 0.0 })),
            Value::Null => Ok(Value::Currency(0.0)),
            _ => Err(Error::new("Cannot cast to Currency", None)),
        },
        "to_boolean" => match recv {
            Value::Boolean(b) => Ok(Value::Boolean(*b)),
            Value::Number(n) => Ok(Value::Boolean(*n != 0.0)),
            Value::Currency(n) => Ok(Value::Boolean(*n != 0.0)),
            Value::String(s) => Ok(Value::Boolean(!s.trim().is_empty())),
            Value::Array(items) => Ok(Value::Boolean(!items.is_empty())),
            Value::Null => Ok(Value::Boolean(false)),
            Value::DateTime(ts) => Ok(Value::Boolean(*ts != 0)),
            Value::Json(s) => Ok(Value::Boolean(!s.trim().is_empty())),
        },

        _ => Err(Error::new(format!("Unknown method: .{}()", name), None)),
    }
}

pub fn exec_method_with_custom(
    name: &str,
    predicate: bool,
    recv: &Value,
    args_expr: &[Expr],
    base_vars: Option<&HashMap<String, Value>>,
    custom_registry: &Arc<RwLock<FunctionRegistry>>,
) -> Result<Value, Error> {
    let lname = name.to_lowercase();
    if predicate {
        return match lname.as_str() {
            "positive" => Ok(Value::Boolean(
                recv.as_number().map(|n| n > 0.0).unwrap_or(false),
            )),
            "negative" => Ok(Value::Boolean(
                recv.as_number().map(|n| n < 0.0).unwrap_or(false),
            )),
            "zero" => Ok(Value::Boolean(
                recv.as_number().map(|n| n == 0.0).unwrap_or(false),
            )),
            "even" => Ok(Value::Boolean(
                recv.as_number()
                    .map(|n| (n as i64) % 2 == 0)
                    .unwrap_or(false),
            )),
            "odd" => Ok(Value::Boolean(
                recv.as_number()
                    .map(|n| (n as i64) % 2 != 0)
                    .unwrap_or(false),
            )),
            "numeric" => Ok(Value::Boolean(matches!(recv, Value::Number(_)))),
            "array" => Ok(Value::Boolean(matches!(recv, Value::Array(_)))),
            "nil" => Ok(Value::Boolean(matches!(recv, Value::Null))),
            "blank" => Ok(Value::Boolean(is_blank(recv))),
            "present" => Ok(Value::Boolean(!is_blank(recv))),
            _ => Err(Error::new(
                format!("Unknown predicate method: {}?", name),
                None,
            )),
        };
    }

    // Helper to evaluate argument expressions with spread handling
    let eval_args = |exprs: &[Expr]| -> Result<Vec<Value>, Error> {
        let mut out = Vec::new();
        for e in exprs {
            match e {
                Expr::Spread(inner) => {
                    let v = match base_vars {
                        Some(env) => eval_with_vars_and_custom(inner, env, custom_registry)?,
                        None => eval_with_vars_and_custom(inner, &HashMap::new(), custom_registry)?,
                    };
                    if let Value::Array(items) = v {
                        out.extend(items);
                    } else {
                        return Err(Error::new("Spread expects array", None));
                    }
                }
                _ => {
                    let v = match base_vars {
                        Some(env) => eval_with_vars_and_custom(e, env, custom_registry)?,
                        None => eval_with_vars_and_custom(e, &HashMap::new(), custom_registry)?,
                    };
                    out.push(v);
                }
            }
        }
        Ok(out)
    };

    match lname.as_str() {
        // Numeric transformations on receiver
        "abs" => Ok(Value::Number(
            recv.as_number()
                .ok_or_else(|| Error::new("abs expects number receiver", None))?
                .abs(),
        )),
        "round" => {
            let n = recv
                .as_number()
                .ok_or_else(|| Error::new("round expects number receiver", None))?;
            let a = eval_args(args_expr)?;
            let decimals = match a.get(0) {
                Some(Value::Number(d)) => *d as i32,
                _ => 0,
            };
            let factor = 10f64.powi(decimals.max(0));
            Ok(Value::Number((n * factor).round() / factor))
        }
        "floor" => Ok(Value::Number(
            recv.as_number()
                .ok_or_else(|| Error::new("floor expects number receiver", None))?
                .floor(),
        )),
        "ceil" => Ok(Value::Number(
            recv.as_number()
                .ok_or_else(|| Error::new("ceil expects number receiver", None))?
                .ceil(),
        )),
        "between" => {
            let value = recv.as_number()
                .ok_or_else(|| Error::new("between expects number receiver", None))?;
            let a = eval_args(args_expr)?;
            if a.len() != 2 {
                return Err(Error::new("between expects 2 arguments: min, max", None));
            }
            let min = a[0].as_number()
                .ok_or_else(|| Error::new("between min must be a number", None))?;
            let max = a[1].as_number()
                .ok_or_else(|| Error::new("between max must be a number", None))?;
            Ok(Value::Boolean(value >= min && value <= max))
        }

        // String transforms
        "upper" => match recv {
            Value::String(s) => Ok(Value::String(s.to_uppercase())),
            _ => Err(Error::new("upper expects string receiver", None)),
        },
        "lower" => match recv {
            Value::String(s) => Ok(Value::String(s.to_lowercase())),
            _ => Err(Error::new("lower expects string receiver", None)),
        },
        "trim" => match recv {
            Value::String(s) => Ok(Value::String(s.trim().to_string())),
            _ => Err(Error::new("trim expects string receiver", None)),
        },
        "reverse" => match recv {
            Value::String(s) => Ok(Value::String(s.chars().rev().collect())),
            Value::Array(items) => {
                let mut v = items.clone();
                v.reverse();
                Ok(Value::Array(v))
            }
            _ => Err(Error::new("reverse expects string or array receiver", None)),
        },
        "includes" => {
            // String includes method: "hello world".includes("world") -> true
            if let Value::String(s) = recv {
                let a = eval_args(args_expr)?;
                if let Some(Value::String(substring)) = a.get(0) {
                    Ok(Value::Boolean(s.contains(substring)))
                } else {
                    Err(Error::new("includes expects string argument", None))
                }
            } else {
                Err(Error::new("includes expects string receiver", None))
            }
        },

        // Array accessors / transforms
        "length" | "size" => match recv {
            Value::Array(items) => Ok(Value::Number(items.len() as f64)),
            Value::String(s) => Ok(Value::Number(s.chars().count() as f64)),
            Value::Null => Ok(Value::Number(0.0)),
            _ => Err(Error::new("length expects array or string receiver", None)),
        },
        "first" => match recv {
            Value::Array(items) => items
                .first()
                .cloned()
                .ok_or_else(|| Error::new("first on empty array", None)),
            _ => Err(Error::new("first expects array receiver", None)),
        },
        "last" => match recv {
            Value::Array(items) => items
                .last()
                .cloned()
                .ok_or_else(|| Error::new("last on empty array", None)),
            _ => Err(Error::new("last expects array receiver", None)),
        },
        "sort" => match recv {
            Value::Array(items) => {
                let mut nums: Vec<f64> = Vec::with_capacity(items.len());
                for it in items {
                    match it {
                        Value::Number(n) => nums.push(*n),
                        _ => return Err(Error::new("sort expects numeric array", None)),
                    }
                }
                nums.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
                Ok(Value::Array(nums.into_iter().map(Value::Number).collect()))
            }
            _ => Err(Error::new("sort expects array receiver", None)),
        },
        "unique" => match recv {
            Value::Array(items) => {
                let mut set = BTreeSet::new();
                let mut out = Vec::new();
                for it in items {
                    match it {
                        Value::Number(n) => {
                            if set.insert((*n).to_bits()) {
                                out.push(Value::Number(*n));
                            }
                        }
                        _ => return Err(Error::new("unique expects numeric array", None)),
                    }
                }
                Ok(Value::Array(out))
            }
            _ => Err(Error::new("unique expects array receiver", None)),
        },
        "sum" => match recv {
            Value::Array(items) => {
                let mut acc = 0.0;
                for it in items {
                    if let Value::Number(n) = it {
                        acc += n;
                    } else {
                        return Err(Error::new("sum expects numeric array", None));
                    }
                }
                Ok(Value::Number(acc))
            }
            _ => Err(Error::new("sum expects array receiver", None)),
        },
        "avg" => match recv {
            Value::Array(items) => {
                let mut acc = 0.0;
                let mut count = 0usize;
                for it in items {
                    if let Value::Number(n) = it {
                        acc += n;
                        count += 1;
                    } else {
                        return Err(Error::new("avg expects numeric array", None));
                    }
                }
                Ok(Value::Number(if count == 0 {
                    0.0
                } else {
                    acc / count as f64
                }))
            }
            _ => Err(Error::new("avg expects array receiver", None)),
        },
        "min" => match recv {
            Value::Array(items) => {
                let mut cur: Option<f64> = None;
                for it in items {
                    if let Value::Number(n) = it {
                        cur = Some(cur.map_or(*n, |c| c.min(*n)));
                    } else {
                        return Err(Error::new("min expects numeric array", None));
                    }
                }
                Ok(Value::Number(cur.unwrap_or(0.0)))
            }
            _ => Err(Error::new("min expects array receiver", None)),
        },
        "max" => match recv {
            Value::Array(items) => {
                let mut cur: Option<f64> = None;
                for it in items {
                    if let Value::Number(n) = it {
                        cur = Some(cur.map_or(*n, |c| c.max(*n)));
                    } else {
                        return Err(Error::new("max expects numeric array", None));
                    }
                }
                Ok(Value::Number(cur.unwrap_or(0.0)))
            }
            _ => Err(Error::new("max expects array receiver", None)),
        },
        "filter" => match recv {
            Value::Array(items) => {
                let expr = args_expr
                    .get(0)
                    .cloned()
                    .ok_or_else(|| Error::new("filter expects an expression", None))?;
                // Optional param name as second arg
                let param_vals = eval_args(&args_expr[1..])?;
                let param_name = match param_vals.get(0) {
                    Some(Value::String(s)) => s.clone(),
                    _ => "x".to_string(),
                };
                let mut out = Vec::new();
                for it in items {
                    let mut env = HashMap::new();
                    env.insert(param_name.clone(), it.clone());
                    if let Some(base) = base_vars {
                        for (k, v) in base.iter() {
                            env.insert(k.clone(), v.clone());
                        }
                    }
                    let keep = match eval_with_vars_and_custom(&expr, &env, custom_registry)? {
                        Value::Boolean(b) => b,
                        _ => false,
                    };
                    if keep {
                        out.push(it.clone());
                    }
                }
                Ok(Value::Array(out))
            }
            _ => Err(Error::new("filter expects array receiver", None)),
        },
        "find" => match recv {
            Value::Array(items) => {
                let expr = args_expr
                    .get(0)
                    .cloned()
                    .ok_or_else(|| Error::new("find expects an expression", None))?;
                // Optional param name as second arg
                let param_vals = eval_args(&args_expr[1..])?;
                let param_name = match param_vals.get(0) {
                    Some(Value::String(s)) => s.clone(),
                    _ => "x".to_string(),
                };
                for it in items {
                    let mut env = HashMap::new();
                    env.insert(param_name.clone(), it.clone());
                    if let Some(base) = base_vars {
                        for (k, v) in base.iter() {
                            env.insert(k.clone(), v.clone());
                        }
                    }
                    let matches = match eval_with_vars_and_custom(&expr, &env, custom_registry)? {
                        Value::Boolean(b) => b,
                        _ => false,
                    };
                    if matches {
                        return Ok(it.clone());
                    }
                }
                Ok(Value::Null)
            }
            _ => Err(Error::new("find expects array receiver", None)),
        },
        "map" => match recv {
            Value::Array(items) => {
                let expr = args_expr
                    .get(0)
                    .cloned()
                    .ok_or_else(|| Error::new("map expects an expression", None))?;
                let param_vals = eval_args(&args_expr[1..])?;
                let param_name = match param_vals.get(0) {
                    Some(Value::String(s)) => s.clone(),
                    _ => "x".to_string(),
                };
                let mut out = Vec::new();
                for it in items {
                    let mut env = HashMap::new();
                    env.insert(param_name.clone(), it.clone());
                    if let Some(base) = base_vars {
                        for (k, v) in base.iter() {
                            env.insert(k.clone(), v.clone());
                        }
                    }
                    out.push(eval_with_vars_and_custom(&expr, &env, custom_registry)?);
                }
                Ok(Value::Array(out))
            }
            _ => Err(Error::new("map expects array receiver", None)),
        },
        "reduce" => match recv {
            Value::Array(items) => {
                let expr = args_expr
                    .get(0)
                    .cloned()
                    .ok_or_else(|| Error::new("reduce expects expression and initial", None))?;
                let a = eval_args(&args_expr[1..])?;
                let mut acc = a
                    .get(0)
                    .cloned()
                    .ok_or_else(|| Error::new("reduce expects initial value", None))?;
                let val_param = match a.get(1) {
                    Some(Value::String(s)) => s.clone(),
                    _ => "x".to_string(),
                };
                let acc_param = match a.get(2) {
                    Some(Value::String(s)) => s.clone(),
                    _ => "acc".to_string(),
                };
                for it in items {
                    let mut env = HashMap::new();
                    env.insert(val_param.clone(), it.clone());
                    env.insert(acc_param.clone(), acc);
                    if let Some(base) = base_vars {
                        for (k, v) in base.iter() {
                            env.insert(k.clone(), v.clone());
                        }
                    }
                    acc = eval_with_vars_and_custom(&expr, &env, custom_registry)?;
                }
                Ok(acc)
            }
            _ => Err(Error::new("reduce expects array receiver", None)),
        },
        "flatten" => match recv {
            Value::Array(items) => {
                fn flatten(v: &Value, out: &mut Vec<Value>) {
                    match v {
                        Value::Array(inner) => {
                            for it in inner {
                                flatten(it, out);
                            }
                        }
                        other => out.push(other.clone()),
                    }
                }
                let mut out = Vec::new();
                for it in items {
                    flatten(it, &mut out);
                }
                Ok(Value::Array(out))
            }
            _ => Err(Error::new("flatten expects array receiver", None)),
        },
        // compact implemented with Null support
        "compact" => match recv {
            Value::Array(items) => Ok(Value::Array(
                items
                    .iter()
                    .cloned()
                    .filter(|v| !matches!(v, Value::Null))
                    .collect(),
            )),
            _ => Err(Error::new("compact expects array receiver", None)),
        },
        
        // Type casting methods
        "to_s" => match recv {
            Value::String(s) => Ok(Value::String(s.clone())),
            Value::Number(n) => Ok(Value::String(n.to_string())),
            Value::Boolean(b) => Ok(Value::String(if *b { "TRUE".into() } else { "FALSE".into() })),
            Value::Null => Ok(Value::String(String::new())),
            Value::Array(items) => Ok(Value::String(format!("{:?}", items))),
            Value::Currency(n) => Ok(Value::String(format!("{:.4}", n))),
            Value::DateTime(ts) => Ok(Value::String(ts.to_string())),
            Value::Json(s) => Ok(Value::String(s.clone())),
        },
        "to_i" => match recv {
            Value::Number(n) => Ok(Value::Number((*n as i64) as f64)),
            Value::Currency(n) => Ok(Value::Number((*n as i64) as f64)),
            Value::String(s) => {
                let mut clean_s = String::new();
                let mut has_dot = false;
                for (i, c) in s.chars().enumerate() {
                    if i == 0 && (c == '-' || c == '+') {
                        clean_s.push(c);
                    } else if c.is_ascii_digit() {
                        clean_s.push(c);
                    } else if c == '.' && !has_dot {
                        clean_s.push(c);
                        has_dot = true;
                    } else {
                        break;
                    }
                }
                Ok(Value::Number(
                    clean_s.parse::<f64>()
                        .unwrap_or(0.0)
                        .trunc(),
                ))
            },
            Value::Boolean(b) => Ok(Value::Number(if *b { 1.0 } else { 0.0 })),
            Value::Null => Ok(Value::Number(0.0)),
            _ => Err(Error::new("Cannot cast to Integer", None)),
        },
        "to_number" => match recv {
            Value::Number(n) => Ok(Value::Number(*n)),
            Value::Currency(n) => Ok(Value::Number(*n)),
            Value::String(s) => Ok(Value::Number(
                s.parse::<f64>()
                    .map_err(|_| Error::new("Cannot cast String to Number", None))?,
            )),
            Value::Boolean(b) => Ok(Value::Number(if *b { 1.0 } else { 0.0 })),
            Value::Null => Ok(Value::Number(0.0)),
            _ => Err(Error::new("Cannot cast to Number", None)),
        },
        "to_currency" => match recv {
            Value::Currency(n) => Ok(Value::Currency(*n)),
            Value::Number(n) => Ok(Value::Currency(*n)),
            Value::String(s) => Ok(Value::Currency(
                s.parse::<f64>()
                    .map_err(|_| Error::new("Cannot cast String to Currency", None))?,
            )),
            Value::Boolean(b) => Ok(Value::Currency(if *b { 1.0 } else { 0.0 })),
            Value::Null => Ok(Value::Currency(0.0)),
            _ => Err(Error::new("Cannot cast to Currency", None)),
        },
        "to_boolean" => match recv {
            Value::Boolean(b) => Ok(Value::Boolean(*b)),
            Value::Number(n) => Ok(Value::Boolean(*n != 0.0)),
            Value::Currency(n) => Ok(Value::Boolean(*n != 0.0)),
            Value::String(s) => Ok(Value::Boolean(!s.trim().is_empty())),
            Value::Array(items) => Ok(Value::Boolean(!items.is_empty())),
            Value::Null => Ok(Value::Boolean(false)),
            Value::DateTime(ts) => Ok(Value::Boolean(*ts != 0)),
            Value::Json(s) => Ok(Value::Boolean(!s.trim().is_empty())),
        },

        _ => Err(Error::new(format!("Unknown method: .{}()", name), None)),
    }
}
