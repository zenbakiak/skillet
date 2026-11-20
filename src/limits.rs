//! Resource limits that protect the process from hostile input (BL-017).

use crate::error::Error;
use crate::types::Value;

/// Maximum nesting depth of an expression and of any `Value` built while evaluating it.
///
/// The parser counts both its own recursion (parentheses, brackets, call arguments, unary
/// operators, …) and the depth of the AST it builds (including left-deep operator chains
/// and postfix chains). The evaluator applies the same bound to arrays it constructs.
/// Exceeding it returns an `Error` instead of overflowing the stack. 128 matches
/// `serde_json`'s recursion limit, so expression text and JSON variables share one number.
pub const MAX_NESTING_DEPTH: usize = 128;

/// The error returned when [`MAX_NESTING_DEPTH`] is exceeded.
pub(crate) fn nesting_error(position: Option<usize>) -> Error {
    Error::new(
        format!("Expression nested too deeply (max {})", MAX_NESTING_DEPTH),
        position,
    )
}

/// Reject an `Array` value nested deeper than [`MAX_NESTING_DEPTH`].
///
/// Called on every value the evaluator constructs (array literals, function and method
/// results, casts), so deep values built at runtime from a flat AST — e.g.
/// `[1, …].reduce([:acc], 0)` — fail before their recursive `Drop`/`Clone`/`Debug` can
/// overflow the stack. Non-arrays cost one match; arrays cost one pass over their
/// elements, and the walk stops after `MAX_NESTING_DEPTH` levels.
pub(crate) fn check_value_depth(value: Value) -> Result<Value, Error> {
    if depth_within(&value, MAX_NESTING_DEPTH) {
        Ok(value)
    } else {
        Err(nesting_error(None))
    }
}

/// Same depth rule as the parser: non-arrays and empty arrays are 0, an array is one
/// more than its deepest element.
fn depth_within(value: &Value, budget: usize) -> bool {
    match value {
        Value::Array(items) => {
            items.is_empty() || (budget > 0 && items.iter().all(|v| depth_within(v, budget - 1)))
        }
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nest(depth: usize) -> Value {
        let mut v = Value::Number(1.0);
        for _ in 0..depth {
            v = Value::Array(vec![v]);
        }
        v
    }

    #[test]
    fn value_depth_limit_boundary() {
        assert!(check_value_depth(nest(MAX_NESTING_DEPTH)).is_ok());
        let err = check_value_depth(nest(MAX_NESTING_DEPTH + 1)).expect_err("too deep");
        assert_eq!(err.message, "Expression nested too deeply (max 128)");
        assert!(check_value_depth(Value::Array(vec![])).is_ok());
    }

    #[test]
    fn value_depth_walk_stops_early() {
        // Far deeper than the stack could recurse; the walk must stop at the limit.
        let deep = nest(5_000);
        assert!(!depth_within(&deep, MAX_NESTING_DEPTH));
        std::mem::forget(deep); // recursive Drop of 5k levels is not what this tests
    }
}
