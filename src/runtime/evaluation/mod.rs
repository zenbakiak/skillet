pub mod core;
pub mod higher_order;
pub mod assignments;

pub use core::{eval, eval_with_vars, eval_with_vars_and_custom};
pub use assignments::{eval_with_assignments, eval_with_assignments_and_context};