//! Higher-order array methods (`filter`, `map`, `find`, `reduce`).
//!
//! Lambda bodies run in a `Scope` (private, in `runtime::evaluator`): the item (and
//! accumulator) are local bindings over the borrowed caller context, so no variable map is
//! copied per call or per item (BL-030).

use crate::ast::Expr;
use crate::error::Error;
use crate::limits::with_meter;
use crate::runtime::evaluator::{EvaluationContext, Evaluator, Scope};
use crate::types::Value;

fn recv_items<'v>(recv: &'v Value, method: &str) -> Result<&'v [Value], Error> {
    match recv {
        Value::Array(a) => Ok(a),
        _ => Err(Error::new(format!("{} called on non-array", method), None)),
    }
}

/// Parameter name given as a string literal at `idx`, else `default`.
fn param_name(args_expr: &[Expr], idx: usize, default: &str) -> String {
    match args_expr.get(idx) {
        Some(Expr::StringLit(s)) => s.clone(),
        _ => default.to_string(),
    }
}

fn lambda_scope(ctx: &dyn EvaluationContext) -> Scope<'_> {
    Scope::new(ctx)
}

/// Handle FILTER method call (higher-order function)
pub fn exec_filter(
    recv: &Value,
    args_expr: &[Expr],
    ctx: &dyn EvaluationContext,
) -> Result<Value, Error> {
    let items = recv_items(recv, "filter")?;
    let lambda_expr = args_expr
        .first()
        .ok_or_else(|| Error::new("filter expects lambda expression", None))?;
    let param = param_name(args_expr, 1, "x");

    let mut scope = lambda_scope(ctx);
    let mut filtered = Vec::with_capacity(items.len());
    with_meter(|meter| {
        for item in items {
            meter.charge_value(item)?;
            scope.set(&param, item.clone());
            if let Value::Boolean(true) = Evaluator::eval(lambda_expr, &scope)? {
                filtered.push(item.clone());
            }
        }
        Ok(Value::Array(filtered))
    })
}

/// Handle MAP method call (higher-order function)
pub fn exec_map(
    recv: &Value,
    args_expr: &[Expr],
    ctx: &dyn EvaluationContext,
) -> Result<Value, Error> {
    let items = recv_items(recv, "map")?;
    let lambda_expr = args_expr
        .first()
        .ok_or_else(|| Error::new("map expects lambda expression", None))?;
    let param = param_name(args_expr, 1, "x");

    let mut scope = lambda_scope(ctx);
    let mut mapped = Vec::with_capacity(items.len());
    with_meter(|meter| {
        for item in items {
            meter.charge_value(item)?;
            scope.set(&param, item.clone());
            mapped.push(Evaluator::eval(lambda_expr, &scope)?);
        }
        Ok(Value::Array(mapped))
    })
}

/// Handle FIND method call (higher-order function)
pub fn exec_find(
    recv: &Value,
    args_expr: &[Expr],
    ctx: &dyn EvaluationContext,
) -> Result<Value, Error> {
    let items = recv_items(recv, "find")?;
    let lambda_expr = args_expr
        .first()
        .ok_or_else(|| Error::new("find expects lambda expression", None))?;
    let param = param_name(args_expr, 1, "x");

    let mut scope = lambda_scope(ctx);
    with_meter(|meter| {
        for item in items {
            meter.charge_value(item)?;
            scope.set(&param, item.clone());
            if let Value::Boolean(true) = Evaluator::eval(lambda_expr, &scope)? {
                return Ok(item.clone());
            }
        }
        Ok(Value::Null)
    })
}

/// Handle REDUCE method call (higher-order function)
pub fn exec_reduce(
    recv: &Value,
    args_expr: &[Expr],
    ctx: &dyn EvaluationContext,
) -> Result<Value, Error> {
    let items = recv_items(recv, "reduce")?;
    if args_expr.len() < 2 {
        return Err(Error::new(
            "reduce expects lambda expression and initial value",
            None,
        ));
    }
    let lambda_expr = &args_expr[0];
    let mut scope = lambda_scope(ctx);
    let mut accumulator = Evaluator::eval(&args_expr[1], &scope)?;
    let val_param = param_name(args_expr, 2, "x");
    let acc_param = param_name(args_expr, 3, "acc");

    with_meter(|meter| {
        for item in items {
            meter.charge_value(item)?;
            // Same order as before: when both names are equal the accumulator wins.
            scope.set(&val_param, item.clone());
            scope.set(&acc_param, accumulator);
            accumulator = Evaluator::eval(lambda_expr, &scope)?;
        }
        Ok(accumulator)
    })
}
