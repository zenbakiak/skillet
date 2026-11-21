use super::builtin_table::{Category, Dispatch, BUILTINS};
use super::{arithmetic, array, datetime, financial, logical, statistical, string};
use crate::error::Error;
use crate::types::Value;
use std::collections::HashMap;

/// Builtin dispatch driven by the single table in `builtin_table.rs` (BL-010): one hash lookup
/// from name (or alias) to the category whose module runs it.
pub struct FunctionDispatch {
    by_name: HashMap<&'static str, Category>,
}

impl FunctionDispatch {
    /// Index every name and alias of the table's module-dispatched rows.
    pub fn new() -> Self {
        let by_name = BUILTINS
            .iter()
            .filter(|b| b.dispatch == Dispatch::Module)
            .flat_map(|b| b.names().map(move |name| (name, b.category)))
            .collect();
        Self { by_name }
    }

    /// Execute a builtin function by name (exact, upper-case) with evaluated arguments.
    pub fn execute(&self, name: &str, args: &[Value]) -> Result<Value, Error> {
        match self.by_name.get(name) {
            Some(category) => run(*category, name, args),
            None => Err(Error::new(format!("Unknown function: {}", name), None)),
        }
    }

    /// Check if a function is registered in any category
    pub fn has_function(&self, name: &str) -> bool {
        self.by_name.contains_key(name)
    }

    /// Get the total number of registered names (aliases included)
    pub fn count(&self) -> usize {
        self.by_name.len()
    }

    #[cfg(test)]
    fn category_of(&self, name: &str) -> Option<Category> {
        self.by_name.get(name).copied()
    }
}

/// Run `name` in the module for `category`.
fn run(category: Category, name: &str, args: &[Value]) -> Result<Value, Error> {
    match category {
        Category::Arithmetic => arithmetic::exec_arithmetic(name, args),
        Category::Logical => logical::exec_logical(name, args),
        Category::String => string::exec_string(name, args),
        Category::Array => array::exec_array(name, args),
        Category::DateTime => datetime::exec_datetime(name, args),
        Category::Financial => financial::exec_financial(name, args),
        Category::Statistical => statistical::exec_statistical(name, args),
        Category::Json => crate::runtime::json::exec_json(name, args),
    }
}

impl Default for FunctionDispatch {
    fn default() -> Self {
        Self::new()
    }
}

// Global function dispatch table for optimal performance
lazy_static::lazy_static! {
    static ref GLOBAL_DISPATCH: FunctionDispatch = FunctionDispatch::new();
}

/// Optimized builtin function execution using category-based dispatch
pub fn exec_builtin_fast(name: &str, args: &[Value]) -> Result<Value, Error> {
    GLOBAL_DISPATCH.execute(name, args)
}

/// Check if a builtin function exists
pub fn has_builtin_function(name: &str) -> bool {
    GLOBAL_DISPATCH.has_function(name)
}

/// Get count of registered builtin functions
pub fn builtin_function_count() -> usize {
    GLOBAL_DISPATCH.count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_dispatch_basic() {
        let dispatch = FunctionDispatch::new();

        // Test arithmetic function exists
        assert!(dispatch.has_function("SUM"));

        // Test string function exists
        assert!(dispatch.has_function("UPPER"));

        // Test unknown function
        assert!(!dispatch.has_function("NONEXISTENT"));
    }

    #[test]
    fn test_category_lookup() {
        let dispatch = FunctionDispatch::new();

        // Test each category has functions
        assert_eq!(dispatch.category_of("SUM"), Some(Category::Arithmetic));
        assert_eq!(dispatch.category_of("UPPER"), Some(Category::String));
        assert_eq!(dispatch.category_of("FLATTEN"), Some(Category::Array));
        assert_eq!(dispatch.category_of("NOW"), Some(Category::DateTime));
        assert_eq!(dispatch.category_of("PMT"), Some(Category::Financial));
        assert_eq!(dispatch.category_of("MEDIAN"), Some(Category::Statistical));
        // REVERSE exists in string.rs and array.rs; dispatch has always picked array.
        assert_eq!(dispatch.category_of("REVERSE"), Some(Category::Array));
    }

    #[test]
    fn test_global_dispatch() {
        // Test that global dispatch works
        assert!(has_builtin_function("SUM"));
        assert!(!has_builtin_function("NONEXISTENT"));
        assert!(builtin_function_count() > 50); // Should have many functions registered
    }
}
