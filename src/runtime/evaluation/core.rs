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
use super::higher_order;

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Convert a Skillet Value to a serde_json::Value
fn value_to_json(value: &Value) -> Result<serde_json::Value, Error> {
    match value {
        Value::Number(n) => Ok(serde_json::json!(n)),
        Value::String(s) => Ok(serde_json::json!(s)),
        Value::Boolean(b) => Ok(serde_json::json!(b)),
        Value::Currency(c) => Ok(serde_json::json!(c)),
        Value::DateTime(dt) => Ok(serde_json::json!(dt)),
        Value::Null => Ok(serde_json::json!(null)),
        Value::Array(arr) => {
            let mut json_arr = Vec::new();
            for item in arr {
                json_arr.push(value_to_json(item)?);
            }
            Ok(serde_json::Value::Array(json_arr))
        }
        Value::Json(s) => {
            // Already JSON, parse and re-serialize to validate
            serde_json::from_str(s)
                .map_err(|e| Error::new(format!("Invalid JSON: {}", e), None))
        }
    }
}

pub fn eval(expr: &Expr) -> Result<Value, Error> {
    match expr {
        Expr::Number(n) => Ok(Value::Number(*n)),
        Expr::StringLit(s) => Ok(Value::String(s.clone())),
        Expr::Null => Ok(Value::Null),
        
        Expr::Unary(op, e) => {
            let v = eval(e)?;
            match op {
                UnaryOp::Plus => Ok(Value::Number(v.as_number().ok_or_else(|| Error::new("Unary '+' on non-number", None))?)),
                UnaryOp::Minus => Ok(Value::Number(-v.as_number().ok_or_else(|| Error::new("Unary '-' on non-number", None))?)),
                UnaryOp::Not => Ok(Value::Boolean(!v.as_bool().ok_or_else(|| Error::new("Unary '!' on non-boolean", None))?)),
            }
        }
        
        Expr::Binary(l, op, r) => eval_binary_op(l, op, r, None),
        
        Expr::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for e in items { 
                out.push(eval(e)?); 
            }
            Ok(Value::Array(out))
        }
        
        Expr::ObjectLiteral(pairs) => eval_object_literal(pairs, None),
        
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
                Value::Array(items) => slice_array(items, 
                    start.as_ref().map(|e| eval(e)).transpose()?, 
                    end.as_ref().map(|e| eval(e)).transpose()?
                ),
                _ => Err(Error::new("Slicing only supported on arrays", None)),
            }
        }
        
        Expr::FunctionCall { name, args } => eval_function_call(name, args, None),
        
        Expr::MethodCall { target, name, args, predicate } => {
            let recv = eval(target)?;
            exec_method(name, *predicate, &recv, args, None)
        }
        
        // These require variables context
        Expr::Variable(_) => Err(Error::new("Use eval_with_vars for variables", None)),
        Expr::PropertyAccess { .. } => Err(Error::new("Use eval_with_vars for property access", None)),
        Expr::SafePropertyAccess { .. } => Err(Error::new("Use eval_with_vars for safe property access", None)),
        Expr::SafeMethodCall { .. } => Err(Error::new("Use eval_with_vars for safe method calls", None)),
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
            match op {
                UnaryOp::Plus => Ok(Value::Number(v.as_number().ok_or_else(|| Error::new("Unary '+' on non-number", None))?)),
                UnaryOp::Minus => Ok(Value::Number(-v.as_number().ok_or_else(|| Error::new("Unary '-' on non-number", None))?)),
                UnaryOp::Not => Ok(Value::Boolean(!v.as_bool().ok_or_else(|| Error::new("Unary '!' on non-boolean", None))?)),
            }
        }
        
        Expr::Binary(l, op, r) => eval_binary_op(l, op, r, Some(vars)),
        
        Expr::Variable(name) => vars
            .get(name)
            .cloned()
            .ok_or_else(|| Error::new(format!("Missing variable: :{}", name), None)),
        
        Expr::PropertyAccess { target, property } => eval_property_access(target, property, vars, false),
        Expr::SafePropertyAccess { target, property } => eval_property_access(target, property, vars, true),
        
        Expr::SafeMethodCall { target, name, args } => {
            let target_value = eval_with_vars(target, vars)?;
            if matches!(target_value, Value::Null) {
                return Ok(Value::Null);
            }
            exec_method(name, false, &target_value, args, Some(vars))
        }
        
        Expr::Array(items) => {
            let mut out = Vec::with_capacity(items.len());
            for e in items { 
                out.push(eval_with_vars(e, vars)?); 
            }
            Ok(Value::Array(out))
        }
        
        Expr::ObjectLiteral(pairs) => eval_object_literal(pairs, Some(vars)),
        
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
                Value::Array(items) => slice_array(items, 
                    start.as_ref().map(|e| eval_with_vars(e, vars)).transpose()?, 
                    end.as_ref().map(|e| eval_with_vars(e, vars)).transpose()?
                ),
                _ => Err(Error::new("Slice on non-array", None)),
            }
        }
        
        Expr::FunctionCall { name, args } => eval_function_call(name, args, Some(vars)),
        
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

pub fn eval_with_vars_and_custom(
    expr: &Expr,
    vars: &HashMap<String, Value>,
    custom_registry: &Arc<RwLock<FunctionRegistry>>
) -> Result<Value, Error> {
    match expr {
        Expr::Number(n) => Ok(Value::Number(*n)),
        Expr::StringLit(s) => Ok(Value::String(s.clone())),
        Expr::Null => Ok(Value::Null),
        
        Expr::Unary(op, e) => {
            let v = eval_with_vars_and_custom(e, vars, custom_registry)?;
            match op {
                UnaryOp::Plus => Ok(Value::Number(v.as_number().ok_or_else(|| Error::new("Unary '+' on non-number", None))?)),
                UnaryOp::Minus => Ok(Value::Number(-v.as_number().ok_or_else(|| Error::new("Unary '-' on non-number", None))?)),
                UnaryOp::Not => Ok(Value::Boolean(!v.as_bool().ok_or_else(|| Error::new("Unary '!' on non-boolean", None))?)),
            }
        }
        
        Expr::Binary(l, op, r) => eval_binary_op_with_custom(l, op, r, vars, custom_registry),
        
        Expr::Variable(name) => {
            vars.get(name).cloned().ok_or_else(|| Error::new(format!("Undefined variable: {}", name), None))
        }
        
        Expr::PropertyAccess { target, property } => eval_property_access_with_custom(target, property, vars, custom_registry, false),
        Expr::SafePropertyAccess { target, property } => eval_property_access_with_custom(target, property, vars, custom_registry, true),
        
        Expr::SafeMethodCall { target, name, args } => {
            let target_value = eval_with_vars_and_custom(target, vars, custom_registry)?;
            if matches!(target_value, Value::Null) {
                return Ok(Value::Null);
            }
            exec_method_with_custom(name, false, &target_value, args, Some(vars), custom_registry)
        }
        
        Expr::Array(exprs) => {
            let mut items = Vec::new();
            for e in exprs {
                items.push(eval_with_vars_and_custom(e, vars, custom_registry)?);
            }
            Ok(Value::Array(items))
        }
        
        Expr::ObjectLiteral(pairs) => eval_object_literal_with_custom(pairs, vars, custom_registry),
        
        Expr::Index { target, index } => eval_index_with_custom(target, index, vars, custom_registry),
        Expr::Slice { target, start, end } => eval_slice_with_custom(target, start, end, vars, custom_registry),
        
        Expr::TypeCast { expr, ty } => {
            let v = eval_with_vars_and_custom(expr, vars, custom_registry)?;
            cast_value(v, ty)
        }
        
        Expr::FunctionCall { name, args } => eval_function_call_with_custom(name, args, vars, custom_registry),
        
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

// Helper functions for binary operations
fn eval_binary_op(l: &Expr, op: &BinaryOp, r: &Expr, vars: Option<&HashMap<String, Value>>) -> Result<Value, Error> {
    let (a, b) = match vars {
        Some(v) => (eval_with_vars(l, v)?, eval_with_vars(r, v)?),
        None => (eval(l)?, eval(r)?)
    };
    
    match op {
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod | BinaryOp::Pow => {
            let an = a.as_number().ok_or_else(|| Error::new("Arithmetic op on non-number", None))?;
            let bn = b.as_number().ok_or_else(|| Error::new("Arithmetic op on non-number", None))?;
            match op {
                BinaryOp::Add => Ok(Value::Number(an + bn)),
                BinaryOp::Sub => Ok(Value::Number(an - bn)),
                BinaryOp::Mul => Ok(Value::Number(an * bn)),
                BinaryOp::Div => Ok(Value::Number(an / bn)),
                BinaryOp::Mod => Ok(Value::Number(an % bn)),
                BinaryOp::Pow => Ok(Value::Number(an.powf(bn))),
                _ => unreachable!(),
            }
        }
        BinaryOp::Gt | BinaryOp::Lt | BinaryOp::Ge | BinaryOp::Le | BinaryOp::Eq | BinaryOp::Ne => {
            if vars.is_some() {
                // Enhanced comparison for eval_with_vars
                match (a, b) {
                    (Value::Number(x), Value::Number(y)) => Ok(Value::Boolean(match op {
                        BinaryOp::Eq => x == y,
                        BinaryOp::Ne => x != y,
                        BinaryOp::Lt => x < y,
                        BinaryOp::Le => x <= y,
                        BinaryOp::Gt => x > y,
                        BinaryOp::Ge => x >= y,
                        _ => unreachable!()
                    })),
                    (Value::String(x), Value::String(y)) => Ok(Value::Boolean(match op {
                        BinaryOp::Eq => x == y,
                        BinaryOp::Ne => x != y,
                        BinaryOp::Lt => x < y,
                        BinaryOp::Le => x <= y,
                        BinaryOp::Gt => x > y,
                        BinaryOp::Ge => x >= y,
                        _ => unreachable!()
                    })),
                    (Value::Boolean(x), Value::Boolean(y)) => Ok(Value::Boolean(match op {
                        BinaryOp::Eq => x == y,
                        BinaryOp::Ne => x != y,
                        _ => false
                    })),
                    _ => match op {
                        BinaryOp::Eq => Ok(Value::Boolean(false)),
                        BinaryOp::Ne => Ok(Value::Boolean(true)),
                        _ => Err(Error::new("Comparison of incompatible types", None))
                    }
                }
            } else {
                // Simple numeric comparison for eval
                let an = a.as_number().ok_or_else(|| Error::new("Comparison on non-number", None))?;
                let bn = b.as_number().ok_or_else(|| Error::new("Comparison on non-number", None))?;
                Ok(Value::Boolean(match op {
                    BinaryOp::Gt => an > bn,
                    BinaryOp::Lt => an < bn,
                    BinaryOp::Ge => an >= bn,
                    BinaryOp::Le => an <= bn,
                    BinaryOp::Eq => an == bn,
                    BinaryOp::Ne => an != bn,
                    _ => unreachable!(),
                }))
            }
        }
        BinaryOp::And | BinaryOp::Or => {
            let ab = a.as_bool().ok_or_else(|| Error::new("Logical op on non-boolean", None))?;
            let bb = b.as_bool().ok_or_else(|| Error::new("Logical op on non-boolean", None))?;
            Ok(Value::Boolean(match op { 
                BinaryOp::And => ab && bb, 
                BinaryOp::Or => ab || bb, 
                _ => unreachable!() 
            }))
        }
    }
}

fn eval_binary_op_with_custom(
    l: &Expr, 
    op: &BinaryOp, 
    r: &Expr, 
    vars: &HashMap<String, Value>, 
    custom_registry: &Arc<RwLock<FunctionRegistry>>
) -> Result<Value, Error> {
    let a = eval_with_vars_and_custom(l, vars, custom_registry)?;
    let b = eval_with_vars_and_custom(r, vars, custom_registry)?;
    
    match op {
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod | BinaryOp::Pow => {
            let an = a.as_number().ok_or_else(|| Error::new("Arithmetic op on non-number", None))?;
            let bn = b.as_number().ok_or_else(|| Error::new("Arithmetic op on non-number", None))?;
            match op {
                BinaryOp::Add => Ok(Value::Number(an + bn)),
                BinaryOp::Sub => Ok(Value::Number(an - bn)),
                BinaryOp::Mul => Ok(Value::Number(an * bn)),
                BinaryOp::Div => Ok(Value::Number(an / bn)),
                BinaryOp::Mod => Ok(Value::Number(an % bn)),
                BinaryOp::Pow => Ok(Value::Number(an.powf(bn))),
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
            Ok(Value::Boolean(result))
        }
        BinaryOp::And | BinaryOp::Or => {
            let ab = a.as_bool().ok_or_else(|| Error::new("Logical op on non-boolean", None))?;
            let bb = b.as_bool().ok_or_else(|| Error::new("Logical op on non-boolean", None))?;
            Ok(Value::Boolean(match op { 
                BinaryOp::And => ab && bb, 
                BinaryOp::Or => ab || bb, 
                _ => unreachable!() 
            }))
        }
    }
}

// Helper functions for property access
fn eval_property_access(target: &Expr, property: &str, vars: &HashMap<String, Value>, safe: bool) -> Result<Value, Error> {
    let target_value = eval_with_vars(target, vars)?;
    match target_value {
        Value::Json(json_str) => {
            let parsed: serde_json::Value = serde_json::from_str(&json_str)
                .map_err(|e| Error::new(format!("Invalid JSON: {}", e), None))?;
            if let Some(prop_value) = parsed.get(property) {
                crate::json_to_value(prop_value.clone())
            } else if safe {
                Ok(Value::Null) // Safe navigation returns null instead of error
            } else {
                Err(Error::new(format!("Property '{}' not found in JSON object", property), None))
            }
        }
        Value::Null if safe => Ok(Value::Null), // Safe navigation on null returns null
        _ if safe => Err(Error::new("Property access requires JSON object", None)),
        _ => Err(Error::new("Property access requires JSON object", None))
    }
}

fn eval_property_access_with_custom(
    target: &Expr, 
    property: &str, 
    vars: &HashMap<String, Value>, 
    custom_registry: &Arc<RwLock<FunctionRegistry>>, 
    safe: bool
) -> Result<Value, Error> {
    let target_value = eval_with_vars_and_custom(target, vars, custom_registry)?;
    match target_value {
        Value::Json(json_str) => {
            let parsed: serde_json::Value = serde_json::from_str(&json_str)
                .map_err(|e| Error::new(format!("Invalid JSON: {}", e), None))?;
            if let Some(prop_value) = parsed.get(property) {
                crate::json_to_value(prop_value.clone())
            } else if safe {
                Ok(Value::Null) // Safe navigation returns null instead of error
            } else {
                Err(Error::new(format!("Property '{}' not found in JSON object", property), None))
            }
        }
        Value::Null if safe => Ok(Value::Null), // Safe navigation on null returns null
        _ => Err(Error::new(format!("Property access only supported on JSON objects, got {:?}", target_value), None)),
    }
}

// Helper functions for object literals
fn eval_object_literal(pairs: &[(String, Expr)], vars: Option<&HashMap<String, Value>>) -> Result<Value, Error> {
    let mut json_map = serde_json::Map::new();
    for (key, value_expr) in pairs {
        let value = match vars {
            Some(v) => eval_with_vars(value_expr, v)?,
            None => eval(value_expr)?
        };
        let json_value = value_to_json(&value)?;
        json_map.insert(key.clone(), json_value);
    }
    let json_obj = serde_json::Value::Object(json_map);
    let json_str = serde_json::to_string(&json_obj)
        .map_err(|e| Error::new(format!("Failed to serialize object: {}", e), None))?;
    Ok(Value::Json(json_str))
}

fn eval_object_literal_with_custom(
    pairs: &[(String, Expr)], 
    vars: &HashMap<String, Value>, 
    custom_registry: &Arc<RwLock<FunctionRegistry>>
) -> Result<Value, Error> {
    let mut json_map = serde_json::Map::new();
    for (key, value_expr) in pairs {
        let value = eval_with_vars_and_custom(value_expr, vars, custom_registry)?;
        let json_value = value_to_json(&value)?;
        json_map.insert(key.clone(), json_value);
    }
    let json_obj = serde_json::Value::Object(json_map);
    let json_str = serde_json::to_string(&json_obj)
        .map_err(|e| Error::new(format!("Failed to serialize object: {}", e), None))?;
    Ok(Value::Json(json_str))
}

// Helper functions for indexing and slicing with custom
fn eval_index_with_custom(
    target: &Expr, 
    index: &Expr, 
    vars: &HashMap<String, Value>, 
    custom_registry: &Arc<RwLock<FunctionRegistry>>
) -> Result<Value, Error> {
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

fn eval_slice_with_custom(
    target: &Expr, 
    start: &Option<std::rc::Rc<Expr>>, 
    end: &Option<std::rc::Rc<Expr>>, 
    vars: &HashMap<String, Value>, 
    custom_registry: &Arc<RwLock<FunctionRegistry>>
) -> Result<Value, Error> {
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

// Function call evaluation
fn eval_function_call(name: &str, args: &[Expr], vars: Option<&HashMap<String, Value>>) -> Result<Value, Error> {
    match name {
        "__TERNARY__" => {
            if args.len() != 3 { 
                return Err(Error::new("Ternary expects 3 args", None)); 
            }
            let cond = match vars {
                Some(v) => eval_with_vars(&args[0], v)?,
                None => eval(&args[0])?
            }.as_bool().ok_or_else(|| Error::new("Ternary condition must be boolean", None))?;
            
            if cond { 
                match vars {
                    Some(v) => eval_with_vars(&args[1], v),
                    None => eval(&args[1])
                }
            } else { 
                match vars {
                    Some(v) => eval_with_vars(&args[2], v),
                    None => eval(&args[2])
                }
            }
        }
        
        // Higher-order functions
        "FILTER" | "FIND" | "MAP" | "REDUCE" | "SUMIF" | "AVGIF" | "COUNTIF" => {
            match vars {
                Some(v) => higher_order::eval_higher_order_function(name, args, v),
                None => Err(Error::new(format!("{} requires variable context", name), None))
            }
        }
        
        _ => {
            // Regular built-in functions
            let mut ev_args = Vec::new();
            for a in args {
                match a {
                    Expr::Spread(inner) => {
                        let v = match vars {
                            Some(vars) => eval_with_vars(inner, vars)?,
                            None => eval(inner)?
                        };
                        if let Value::Array(items) = v { 
                            ev_args.extend(items); 
                        } else { 
                            return Err(Error::new("Spread expects array", None)); 
                        }
                    }
                    _ => {
                        let val = match vars {
                            Some(vars) => eval_with_vars(a, vars)?,
                            None => eval(a)?
                        };
                        ev_args.push(val);
                    }
                }
            }
            exec_builtin(name, &ev_args)
        }
    }
}

fn eval_function_call_with_custom(
    name: &str, 
    args: &[Expr], 
    vars: &HashMap<String, Value>, 
    custom_registry: &Arc<RwLock<FunctionRegistry>>
) -> Result<Value, Error> {
    match name {
        "__TERNARY__" => {
            if args.len() != 3 { 
                return Err(Error::new("Ternary expects 3 args", None)); 
            }
            let cond = eval_with_vars_and_custom(&args[0], vars, custom_registry)?
                .as_bool().ok_or_else(|| Error::new("Ternary condition must be boolean", None))?;
            
            if cond { 
                eval_with_vars_and_custom(&args[1], vars, custom_registry) 
            } else { 
                eval_with_vars_and_custom(&args[2], vars, custom_registry) 
            }
        }
        
        _ => {
            // Check custom functions first
            if let Ok(registry) = custom_registry.read() {
                if registry.has_function(name) {
                    let mut ev_args = Vec::new();
                    for a in args {
                        match a {
                            Expr::Spread(inner) => {
                                let v = eval_with_vars_and_custom(inner, vars, custom_registry)?;
                                if let Value::Array(items) = v { 
                                    ev_args.extend(items); 
                                } else { 
                                    return Err(Error::new("Spread expects array", None)); 
                                }
                            }
                            _ => ev_args.push(eval_with_vars_and_custom(a, vars, custom_registry)?),
                        }
                    }
                    return registry.execute(name, ev_args);
                }
            }
            
            // Higher-order functions with custom support
            match name {
                "FILTER" | "FIND" | "MAP" | "REDUCE" | "SUMIF" | "AVGIF" | "COUNTIF" => {
                    higher_order::eval_higher_order_function_with_custom(name, args, vars, custom_registry)
                }
                _ => {
                    // Fall back to built-in functions
                    let mut ev_args = Vec::new();
                    for a in args {
                        match a {
                            Expr::Spread(inner) => {
                                let v = eval_with_vars_and_custom(inner, vars, custom_registry)?;
                                if let Value::Array(items) = v { 
                                    ev_args.extend(items); 
                                } else { 
                                    return Err(Error::new("Spread expects array", None)); 
                                }
                            }
                            _ => ev_args.push(eval_with_vars_and_custom(a, vars, custom_registry)?),
                        }
                    }
                    exec_builtin(name, &ev_args)
                }
            }
        }
    }
}