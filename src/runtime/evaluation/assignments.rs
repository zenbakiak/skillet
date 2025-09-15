use crate::ast::Expr;
use crate::error::Error;
use crate::types::Value;
use super::core::eval_with_vars;

use std::collections::HashMap;

/// Evaluate with support for assignments and sequences
/// This function properly handles variable assignments by maintaining a mutable variable context
pub fn eval_with_assignments(expr: &Expr, vars: &HashMap<String, Value>) -> Result<Value, Error> {
    let mut context = vars.clone();
    eval_with_assignments_context(expr, &mut context)
}

/// Evaluate with support for assignments and sequences, returning both result and variable context
/// This function returns both the evaluation result and the final variable assignments
pub fn eval_with_assignments_and_context(expr: &Expr, vars: &HashMap<String, Value>) -> Result<(Value, HashMap<String, Value>), Error> {
    let mut context = vars.clone();
    let result = eval_with_assignments_context(expr, &mut context)?;
    Ok((result, context))
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