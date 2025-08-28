use crate::ast::{BinaryOp, Expr, UnaryOp};
use crate::error::Error;
use crate::types::Value;
use crate::custom::FunctionRegistry;
use crate::runtime::{
    builtin_functions::exec_builtin,
    method_calls::{exec_method, exec_method_with_custom},
    type_casting::cast_value,
    utils::{index_array, slice_array}
};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

pub fn eval(expr: &Expr) -> Result<Value, Error> {
    match expr {
        Expr::Number(n) => Ok(Value::Number(*n)),
        Expr::StringLit(s) => Ok(Value::String(s.clone())),
        Expr::Null => Ok(Value::Null),
        Expr::Unary(op, e) => {
            let v = eval(e)?;
            Ok(match op {
                UnaryOp::Plus => Value::Number(v.as_number().ok_or_else(|| Error::new("Unary '+' on non-number", None))?),
                UnaryOp::Minus => Value::Number(-v.as_number().ok_or_else(|| Error::new("Unary '-' on non-number", None))?),
                UnaryOp::Not => Value::Boolean(!v.as_bool().ok_or_else(|| Error::new("Unary '!' on non-boolean", None))?),
            })
        }
        Expr::Binary(l, op, r) => {
            let a = eval(l)?;
            let b = eval(r)?;
            Ok(match op {
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod | BinaryOp::Pow => {
                    let an = a.as_number().ok_or_else(|| Error::new("Arithmetic op on non-number", None))?;
                    let bn = b.as_number().ok_or_else(|| Error::new("Arithmetic op on non-number", None))?;
                    match op {
                        BinaryOp::Add => Value::Number(an + bn),
                        BinaryOp::Sub => Value::Number(an - bn),
                        BinaryOp::Mul => Value::Number(an * bn),
                        BinaryOp::Div => Value::Number(an / bn),
                        BinaryOp::Mod => Value::Number(an % bn),
                        BinaryOp::Pow => Value::Number(an.powf(bn)),
                        _ => unreachable!(),
                    }
                }
                BinaryOp::Gt | BinaryOp::Lt | BinaryOp::Ge | BinaryOp::Le | BinaryOp::Eq | BinaryOp::Ne => {
                    let an = a.as_number().ok_or_else(|| Error::new("Comparison on non-number", None))?;
                    let bn = b.as_number().ok_or_else(|| Error::new("Comparison on non-number", None))?;
                    Value::Boolean(match op {
                        BinaryOp::Gt => an > bn,
                        BinaryOp::Lt => an < bn,
                        BinaryOp::Ge => an >= bn,
                        BinaryOp::Le => an <= bn,
                        BinaryOp::Eq => an == bn,
                        BinaryOp::Ne => an != bn,
                        _ => unreachable!(),
                    })
                }
                BinaryOp::And | BinaryOp::Or => {
                    let ab = a.as_bool().ok_or_else(|| Error::new("Logical op on non-boolean", None))?;
                    let bb = b.as_bool().ok_or_else(|| Error::new("Logical op on non-boolean", None))?;
                    Value::Boolean(match op { BinaryOp::And => ab && bb, BinaryOp::Or => ab || bb, _ => unreachable!() })
                }
            })
        }
        Expr::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for e in items { out.push(eval(e)?); }
            Ok(Value::Array(out))
        }
        Expr::TypeCast { expr, ty } => {
            let v = eval(expr)?;
            cast_value(v, ty)
        }
        Expr::Index { target, index } => {
            let recv = eval(target)?;
            let idx_v = eval(index)?;
            let idx = idx_v.as_number().ok_or_else(|| Error::new("Index must be number", None))? as isize;
            match recv {
                Value::Array(items) => index_array(items, idx),
                _ => Err(Error::new("Indexing only supported on arrays", None)),
            }
        }
        Expr::Slice { target, start, end } => {
            let recv = eval(target)?;
            match recv {
                Value::Array(items) => slice_array(items, start.as_ref().map(|e| eval(e)).transpose()?, end.as_ref().map(|e| eval(e)).transpose()? ),
                _ => Err(Error::new("Slicing only supported on arrays", None)),
            }
        }
        Expr::FunctionCall { name, args } => {
            if name == "__TERNARY__" {
                if args.len() != 3 { return Err(Error::new("Ternary expects 3 args", None)); }
                let cond = eval(&args[0])?.as_bool().ok_or_else(|| Error::new("Ternary condition must be boolean", None))?;
                if cond { eval(&args[1]) } else { eval(&args[2]) }
            } else if name == "FILTER" {
                if args.len() < 2 { return Err(Error::new("FILTER expects (array, expr, [param])", None)); }
                let arr_v = eval(&args[0])?;
                let lambda = &args[1];
                let param_name = if args.len() > 2 { if let Value::String(s) = eval(&args[2])? { s } else { "x".into() } } else { "x".into() };
                match arr_v {
                    Value::Array(items) => {
                        let mut out = Vec::new();
                        for it in items {
                            let mut env = HashMap::new(); env.insert(param_name.clone(), it.clone());
                            if let Expr::Spread(_) = lambda { return Err(Error::new("Invalid lambda", None)); }
                            if let Value::Boolean(b) = eval_with_vars(lambda, &env)? { if b { out.push(it); } }
                        }
                        Ok(Value::Array(out))
                    }
                    _ => Err(Error::new("FILTER first arg must be array", None)),
                }
            } else if name == "MAP" {
                if args.len() < 2 { return Err(Error::new("MAP expects (array, expr, [param])", None)); }
                let arr_v = eval(&args[0])?;
                let lambda = &args[1];
                let param_name = if args.len() > 2 { if let Value::String(s) = eval(&args[2])? { s } else { "x".into() } } else { "x".into() };
                match arr_v {
                    Value::Array(items) => {
                        let mut out = Vec::new();
                        for it in items {
                            let mut env = HashMap::new(); env.insert(param_name.clone(), it.clone());
                            if let Expr::Spread(_) = lambda { return Err(Error::new("Invalid lambda", None)); }
                            out.push(eval_with_vars(lambda, &env)?);
                        }
                        Ok(Value::Array(out))
                    }
                    _ => Err(Error::new("MAP first arg must be array", None)),
                }
            } else if name == "REDUCE" {
                if args.len() < 3 { return Err(Error::new("REDUCE expects (array, expr, initial, [valParam], [accParam])", None)); }
                let arr_v = eval(&args[0])?;
                let lambda = &args[1];
                let mut acc = eval(&args[2])?;
                let val_param = if args.len() > 3 { if let Value::String(s) = eval(&args[3])? { s } else { "x".into() } } else { "x".into() };
                let acc_param = if args.len() > 4 { if let Value::String(s) = eval(&args[4])? { s } else { "acc".into() } } else { "acc".into() };
                match arr_v {
                    Value::Array(items) => {
                        for it in items {
                            let mut env = HashMap::new(); env.insert(val_param.clone(), it.clone()); env.insert(acc_param.clone(), acc);
                            if let Expr::Spread(_) = lambda { return Err(Error::new("Invalid lambda", None)); }
                            acc = eval_with_vars(lambda, &env)?;
                        }
                        Ok(acc)
                    }
                    _ => Err(Error::new("REDUCE first arg must be array", None)),
                }
            } else if name == "SUMIF" {
                if args.len() != 2 { return Err(Error::new("SUMIF expects (array, expr)", None)); }
                let arr_v = eval(&args[0])?;
                let lambda = &args[1];
                match arr_v {
                    Value::Array(items) => {
                        let mut acc = 0.0;
                        for it in items {
                            let mut env = HashMap::new(); env.insert("x".into(), it.clone());
                            if let Value::Boolean(true) = eval_with_vars(lambda, &env)? {
                                match it { Value::Number(n) => acc += n, Value::Currency(n) => acc += n, _ => {} }
                            }
                        }
                        Ok(Value::Number(acc))
                    }
                    _ => Err(Error::new("SUMIF first arg must be array", None)),
                }
            } else if name == "AVGIF" {
                if args.len() != 2 { return Err(Error::new("AVGIF expects (array, expr)", None)); }
                let arr_v = eval(&args[0])?;
                let lambda = &args[1];
                match arr_v {
                    Value::Array(items) => {
                        let mut acc = 0.0; let mut count = 0usize;
                        for it in items {
                            let mut env = HashMap::new(); env.insert("x".into(), it.clone());
                            if let Value::Boolean(true) = eval_with_vars(lambda, &env)? {
                                match it { Value::Number(n) | Value::Currency(n) => { acc += n; count += 1; }, _ => {} }
                            }
                        }
                        Ok(Value::Number(if count==0 { 0.0 } else { acc / count as f64 }))
                    }
                    _ => Err(Error::new("AVGIF first arg must be array", None)),
                }
            } else if name == "COUNTIF" {
                if args.len() != 2 { return Err(Error::new("COUNTIF expects (array, expr)", None)); }
                let arr_v = eval(&args[0])?;
                let lambda = &args[1];
                match arr_v {
                    Value::Array(items) => {
                        let mut count = 0usize;
                        for it in items {
                            let mut env = HashMap::new(); env.insert("x".into(), it.clone());
                            if let Value::Boolean(true) = eval_with_vars(lambda, &env)? { count += 1; }
                        }
                        Ok(Value::Number(count as f64))
                    }
                    _ => Err(Error::new("COUNTIF first arg must be array", None)),
                }
            } else {
                let mut ev_args = Vec::new();
                for a in args {
                    match a {
                        Expr::Spread(inner) => {
                            let v = eval(inner)?;
                            if let Value::Array(items) = v { ev_args.extend(items); } else { return Err(Error::new("Spread expects array", None)); }
                        }
                        _ => ev_args.push(eval(a)?),
                    }
                }
                exec_builtin(name, &ev_args)
            }
        }
        Expr::MethodCall { target, name, args, predicate } => {
            let recv = eval(target)?;
            exec_method(name, *predicate, &recv, args, None)
        }
        Expr::Variable(_) => Err(Error::new("Use eval_with_vars for variables", None)),
        Expr::PropertyAccess { .. } => Err(Error::new("Use eval_with_vars for property access", None)),
        Expr::Spread(_) => Err(Error::new("Spread not allowed here", None)),
        Expr::Assignment { .. } => Err(Error::new("Use eval_with_vars for assignments", None)),
        Expr::Sequence(_) => Err(Error::new("Use eval_with_vars for sequences", None)),
    }
}

pub fn eval_with_vars(expr: &Expr, vars: &HashMap<String, Value>) -> Result<Value, Error> {
    match expr {
        Expr::Number(n) => Ok(Value::Number(*n)),
        Expr::StringLit(s) => Ok(Value::String(s.clone())),
        Expr::Null => Ok(Value::Null),
        Expr::Unary(op, e) => {
            let v = eval_with_vars(e, vars)?;
            Ok(match op {
                UnaryOp::Plus => Value::Number(v.as_number().ok_or_else(|| Error::new("Unary '+' on non-number", None))?),
                UnaryOp::Minus => Value::Number(-v.as_number().ok_or_else(|| Error::new("Unary '-' on non-number", None))?),
                UnaryOp::Not => Value::Boolean(!v.as_bool().ok_or_else(|| Error::new("Unary '!' on non-boolean", None))?),
            })
        }
        Expr::Binary(l, op, r) => {
            let a = eval_with_vars(l, vars)?;
            let b = eval_with_vars(r, vars)?;
            Ok(match op {
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod | BinaryOp::Pow => {
                    let an = a.as_number().ok_or_else(|| Error::new("Arithmetic op on non-number", None))?;
                    let bn = b.as_number().ok_or_else(|| Error::new("Arithmetic op on non-number", None))?;
                    match op {
                        BinaryOp::Add => Value::Number(an + bn),
                        BinaryOp::Sub => Value::Number(an - bn),
                        BinaryOp::Mul => Value::Number(an * bn),
                        BinaryOp::Div => Value::Number(an / bn),
                        BinaryOp::Mod => Value::Number(an % bn),
                        BinaryOp::Pow => Value::Number(an.powf(bn)),
                        _ => unreachable!(),
                    }
                }
                BinaryOp::Gt | BinaryOp::Lt | BinaryOp::Ge | BinaryOp::Le | BinaryOp::Eq | BinaryOp::Ne => {
                    // Support both numeric and value comparisons
                    match (a, b) {
                        (Value::Number(x), Value::Number(y)) => match op {
                            BinaryOp::Eq => Value::Boolean(x == y),
                            BinaryOp::Ne => Value::Boolean(x != y),
                            BinaryOp::Lt => Value::Boolean(x < y),
                            BinaryOp::Le => Value::Boolean(x <= y),
                            BinaryOp::Gt => Value::Boolean(x > y),
                            BinaryOp::Ge => Value::Boolean(x >= y),
                            _ => unreachable!()
                        },
                        (Value::String(x), Value::String(y)) => match op {
                            BinaryOp::Eq => Value::Boolean(x == y),
                            BinaryOp::Ne => Value::Boolean(x != y),
                            BinaryOp::Lt => Value::Boolean(x < y),
                            BinaryOp::Le => Value::Boolean(x <= y),
                            BinaryOp::Gt => Value::Boolean(x > y),
                            BinaryOp::Ge => Value::Boolean(x >= y),
                            _ => unreachable!()
                        },
                        (Value::Boolean(x), Value::Boolean(y)) => match op {
                            BinaryOp::Eq => Value::Boolean(x == y),
                            BinaryOp::Ne => Value::Boolean(x != y),
                            _ => Value::Boolean(false)
                        },
                        _ => match op {
                            BinaryOp::Eq => Value::Boolean(false),
                            BinaryOp::Ne => Value::Boolean(true),
                            _ => return Err(Error::new("Comparison of incompatible types", None))
                        }
                    }
                }
                BinaryOp::And | BinaryOp::Or => {
                    let ab = a.as_bool().ok_or_else(|| Error::new("Logical op on non-boolean", None))?;
                    let bb = b.as_bool().ok_or_else(|| Error::new("Logical op on non-boolean", None))?;
                    Value::Boolean(match op { BinaryOp::And => ab && bb, BinaryOp::Or => ab || bb, _ => unreachable!() })
                }
            })
        }
        Expr::Variable(name) => vars
            .get(name)
            .cloned()
            .ok_or_else(|| Error::new(format!("Missing variable: :{}", name), None)),
        Expr::PropertyAccess { target, property } => {
            let target_value = eval_with_vars(target, vars)?;
            match target_value {
                Value::Json(json_str) => {
                    let parsed: serde_json::Value = serde_json::from_str(&json_str)
                        .map_err(|e| Error::new(format!("Invalid JSON: {}", e), None))?;
                    if let Some(prop_value) = parsed.get(property) {
                        crate::json_to_value(prop_value.clone())
                    } else {
                        Err(Error::new(format!("Property '{}' not found in JSON object", property), None))
                    }
                }
                _ => Err(Error::new(format!("Property access only supported on JSON objects, got {:?}", target_value), None)),
            }
        }
        Expr::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for e in items { out.push(eval_with_vars(e, vars)?); }
            Ok(Value::Array(out))
        }
        Expr::TypeCast { expr, ty } => {
            let v = eval_with_vars(expr, vars)?;
            cast_value(v, ty)
        }
        Expr::Index { target, index } => {
            let recv = eval_with_vars(target, vars)?;
            let idx_v = eval_with_vars(index, vars)?;
            let idx = idx_v.as_number().ok_or_else(|| Error::new("Index must be number", None))? as isize;
            match recv {
                Value::Array(items) => index_array(items, idx),
                _ => Err(Error::new("Index on non-array", None)),
            }
        }
        Expr::Slice { target, start, end } => {
            let recv = eval_with_vars(target, vars)?;
            match recv {
                Value::Array(items) => slice_array(items, start.as_ref().map(|e| eval_with_vars(e, vars)).transpose()?, end.as_ref().map(|e| eval_with_vars(e, vars)).transpose()? ),
                _ => Err(Error::new("Slice on non-array", None)),
            }
        }
        Expr::FunctionCall { name, args } => {
            if name == "__TERNARY__" {
                if args.len() != 3 { return Err(Error::new("Ternary expects 3 args", None)); }
                let cond = eval_with_vars(&args[0], vars)?.as_bool().ok_or_else(|| Error::new("Ternary condition must be boolean", None))?;
                if cond { eval_with_vars(&args[1], vars) } else { eval_with_vars(&args[2], vars) }
            } else {
                // Fall back to built-in functions
                let mut ev_args = Vec::new();
                for a in args {
                    match a {
                        Expr::Spread(inner) => {
                            let v = eval_with_vars(inner, vars)?;
                            if let Value::Array(items) = v { ev_args.extend(items); } else { return Err(Error::new("Spread expects array", None)); }
                        }
                        _ => ev_args.push(eval_with_vars(a, vars)?),
                    }
                }
                exec_builtin(name, &ev_args)
            }
        }
        Expr::MethodCall { target, name, args, predicate } => {
            let recv = eval_with_vars(target, vars)?;
            exec_method(name, *predicate, &recv, args, Some(vars))
        }
        Expr::Spread(_) => Err(Error::new("Spread not allowed here", None)),
        Expr::Assignment { variable: _, value } => {
            let result = eval_with_vars(value, vars)?;
            // For assignments, we need a mutable variables map, but the current API doesn't support that
            // This is a limitation - assignments need to be handled at a higher level
            // For now, we'll return the assigned value but not actually store it
            Ok(result)
        }
        Expr::Sequence(exprs) => {
            let mut last_result = Value::Null;
            for expr in exprs {
                last_result = eval_with_vars(expr, vars)?;
            }
            Ok(last_result)
        }
    }
}

pub fn eval_with_vars_and_custom(expr: &Expr, vars: &HashMap<String, Value>, custom_registry: &Arc<RwLock<FunctionRegistry>>) -> Result<Value, Error> {
    match expr {
        Expr::Number(n) => Ok(Value::Number(*n)),
        Expr::StringLit(s) => Ok(Value::String(s.clone())),
        Expr::Null => Ok(Value::Null),
        Expr::Unary(op, e) => {
            let v = eval_with_vars_and_custom(e, vars, custom_registry)?;
            Ok(match op {
                UnaryOp::Plus => Value::Number(v.as_number().ok_or_else(|| Error::new("Unary '+' on non-number", None))?),
                UnaryOp::Minus => Value::Number(-v.as_number().ok_or_else(|| Error::new("Unary '-' on non-number", None))?),
                UnaryOp::Not => Value::Boolean(!v.as_bool().ok_or_else(|| Error::new("Unary '!' on non-boolean", None))?),
            })
        }
        Expr::Binary(l, op, r) => {
            let a = eval_with_vars_and_custom(l, vars, custom_registry)?;
            let b = eval_with_vars_and_custom(r, vars, custom_registry)?;
            Ok(match op {
                BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod | BinaryOp::Pow => {
                    let an = a.as_number().ok_or_else(|| Error::new("Arithmetic op on non-number", None))?;
                    let bn = b.as_number().ok_or_else(|| Error::new("Arithmetic op on non-number", None))?;
                    match op {
                        BinaryOp::Add => Value::Number(an + bn),
                        BinaryOp::Sub => Value::Number(an - bn),
                        BinaryOp::Mul => Value::Number(an * bn),
                        BinaryOp::Div => Value::Number(an / bn),
                        BinaryOp::Mod => Value::Number(an % bn),
                        BinaryOp::Pow => Value::Number(an.powf(bn)),
                        _ => unreachable!(),
                    }
                }
                BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                    let result = match (a, b) {
                        (Value::Number(x), Value::Number(y)) => match op {
                            BinaryOp::Eq => x == y,
                            BinaryOp::Ne => x != y,
                            BinaryOp::Lt => x < y,
                            BinaryOp::Le => x <= y,
                            BinaryOp::Gt => x > y,
                            BinaryOp::Ge => x >= y,
                            _ => unreachable!(),
                        },
                        (Value::String(x), Value::String(y)) => match op {
                            BinaryOp::Eq => x == y,
                            BinaryOp::Ne => x != y,
                            BinaryOp::Lt => x < y,
                            BinaryOp::Le => x <= y,
                            BinaryOp::Gt => x > y,
                            BinaryOp::Ge => x >= y,
                            _ => unreachable!(),
                        },
                        (Value::Boolean(x), Value::Boolean(y)) => match op {
                            BinaryOp::Eq => x == y,
                            BinaryOp::Ne => x != y,
                            _ => false,
                        },
                        _ => match op {
                            BinaryOp::Eq => false,
                            BinaryOp::Ne => true,
                            _ => return Err(Error::new("Comparison of incompatible types", None)),
                        }
                    };
                    Value::Boolean(result)
                }
                BinaryOp::And | BinaryOp::Or => {
                    let ab = a.as_bool().ok_or_else(|| Error::new("Logical op on non-boolean", None))?;
                    let bb = b.as_bool().ok_or_else(|| Error::new("Logical op on non-boolean", None))?;
                    Value::Boolean(match op { BinaryOp::And => ab && bb, BinaryOp::Or => ab || bb, _ => unreachable!() })
                }
            })
        }
        Expr::Variable(name) => {
            vars.get(name).cloned().ok_or_else(|| Error::new(format!("Undefined variable: {}", name), None))
        }
        Expr::PropertyAccess { target, property } => {
            let target_value = eval_with_vars_and_custom(target, vars, custom_registry)?;
            match target_value {
                Value::Json(json_str) => {
                    let parsed: serde_json::Value = serde_json::from_str(&json_str)
                        .map_err(|e| Error::new(format!("Invalid JSON: {}", e), None))?;
                    if let Some(prop_value) = parsed.get(property) {
                        crate::json_to_value(prop_value.clone())
                    } else {
                        Err(Error::new(format!("Property '{}' not found in JSON object", property), None))
                    }
                }
                _ => Err(Error::new(format!("Property access only supported on JSON objects, got {:?}", target_value), None)),
            }
        }
        Expr::Array(exprs) => {
            let mut items = Vec::new();
            for e in exprs {
                items.push(eval_with_vars_and_custom(e, vars, custom_registry)?);
            }
            Ok(Value::Array(items))
        }
        Expr::Index { target, index } => {
            let arr = eval_with_vars_and_custom(target, vars, custom_registry)?;
            let idx = eval_with_vars_and_custom(index, vars, custom_registry)?;
            match arr {
                Value::Array(items) => {
                    let i = idx.as_number().ok_or_else(|| Error::new("Index must be number", None))? as i32;
                    let len = items.len() as i32;
                    let real_i = if i < 0 { len + i } else { i };
                    if real_i < 0 || real_i >= len {
                        Err(Error::new("Index out of bounds", None))
                    } else {
                        Ok(items[real_i as usize].clone())
                    }
                }
                _ => Err(Error::new("Index on non-array", None)),
            }
        }
        Expr::Slice { target, start, end } => {
            let arr = eval_with_vars_and_custom(target, vars, custom_registry)?;
            match arr {
                Value::Array(items) => {
                    let len = items.len() as i32;
                    let start_i = if let Some(s) = start {
                        let si = eval_with_vars_and_custom(s, vars, custom_registry)?.as_number().ok_or_else(|| Error::new("Slice start must be number", None))? as i32;
                        if si < 0 { (len + si).max(0) } else { si.min(len) }
                    } else { 0 };
                    let end_i = if let Some(e) = end {
                        let ei = eval_with_vars_and_custom(e, vars, custom_registry)?.as_number().ok_or_else(|| Error::new("Slice end must be number", None))? as i32;
                        if ei < 0 { (len + ei).max(0) } else { ei.min(len) }
                    } else { len };
                    if start_i <= end_i {
                        Ok(Value::Array(items[(start_i as usize)..(end_i as usize)].to_vec()))
                    } else {
                        Ok(Value::Array(vec![]))
                    }
                }
                _ => Err(Error::new("Slice on non-array", None)),
            }
        }
        Expr::TypeCast { expr, ty } => {
            let v = eval_with_vars_and_custom(expr, vars, custom_registry)?;
            cast_value(v, ty)
        }
        Expr::FunctionCall { name, args } => {
            // Handle ternary first (must be lazy-evaluated)
            if name == "__TERNARY__" {
                if args.len() != 3 { return Err(Error::new("Ternary expects 3 args", None)); }
                let cond = eval_with_vars_and_custom(&args[0], vars, custom_registry)?.as_bool().ok_or_else(|| Error::new("Ternary condition must be boolean", None))?;
                return if cond { eval_with_vars_and_custom(&args[1], vars, custom_registry) } else { eval_with_vars_and_custom(&args[2], vars, custom_registry) };
            }
            // Check custom functions first
            else if let Ok(registry) = custom_registry.read() {
                if registry.has_function(name) {
                    let mut ev_args = Vec::new();
                    for a in args {
                        match a {
                            Expr::Spread(inner) => {
                                let v = eval_with_vars_and_custom(inner, vars, custom_registry)?;
                                if let Value::Array(items) = v { ev_args.extend(items); } else { return Err(Error::new("Spread expects array", None)); }
                            }
                            _ => ev_args.push(eval_with_vars_and_custom(a, vars, custom_registry)?),
                        }
                    }
                    return registry.execute(name, ev_args);
                }
            }
            
            // Fall back to built-in functions with higher-order support
            if name == "FILTER" {
                if args.len() < 2 { return Err(Error::new("FILTER expects (array, expr)", None)); }
                let arr_v = eval_with_vars_and_custom(&args[0], vars, custom_registry)?;
                let lambda = &args[1];
                match arr_v {
                    Value::Array(items) => {
                        let mut out = Vec::new();
                        for it in items {
                            let mut env = vars.clone(); env.insert("x".into(), it.clone());
                            if let Value::Boolean(true) = eval_with_vars_and_custom(lambda, &env, custom_registry)? {
                                out.push(it);
                            }
                        }
                        Ok(Value::Array(out))
                    }
                    _ => Err(Error::new("FILTER first arg must be array", None)),
                }
            } else if name == "MAP" {
                if args.len() < 2 { return Err(Error::new("MAP expects (array, expr)", None)); }
                let arr_v = eval_with_vars_and_custom(&args[0], vars, custom_registry)?;
                let lambda = &args[1];
                match arr_v {
                    Value::Array(items) => {
                        let mut out = Vec::new();
                        for it in items {
                            let mut env = vars.clone(); env.insert("x".into(), it);
                            out.push(eval_with_vars_and_custom(lambda, &env, custom_registry)?);
                        }
                        Ok(Value::Array(out))
                    }
                    _ => Err(Error::new("MAP first arg must be array", None)),
                }
            } else if name == "REDUCE" {
                if args.len() < 3 { return Err(Error::new("REDUCE expects (array, expr, initial)", None)); }
                let arr_v = eval_with_vars_and_custom(&args[0], vars, custom_registry)?;
                let lambda = &args[1];
                let mut acc = eval_with_vars_and_custom(&args[2], vars, custom_registry)?;
                match arr_v {
                    Value::Array(items) => {
                        for it in items {
                            let mut env = vars.clone(); env.insert("acc".into(), acc); env.insert("x".into(), it);
                            acc = eval_with_vars_and_custom(lambda, &env, custom_registry)?;
                        }
                        Ok(acc)
                    }
                    _ => Err(Error::new("REDUCE first arg must be array", None)),
                }
            } else if name == "SUMIF" {
                if args.len() != 2 { return Err(Error::new("SUMIF expects (array, expr)", None)); }
                let arr_v = eval_with_vars_and_custom(&args[0], vars, custom_registry)?;
                let lambda = &args[1];
                match arr_v {
                    Value::Array(items) => {
                        let mut acc = 0.0;
                        for it in items {
                            let mut env = vars.clone(); env.insert("x".into(), it.clone());
                            if let Value::Boolean(true) = eval_with_vars_and_custom(lambda, &env, custom_registry)? {
                                match it { Value::Number(n) | Value::Currency(n) => acc += n, _ => {} }
                            }
                        }
                        Ok(Value::Number(acc))
                    }
                    _ => Err(Error::new("SUMIF first arg must be array", None)),
                }
            } else if name == "AVGIF" {
                if args.len() != 2 { return Err(Error::new("AVGIF expects (array, expr)", None)); }
                let arr_v = eval_with_vars_and_custom(&args[0], vars, custom_registry)?;
                let lambda = &args[1];
                match arr_v {
                    Value::Array(items) => {
                        let mut acc = 0.0; let mut count = 0usize;
                        for it in items {
                            let mut env = vars.clone(); env.insert("x".into(), it.clone());
                            if let Value::Boolean(true) = eval_with_vars_and_custom(lambda, &env, custom_registry)? {
                                match it { Value::Number(n) | Value::Currency(n) => { acc += n; count += 1; }, _ => {} }
                            }
                        }
                        Ok(Value::Number(if count==0 { 0.0 } else { acc / count as f64 }))
                    }
                    _ => Err(Error::new("AVGIF first arg must be array", None)),
                }
            } else if name == "COUNTIF" {
                if args.len() != 2 { return Err(Error::new("COUNTIF expects (array, expr)", None)); }
                let arr_v = eval_with_vars_and_custom(&args[0], vars, custom_registry)?;
                let lambda = &args[1];
                match arr_v {
                    Value::Array(items) => {
                        let mut count = 0usize;
                        for it in items {
                            let mut env = vars.clone(); env.insert("x".into(), it.clone());
                            if let Value::Boolean(true) = eval_with_vars_and_custom(lambda, &env, custom_registry)? { count += 1; }
                        }
                        Ok(Value::Number(count as f64))
                    }
                    _ => Err(Error::new("COUNTIF first arg must be array", None)),
                }
            } else {
                // Check custom functions first
                if let Ok(registry) = custom_registry.read() {
                    if registry.has_function(name) {
                        let mut ev_args = Vec::new();
                        for a in args {
                            match a {
                                Expr::Spread(inner) => {
                                    let v = eval_with_vars_and_custom(inner, vars, custom_registry)?;
                                    if let Value::Array(items) = v { ev_args.extend(items); } else { return Err(Error::new("Spread expects array", None)); }
                                }
                                _ => ev_args.push(eval_with_vars_and_custom(a, vars, custom_registry)?),
                            }
                        }
                        return registry.execute(name, ev_args);
                    }
                }
                
                // Fall back to built-in functions
                let mut ev_args = Vec::new();
                for a in args {
                    match a {
                        Expr::Spread(inner) => {
                            let v = eval_with_vars_and_custom(inner, vars, custom_registry)?;
                            if let Value::Array(items) = v { ev_args.extend(items); } else { return Err(Error::new("Spread expects array", None)); }
                        }
                        _ => ev_args.push(eval_with_vars_and_custom(a, vars, custom_registry)?),
                    }
                }
                exec_builtin(name, &ev_args)
            }
        }
        Expr::MethodCall { target, name, args, predicate } => {
            let recv = eval_with_vars_and_custom(target, vars, custom_registry)?;
            exec_method_with_custom(name, *predicate, &recv, args, Some(vars), custom_registry)
        }
        Expr::Spread(_) => Err(Error::new("Spread not allowed here", None)),
        Expr::Assignment { variable: _, value } => {
            let result = eval_with_vars_and_custom(value, vars, custom_registry)?;
            // For assignments, we need a mutable variables map, but the current API doesn't support that
            // This is a limitation - assignments need to be handled at a higher level
            // For now, we'll return the assigned value but not actually store it
            Ok(result)
        }
        Expr::Sequence(exprs) => {
            let mut last_result = Value::Null;
            for expr in exprs {
                last_result = eval_with_vars_and_custom(expr, vars, custom_registry)?;
            }
            Ok(last_result)
        }
    }
}

/// Evaluate with support for assignments and sequences
/// This function properly handles variable assignments by maintaining a mutable variable context
pub fn eval_with_assignments(expr: &Expr, vars: &HashMap<String, Value>) -> Result<Value, Error> {
    let mut context = vars.clone();
    eval_with_assignments_context(expr, &mut context)
}

fn eval_with_assignments_context(expr: &Expr, context: &mut HashMap<String, Value>) -> Result<Value, Error> {
    match expr {
        Expr::Assignment { variable, value } => {
            let result = eval_with_assignments_context(value, context)?;
            context.insert(variable.clone(), result.clone());
            Ok(result)
        }
        Expr::Sequence(exprs) => {
            let mut last_result = Value::Null;
            for expr in exprs {
                last_result = eval_with_assignments_context(expr, context)?;
            }
            Ok(last_result)
        }
        // For all other expressions, delegate to eval_with_vars with current context
        _ => eval_with_vars(expr, context)
    }
}