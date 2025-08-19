use crate::ast::{BinaryOp, Expr, TypeName, UnaryOp};
use crate::error::Error;
use crate::types::Value;
use crate::custom::FunctionRegistry;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use chrono::{DateTime, Local, NaiveDate, Utc, Datelike, Timelike};

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
        Expr::Spread(_) => Err(Error::new("Spread not allowed here", None)),
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
                _ => Err(Error::new("Indexing only supported on arrays", None)),
            }
        }
        Expr::Slice { target, start, end } => {
            let recv = eval_with_vars(target, vars)?;
            match recv {
                Value::Array(items) => slice_array(items,
                    match &start { Some(e) => Some(eval_with_vars(e, vars)?), None => None },
                    match &end { Some(e) => Some(eval_with_vars(e, vars)?), None => None }
                ),
                _ => Err(Error::new("Slicing only supported on arrays", None)),
            }
        }
        Expr::Variable(name) => vars
            .get(name)
            .cloned()
            .ok_or_else(|| Error::new(format!("Missing variable: :{}", name), None)),
        Expr::FunctionCall { name, args } => {
            if name == "__TERNARY__" {
                if args.len() != 3 { return Err(Error::new("Ternary expects 3 args", None)); }
                let cond = eval_with_vars(&args[0], vars)?.as_bool().ok_or_else(|| Error::new("Ternary condition must be boolean", None))?;
                if cond { eval_with_vars(&args[1], vars) } else { eval_with_vars(&args[2], vars) }
            } else if name == "FILTER" {
                if args.len() < 2 { return Err(Error::new("FILTER expects (array, expr, [param])", None)); }
                let arr_v = eval_with_vars(&args[0], vars)?;
                let lambda = &args[1];
                let param_name = if args.len() > 2 { if let Value::String(s) = eval_with_vars(&args[2], vars)? { s } else { "x".into() } } else { "x".into() };
                match arr_v {
                    Value::Array(items) => {
                        let mut out = Vec::new();
                        for it in items {
                            let mut env = vars.clone(); env.insert(param_name.clone(), it.clone());
                            if let Value::Boolean(b) = eval_with_vars(lambda, &env)? { if b { out.push(it); } }
                        }
                        Ok(Value::Array(out))
                    }
                    _ => Err(Error::new("FILTER first arg must be array", None)),
                }
            } else if name == "MAP" {
                if args.len() < 2 { return Err(Error::new("MAP expects (array, expr, [param])", None)); }
                let arr_v = eval_with_vars(&args[0], vars)?;
                let lambda = &args[1];
                let param_name = if args.len() > 2 { if let Value::String(s) = eval_with_vars(&args[2], vars)? { s } else { "x".into() } } else { "x".into() };
                match arr_v {
                    Value::Array(items) => {
                        let mut out = Vec::new();
                        for it in items {
                            let mut env = vars.clone(); env.insert(param_name.clone(), it.clone());
                            out.push(eval_with_vars(lambda, &env)?);
                        }
                        Ok(Value::Array(out))
                    }
                    _ => Err(Error::new("MAP first arg must be array", None)),
                }
            } else if name == "REDUCE" {
                if args.len() < 3 { return Err(Error::new("REDUCE expects (array, expr, initial, [valParam], [accParam])", None)); }
                let arr_v = eval_with_vars(&args[0], vars)?;
                let lambda = &args[1];
                let mut acc = eval_with_vars(&args[2], vars)?;
                let val_param = if args.len() > 3 { if let Value::String(s) = eval_with_vars(&args[3], vars)? { s } else { "x".into() } } else { "x".into() };
                let acc_param = if args.len() > 4 { if let Value::String(s) = eval_with_vars(&args[4], vars)? { s } else { "acc".into() } } else { "acc".into() };
                match arr_v {
                    Value::Array(items) => {
                        for it in items {
                            let mut env = vars.clone(); env.insert(val_param.clone(), it.clone()); env.insert(acc_param.clone(), acc);
                            acc = eval_with_vars(lambda, &env)?;
                        }
                        Ok(acc)
                    }
                    _ => Err(Error::new("REDUCE first arg must be array", None)),
                }
            } else if name == "SUMIF" {
                if args.len() != 2 { return Err(Error::new("SUMIF expects (array, expr)", None)); }
                let arr_v = eval_with_vars(&args[0], vars)?;
                let lambda = &args[1];
                match arr_v {
                    Value::Array(items) => {
                        let mut acc = 0.0;
                        for it in items {
                            let mut env = vars.clone(); env.insert("x".into(), it.clone());
                            if let Value::Boolean(true) = eval_with_vars(lambda, &env)? {
                                match it { Value::Number(n) | Value::Currency(n) => acc += n, _ => {} }
                            }
                        }
                        Ok(Value::Number(acc))
                    }
                    _ => Err(Error::new("SUMIF first arg must be array", None)),
                }
            } else if name == "AVGIF" {
                if args.len() != 2 { return Err(Error::new("AVGIF expects (array, expr)", None)); }
                let arr_v = eval_with_vars(&args[0], vars)?;
                let lambda = &args[1];
                match arr_v {
                    Value::Array(items) => {
                        let mut acc = 0.0; let mut count = 0usize;
                        for it in items {
                            let mut env = vars.clone(); env.insert("x".into(), it.clone());
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
                let arr_v = eval_with_vars(&args[0], vars)?;
                let lambda = &args[1];
                match arr_v {
                    Value::Array(items) => {
                        let mut count = 0usize;
                        for it in items {
                            let mut env = vars.clone(); env.insert("x".into(), it.clone());
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
                        _ => match op {
                            BinaryOp::Eq => false,
                            BinaryOp::Ne => true,
                            _ => return Err(Error::new("Type mismatch in comparison", None)),
                        },
                    };
                    Value::Boolean(result)
                }
                BinaryOp::And | BinaryOp::Or => {
                    let ab = a.as_bool().ok_or_else(|| Error::new("Logical op on non-boolean", None))?;
                    let bb = b.as_bool().ok_or_else(|| Error::new("Logical op on non-boolean", None))?;
                    match op {
                        BinaryOp::And => Value::Boolean(ab && bb),
                        BinaryOp::Or => Value::Boolean(ab || bb),
                        _ => unreachable!(),
                    }
                }
            })
        }
        Expr::Variable(name) => {
            vars.get(name).cloned().ok_or_else(|| Error::new(format!("Undefined variable: {}", name), None))
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
            Ok(match ty {
                TypeName::Integer => Value::Number(v.as_number().unwrap_or(0.0).floor()),
                TypeName::Float => Value::Number(v.as_number().unwrap_or(0.0)),
                TypeName::String => Value::String(format!("{:?}", v)),
                TypeName::Boolean => Value::Boolean(v.as_bool().unwrap_or(false)),
                TypeName::Array => match v { Value::Array(_) => v, _ => Value::Array(vec![v]) },
                TypeName::Currency => Value::Currency(v.as_number().unwrap_or(0.0)),
                TypeName::DateTime => Value::DateTime(v.as_number().unwrap_or(0.0) as i64),
                TypeName::Json => Value::Json(format!("{:?}", v)),
            })
        }
        Expr::FunctionCall { name, args } => {
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
    }
}

fn exec_builtin(name: &str, args: &[Value]) -> Result<Value, Error> {
    match name {
        "__CONST_TRUE__" => Ok(Value::Boolean(true)),
        "__CONST_FALSE__" => Ok(Value::Boolean(false)),
        "__TERNARY__" => {
            if args.len() != 3 { return Err(Error::new("Ternary expects 3 args", None)); }
            let cond = args[0].as_bool().ok_or_else(|| Error::new("Ternary condition must be boolean", None))?;
            Ok(if cond { args[1].clone() } else { args[2].clone() })
        }
        "SUM" => {
            let mut acc = 0.0;
            fn sum_value(v: &Value, acc: &mut f64) {
                match v {
                    Value::Number(n) => *acc += *n,
                    Value::Array(items) => {
                        for it in items { sum_value(it, acc); }
                    }
                    Value::Boolean(_) => {}
                    Value::String(_) => {}
                    Value::Null => {}
                    Value::Currency(n) => *acc += *n,
                    Value::DateTime(_) => {}
                    Value::Json(_) => {}
                }
            }
            for a in args { sum_value(a, &mut acc); }
            Ok(Value::Number(acc))
        }
        "AVG" | "AVERAGE" => {
            let mut acc = 0.0;
            let mut count = 0usize;
            fn visit(v: &Value, acc: &mut f64, count: &mut usize) {
                match v {
                    Value::Number(n) => { *acc += *n; *count += 1; }
                    Value::Array(items) => for it in items { visit(it, acc, count); },
                    Value::Boolean(_) => {}
                    Value::String(_) => {}
                    Value::Null => {}
                    Value::Currency(n) => { *acc += *n; *count += 1; }
                    Value::DateTime(_) => {}
                    Value::Json(_) => {}
                }
            }
            for a in args { visit(a, &mut acc, &mut count); }
            let avg = if count == 0 { 0.0 } else { acc / count as f64 };
            Ok(Value::Number(avg))
        }
        "MIN" => {
            let mut cur: Option<f64> = None;
            fn visit(v: &Value, cur: &mut Option<f64>) {
                match v {
                    Value::Number(n) => { *cur = Some(cur.map_or(*n, |c| c.min(*n))); }
                    Value::Array(items) => for it in items { visit(it, cur); },
                    Value::Boolean(_) => {}
                    Value::String(_) => {}
                    Value::Null => {}
                    Value::Currency(n) => { *cur = Some(cur.map_or(*n, |c| c.min(*n))); }
                    Value::DateTime(_) => {}
                    Value::Json(_) => {}
                }
            }
            for a in args { visit(a, &mut cur); }
            Ok(Value::Number(cur.unwrap_or(0.0)))
        }
        "MAX" => {
            let mut cur: Option<f64> = None;
            fn visit(v: &Value, cur: &mut Option<f64>) {
                match v {
                    Value::Number(n) => { *cur = Some(cur.map_or(*n, |c| c.max(*n))); }
                    Value::Array(items) => for it in items { visit(it, cur); },
                    Value::Boolean(_) => {}
                    Value::String(_) => {}
                    Value::Null => {}
                    Value::Currency(n) => { *cur = Some(cur.map_or(*n, |c| c.max(*n))); }
                    Value::DateTime(_) => {}
                    Value::Json(_) => {}
                }
            }
            for a in args { visit(a, &mut cur); }
            Ok(Value::Number(cur.unwrap_or(0.0)))
        }
        "ROUND" => {
            if args.is_empty() { return Ok(Value::Number(0.0)); }
            let n = match args[0] { Value::Number(n) => n, _ => return Err(Error::new("ROUND expects number", None)) };
            let decimals = if args.len() > 1 { match args[1] { Value::Number(d) => d as i32, _ => 0 } } else { 0 };
            let factor = 10f64.powi(decimals.max(0));
            Ok(Value::Number((n * factor).round() / factor))
        }
        "CEIL" => {
            let n = match args.get(0) { Some(Value::Number(n)) => *n, _ => 0.0 };
            Ok(Value::Number(n.ceil()))
        }
        "FLOOR" => {
            let n = match args.get(0) { Some(Value::Number(n)) => *n, _ => 0.0 };
            Ok(Value::Number(n.floor()))
        }
        "ABS" => {
            let n = match args.get(0) { Some(Value::Number(n)) => *n, _ => 0.0 };
            Ok(Value::Number(n.abs()))
        }
        "SQRT" => {
            let n = match args.get(0) { Some(Value::Number(n)) => *n, _ => 0.0 };
            Ok(Value::Number(n.sqrt()))
        }
        "POW" | "POWER" => {
            let a = match args.get(0) { Some(Value::Number(n)) => *n, _ => 0.0 };
            let b = match args.get(1) { Some(Value::Number(n)) => *n, _ => 0.0 };
            Ok(Value::Number(a.powf(b)))
        }
        "MOD" => {
            let a = match args.get(0) { Some(Value::Number(n)) => *n, _ => 0.0 };
            let b = match args.get(1) { Some(Value::Number(n)) => *n, _ => 1.0 };
            Ok(Value::Number(a % b))
        }
        "INT" => {
            let n = match args.get(0) { Some(Value::Number(n)) => *n, _ => 0.0 };
            Ok(Value::Number(n.floor()))
        }
        "CEILING" => {
            let n = match args.get(0) { Some(Value::Number(n)) => *n, _ => 0.0 };
            let _significance = match args.get(1) { Some(Value::Number(s)) => *s, _ => 1.0 };
            Ok(Value::Number(n.ceil()))
        }
        "XOR" => {
            if args.len() != 2 { return Err(Error::new("XOR expects 2 arguments", None)); }
            let a = match &args[0] { Value::Boolean(b) => *b, Value::Number(n) => *n != 0.0, _ => false };
            let b = match &args[1] { Value::Boolean(b) => *b, Value::Number(n) => *n != 0.0, _ => false };
            Ok(Value::Boolean(a != b))
        }
        "AND" => {
            let mut result = true;
            for arg in args {
                let val = match arg { Value::Boolean(b) => *b, Value::Number(n) => *n != 0.0, _ => false };
                result = result && val;
                if !result { break; }
            }
            Ok(Value::Boolean(result))
        }
        "OR" => {
            let mut result = false;
            for arg in args {
                let val = match arg { Value::Boolean(b) => *b, Value::Number(n) => *n != 0.0, _ => false };
                result = result || val;
                if result { break; }
            }
            Ok(Value::Boolean(result))
        }
        "NOT" => {
            let val = match args.get(0) { Some(Value::Boolean(b)) => *b, Some(Value::Number(n)) => *n != 0.0, _ => false };
            Ok(Value::Boolean(!val))
        }
        "IF" => {
            if args.len() < 2 { return Err(Error::new("IF expects at least 2 arguments", None)); }
            let cond = match &args[0] { Value::Boolean(b) => *b, Value::Number(n) => *n != 0.0, _ => false };
            if cond {
                Ok(args[1].clone())
            } else {
                Ok(args.get(2).cloned().unwrap_or(Value::Boolean(false)))
            }
        }
        "IFS" => {
            if args.len() % 2 != 0 { return Err(Error::new("IFS expects pairs of condition,value arguments", None)); }
            for chunk in args.chunks(2) {
                if chunk.len() == 2 {
                    let cond = match &chunk[0] { Value::Boolean(b) => *b, Value::Number(n) => *n != 0.0, _ => false };
                    if cond {
                        return Ok(chunk[1].clone());
                    }
                }
            }
            Ok(Value::Boolean(false))
        }
        "LENGTH" => {
            match args.get(0) {
                Some(Value::Array(items)) => Ok(Value::Number(items.len() as f64)),
                Some(Value::String(s)) => Ok(Value::Number(s.chars().count() as f64)),
                Some(Value::Null) => Ok(Value::Number(0.0)),
                Some(_) | None => Err(Error::new("LENGTH expects array or string", None)),
            }
        }
        "CONCAT" => {
            let mut out = String::new();
            fn push_val(s: &mut String, v: &Value) -> Result<(), Error> {
                match v {
                    Value::String(st) => { s.push_str(st); Ok(()) }
                    Value::Number(n) => { s.push_str(&n.to_string()); Ok(()) }
                    Value::Array(arr) => { for it in arr { push_val(s, it)?; } Ok(()) }
                    Value::Boolean(b) => { s.push_str(if *b {"TRUE"} else {"FALSE"}); Ok(()) }
                    Value::Null => Ok(()),
                    Value::Currency(_) => Ok(()),
                    Value::DateTime(_) => Ok(()),
                    Value::Json(_) => Ok(())
                }
            }
            for a in args { if let Value::Null = a { /* skip */ } else { push_val(&mut out, a)?; } }
            Ok(Value::String(out))
        }
        "UPPER" => match args.get(0) { Some(Value::String(s)) => Ok(Value::String(s.to_uppercase())), _ => Err(Error::new("UPPER expects string", None)) },
        "LOWER" => match args.get(0) { Some(Value::String(s)) => Ok(Value::String(s.to_lowercase())), _ => Err(Error::new("LOWER expects string", None)) },
        "TRIM" => match args.get(0) { Some(Value::String(s)) => Ok(Value::String(s.trim().to_string())), _ => Err(Error::new("TRIM expects string", None)) },
        "SUBSTRING" => {
            if args.len() < 2 {
                return Err(Error::new("SUBSTRING expects string, start, [length]", None));
            }
            let string = match args.get(0) {
                Some(Value::String(s)) => s,
                _ => return Err(Error::new("SUBSTRING expects string as first argument", None)),
            };
            let start = match args.get(1) {
                Some(Value::Number(n)) => *n as usize,
                _ => return Err(Error::new("SUBSTRING expects number as second argument", None)),
            };
            
            // Convert to characters for proper Unicode handling
            let chars: Vec<char> = string.chars().collect();
            let string_len = chars.len();
            
            // Handle optional length parameter
            let end = if let Some(Value::Number(len)) = args.get(2) {
                let length = *len as usize;
                start.saturating_add(length).min(string_len)
            } else {
                string_len
            };
            
            // Clamp start to string bounds
            let start = start.min(string_len);
            let end = end.max(start);
            
            if start >= string_len {
                Ok(Value::String(String::new()))
            } else {
                let substring: String = chars[start..end].iter().collect();
                Ok(Value::String(substring))
            }
        }
        "ISBLANK" => {
            let v = args.get(0).cloned().unwrap_or(Value::Null);
            Ok(Value::Boolean(is_blank(&v)))
        }
        "ISNUMBER" => {
            let v = args.get(0).cloned().unwrap_or(Value::Null);
            Ok(Value::Boolean(matches!(v, Value::Number(_) | Value::Currency(_))))
        }
        "ISTEXT" => {
            let v = args.get(0).cloned().unwrap_or(Value::Null);
            Ok(Value::Boolean(matches!(v, Value::String(_))))
        }
        "ARRAY" => Ok(Value::Array(args.to_vec())),
        "FLATTEN" => {
            fn flatten(v: &Value, out: &mut Vec<Value>) {
                match v {
                    Value::Array(items) => { for it in items { flatten(it, out); } }
                    other => out.push(other.clone()),
                }
            }
            let mut out = Vec::new();
            for a in args { flatten(a, &mut out); }
            Ok(Value::Array(out))
        }
        "FIRST" => match args.get(0) { Some(Value::Array(items)) => items.first().cloned().ok_or_else(|| Error::new("FIRST on empty array", None)), _ => Err(Error::new("FIRST expects array", None)) },
        "LAST" => match args.get(0) { Some(Value::Array(items)) => items.last().cloned().ok_or_else(|| Error::new("LAST on empty array", None)), _ => Err(Error::new("LAST expects array", None)) },
        "CONTAINS" => {
            if let Some(Value::Array(items)) = args.get(0) {
                let needle = args.get(1).cloned().unwrap_or(Value::Null);
                Ok(Value::Boolean(items.iter().any(|v| values_equal(v, &needle))))
            } else { Err(Error::new("CONTAINS expects array, value", None)) }
        }
        "UNIQUE" => match args.get(0) {
            Some(Value::Array(items)) => {
                use std::collections::BTreeSet;
                let mut set = BTreeSet::new();
                let mut out = Vec::new();
                for it in items { if let Value::Number(n) = it { if set.insert(n.to_bits()) { out.push(Value::Number(*n)); } } }
                Ok(Value::Array(out))
            }
            _ => Err(Error::new("UNIQUE expects array", None))
        },
        "SORT" => match args.get(0) {
            Some(Value::Array(items)) => {
                let desc = matches!(args.get(1), Some(Value::String(s)) if s.eq_ignore_ascii_case("DESC"));
                let mut nums: Vec<f64> = Vec::new();
                for it in items { if let Value::Number(n) = it { nums.push(*n); } else { return Err(Error::new("SORT expects numeric array", None)); } }
                nums.sort_by(|a,b| a.partial_cmp(b).unwrap());
                if desc { nums.reverse(); }
                Ok(Value::Array(nums.into_iter().map(Value::Number).collect()))
            }
            _ => Err(Error::new("SORT expects array", None))
        },
        "REVERSE" => match args.get(0) {
            Some(Value::Array(items)) => { let mut v = items.clone(); v.reverse(); Ok(Value::Array(v)) }
            Some(Value::String(s)) => Ok(Value::String(s.chars().rev().collect())),
            _ => Err(Error::new("REVERSE expects array or string", None))
        },
        "SPLIT" => match (args.get(0), args.get(1)) {
            (Some(Value::String(s)), Some(Value::String(sep))) => Ok(Value::Array(s.split(sep).map(|p| Value::String(p.to_string())).collect())),
            (Some(Value::String(s)), None) => Ok(Value::Array(s.split(',').map(|p| Value::String(p.trim().to_string())).collect())),
            _ => Err(Error::new("SPLIT expects string, [separator]", None))
        },
        "REPLACE" => match (args.get(0), args.get(1), args.get(2)) {
            (Some(Value::String(s)), Some(Value::String(from)), Some(Value::String(to))) => Ok(Value::String(s.replace(from, to))),
            _ => Err(Error::new("REPLACE expects string, search, replace", None))
        },
        "JOIN" => match args.get(0) {
            Some(Value::Array(items)) => {
                let sep = match args.get(1) { Some(Value::String(s)) => s.as_str(), _ => "," };
                let mut parts: Vec<String> = Vec::new();
                for it in items {
                    match it {
                        Value::String(s) => parts.push(s.clone()),
                        Value::Number(n) => parts.push(n.to_string()),
                        Value::Boolean(b) => parts.push(if *b {"TRUE".into()} else {"FALSE".into()}),
                        Value::Null => parts.push(String::new()),
                        Value::Currency(n) => parts.push(format!("{:.4}", n)),
                        Value::DateTime(ts) => parts.push(ts.to_string()),
                        Value::Json(s) => parts.push(s.clone()),
                        Value::Array(_) => return Err(Error::new("JOIN does not flatten nested arrays", None)),
                    }
                }
                Ok(Value::String(parts.join(sep)))
            }
            _ => Err(Error::new("JOIN expects array, [separator]", None))
        },
        "MEDIAN" => {
            let mut nums: Vec<f64> = Vec::new();
            fn collect_nums(v: &Value, nums: &mut Vec<f64>) {
                match v {
                    Value::Number(n) => nums.push(*n),
                    Value::Currency(n) => nums.push(*n),
                    Value::Array(items) => for item in items { collect_nums(item, nums); },
                    _ => {}
                }
            }
            for arg in args { collect_nums(arg, &mut nums); }
            if nums.is_empty() { return Ok(Value::Number(0.0)); }
            nums.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let len = nums.len();
            Ok(Value::Number(if len % 2 == 0 {
                (nums[len / 2 - 1] + nums[len / 2]) / 2.0
            } else {
                nums[len / 2]
            }))
        }
        "MODE.SNGL" | "MODESNGL" | "MODE_SNGL" => {
            let mut nums: Vec<f64> = Vec::new();
            fn collect_nums(v: &Value, nums: &mut Vec<f64>) {
                match v {
                    Value::Number(n) => nums.push(*n),
                    Value::Currency(n) => nums.push(*n),
                    Value::Array(items) => for item in items { collect_nums(item, nums); },
                    _ => {}
                }
            }
            for arg in args { collect_nums(arg, &mut nums); }
            if nums.is_empty() { return Ok(Value::Number(0.0)); }
            
            use std::collections::HashMap;
            let mut counts = HashMap::new();
            let mut first_occurrence = HashMap::new();
            for (index, &n) in nums.iter().enumerate() {
                let bits = n.to_bits();
                *counts.entry(bits).or_insert(0) += 1;
                first_occurrence.entry(bits).or_insert(index);
            }
            
            let max_count = *counts.values().max().unwrap();
            let mode_bits = counts.into_iter()
                .filter(|(_, count)| *count == max_count)
                .min_by_key(|(bits, _)| first_occurrence[bits])
                .unwrap().0;
            
            Ok(Value::Number(f64::from_bits(mode_bits)))
        }
        "STDEV.P" | "STDEVP" | "STDEV_P" => {
            let mut nums: Vec<f64> = Vec::new();
            fn collect_nums(v: &Value, nums: &mut Vec<f64>) {
                match v {
                    Value::Number(n) => nums.push(*n),
                    Value::Currency(n) => nums.push(*n),
                    Value::Array(items) => for item in items { collect_nums(item, nums); },
                    _ => {}
                }
            }
            for arg in args { collect_nums(arg, &mut nums); }
            if nums.is_empty() { return Ok(Value::Number(0.0)); }
            
            let mean = nums.iter().sum::<f64>() / nums.len() as f64;
            let variance = nums.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / nums.len() as f64;
            Ok(Value::Number(variance.sqrt()))
        }
        "VAR.P" | "VARP" | "VAR_P" => {
            let mut nums: Vec<f64> = Vec::new();
            fn collect_nums(v: &Value, nums: &mut Vec<f64>) {
                match v {
                    Value::Number(n) => nums.push(*n),
                    Value::Currency(n) => nums.push(*n),
                    Value::Array(items) => for item in items { collect_nums(item, nums); },
                    _ => {}
                }
            }
            for arg in args { collect_nums(arg, &mut nums); }
            if nums.is_empty() { return Ok(Value::Number(0.0)); }
            
            let mean = nums.iter().sum::<f64>() / nums.len() as f64;
            let variance = nums.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / nums.len() as f64;
            Ok(Value::Number(variance))
        }
        "PERCENTILE.INC" | "PERCENTILEINC" | "PERCENTILE_INC" => {
            if args.len() < 2 { return Err(Error::new("PERCENTILE.INC expects array and percentile", None)); }
            let mut nums: Vec<f64> = Vec::new();
            fn collect_nums(v: &Value, nums: &mut Vec<f64>) {
                match v {
                    Value::Number(n) => nums.push(*n),
                    Value::Currency(n) => nums.push(*n),
                    Value::Array(items) => for item in items { collect_nums(item, nums); },
                    _ => {}
                }
            }
            for i in 0..args.len()-1 { collect_nums(&args[i], &mut nums); }
            let percentile = match args.last() { Some(Value::Number(p)) => *p, _ => return Err(Error::new("Percentile must be a number", None)) };
            
            if nums.is_empty() { return Ok(Value::Number(0.0)); }
            if percentile < 0.0 || percentile > 1.0 { return Err(Error::new("Percentile must be between 0 and 1", None)); }
            
            nums.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let len = nums.len() as f64;
            let rank = percentile * (len - 1.0);
            let rank_floor = rank.floor() as usize;
            let rank_ceil = rank.ceil() as usize;
            
            if rank_floor == rank_ceil || rank_ceil >= nums.len() {
                Ok(Value::Number(nums[rank_floor.min(nums.len() - 1)]))
            } else {
                let weight = rank - rank_floor as f64;
                Ok(Value::Number(nums[rank_floor] * (1.0 - weight) + nums[rank_ceil] * weight))
            }
        }
        "QUARTILE.INC" | "QUARTILEINC" | "QUARTILE_INC" => {
            if args.len() < 2 { return Err(Error::new("QUARTILE.INC expects array and quartile", None)); }
            let mut nums: Vec<f64> = Vec::new();
            fn collect_nums(v: &Value, nums: &mut Vec<f64>) {
                match v {
                    Value::Number(n) => nums.push(*n),
                    Value::Currency(n) => nums.push(*n),
                    Value::Array(items) => for item in items { collect_nums(item, nums); },
                    _ => {}
                }
            }
            for i in 0..args.len()-1 { collect_nums(&args[i], &mut nums); }
            let quartile = match args.last() { Some(Value::Number(q)) => *q as i32, _ => return Err(Error::new("Quartile must be a number", None)) };
            
            if nums.is_empty() { return Ok(Value::Number(0.0)); }
            if quartile < 0 || quartile > 4 { return Err(Error::new("Quartile must be 0-4", None)); }
            
            let percentile = quartile as f64 / 4.0;
            nums.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let len = nums.len() as f64;
            let rank = percentile * (len - 1.0);
            let rank_floor = rank.floor() as usize;
            let rank_ceil = rank.ceil() as usize;
            
            if rank_floor == rank_ceil || rank_ceil >= nums.len() {
                Ok(Value::Number(nums[rank_floor.min(nums.len() - 1)]))
            } else {
                let weight = rank - rank_floor as f64;
                Ok(Value::Number(nums[rank_floor] * (1.0 - weight) + nums[rank_ceil] * weight))
            }
        }
        
        // Date/Time Functions
        "NOW" => {
            let now = Utc::now();
            Ok(Value::DateTime(now.timestamp()))
        }
        "DATE" => {
            let today = Local::now().date_naive();
            let timestamp = today.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp();
            Ok(Value::DateTime(timestamp))
        }
        "TIME" => {
            let now = Local::now().time();
            let seconds_since_midnight = now.num_seconds_from_midnight() as f64;
            Ok(Value::Number(seconds_since_midnight))
        }
        "YEAR" => {
            if let Some(Value::DateTime(timestamp)) = args.get(0) {
                let dt = DateTime::from_timestamp(*timestamp, 0)
                    .ok_or_else(|| Error::new("Invalid timestamp", None))?;
                Ok(Value::Number(dt.year() as f64))
            } else {
                Err(Error::new("YEAR expects datetime", None))
            }
        }
        "MONTH" => {
            if let Some(Value::DateTime(timestamp)) = args.get(0) {
                let dt = DateTime::from_timestamp(*timestamp, 0)
                    .ok_or_else(|| Error::new("Invalid timestamp", None))?;
                Ok(Value::Number(dt.month() as f64))
            } else {
                Err(Error::new("MONTH expects datetime", None))
            }
        }
        "DAY" => {
            if let Some(Value::DateTime(timestamp)) = args.get(0) {
                let dt = DateTime::from_timestamp(*timestamp, 0)
                    .ok_or_else(|| Error::new("Invalid timestamp", None))?;
                Ok(Value::Number(dt.day() as f64))
            } else {
                Err(Error::new("DAY expects datetime", None))
            }
        }
        "DATEADD" => {
            if args.len() < 3 {
                return Err(Error::new("DATEADD expects date, interval, unit", None));
            }
            let timestamp = match args.get(0) {
                Some(Value::DateTime(ts)) => *ts,
                _ => return Err(Error::new("DATEADD expects datetime as first argument", None)),
            };
            let interval = match args.get(1) {
                Some(Value::Number(n)) => *n as i64,
                _ => return Err(Error::new("DATEADD expects number as second argument", None)),
            };
            let unit = match args.get(2) {
                Some(Value::String(s)) => s.to_lowercase(),
                _ => return Err(Error::new("DATEADD expects string unit as third argument", None)),
            };
            
            let dt = DateTime::from_timestamp(timestamp, 0)
                .ok_or_else(|| Error::new("Invalid timestamp", None))?;
            
            let new_dt = match unit.as_str() {
                "days" | "day" | "d" => dt + chrono::Duration::days(interval),
                "hours" | "hour" | "h" => dt + chrono::Duration::hours(interval),
                "minutes" | "minute" | "m" => dt + chrono::Duration::minutes(interval),
                "seconds" | "second" | "s" => dt + chrono::Duration::seconds(interval),
                "weeks" | "week" | "w" => dt + chrono::Duration::weeks(interval),
                "months" | "month" => {
                    // Handle months specially since Duration doesn't support months
                    let mut year = dt.year();
                    let mut month = dt.month() as i32;
                    month += interval as i32;
                    while month > 12 {
                        year += 1;
                        month -= 12;
                    }
                    while month < 1 {
                        year -= 1;
                        month += 12;
                    }
                    let new_date = NaiveDate::from_ymd_opt(year, month as u32, dt.day())
                        .unwrap_or_else(|| NaiveDate::from_ymd_opt(year, month as u32, 28).unwrap());
                    new_date.and_time(dt.time()).and_utc()
                }
                "years" | "year" | "y" => {
                    let new_year = dt.year() + interval as i32;
                    let new_date = NaiveDate::from_ymd_opt(new_year, dt.month(), dt.day())
                        .unwrap_or_else(|| NaiveDate::from_ymd_opt(new_year, dt.month(), 28).unwrap());
                    new_date.and_time(dt.time()).and_utc()
                }
                _ => return Err(Error::new("DATEADD unit must be one of: days, hours, minutes, seconds, weeks, months, years", None)),
            };
            
            Ok(Value::DateTime(new_dt.timestamp()))
        }
        "DATEDIFF" => {
            if args.len() < 3 {
                return Err(Error::new("DATEDIFF expects date1, date2, unit", None));
            }
            let timestamp1 = match args.get(0) {
                Some(Value::DateTime(ts)) => *ts,
                _ => return Err(Error::new("DATEDIFF expects datetime as first argument", None)),
            };
            let timestamp2 = match args.get(1) {
                Some(Value::DateTime(ts)) => *ts,
                _ => return Err(Error::new("DATEDIFF expects datetime as second argument", None)),
            };
            let unit = match args.get(2) {
                Some(Value::String(s)) => s.to_lowercase(),
                _ => return Err(Error::new("DATEDIFF expects string unit as third argument", None)),
            };
            
            let dt1 = DateTime::from_timestamp(timestamp1, 0)
                .ok_or_else(|| Error::new("Invalid timestamp1", None))?;
            let dt2 = DateTime::from_timestamp(timestamp2, 0)
                .ok_or_else(|| Error::new("Invalid timestamp2", None))?;
            
            let duration = dt2.signed_duration_since(dt1);
            
            let diff = match unit.as_str() {
                "days" | "day" | "d" => duration.num_days() as f64,
                "hours" | "hour" | "h" => duration.num_hours() as f64,
                "minutes" | "minute" | "m" => duration.num_minutes() as f64,
                "seconds" | "second" | "s" => duration.num_seconds() as f64,
                "weeks" | "week" | "w" => duration.num_weeks() as f64,
                "months" | "month" => {
                    // Approximate months calculation
                    let years_diff = dt2.year() - dt1.year();
                    let months_diff = dt2.month() as i32 - dt1.month() as i32;
                    (years_diff * 12 + months_diff) as f64
                }
                "years" | "year" | "y" => (dt2.year() - dt1.year()) as f64,
                _ => return Err(Error::new("DATEDIFF unit must be one of: days, hours, minutes, seconds, weeks, months, years", None)),
            };
            
            Ok(Value::Number(diff))
        }
        
        // Financial Functions
        "PMT" => {
            // PMT(rate, nper, pv, [fv], [type])
            // Calculates the payment for a loan based on constant payments and a constant interest rate
            if args.len() < 3 || args.len() > 5 {
                return Err(Error::new("PMT expects 3-5 arguments: rate, nper, pv, [fv], [type]", None));
            }
            
            let rate = args[0].as_number().ok_or_else(|| Error::new("PMT rate must be a number", None))?;
            let nper = args[1].as_number().ok_or_else(|| Error::new("PMT nper must be a number", None))?;
            let pv = args[2].as_number().ok_or_else(|| Error::new("PMT pv must be a number", None))?;
            let fv = args.get(3).and_then(|v| v.as_number()).unwrap_or(0.0);
            let payment_type = args.get(4).and_then(|v| v.as_number()).unwrap_or(0.0);
            
            // Validate inputs
            if nper <= 0.0 {
                return Err(Error::new("PMT nper must be positive", None));
            }
            
            let payment_at_beginning = payment_type != 0.0;
            
            let pmt = if rate == 0.0 {
                // Special case: no interest
                -(pv + fv) / nper
            } else {
                // Standard PMT formula
                let pvif = (1.0 + rate).powf(nper);
                let payment = -(pv * pvif + fv) / (((pvif - 1.0) / rate) * if payment_at_beginning { 1.0 + rate } else { 1.0 });
                payment
            };
            
            Ok(Value::Number(pmt))
        }
        
        // SUMIF/AVGIF/COUNTIF handled in FunctionCall branch to preserve lambda expr
        _ => Err(Error::new(format!("Unknown function: {}", name), None)),
    }
}

fn exec_method(name: &str, predicate: bool, recv: &Value, args_expr: &[Expr], base_vars: Option<&HashMap<String, Value>>) -> Result<Value, Error> {
    let lname = name.to_lowercase();
    if predicate {
        return match lname.as_str() {
            "positive" => Ok(Value::Boolean(recv.as_number().map(|n| n > 0.0).unwrap_or(false))),
            "negative" => Ok(Value::Boolean(recv.as_number().map(|n| n < 0.0).unwrap_or(false))),
            "zero" => Ok(Value::Boolean(recv.as_number().map(|n| n == 0.0).unwrap_or(false))),
            "even" => Ok(Value::Boolean(recv.as_number().map(|n| (n as i64) % 2 == 0).unwrap_or(false))),
            "odd" => Ok(Value::Boolean(recv.as_number().map(|n| (n as i64) % 2 != 0).unwrap_or(false))),
            "numeric" => Ok(Value::Boolean(matches!(recv, Value::Number(_)))),
            "array" => Ok(Value::Boolean(matches!(recv, Value::Array(_)))),
            "nil" => Ok(Value::Boolean(matches!(recv, Value::Null))),
            "blank" => Ok(Value::Boolean(is_blank(recv))),
            "present" => Ok(Value::Boolean(!is_blank(recv))),
            _ => Err(Error::new(format!("Unknown predicate method: {}?", name), None)),
        };
    }

    // Helper to evaluate argument expressions with spread handling
    let eval_args = |exprs: &[Expr]| -> Result<Vec<Value>, Error> {
        let mut out = Vec::new();
        for e in exprs {
            match e {
                Expr::Spread(inner) => {
                    let v = match base_vars { Some(env) => eval_with_vars(inner, env)?, None => eval(inner)? };
                    if let Value::Array(items) = v { out.extend(items); } else { return Err(Error::new("Spread expects array", None)); }
                }
                _ => {
                    let v = match base_vars { Some(env) => eval_with_vars(e, env)?, None => eval(e)? };
                    out.push(v);
                }
            }
        }
        Ok(out)
    };

    match lname.as_str() {
        // Numeric transformations on receiver
        "abs" => Ok(Value::Number(recv.as_number().ok_or_else(|| Error::new("abs expects number receiver", None))?.abs())),
        "round" => {
            let n = recv.as_number().ok_or_else(|| Error::new("round expects number receiver", None))?;
            let a = eval_args(args_expr)?;
            let decimals = match a.get(0) { Some(Value::Number(d)) => *d as i32, _ => 0 };
            let factor = 10f64.powi(decimals.max(0));
            Ok(Value::Number((n * factor).round() / factor))
        }
        "floor" => Ok(Value::Number(recv.as_number().ok_or_else(|| Error::new("floor expects number receiver", None))?.floor())),
        "ceil" => Ok(Value::Number(recv.as_number().ok_or_else(|| Error::new("ceil expects number receiver", None))?.ceil())),

        // String transforms
        "upper" => match recv { Value::String(s) => Ok(Value::String(s.to_uppercase())), _ => Err(Error::new("upper expects string receiver", None)) },
        "lower" => match recv { Value::String(s) => Ok(Value::String(s.to_lowercase())), _ => Err(Error::new("lower expects string receiver", None)) },
        "trim" => match recv { Value::String(s) => Ok(Value::String(s.trim().to_string())), _ => Err(Error::new("trim expects string receiver", None)) },
        "reverse" => match recv {
            Value::String(s) => Ok(Value::String(s.chars().rev().collect())),
            Value::Array(items) => { let mut v = items.clone(); v.reverse(); Ok(Value::Array(v)) },
            _ => Err(Error::new("reverse expects string or array receiver", None))
        },

        // Array accessors / transforms
        "length" | "size" => match recv {
            Value::Array(items) => Ok(Value::Number(items.len() as f64)),
            Value::String(s) => Ok(Value::Number(s.chars().count() as f64)),
            Value::Null => Ok(Value::Number(0.0)),
            _ => Err(Error::new("length expects array or string receiver", None))
        },
        "first" => match recv { Value::Array(items) => items.first().cloned().ok_or_else(|| Error::new("first on empty array", None)), _ => Err(Error::new("first expects array receiver", None)) },
        "last" => match recv { Value::Array(items) => items.last().cloned().ok_or_else(|| Error::new("last on empty array", None)), _ => Err(Error::new("last expects array receiver", None)) },
        "sort" => match recv {
            Value::Array(items) => {
                let mut nums: Vec<f64> = Vec::with_capacity(items.len());
                for it in items { match it { Value::Number(n) => nums.push(*n), _ => return Err(Error::new("sort expects numeric array", None)) } }
                nums.sort_by(|a,b| a.partial_cmp(b).unwrap());
                Ok(Value::Array(nums.into_iter().map(Value::Number).collect()))
            }
            _ => Err(Error::new("sort expects array receiver", None))
        },
        "unique" => match recv {
            Value::Array(items) => {
                use std::collections::BTreeSet;
                let mut set = BTreeSet::new();
                let mut out = Vec::new();
                for it in items {
                    match it { Value::Number(n) => { if set.insert((*n).to_bits()) { out.push(Value::Number(*n)); } }, _ => return Err(Error::new("unique expects numeric array", None)) }
                }
                Ok(Value::Array(out))
            }
            _ => Err(Error::new("unique expects array receiver", None))
        },
        "sum" => match recv {
            Value::Array(items) => {
                let mut acc = 0.0; for it in items { if let Value::Number(n) = it { acc += n; } else { return Err(Error::new("sum expects numeric array", None)); } }
                Ok(Value::Number(acc))
            }
            _ => Err(Error::new("sum expects array receiver", None))
        },
        "avg" => match recv {
            Value::Array(items) => {
                let mut acc = 0.0; let mut count = 0usize; for it in items { if let Value::Number(n) = it { acc += n; count += 1; } else { return Err(Error::new("avg expects numeric array", None)); } }
                Ok(Value::Number(if count==0 { 0.0 } else { acc / count as f64 }))
            }
            _ => Err(Error::new("avg expects array receiver", None))
        },
        "min" => match recv {
            Value::Array(items) => {
                let mut cur: Option<f64> = None; for it in items { if let Value::Number(n) = it { cur = Some(cur.map_or(*n, |c| c.min(*n))); } else { return Err(Error::new("min expects numeric array", None)); } }
                Ok(Value::Number(cur.unwrap_or(0.0)))
            }
            _ => Err(Error::new("min expects array receiver", None))
        },
        "max" => match recv {
            Value::Array(items) => {
                let mut cur: Option<f64> = None; for it in items { if let Value::Number(n) = it { cur = Some(cur.map_or(*n, |c| c.max(*n))); } else { return Err(Error::new("max expects numeric array", None)); } }
                Ok(Value::Number(cur.unwrap_or(0.0)))
            }
            _ => Err(Error::new("max expects array receiver", None))
        },
        "filter" => match recv {
            Value::Array(items) => {
                let expr = args_expr.get(0).cloned().ok_or_else(|| Error::new("filter expects an expression", None))?;
                // Optional param name as second arg
                let param_vals = eval_args(&args_expr[1..])?;
                let param_name = match param_vals.get(0) { Some(Value::String(s)) => s.clone(), _ => "x".to_string() };
                let mut out = Vec::new();
                for it in items {
                    let mut env = HashMap::new(); env.insert(param_name.clone(), it.clone());
                    if let Some(base) = base_vars { for (k,v) in base.iter() { env.insert(k.clone(), v.clone()); } }
                    let keep = match eval_with_vars(&expr, &env)? { Value::Boolean(b) => b, _ => false };
                    if keep { out.push(it.clone()); }
                }
                Ok(Value::Array(out))
            }
            _ => Err(Error::new("filter expects array receiver", None))
        },
        "map" => match recv {
            Value::Array(items) => {
                let expr = args_expr.get(0).cloned().ok_or_else(|| Error::new("map expects an expression", None))?;
                let param_vals = eval_args(&args_expr[1..])?;
                let param_name = match param_vals.get(0) { Some(Value::String(s)) => s.clone(), _ => "x".to_string() };
                let mut out = Vec::new();
                for it in items {
                    let mut env = HashMap::new(); env.insert(param_name.clone(), it.clone());
                    if let Some(base) = base_vars { for (k,v) in base.iter() { env.insert(k.clone(), v.clone()); } }
                    out.push(eval_with_vars(&expr, &env)?);
                }
                Ok(Value::Array(out))
            }
            _ => Err(Error::new("map expects array receiver", None))
        },
        "reduce" => match recv {
            Value::Array(items) => {
                let expr = args_expr.get(0).cloned().ok_or_else(|| Error::new("reduce expects expression and initial", None))?;
                let a = eval_args(&args_expr[1..])?;
                let mut acc = a.get(0).cloned().ok_or_else(|| Error::new("reduce expects initial value", None))?;
                let val_param = match a.get(1) { Some(Value::String(s)) => s.clone(), _ => "x".to_string() };
                let acc_param = match a.get(2) { Some(Value::String(s)) => s.clone(), _ => "acc".to_string() };
                for it in items {
                    let mut env = HashMap::new(); env.insert(val_param.clone(), it.clone()); env.insert(acc_param.clone(), acc);
                    if let Some(base) = base_vars { for (k,v) in base.iter() { env.insert(k.clone(), v.clone()); } }
                    acc = eval_with_vars(&expr, &env)?;
                }
                Ok(acc)
            }
            _ => Err(Error::new("reduce expects array receiver", None))
        },
        "flatten" => match recv {
            Value::Array(items) => {
                fn flatten(v: &Value, out: &mut Vec<Value>) {
                    match v { Value::Array(inner) => for it in inner { flatten(it, out); }, other => out.push(other.clone()) }
                }
                let mut out = Vec::new(); for it in items { flatten(it, &mut out); }
                Ok(Value::Array(out))
            }
            _ => Err(Error::new("flatten expects array receiver", None))
        },
        // compact implemented with Null support
        "compact" => match recv {
            Value::Array(items) => Ok(Value::Array(items.iter().cloned().filter(|v| !matches!(v, Value::Null)).collect())),
            _ => Err(Error::new("compact expects array receiver", None))
        },

        _ => Err(Error::new(format!("Unknown method: .{}()", name), None)),
    }
}

fn exec_method_with_custom(name: &str, predicate: bool, recv: &Value, args_expr: &[Expr], base_vars: Option<&HashMap<String, Value>>, custom_registry: &Arc<RwLock<crate::custom::FunctionRegistry>>) -> Result<Value, Error> {
    let lname = name.to_lowercase();
    if predicate {
        return match lname.as_str() {
            "positive" => Ok(Value::Boolean(recv.as_number().map(|n| n > 0.0).unwrap_or(false))),
            "negative" => Ok(Value::Boolean(recv.as_number().map(|n| n < 0.0).unwrap_or(false))),
            "zero" => Ok(Value::Boolean(recv.as_number().map(|n| n == 0.0).unwrap_or(false))),
            "even" => Ok(Value::Boolean(recv.as_number().map(|n| (n as i64) % 2 == 0).unwrap_or(false))),
            "odd" => Ok(Value::Boolean(recv.as_number().map(|n| (n as i64) % 2 != 0).unwrap_or(false))),
            "numeric" => Ok(Value::Boolean(matches!(recv, Value::Number(_)))),
            "array" => Ok(Value::Boolean(matches!(recv, Value::Array(_)))),
            "nil" => Ok(Value::Boolean(matches!(recv, Value::Null))),
            "blank" => Ok(Value::Boolean(is_blank(recv))),
            "present" => Ok(Value::Boolean(!is_blank(recv))),
            _ => Err(Error::new(format!("Unknown predicate method: {}?", name), None)),
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
                        None => eval_with_vars_and_custom(inner, &HashMap::new(), custom_registry)? 
                    };
                    if let Value::Array(items) = v { out.extend(items); } else { return Err(Error::new("Spread expects array", None)); }
                }
                _ => {
                    let v = match base_vars { 
                        Some(env) => eval_with_vars_and_custom(e, env, custom_registry)?, 
                        None => eval_with_vars_and_custom(e, &HashMap::new(), custom_registry)? 
                    };
                    out.push(v);
                }
            }
        }
        Ok(out)
    };

    match lname.as_str() {
        // Numeric transformations on receiver
        "abs" => Ok(Value::Number(recv.as_number().ok_or_else(|| Error::new("abs expects number receiver", None))?.abs())),
        "round" => {
            let n = recv.as_number().ok_or_else(|| Error::new("round expects number receiver", None))?;
            let a = eval_args(args_expr)?;
            let decimals = match a.get(0) { Some(Value::Number(d)) => *d as i32, _ => 0 };
            let factor = 10f64.powi(decimals.max(0));
            Ok(Value::Number((n * factor).round() / factor))
        }
        "floor" => Ok(Value::Number(recv.as_number().ok_or_else(|| Error::new("floor expects number receiver", None))?.floor())),
        "ceil" => Ok(Value::Number(recv.as_number().ok_or_else(|| Error::new("ceil expects number receiver", None))?.ceil())),

        // String transforms
        "upper" => match recv { Value::String(s) => Ok(Value::String(s.to_uppercase())), _ => Err(Error::new("upper expects string receiver", None)) },
        "lower" => match recv { Value::String(s) => Ok(Value::String(s.to_lowercase())), _ => Err(Error::new("lower expects string receiver", None)) },
        "trim" => match recv { Value::String(s) => Ok(Value::String(s.trim().to_string())), _ => Err(Error::new("trim expects string receiver", None)) },
        "reverse" => match recv {
            Value::String(s) => Ok(Value::String(s.chars().rev().collect())),
            Value::Array(items) => { let mut v = items.clone(); v.reverse(); Ok(Value::Array(v)) },
            _ => Err(Error::new("reverse expects string or array receiver", None))
        },

        // Array accessors / transforms
        "length" | "size" => match recv {
            Value::Array(items) => Ok(Value::Number(items.len() as f64)),
            Value::String(s) => Ok(Value::Number(s.chars().count() as f64)),
            Value::Null => Ok(Value::Number(0.0)),
            _ => Err(Error::new("length expects array or string receiver", None))
        },
        "first" => match recv { Value::Array(items) => items.first().cloned().ok_or_else(|| Error::new("first on empty array", None)), _ => Err(Error::new("first expects array receiver", None)) },
        "last" => match recv { Value::Array(items) => items.last().cloned().ok_or_else(|| Error::new("last on empty array", None)), _ => Err(Error::new("last expects array receiver", None)) },
        "sort" => match recv {
            Value::Array(items) => {
                let mut nums: Vec<f64> = Vec::with_capacity(items.len());
                for it in items { match it { Value::Number(n) => nums.push(*n), _ => return Err(Error::new("sort expects numeric array", None)) } }
                nums.sort_by(|a,b| a.partial_cmp(b).unwrap());
                Ok(Value::Array(nums.into_iter().map(Value::Number).collect()))
            }
            _ => Err(Error::new("sort expects array receiver", None))
        },
        "unique" => match recv {
            Value::Array(items) => {
                use std::collections::BTreeSet;
                let mut set = BTreeSet::new();
                let mut out = Vec::new();
                for it in items {
                    match it { Value::Number(n) => { if set.insert((*n).to_bits()) { out.push(Value::Number(*n)); } }, _ => return Err(Error::new("unique expects numeric array", None)) }
                }
                Ok(Value::Array(out))
            }
            _ => Err(Error::new("unique expects array receiver", None))
        },
        "sum" => match recv {
            Value::Array(items) => {
                let mut acc = 0.0; for it in items { if let Value::Number(n) = it { acc += n; } else { return Err(Error::new("sum expects numeric array", None)); } }
                Ok(Value::Number(acc))
            }
            _ => Err(Error::new("sum expects array receiver", None))
        },
        "avg" => match recv {
            Value::Array(items) => {
                let mut acc = 0.0; let mut count = 0usize; for it in items { if let Value::Number(n) = it { acc += n; count += 1; } else { return Err(Error::new("avg expects numeric array", None)); } }
                Ok(Value::Number(if count==0 { 0.0 } else { acc / count as f64 }))
            }
            _ => Err(Error::new("avg expects array receiver", None))
        },
        "min" => match recv {
            Value::Array(items) => {
                let mut cur: Option<f64> = None; for it in items { if let Value::Number(n) = it { cur = Some(cur.map_or(*n, |c| c.min(*n))); } else { return Err(Error::new("min expects numeric array", None)); } }
                Ok(Value::Number(cur.unwrap_or(0.0)))
            }
            _ => Err(Error::new("min expects array receiver", None))
        },
        "max" => match recv {
            Value::Array(items) => {
                let mut cur: Option<f64> = None; for it in items { if let Value::Number(n) = it { cur = Some(cur.map_or(*n, |c| c.max(*n))); } else { return Err(Error::new("max expects numeric array", None)); } }
                Ok(Value::Number(cur.unwrap_or(0.0)))
            }
            _ => Err(Error::new("max expects array receiver", None))
        },
        "filter" => match recv {
            Value::Array(items) => {
                let expr = args_expr.get(0).cloned().ok_or_else(|| Error::new("filter expects an expression", None))?;
                // Optional param name as second arg
                let param_vals = eval_args(&args_expr[1..])?;
                let param_name = match param_vals.get(0) { Some(Value::String(s)) => s.clone(), _ => "x".to_string() };
                let mut out = Vec::new();
                for it in items {
                    let mut env = HashMap::new(); env.insert(param_name.clone(), it.clone());
                    if let Some(base) = base_vars { for (k,v) in base.iter() { env.insert(k.clone(), v.clone()); } }
                    let keep = match eval_with_vars_and_custom(&expr, &env, custom_registry)? { Value::Boolean(b) => b, _ => false };
                    if keep { out.push(it.clone()); }
                }
                Ok(Value::Array(out))
            }
            _ => Err(Error::new("filter expects array receiver", None))
        },
        "map" => match recv {
            Value::Array(items) => {
                let expr = args_expr.get(0).cloned().ok_or_else(|| Error::new("map expects an expression", None))?;
                let param_vals = eval_args(&args_expr[1..])?;
                let param_name = match param_vals.get(0) { Some(Value::String(s)) => s.clone(), _ => "x".to_string() };
                let mut out = Vec::new();
                for it in items {
                    let mut env = HashMap::new(); env.insert(param_name.clone(), it.clone());
                    if let Some(base) = base_vars { for (k,v) in base.iter() { env.insert(k.clone(), v.clone()); } }
                    out.push(eval_with_vars_and_custom(&expr, &env, custom_registry)?);
                }
                Ok(Value::Array(out))
            }
            _ => Err(Error::new("map expects array receiver", None))
        },
        "reduce" => match recv {
            Value::Array(items) => {
                let expr = args_expr.get(0).cloned().ok_or_else(|| Error::new("reduce expects expression and initial", None))?;
                let a = eval_args(&args_expr[1..])?;
                let mut acc = a.get(0).cloned().ok_or_else(|| Error::new("reduce expects initial value", None))?;
                let val_param = match a.get(1) { Some(Value::String(s)) => s.clone(), _ => "x".to_string() };
                let acc_param = match a.get(2) { Some(Value::String(s)) => s.clone(), _ => "acc".to_string() };
                for it in items {
                    let mut env = HashMap::new(); env.insert(val_param.clone(), it.clone()); env.insert(acc_param.clone(), acc);
                    if let Some(base) = base_vars { for (k,v) in base.iter() { env.insert(k.clone(), v.clone()); } }
                    acc = eval_with_vars_and_custom(&expr, &env, custom_registry)?;
                }
                Ok(acc)
            }
            _ => Err(Error::new("reduce expects array receiver", None))
        },
        "flatten" => match recv {
            Value::Array(items) => {
                fn flatten(v: &Value, out: &mut Vec<Value>) {
                    match v { Value::Array(inner) => for it in inner { flatten(it, out); }, other => out.push(other.clone()) }
                }
                let mut out = Vec::new(); for it in items { flatten(it, &mut out); }
                Ok(Value::Array(out))
            }
            _ => Err(Error::new("flatten expects array receiver", None))
        },
        // compact implemented with Null support
        "compact" => match recv {
            Value::Array(items) => Ok(Value::Array(items.iter().cloned().filter(|v| !matches!(v, Value::Null)).collect())),
            _ => Err(Error::new("compact expects array receiver", None))
        },

        _ => Err(Error::new(format!("Unknown method: .{}()", name), None)),
    }
}

fn is_blank(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.trim().is_empty(),
        Value::Array(items) => items.is_empty(),
        _ => false,
    }
}

fn clamp_index(len: usize, idx: isize) -> Option<usize> {
    if idx >= 0 {
        let i = idx as usize;
        if i < len { Some(i) } else { None }
    } else {
        let neg = (-idx) as usize; // idx is negative
        if neg <= len { Some(len - neg) } else { None }
    }
}

fn index_array(items: Vec<Value>, idx: isize) -> Result<Value, Error> {
    match clamp_index(items.len(), idx) {
        Some(i) => Ok(items[i].clone()),
        None => Err(Error::new("Index out of bounds", None)),
    }
}

fn slice_array(items: Vec<Value>, start: Option<Value>, end: Option<Value>) -> Result<Value, Error> {
    let len = items.len() as isize;
    let s = match start { Some(Value::Number(n)) => n as isize, None => 0, Some(_) => return Err(Error::new("Slice bounds must be numbers", None)) };
    let e = match end { Some(Value::Number(n)) => n as isize, None => len, Some(_) => return Err(Error::new("Slice bounds must be numbers", None)) };
    let s_norm = if s < 0 { len + s } else { s };
    let e_norm = if e < 0 { len + e } else { e };
    let s_idx = s_norm.max(0).min(len) as usize;
    let e_idx = e_norm.max(0).min(len) as usize;
    if s_idx > e_idx { return Ok(Value::Array(Vec::new())); }
    Ok(Value::Array(items[s_idx..e_idx].to_vec()))
}

fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x == y,
        (Value::Currency(x), Value::Currency(y)) => x == y,
        (Value::Boolean(x), Value::Boolean(y)) => x == y,
        (Value::String(x), Value::String(y)) => x == y,
        (Value::DateTime(x), Value::DateTime(y)) => x == y,
        (Value::Json(x), Value::Json(y)) => x == y,
        (Value::Null, Value::Null) => true,
        // Arrays: shallow equality by elements
        (Value::Array(ax), Value::Array(ay)) => ax.len() == ay.len() && ax.iter().zip(ay.iter()).all(|(u,v)| values_equal(u,v)),
        _ => false,
    }
}

fn cast_value(v: Value, ty: &TypeName) -> Result<Value, Error> {
    Ok(match ty {
        TypeName::Float => match v { Value::Number(n) => Value::Number(n), Value::Currency(n) => Value::Number(n), Value::String(s) => Value::Number(s.parse::<f64>().map_err(|_| Error::new("Cannot cast String to Float", None))?), Value::Boolean(b) => Value::Number(if b {1.0} else {0.0}), Value::Null => Value::Number(0.0), _ => return Err(Error::new("Cannot cast to Float", None)) },
        TypeName::Integer => match v { Value::Number(n) => Value::Number((n as i64) as f64), Value::Currency(n) => Value::Number((n as i64) as f64), Value::String(s) => Value::Number(s.parse::<f64>().map_err(|_| Error::new("Cannot cast String to Integer", None))?.trunc()), Value::Boolean(b) => Value::Number(if b {1.0} else {0.0}), Value::Null => Value::Number(0.0), _ => return Err(Error::new("Cannot cast to Integer", None)) },
        TypeName::String => match v { Value::String(s) => Value::String(s), Value::Number(n) => Value::String(n.to_string()), Value::Boolean(b) => Value::String(if b {"TRUE".into()} else {"FALSE".into()}), Value::Null => Value::String(String::new()), Value::Array(items) => Value::String(format!("{:?}", items)), Value::Currency(n) => Value::String(format!("{:.4}", n)), Value::DateTime(ts) => Value::String(ts.to_string()), Value::Json(s) => Value::String(s) },
        TypeName::Boolean => match v { Value::Boolean(b) => Value::Boolean(b), Value::Number(n) => Value::Boolean(n != 0.0), Value::Currency(n) => Value::Boolean(n != 0.0), Value::String(s) => Value::Boolean(!s.trim().is_empty()), Value::Array(items) => Value::Boolean(!items.is_empty()), Value::Null => Value::Boolean(false) , Value::DateTime(ts) => Value::Boolean(ts != 0), Value::Json(s) => Value::Boolean(!s.trim().is_empty())},
        TypeName::Array => match v { Value::Array(items) => Value::Array(items), other => Value::Array(vec![other]) },
        TypeName::Currency => match v { Value::Currency(n) => Value::Currency(n), Value::Number(n) => Value::Currency(n), Value::String(s) => Value::Currency(s.parse::<f64>().map_err(|_| Error::new("Cannot cast String to Currency", None))?), Value::Boolean(b) => Value::Currency(if b {1.0} else {0.0}), Value::Null => Value::Currency(0.0), _ => return Err(Error::new("Cannot cast to Currency", None)) },
        TypeName::DateTime => match v { Value::DateTime(ts) => Value::DateTime(ts), Value::Number(n) => Value::DateTime(n as i64), Value::String(s) => Value::DateTime(s.parse::<i64>().map_err(|_| Error::new("Cannot cast String to DateTime", None))?), _ => return Err(Error::new("Cannot cast to DateTime", None)) },
        TypeName::Json => match v { Value::Json(s) => Value::Json(s), Value::String(s) => Value::Json(s), Value::Array(items) => Value::Json(format!("{:?}", items)), _ => return Err(Error::new("Cannot cast to Json", None)) },
    })
}
