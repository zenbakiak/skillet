use super::arithmetic;
use super::array;
use super::datetime;
use super::financial;
use super::logical;
use super::statistical;
use super::string;
use crate::error::Error;
use crate::types::Value;

/// Old builtin entry point, unused by the evaluator.
///
/// It tries each category in turn and turns any error into `Unknown function`, so a wrong
/// argument is reported as an unknown name. Use
/// [`exec_builtin_fast`](crate::runtime::function_dispatch::exec_builtin_fast), the dispatch the
/// evaluator uses, or evaluate an expression with [`crate::evaluate`].
#[deprecated(
    since = "0.7.0",
    note = "unused by the evaluator and reports argument errors as `Unknown function`; use \
            `runtime::function_dispatch::exec_builtin_fast` or `skillet::evaluate`"
)]
pub fn exec_builtin(name: &str, args: &[Value]) -> Result<Value, Error> {
    // Try arithmetic functions first
    if let Ok(result) = arithmetic::exec_arithmetic(name, args) {
        return Ok(result);
    }

    // Try logical functions
    if let Ok(result) = logical::exec_logical(name, args) {
        return Ok(result);
    }

    // Try string functions
    if let Ok(result) = string::exec_string(name, args) {
        return Ok(result);
    }

    // Try array functions
    if let Ok(result) = array::exec_array(name, args) {
        return Ok(result);
    }

    // Try datetime functions
    if datetime::is_datetime_function(name) {
        return datetime::exec_datetime(name, args);
    }

    // Try financial functions
    if let Ok(result) = financial::exec_financial(name, args) {
        return Ok(result);
    }

    // Try statistical functions
    if let Ok(result) = statistical::exec_statistical(name, args) {
        return Ok(result);
    }

    // Handle remaining functions not yet modularized
    Err(Error::new(format!("Unknown function: {}", name), None))
}
