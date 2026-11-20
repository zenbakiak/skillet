pub mod arithmetic;
pub mod array;
pub mod builtin_functions;
pub mod datetime;
pub mod evaluation;
pub mod evaluator;
pub mod financial;
pub mod function_dispatch;
pub mod json;
pub mod jsonpath;
pub mod logical;
pub mod method_calls;
pub mod statistical;
pub mod string;
pub mod type_casting;
pub mod utils;

// Re-export the main public functions
pub use evaluator::{
    eval, eval_with_assignments, eval_with_assignments_and_context, eval_with_vars,
    eval_with_vars_and_custom,
};
pub use type_casting::cast_value;
pub use utils::{clamp_index, index_array, is_blank, slice_array, values_equal};
