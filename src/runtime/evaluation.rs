//! Compatibility path for the former duplicate evaluator (BL-001).
//!
//! `runtime::evaluator` is the single implementation; these re-exports keep
//! `skillet::runtime::evaluation::*` compiling.

pub use super::evaluator::{
    eval, eval_with_assignments, eval_with_assignments_and_context, eval_with_vars,
    eval_with_vars_and_custom,
};
