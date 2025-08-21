pub mod evaluation;
pub mod builtin_functions;
pub mod method_calls;
pub mod type_casting;
pub mod utils;

// Re-export the main public functions
pub use evaluation::{eval, eval_with_vars, eval_with_vars_and_custom};
pub use type_casting::cast_value;
pub use utils::{is_blank, clamp_index, index_array, slice_array, values_equal};