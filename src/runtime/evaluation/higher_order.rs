use crate::ast::Expr;
use crate::error::Error;
use crate::types::Value;
use crate::custom::FunctionRegistry;
use super::core::{eval_with_vars, eval_with_vars_and_custom};

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub fn eval_higher_order_function(
    name: &str, 
    args: &[Expr], 
    vars: &HashMap<String, Value>
) -> Result<Value, Error> {
    match name {
        "FILTER" => eval_filter(args, vars),
        "FIND" => eval_find(args, vars),
        "MAP" => eval_map(args, vars),
        "REDUCE" => eval_reduce(args, vars),
        "SUMIF" => eval_sumif(args, vars),
        "AVGIF" => eval_avgif(args, vars),
        "COUNTIF" => eval_countif(args, vars),
        _ => Err(Error::new(format!("Unknown higher-order function: {}", name), None)),
    }
}

pub fn eval_higher_order_function_with_custom(
    name: &str, 
    args: &[Expr], 
    vars: &HashMap<String, Value>,
    custom_registry: &Arc<RwLock<FunctionRegistry>>
) -> Result<Value, Error> {
    match name {
        "FILTER" => eval_filter_with_custom(args, vars, custom_registry),
        "FIND" => eval_find_with_custom(args, vars, custom_registry),
        "MAP" => eval_map_with_custom(args, vars, custom_registry),
        "REDUCE" => eval_reduce_with_custom(args, vars, custom_registry),
        "SUMIF" => eval_sumif_with_custom(args, vars, custom_registry),
        "AVGIF" => eval_avgif_with_custom(args, vars, custom_registry),
        "COUNTIF" => eval_countif_with_custom(args, vars, custom_registry),
        _ => Err(Error::new(format!("Unknown higher-order function: {}", name), None)),
    }
}

// FILTER implementation
fn eval_filter(args: &[Expr], vars: &HashMap<String, Value>) -> Result<Value, Error> {
    if args.len() < 2 { 
        return Err(Error::new("FILTER expects (array, expr, [param])", None)); 
    }
    
    let arr_v = eval_with_vars(&args[0], vars)?;
    let lambda = &args[1];
    let param_name = get_param_name(args.get(2), vars)?;
    
    match arr_v {
        Value::Array(items) => {
            let mut out = Vec::new();
            for it in items {
                let mut env = vars.clone(); 
                env.insert(param_name.clone(), it.clone());
                if let Expr::Spread(_) = lambda { 
                    return Err(Error::new("Invalid lambda", None)); 
                }
                if let Value::Boolean(true) = eval_with_vars(lambda, &env)? { 
                    out.push(it); 
                }
            }
            Ok(Value::Array(out))
        }
        _ => Err(Error::new("FILTER first arg must be array", None)),
    }
}

fn eval_filter_with_custom(
    args: &[Expr], 
    vars: &HashMap<String, Value>,
    custom_registry: &Arc<RwLock<FunctionRegistry>>
) -> Result<Value, Error> {
    if args.len() < 2 { 
        return Err(Error::new("FILTER expects (array, expr)", None)); 
    }
    
    let arr_v = eval_with_vars_and_custom(&args[0], vars, custom_registry)?;
    let lambda = &args[1];
    
    match arr_v {
        Value::Array(items) => {
            let mut out = Vec::new();
            for it in items {
                let mut env = vars.clone(); 
                env.insert("x".into(), it.clone());
                if let Value::Boolean(true) = eval_with_vars_and_custom(lambda, &env, custom_registry)? {
                    out.push(it);
                }
            }
            Ok(Value::Array(out))
        }
        _ => Err(Error::new("FILTER first arg must be array", None)),
    }
}

// FIND implementation
fn eval_find(args: &[Expr], vars: &HashMap<String, Value>) -> Result<Value, Error> {
    if args.len() < 2 { 
        return Err(Error::new("FIND expects (array, expr, [param])", None)); 
    }
    
    let arr_v = eval_with_vars(&args[0], vars)?;
    let lambda = &args[1];
    let param_name = get_param_name(args.get(2), vars)?;
    
    match arr_v {
        Value::Array(items) => {
            for it in items {
                let mut env = vars.clone(); 
                env.insert(param_name.clone(), it.clone());
                if let Expr::Spread(_) = lambda { 
                    return Err(Error::new("Invalid lambda", None)); 
                }
                if let Value::Boolean(true) = eval_with_vars(lambda, &env)? { 
                    return Ok(it); 
                }
            }
            Ok(Value::Null)
        }
        _ => Err(Error::new("FIND first arg must be array", None)),
    }
}

fn eval_find_with_custom(
    args: &[Expr], 
    vars: &HashMap<String, Value>,
    custom_registry: &Arc<RwLock<FunctionRegistry>>
) -> Result<Value, Error> {
    if args.len() < 2 { 
        return Err(Error::new("FIND expects (array, expr)", None)); 
    }
    
    let arr_v = eval_with_vars_and_custom(&args[0], vars, custom_registry)?;
    let lambda = &args[1];
    
    match arr_v {
        Value::Array(items) => {
            for it in items {
                let mut env = vars.clone(); 
                env.insert("x".into(), it.clone());
                if let Value::Boolean(true) = eval_with_vars_and_custom(lambda, &env, custom_registry)? {
                    return Ok(it);
                }
            }
            Ok(Value::Null)
        }
        _ => Err(Error::new("FIND first arg must be array", None)),
    }
}

// MAP implementation
fn eval_map(args: &[Expr], vars: &HashMap<String, Value>) -> Result<Value, Error> {
    if args.len() < 2 { 
        return Err(Error::new("MAP expects (array, expr, [param])", None)); 
    }
    
    let arr_v = eval_with_vars(&args[0], vars)?;
    let lambda = &args[1];
    let param_name = get_param_name(args.get(2), vars)?;
    
    match arr_v {
        Value::Array(items) => {
            let mut out = Vec::new();
            for it in items {
                let mut env = vars.clone(); 
                env.insert(param_name.clone(), it.clone());
                if let Expr::Spread(_) = lambda { 
                    return Err(Error::new("Invalid lambda", None)); 
                }
                out.push(eval_with_vars(lambda, &env)?);
            }
            Ok(Value::Array(out))
        }
        _ => Err(Error::new("MAP first arg must be array", None)),
    }
}

fn eval_map_with_custom(
    args: &[Expr], 
    vars: &HashMap<String, Value>,
    custom_registry: &Arc<RwLock<FunctionRegistry>>
) -> Result<Value, Error> {
    if args.len() < 2 { 
        return Err(Error::new("MAP expects (array, expr)", None)); 
    }
    
    let arr_v = eval_with_vars_and_custom(&args[0], vars, custom_registry)?;
    let lambda = &args[1];
    
    match arr_v {
        Value::Array(items) => {
            let mut out = Vec::new();
            for it in items {
                let mut env = vars.clone(); 
                env.insert("x".into(), it);
                out.push(eval_with_vars_and_custom(lambda, &env, custom_registry)?);
            }
            Ok(Value::Array(out))
        }
        _ => Err(Error::new("MAP first arg must be array", None)),
    }
}

// REDUCE implementation
fn eval_reduce(args: &[Expr], vars: &HashMap<String, Value>) -> Result<Value, Error> {
    if args.len() < 3 { 
        return Err(Error::new("REDUCE expects (array, expr, initial, [valParam], [accParam])", None)); 
    }
    
    let arr_v = eval_with_vars(&args[0], vars)?;
    let lambda = &args[1];
    let mut acc = eval_with_vars(&args[2], vars)?;
    
    let val_param = get_param_name(args.get(3), vars).unwrap_or_else(|_| "x".into());
    let acc_param = get_param_name(args.get(4), vars).unwrap_or_else(|_| "acc".into());
    
    match arr_v {
        Value::Array(items) => {
            for it in items {
                let mut env = vars.clone(); 
                env.insert(val_param.clone(), it.clone()); 
                env.insert(acc_param.clone(), acc);
                if let Expr::Spread(_) = lambda { 
                    return Err(Error::new("Invalid lambda", None)); 
                }
                acc = eval_with_vars(lambda, &env)?;
            }
            Ok(acc)
        }
        _ => Err(Error::new("REDUCE first arg must be array", None)),
    }
}

fn eval_reduce_with_custom(
    args: &[Expr], 
    vars: &HashMap<String, Value>,
    custom_registry: &Arc<RwLock<FunctionRegistry>>
) -> Result<Value, Error> {
    if args.len() < 3 { 
        return Err(Error::new("REDUCE expects (array, expr, initial)", None)); 
    }
    
    let arr_v = eval_with_vars_and_custom(&args[0], vars, custom_registry)?;
    let lambda = &args[1];
    let mut acc = eval_with_vars_and_custom(&args[2], vars, custom_registry)?;
    
    match arr_v {
        Value::Array(items) => {
            for it in items {
                let mut env = vars.clone(); 
                env.insert("acc".into(), acc); 
                env.insert("x".into(), it);
                acc = eval_with_vars_and_custom(lambda, &env, custom_registry)?;
            }
            Ok(acc)
        }
        _ => Err(Error::new("REDUCE first arg must be array", None)),
    }
}

// SUMIF implementation
fn eval_sumif(args: &[Expr], vars: &HashMap<String, Value>) -> Result<Value, Error> {
    if args.len() != 2 { 
        return Err(Error::new("SUMIF expects (array, expr)", None)); 
    }
    
    let arr_v = eval_with_vars(&args[0], vars)?;
    let lambda = &args[1];
    
    match arr_v {
        Value::Array(items) => {
            let mut acc = 0.0;
            for it in items {
                let mut env = vars.clone(); 
                env.insert("x".into(), it.clone());
                if let Value::Boolean(true) = eval_with_vars(lambda, &env)? {
                    match it { 
                        Value::Number(n) => acc += n, 
                        Value::Currency(n) => acc += n, 
                        _ => {} 
                    }
                }
            }
            Ok(Value::Number(acc))
        }
        _ => Err(Error::new("SUMIF first arg must be array", None)),
    }
}

fn eval_sumif_with_custom(
    args: &[Expr], 
    vars: &HashMap<String, Value>,
    custom_registry: &Arc<RwLock<FunctionRegistry>>
) -> Result<Value, Error> {
    if args.len() != 2 { 
        return Err(Error::new("SUMIF expects (array, expr)", None)); 
    }
    
    let arr_v = eval_with_vars_and_custom(&args[0], vars, custom_registry)?;
    let lambda = &args[1];
    
    match arr_v {
        Value::Array(items) => {
            let mut acc = 0.0;
            for it in items {
                let mut env = vars.clone(); 
                env.insert("x".into(), it.clone());
                if let Value::Boolean(true) = eval_with_vars_and_custom(lambda, &env, custom_registry)? {
                    match it { 
                        Value::Number(n) | Value::Currency(n) => acc += n, 
                        _ => {} 
                    }
                }
            }
            Ok(Value::Number(acc))
        }
        _ => Err(Error::new("SUMIF first arg must be array", None)),
    }
}

// AVGIF implementation
fn eval_avgif(args: &[Expr], vars: &HashMap<String, Value>) -> Result<Value, Error> {
    if args.len() != 2 { 
        return Err(Error::new("AVGIF expects (array, expr)", None)); 
    }
    
    let arr_v = eval_with_vars(&args[0], vars)?;
    let lambda = &args[1];
    
    match arr_v {
        Value::Array(items) => {
            let mut acc = 0.0; 
            let mut count = 0usize;
            for it in items {
                let mut env = vars.clone(); 
                env.insert("x".into(), it.clone());
                if let Value::Boolean(true) = eval_with_vars(lambda, &env)? {
                    match it { 
                        Value::Number(n) | Value::Currency(n) => { 
                            acc += n; 
                            count += 1; 
                        }, 
                        _ => {} 
                    }
                }
            }
            Ok(Value::Number(if count == 0 { 0.0 } else { acc / count as f64 }))
        }
        _ => Err(Error::new("AVGIF first arg must be array", None)),
    }
}

fn eval_avgif_with_custom(
    args: &[Expr], 
    vars: &HashMap<String, Value>,
    custom_registry: &Arc<RwLock<FunctionRegistry>>
) -> Result<Value, Error> {
    if args.len() != 2 { 
        return Err(Error::new("AVGIF expects (array, expr)", None)); 
    }
    
    let arr_v = eval_with_vars_and_custom(&args[0], vars, custom_registry)?;
    let lambda = &args[1];
    
    match arr_v {
        Value::Array(items) => {
            let mut acc = 0.0; 
            let mut count = 0usize;
            for it in items {
                let mut env = vars.clone(); 
                env.insert("x".into(), it.clone());
                if let Value::Boolean(true) = eval_with_vars_and_custom(lambda, &env, custom_registry)? {
                    match it { 
                        Value::Number(n) | Value::Currency(n) => { 
                            acc += n; 
                            count += 1; 
                        }, 
                        _ => {} 
                    }
                }
            }
            Ok(Value::Number(if count == 0 { 0.0 } else { acc / count as f64 }))
        }
        _ => Err(Error::new("AVGIF first arg must be array", None)),
    }
}

// COUNTIF implementation
fn eval_countif(args: &[Expr], vars: &HashMap<String, Value>) -> Result<Value, Error> {
    if args.len() != 2 { 
        return Err(Error::new("COUNTIF expects (array, expr)", None)); 
    }
    
    let arr_v = eval_with_vars(&args[0], vars)?;
    let lambda = &args[1];
    
    match arr_v {
        Value::Array(items) => {
            let mut count = 0usize;
            for it in items {
                let mut env = vars.clone(); 
                env.insert("x".into(), it.clone());
                if let Value::Boolean(true) = eval_with_vars(lambda, &env)? { 
                    count += 1; 
                }
            }
            Ok(Value::Number(count as f64))
        }
        _ => Err(Error::new("COUNTIF first arg must be array", None)),
    }
}

fn eval_countif_with_custom(
    args: &[Expr], 
    vars: &HashMap<String, Value>,
    custom_registry: &Arc<RwLock<FunctionRegistry>>
) -> Result<Value, Error> {
    if args.len() != 2 { 
        return Err(Error::new("COUNTIF expects (array, expr)", None)); 
    }
    
    let arr_v = eval_with_vars_and_custom(&args[0], vars, custom_registry)?;
    let lambda = &args[1];
    
    match arr_v {
        Value::Array(items) => {
            let mut count = 0usize;
            for it in items {
                let mut env = vars.clone(); 
                env.insert("x".into(), it.clone());
                if let Value::Boolean(true) = eval_with_vars_and_custom(lambda, &env, custom_registry)? { 
                    count += 1; 
                }
            }
            Ok(Value::Number(count as f64))
        }
        _ => Err(Error::new("COUNTIF first arg must be array", None)),
    }
}

// Helper function to extract parameter name
fn get_param_name(arg: Option<&Expr>, vars: &HashMap<String, Value>) -> Result<String, Error> {
    match arg {
        Some(expr) => {
            if let Value::String(s) = eval_with_vars(expr, vars)? { 
                Ok(s) 
            } else { 
                Ok("x".into()) 
            }
        }
        None => Ok("x".into())
    }
}