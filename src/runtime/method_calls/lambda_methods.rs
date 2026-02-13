use crate::ast::Expr;
use crate::custom::FunctionRegistry;
use crate::error::Error;
use crate::runtime::evaluation::{eval_with_vars, eval_with_vars_and_custom};
use crate::types::Value;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Handle FILTER method call (higher-order function)
pub fn exec_filter(
    recv: &Value,
    args_expr: &[Expr],
    base_vars: Option<&HashMap<String, Value>>,
) -> Result<Value, Error> {
    let recv_array = match recv {
        Value::Array(a) => a,
        _ => return Err(Error::new("filter called on non-array", None)),
    };
    
    if args_expr.is_empty() {
        return Err(Error::new("filter expects lambda expression", None));
    }
    
    let lambda_expr = &args_expr[0];
    let param_name = if args_expr.len() > 1 {
        match &args_expr[1] {
            Expr::StringLit(s) => s.clone(),
            _ => "x".to_string(),
        }
    } else {
        "x".to_string()
    };
    
    let mut filtered = Vec::with_capacity(recv_array.len());
    let mut vars = base_vars.cloned().unwrap_or_default();

    for item in recv_array {
        vars.insert(param_name.clone(), item.clone());
        let result = eval_with_vars(lambda_expr, &vars)?;
        if let Value::Boolean(true) = result {
            filtered.push(item.clone());
        }
    }

    Ok(Value::Array(filtered))
}

/// Handle FILTER method call with custom function support
pub fn exec_filter_with_custom(
    recv: &Value,
    args_expr: &[Expr],
    base_vars: Option<&HashMap<String, Value>>,
    custom_registry: &Arc<RwLock<FunctionRegistry>>,
) -> Result<Value, Error> {
    let recv_array = match recv {
        Value::Array(a) => a,
        _ => return Err(Error::new("filter called on non-array", None)),
    };

    if args_expr.is_empty() {
        return Err(Error::new("filter expects lambda expression", None));
    }

    let lambda_expr = &args_expr[0];
    let param_name = if args_expr.len() > 1 {
        match &args_expr[1] {
            Expr::StringLit(s) => s.clone(),
            _ => "x".to_string(),
        }
    } else {
        "x".to_string()
    };

    let mut filtered = Vec::with_capacity(recv_array.len());
    let mut vars = base_vars.cloned().unwrap_or_default();

    for item in recv_array {
        vars.insert(param_name.clone(), item.clone());
        let result = eval_with_vars_and_custom(lambda_expr, &vars, custom_registry)?;
        if let Value::Boolean(true) = result {
            filtered.push(item.clone());
        }
    }

    Ok(Value::Array(filtered))
}

/// Handle MAP method call (higher-order function)
pub fn exec_map(
    recv: &Value,
    args_expr: &[Expr],
    base_vars: Option<&HashMap<String, Value>>,
) -> Result<Value, Error> {
    let recv_array = match recv {
        Value::Array(a) => a,
        _ => return Err(Error::new("map called on non-array", None)),
    };
    
    if args_expr.is_empty() {
        return Err(Error::new("map expects lambda expression", None));
    }
    
    let lambda_expr = &args_expr[0];
    let param_name = if args_expr.len() > 1 {
        match &args_expr[1] {
            Expr::StringLit(s) => s.clone(),
            _ => "x".to_string(),
        }
    } else {
        "x".to_string()
    };
    
    let mut mapped = Vec::with_capacity(recv_array.len());
    let mut vars = base_vars.cloned().unwrap_or_default();

    for item in recv_array {
        vars.insert(param_name.clone(), item.clone());
        let result = eval_with_vars(lambda_expr, &vars)?;
        mapped.push(result);
    }

    Ok(Value::Array(mapped))
}

/// Handle MAP method call with custom function support
pub fn exec_map_with_custom(
    recv: &Value,
    args_expr: &[Expr],
    base_vars: Option<&HashMap<String, Value>>,
    custom_registry: &Arc<RwLock<FunctionRegistry>>,
) -> Result<Value, Error> {
    let recv_array = match recv {
        Value::Array(a) => a,
        _ => return Err(Error::new("map called on non-array", None)),
    };

    if args_expr.is_empty() {
        return Err(Error::new("map expects lambda expression", None));
    }

    let lambda_expr = &args_expr[0];
    let param_name = if args_expr.len() > 1 {
        match &args_expr[1] {
            Expr::StringLit(s) => s.clone(),
            _ => "x".to_string(),
        }
    } else {
        "x".to_string()
    };

    let mut mapped = Vec::with_capacity(recv_array.len());
    let mut vars = base_vars.cloned().unwrap_or_default();

    for item in recv_array {
        vars.insert(param_name.clone(), item.clone());
        let result = eval_with_vars_and_custom(lambda_expr, &vars, custom_registry)?;
        mapped.push(result);
    }

    Ok(Value::Array(mapped))
}

/// Handle FIND method call (higher-order function)
pub fn exec_find(
    recv: &Value,
    args_expr: &[Expr],
    base_vars: Option<&HashMap<String, Value>>,
) -> Result<Value, Error> {
    let recv_array = match recv {
        Value::Array(a) => a,
        _ => return Err(Error::new("find called on non-array", None)),
    };
    
    if args_expr.is_empty() {
        return Err(Error::new("find expects lambda expression", None));
    }
    
    let lambda_expr = &args_expr[0];
    let param_name = if args_expr.len() > 1 {
        match &args_expr[1] {
            Expr::StringLit(s) => s.clone(),
            _ => "x".to_string(),
        }
    } else {
        "x".to_string()
    };
    
    let mut vars = base_vars.cloned().unwrap_or_default();
    
    for item in recv_array {
        vars.insert(param_name.clone(), item.clone());
        let result = eval_with_vars(lambda_expr, &vars)?;
        if let Value::Boolean(true) = result {
            return Ok(item.clone());
        }
    }
    
    Ok(Value::Null)
}

/// Handle FIND method call with custom function support
pub fn exec_find_with_custom(
    recv: &Value,
    args_expr: &[Expr],
    base_vars: Option<&HashMap<String, Value>>,
    custom_registry: &Arc<RwLock<FunctionRegistry>>,
) -> Result<Value, Error> {
    let recv_array = match recv {
        Value::Array(a) => a,
        _ => return Err(Error::new("find called on non-array", None)),
    };
    
    if args_expr.is_empty() {
        return Err(Error::new("find expects lambda expression", None));
    }
    
    let lambda_expr = &args_expr[0];
    let param_name = if args_expr.len() > 1 {
        match &args_expr[1] {
            Expr::StringLit(s) => s.clone(),
            _ => "x".to_string(),
        }
    } else {
        "x".to_string()
    };
    
    let mut vars = base_vars.cloned().unwrap_or_default();
    
    for item in recv_array {
        vars.insert(param_name.clone(), item.clone());
        let result = eval_with_vars_and_custom(lambda_expr, &vars, custom_registry)?;
        if let Value::Boolean(true) = result {
            return Ok(item.clone());
        }
    }
    
    Ok(Value::Null)
}

/// Handle REDUCE method call (higher-order function)
pub fn exec_reduce(
    recv: &Value,
    args_expr: &[Expr],
    base_vars: Option<&HashMap<String, Value>>,
) -> Result<Value, Error> {
    let recv_array = match recv {
        Value::Array(a) => a,
        _ => return Err(Error::new("reduce called on non-array", None)),
    };
    
    if args_expr.len() < 2 {
        return Err(Error::new("reduce expects lambda expression and initial value", None));
    }
    
    let lambda_expr = &args_expr[0];
    let mut vars = base_vars.cloned().unwrap_or_default();
    let mut accumulator = eval_with_vars(&args_expr[1], &vars)?;

    let val_param = if args_expr.len() > 2 {
        match &args_expr[2] {
            Expr::StringLit(s) => s.clone(),
            _ => "x".to_string(),
        }
    } else {
        "x".to_string()
    };

    let acc_param = if args_expr.len() > 3 {
        match &args_expr[3] {
            Expr::StringLit(s) => s.clone(),
            _ => "acc".to_string(),
        }
    } else {
        "acc".to_string()
    };

    for item in recv_array {
        vars.insert(val_param.clone(), item.clone());
        vars.insert(acc_param.clone(), accumulator);
        accumulator = eval_with_vars(lambda_expr, &vars)?;
    }

    Ok(accumulator)
}

/// Handle REDUCE method call with custom function support
pub fn exec_reduce_with_custom(
    recv: &Value,
    args_expr: &[Expr],
    base_vars: Option<&HashMap<String, Value>>,
    custom_registry: &Arc<RwLock<FunctionRegistry>>,
) -> Result<Value, Error> {
    let recv_array = match recv {
        Value::Array(a) => a,
        _ => return Err(Error::new("reduce called on non-array", None)),
    };

    if args_expr.len() < 2 {
        return Err(Error::new("reduce expects lambda expression and initial value", None));
    }

    let lambda_expr = &args_expr[0];
    let mut vars = base_vars.cloned().unwrap_or_default();
    let mut accumulator = eval_with_vars_and_custom(&args_expr[1], &vars, custom_registry)?;

    let val_param = if args_expr.len() > 2 {
        match &args_expr[2] {
            Expr::StringLit(s) => s.clone(),
            _ => "x".to_string(),
        }
    } else {
        "x".to_string()
    };

    let acc_param = if args_expr.len() > 3 {
        match &args_expr[3] {
            Expr::StringLit(s) => s.clone(),
            _ => "acc".to_string(),
        }
    } else {
        "acc".to_string()
    };

    for item in recv_array {
        vars.insert(val_param.clone(), item.clone());
        vars.insert(acc_param.clone(), accumulator);
        accumulator = eval_with_vars_and_custom(lambda_expr, &vars, custom_registry)?;
    }

    Ok(accumulator)
}