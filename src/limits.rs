//! Resource limits that protect the process from hostile input (BL-017).

use crate::error::Error;

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
