use crate::ast::{BinaryOp, Expr, UnaryOp};
use crate::error::Error;
use crate::types::Value;
use crate::custom::FunctionRegistry;
use crate::runtime::{
    function_dispatch::exec_builtin_fast,
    method_calls::{exec_method, exec_method_with_custom},
    type_casting::cast_value,
    utils::{index_array, slice_array}
};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::borrow::Cow;

/// Evaluation context that provides access to variables and custom functions
pub trait EvaluationContext {
    fn get_variable(&self, name: &str) -> Option<&Value>;
    fn get_custom_registry(&self) -> Option<&Arc<RwLock<FunctionRegistry>>>;
    fn clone_variables(&self) -> HashMap<String, Value>;
}

/// Empty context for basic evaluation without variables
pub struct EmptyContext;

impl EvaluationContext for EmptyContext {
    fn get_variable(&self, _name: &str) -> Option<&Value> {
        None
    }
    
    fn get_custom_registry(&self) -> Option<&Arc<RwLock<FunctionRegistry>>> {
        None
    }
    
    fn clone_variables(&self) -> HashMap<String, Value> {
        HashMap::new()
    }
}

/// Variable context with optional custom function support
pub struct VariableContext<'a> {
    variables: Cow<'a, HashMap<String, Value>>,
    custom_registry: Option<&'a Arc<RwLock<FunctionRegistry>>>,
}

impl<'a> VariableContext<'a> {
    pub fn new(vars: &'a HashMap<String, Value>) -> Self {
        Self {
            variables: Cow::Borrowed(vars),
            custom_registry: None,
        }
    }
    
    pub fn with_custom(vars: &'a HashMap<String, Value>, registry: &'a Arc<RwLock<FunctionRegistry>>) -> Self {
        Self {
            variables: Cow::Borrowed(vars),
            custom_registry: Some(registry),
        }
    }
    
    pub fn with_owned(vars: HashMap<String, Value>) -> Self {
        Self {
            variables: Cow::Owned(vars),
            custom_registry: None,
        }
    }
    
    /// Make variables mutable, cloning if necessary
    pub fn make_mut(&mut self) -> &mut HashMap<String, Value> {
        self.variables.to_mut()
    }

    /// Consume the context and return the owned variables HashMap
    pub fn into_variables(self) -> HashMap<String, Value> {
        self.variables.into_owned()
    }
}

impl<'a> EvaluationContext for VariableContext<'a> {
    fn get_variable(&self, name: &str) -> Option<&Value> {
        self.variables.get(name)
    }
    
    fn get_custom_registry(&self) -> Option<&Arc<RwLock<FunctionRegistry>>> {
        self.custom_registry
    }
    
    fn clone_variables(&self) -> HashMap<String, Value> {
        self.variables.as_ref().clone()
    }
}

/// Unified evaluator that handles all expression types efficiently
pub struct Evaluator;

impl Evaluator {
    /// Evaluate expression with any context type
    pub fn eval<C: EvaluationContext>(expr: &Expr, context: &C) -> Result<Value, Error> {
        match expr {
            Expr::Number(n) => Ok(Value::Number(*n)),
            Expr::StringLit(s) => Ok(Value::String(s.clone())),
            Expr::Null => Ok(Value::Null),
            
            Expr::Unary(op, e) => {
                let v = Self::eval(e, context)?;
                Self::eval_unary_op(*op, v)
            }
            
            Expr::Binary(l, op, r) => {
                let a = Self::eval(l, context)?;
                let b = Self::eval(r, context)?;
                Self::eval_binary_op(*op, a, b)
            }
            
            Expr::Variable(name) => {
                context.get_variable(name)
                    .cloned()
                    .ok_or_else(|| Error::new(format!("Missing variable: :{}", name), None))
            }
            
            Expr::PropertyAccess { target, property } => {
                let target_value = Self::eval(target, context)?;
                Self::eval_property_access(target_value, property, false)
            }
            
            Expr::SafePropertyAccess { target, property } => {
                let target_value = Self::eval(target, context)?;
                Self::eval_property_access(target_value, property, true)
            }
            
            Expr::SafeMethodCall { target, name, args } => {
                let target_value = Self::eval(target, context)?;
                if matches!(target_value, Value::Null) {
                    return Ok(Value::Null);
                }
                if let Some(registry) = context.get_custom_registry() {
                    exec_method_with_custom(name, false, &target_value, args, Some(&context.clone_variables()), registry)
                } else {
                    exec_method(name, false, &target_value, args, Some(&context.clone_variables()))
                }
            }
            
            Expr::Array(items) => {
                let mut out = Vec::with_capacity(items.len());
                for e in items { 
                    out.push(Self::eval(e, context)?); 
                }
                Ok(Value::Array(out))
            }
            
            Expr::ObjectLiteral(pairs) => {
                let mut json_map = serde_json::Map::new();
                for (key, value_expr) in pairs {
                    let value = Self::eval(value_expr, context)?;
                    let json_value = Self::value_to_json(&value)?;
                    json_map.insert(key.clone(), json_value);
                }
                let json_obj = serde_json::Value::Object(json_map);
                let json_str = serde_json::to_string(&json_obj)
                    .map_err(|e| Error::new(format!("Failed to serialize object: {}", e), None))?;
                Ok(Value::Json(json_str))
            }
            
            Expr::TypeCast { expr, ty } => {
                let v = Self::eval(expr, context)?;
                cast_value(v, ty)
            }
            
            Expr::Index { target, index } => {
                let recv = Self::eval(target, context)?;
                let idx_v = Self::eval(index, context)?;
                let idx = idx_v.as_number().ok_or_else(|| Error::new("Index must be number", None))? as isize;
                match recv {
                    Value::Array(items) => index_array(items, idx),
                    _ => Err(Error::new("Index on non-array", None)),
                }
            }
            
            Expr::Slice { target, start, end } => {
                let recv = Self::eval(target, context)?;
                match recv {
                    Value::Array(items) => {
                        let start_val = start.as_ref().map(|e| Self::eval(e, context)).transpose()?;
                        let end_val = end.as_ref().map(|e| Self::eval(e, context)).transpose()?;
                        slice_array(items, start_val, end_val)
                    },
                    _ => Err(Error::new("Slice on non-array", None)),
                }
            }
            
            Expr::FunctionCall { name, args } => {
                Self::eval_function_call(name, args, context)
            }
            
            Expr::MethodCall { target, name, args, predicate } => {
                let recv = Self::eval(target, context)?;
                if let Some(registry) = context.get_custom_registry() {
                    exec_method_with_custom(name, *predicate, &recv, args, Some(&context.clone_variables()), registry)
                } else {
                    exec_method(name, *predicate, &recv, args, Some(&context.clone_variables()))
                }
            }
            
            Expr::Spread(_) => Err(Error::new("Spread not allowed here", None)),
            
            Expr::Assignment { variable: _, value } => {
                // For now, return the value - assignments need mutable context
                Self::eval(value, context)
            }
            
            Expr::Sequence(exprs) => {
                let mut last_result = Value::Null;
                for expr in exprs {
                    last_result = Self::eval(expr, context)?;
                }
                Ok(last_result)
            }
        }
    }
    
    /// Evaluate unary operations
    fn eval_unary_op(op: UnaryOp, v: Value) -> Result<Value, Error> {
        match op {
            UnaryOp::Plus => Ok(Value::Number(v.as_number().ok_or_else(|| Error::new("Unary '+' on non-number", None))?)),
            UnaryOp::Minus => Ok(Value::Number(-v.as_number().ok_or_else(|| Error::new("Unary '-' on non-number", None))?)),
            UnaryOp::Not => Ok(Value::Boolean(!v.as_bool().ok_or_else(|| Error::new("Unary '!' on non-boolean", None))?)),
        }
    }
    
    /// Evaluate binary operations
    fn eval_binary_op(op: BinaryOp, a: Value, b: Value) -> Result<Value, Error> {
        match op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div | BinaryOp::Mod | BinaryOp::Pow => {
                let an = a.as_number().ok_or_else(|| Error::new("Arithmetic op on non-number", None))?;
                let bn = b.as_number().ok_or_else(|| Error::new("Arithmetic op on non-number", None))?;
                Ok(Value::Number(match op {
                    BinaryOp::Add => an + bn,
                    BinaryOp::Sub => an - bn,
                    BinaryOp::Mul => an * bn,
                    BinaryOp::Div => an / bn,
                    BinaryOp::Mod => an % bn,
                    BinaryOp::Pow => an.powf(bn),
                    _ => unreachable!(),
                }))
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
    
    /// Evaluate property access (both safe and unsafe)
    fn eval_property_access(target_value: Value, property: &str, safe: bool) -> Result<Value, Error> {
        match target_value {
            Value::Json(json_str) => {
                let parsed: serde_json::Value = serde_json::from_str(&json_str)
                    .map_err(|e| Error::new(format!("Invalid JSON: {}", e), None))?;
                if let Some(prop_value) = parsed.get(property) {
                    crate::json_to_value(prop_value.clone())
                } else if safe {
                    Ok(Value::Null)
                } else {
                    Err(Error::new(format!("Property '{}' not found in JSON object", property), None))
                }
            }
            Value::Null if safe => Ok(Value::Null),
            _ => Err(Error::new("Property access requires JSON object", None))
        }
    }
    
    /// Evaluate function calls with optimized dispatch
    fn eval_function_call<C: EvaluationContext>(name: &str, args: &[Expr], context: &C) -> Result<Value, Error> {
        // Handle special functions first
        match name {
            "__TERNARY__" => {
                if args.len() != 3 { 
                    return Err(Error::new("Ternary expects 3 args", None)); 
                }
                let cond = Self::eval(&args[0], context)?.as_bool()
                    .ok_or_else(|| Error::new("Ternary condition must be boolean", None))?;
                return if cond { 
                    Self::eval(&args[1], context) 
                } else { 
                    Self::eval(&args[2], context) 
                };
            }
            "__CONST_TRUE__" => return Ok(Value::Boolean(true)),
            "__CONST_FALSE__" => return Ok(Value::Boolean(false)),
            _ => {}
        }
        
        // Check custom functions first
        if let Some(registry) = context.get_custom_registry() {
            if let Ok(reg) = registry.read() {
                if reg.has_function(name) {
                    let mut ev_args = Vec::new();
                    for a in args {
                        match a {
                            Expr::Spread(inner) => {
                                let v = Self::eval(inner, context)?;
                                if let Value::Array(items) = v { 
                                    ev_args.extend(items); 
                                } else { 
                                    return Err(Error::new("Spread expects array", None)); 
                                }
                            }
                            _ => {
                                let val = Self::eval(a, context)?;
                                ev_args.push(val);
                            }
                        }
                    }
                    return reg.execute(name, ev_args);
                }
            }
        }
        
        // Handle higher-order functions
        match name {
            "FILTER" => Self::eval_filter(args, context),
            "FIND" => Self::eval_find(args, context),
            "MAP" => Self::eval_map(args, context),
            "REDUCE" => Self::eval_reduce(args, context),
            "SUMIF" => Self::eval_sumif(args, context),
            "AVGIF" => Self::eval_avgif(args, context),
            "COUNTIF" => Self::eval_countif(args, context),
            "JQ" => {
                if args.len() != 2 {
                    return Err(Error::new("JQ expects exactly 2 arguments: json_data, jsonpath_expression", None));
                }

                let json_data = Self::eval(&args[0], context)?;
                let path_expr = Self::eval(&args[1], context)?;

                let path = match path_expr {
                    Value::String(s) => s,
                    _ => return Err(Error::new("JQ second argument must be a string", None)),
                };

                if !crate::runtime::jsonpath::is_jsonpath(&path) {
                    return Err(Error::new("JQ second argument must be a valid JSONPath expression starting with $", None));
                }

                crate::runtime::jsonpath::apply_jsonpath(&json_data, &path)
            }
            _ => {
                // Evaluate arguments for built-in functions
                let mut ev_args = Vec::new();
                for a in args {
                    match a {
                        Expr::Spread(inner) => {
                            let v = Self::eval(inner, context)?;
                            if let Value::Array(items) = v {
                                ev_args.extend(items);
                            } else {
                                return Err(Error::new("Spread expects array", None));
                            }
                        }
                        _ => {
                            let val = Self::eval(a, context)?;
                            ev_args.push(val);
                        }
                    }
                }
                exec_builtin_fast(name, &ev_args)
            }
        }
    }
    
    /// Helper for higher-order functions - these need access to context for lambda evaluation
    fn eval_filter<C: EvaluationContext>(args: &[Expr], context: &C) -> Result<Value, Error> {
        if args.len() < 2 { 
            return Err(Error::new("FILTER expects (array, expr)", None)); 
        }
        let arr_v = Self::eval(&args[0], context)?;
        let lambda = &args[1];
        let param_name = if args.len() > 2 { 
            if let Value::String(s) = Self::eval(&args[2], context)? { s } else { "x".into() }
        } else { "x".into() };
        
        match arr_v {
            Value::Array(items) => {
                let mut out = Vec::with_capacity(items.len());
                let mut env = context.clone_variables();
                for it in items {
                    env.insert(param_name.clone(), it.clone());
                    let var_context = VariableContext::with_owned(env);
                    let matches = matches!(Self::eval(lambda, &var_context)?, Value::Boolean(true));
                    env = var_context.into_variables();
                    if matches {
                        out.push(it);
                    }
                }
                Ok(Value::Array(out))
            }
            _ => Err(Error::new("FILTER first arg must be array", None)),
        }
    }

    fn eval_find<C: EvaluationContext>(args: &[Expr], context: &C) -> Result<Value, Error> {
        if args.len() < 2 {
            return Err(Error::new("FIND expects (array, expr)", None));
        }
        let arr_v = Self::eval(&args[0], context)?;
        let lambda = &args[1];
        let param_name = if args.len() > 2 {
            if let Value::String(s) = Self::eval(&args[2], context)? { s } else { "x".into() }
        } else { "x".into() };

        match arr_v {
            Value::Array(items) => {
                let mut env = context.clone_variables();
                for it in items {
                    env.insert(param_name.clone(), it.clone());
                    let var_context = VariableContext::with_owned(env);
                    let matches = matches!(Self::eval(lambda, &var_context)?, Value::Boolean(true));
                    env = var_context.into_variables();
                    if matches {
                        return Ok(it);
                    }
                }
                Ok(Value::Null)
            }
            _ => Err(Error::new("FIND first arg must be array", None)),
        }
    }

    fn eval_map<C: EvaluationContext>(args: &[Expr], context: &C) -> Result<Value, Error> {
        if args.len() < 2 {
            return Err(Error::new("MAP expects (array, expr)", None));
        }
        let arr_v = Self::eval(&args[0], context)?;
        let lambda = &args[1];
        let param_name = if args.len() > 2 {
            if let Value::String(s) = Self::eval(&args[2], context)? { s } else { "x".into() }
        } else { "x".into() };

        match arr_v {
            Value::Array(items) => {
                let mut out = Vec::with_capacity(items.len());
                let mut env = context.clone_variables();
                for it in items {
                    env.insert(param_name.clone(), it);
                    let var_context = VariableContext::with_owned(env);
                    let result = Self::eval(lambda, &var_context)?;
                    env = var_context.into_variables();
                    out.push(result);
                }
                Ok(Value::Array(out))
            }
            _ => Err(Error::new("MAP first arg must be array", None)),
        }
    }

    fn eval_reduce<C: EvaluationContext>(args: &[Expr], context: &C) -> Result<Value, Error> {
        if args.len() < 3 {
            return Err(Error::new("REDUCE expects (array, expr, initial)", None));
        }
        let arr_v = Self::eval(&args[0], context)?;
        let lambda = &args[1];
        let mut acc = Self::eval(&args[2], context)?;
        let val_param = if args.len() > 3 {
            if let Value::String(s) = Self::eval(&args[3], context)? { s } else { "x".into() }
        } else { "x".into() };
        let acc_param = if args.len() > 4 {
            if let Value::String(s) = Self::eval(&args[4], context)? { s } else { "acc".into() }
        } else { "acc".into() };

        match arr_v {
            Value::Array(items) => {
                let mut env = context.clone_variables();
                for it in items {
                    env.insert(val_param.clone(), it);
                    env.insert(acc_param.clone(), acc);
                    let var_context = VariableContext::with_owned(env);
                    acc = Self::eval(lambda, &var_context)?;
                    env = var_context.into_variables();
                }
                Ok(acc)
            }
            _ => Err(Error::new("REDUCE first arg must be array", None)),
        }
    }
    
    fn eval_sumif<C: EvaluationContext>(args: &[Expr], context: &C) -> Result<Value, Error> {
        if args.len() < 2 || args.len() > 3 { 
            return Err(Error::new("SUMIF expects (array, criteria) or (array, criteria, sum_array)", None)); 
        }
        let arr_v = Self::eval(&args[0], context)?;
        let criteria_expr = &args[1];
        let sum_array = if args.len() == 3 { Some(Self::eval(&args[2], context)?) } else { None };
        
        // First try to evaluate the criteria as a static value (Excel-style string criteria)
        if let Ok(criteria_value) = Self::eval(criteria_expr, context) {
            if let Value::String(_) | Value::Number(_) = criteria_value {
                // Excel-style criteria - use string/numeric comparison logic
                return Self::eval_sumif_excel_style(&arr_v, &criteria_value, sum_array.as_ref().unwrap_or(&arr_v));
            }
        }
        
        // If that fails, fall back to lambda-based evaluation (existing behavior)
        if args.len() != 2 {
            return Err(Error::new("Lambda-style SUMIF expects exactly (array, expr)", None));
        }
        
        match arr_v {
            Value::Array(items) => {
                let mut acc = 0.0;
                let mut env = context.clone_variables();
                for it in items {
                    env.insert("x".into(), it.clone());
                    let var_context = VariableContext::with_owned(env);
                    let matches = matches!(Self::eval(criteria_expr, &var_context)?, Value::Boolean(true));
                    env = var_context.into_variables();
                    if matches {
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
    
    fn eval_sumif_excel_style(range: &Value, criteria: &Value, sum_range: &Value) -> Result<Value, Error> {
        fn meets_criteria(value: &Value, criteria: &Value) -> bool {
            match criteria {
                Value::String(crit) => {
                    if let Some(stripped) = crit.strip_prefix(">=") {
                        if let Ok(threshold) = stripped.parse::<f64>() {
                            match value {
                                Value::Number(n) => *n >= threshold,
                                Value::Currency(n) => *n >= threshold,
                                _ => false,
                            }
                        } else { false }
                    } else if let Some(stripped) = crit.strip_prefix("<=") {
                        if let Ok(threshold) = stripped.parse::<f64>() {
                            match value {
                                Value::Number(n) => *n <= threshold,
                                Value::Currency(n) => *n <= threshold,
                                _ => false,
                            }
                        } else { false }
                    } else if let Some(stripped) = crit.strip_prefix("<>") {
                        if let Ok(threshold) = stripped.parse::<f64>() {
                            match value {
                                Value::Number(n) => *n != threshold,
                                Value::Currency(n) => *n != threshold,
                                _ => true,
                            }
                        } else { 
                            match value {
                                Value::String(s) => s != stripped,
                                _ => true,
                            }
                        }
                    } else if let Some(stripped) = crit.strip_prefix('>') {
                        if let Ok(threshold) = stripped.parse::<f64>() {
                            match value {
                                Value::Number(n) => *n > threshold,
                                Value::Currency(n) => *n > threshold,
                                _ => false,
                            }
                        } else { false }
                    } else if let Some(stripped) = crit.strip_prefix('<') {
                        if let Ok(threshold) = stripped.parse::<f64>() {
                            match value {
                                Value::Number(n) => *n < threshold,
                                Value::Currency(n) => *n < threshold,
                                _ => false,
                            }
                        } else { false }
                    } else if let Some(stripped) = crit.strip_prefix('=') {
                        if let Ok(threshold) = stripped.parse::<f64>() {
                            match value {
                                Value::Number(n) => *n == threshold,
                                Value::Currency(n) => *n == threshold,
                                _ => false,
                            }
                        } else {
                            match value {
                                Value::String(s) => s == stripped,
                                _ => false,
                            }
                        }
                    } else if let Ok(threshold) = crit.parse::<f64>() {
                        match value {
                            Value::Number(n) => *n == threshold,
                            Value::Currency(n) => *n == threshold,
                            _ => false,
                        }
                    } else {
                        match value {
                            Value::String(s) => s == crit,
                            _ => false,
                        }
                    }
                }
                Value::Number(threshold) => {
                    match value {
                        Value::Number(n) => *n == *threshold,
                        Value::Currency(n) => *n == *threshold,
                        _ => false,
                    }
                }
                _ => false,
            }
        }
        
        fn sum_if_helper(range_val: &Value, sum_val: &Value, criteria: &Value) -> f64 {
            match (range_val, sum_val) {
                (Value::Array(range_items), Value::Array(sum_items)) => {
                    let mut acc = 0.0;
                    let min_len = std::cmp::min(range_items.len(), sum_items.len());
                    for i in 0..min_len {
                        if meets_criteria(&range_items[i], criteria) {
                            match &sum_items[i] {
                                Value::Number(n) => acc += *n,
                                Value::Currency(n) => acc += *n,
                                _ => {}
                            }
                        }
                    }
                    acc
                }
                (range_val, sum_val) => {
                    if meets_criteria(range_val, criteria) {
                        match sum_val {
                            Value::Number(n) => *n,
                            Value::Currency(n) => *n,
                            _ => 0.0,
                        }
                    } else {
                        0.0
                    }
                }
            }
        }
        
        let result = sum_if_helper(range, sum_range, criteria);
        Ok(Value::Number(result))
    }
    
    fn eval_avgif<C: EvaluationContext>(args: &[Expr], context: &C) -> Result<Value, Error> {
        if args.len() != 2 { 
            return Err(Error::new("AVGIF expects (array, expr)", None)); 
        }
        let arr_v = Self::eval(&args[0], context)?;
        let lambda = &args[1];
        
        match arr_v {
            Value::Array(items) => {
                let mut acc = 0.0;
                let mut count = 0usize;
                let mut env = context.clone_variables();
                for it in items {
                    env.insert("x".into(), it.clone());
                    let var_context = VariableContext::with_owned(env);
                    let matches = matches!(Self::eval(lambda, &var_context)?, Value::Boolean(true));
                    env = var_context.into_variables();
                    if matches {
                        match it {
                            Value::Number(n) | Value::Currency(n) => { acc += n; count += 1; },
                            _ => {}
                        }
                    }
                }
                Ok(Value::Number(if count == 0 { 0.0 } else { acc / count as f64 }))
            }
            _ => Err(Error::new("AVGIF first arg must be array", None)),
        }
    }

    fn eval_countif<C: EvaluationContext>(args: &[Expr], context: &C) -> Result<Value, Error> {
        if args.len() != 2 {
            return Err(Error::new("COUNTIF expects (array, expr)", None));
        }
        let arr_v = Self::eval(&args[0], context)?;
        let lambda = &args[1];

        match arr_v {
            Value::Array(items) => {
                let mut count = 0usize;
                let mut env = context.clone_variables();
                for it in items {
                    env.insert("x".into(), it);
                    let var_context = VariableContext::with_owned(env);
                    let matches = matches!(Self::eval(lambda, &var_context)?, Value::Boolean(true));
                    env = var_context.into_variables();
                    if matches {
                        count += 1;
                    }
                }
                Ok(Value::Number(count as f64))
            }
            _ => Err(Error::new("COUNTIF first arg must be array", None)),
        }
    }
    
    /// Helper to convert Value to JSON
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
                    json_arr.push(Self::value_to_json(item)?);
                }
                Ok(serde_json::Value::Array(json_arr))
            }
            Value::Json(s) => {
                serde_json::from_str(s)
                    .map_err(|e| Error::new(format!("Invalid JSON: {}", e), None))
            }
        }
    }
}

// Convenience functions for backward compatibility
pub fn eval(expr: &Expr) -> Result<Value, Error> {
    let context = EmptyContext;
    Evaluator::eval(expr, &context)
}

pub fn eval_with_vars(expr: &Expr, vars: &HashMap<String, Value>) -> Result<Value, Error> {
    let context = VariableContext::new(vars);
    Evaluator::eval(expr, &context)
}

pub fn eval_with_vars_and_custom(expr: &Expr, vars: &HashMap<String, Value>, custom_registry: &Arc<RwLock<FunctionRegistry>>) -> Result<Value, Error> {
    let context = VariableContext::with_custom(vars, custom_registry);
    Evaluator::eval(expr, &context)
}

/// Evaluate with support for assignments and sequences
pub fn eval_with_assignments(expr: &Expr, vars: &HashMap<String, Value>) -> Result<Value, Error> {
    let mut context = VariableContext::with_owned(vars.clone());
    eval_with_assignments_context(expr, &mut context)
}

/// Evaluate with support for assignments and sequences, returning both result and variable context
pub fn eval_with_assignments_and_context(expr: &Expr, vars: &HashMap<String, Value>) -> Result<(Value, HashMap<String, Value>), Error> {
    let mut context = VariableContext::with_owned(vars.clone());
    let result = eval_with_assignments_context(expr, &mut context)?;
    let final_vars = context.into_variables();
    Ok((result, final_vars))
}

fn eval_with_assignments_context(expr: &Expr, context: &mut VariableContext) -> Result<Value, Error> {
    match expr {
        Expr::Assignment { variable, value } => {
            let result = Evaluator::eval(value, context)?;
            context.make_mut().insert(variable.clone(), result.clone());
            Ok(result)
        }
        Expr::Sequence(exprs) => {
            let mut last_result = Value::Null;
            for expr in exprs {
                last_result = eval_with_assignments_context(expr, context)?;
            }
            Ok(last_result)
        }
        // For all other expressions, delegate to unified evaluator
        _ => Evaluator::eval(expr, context)
    }
}