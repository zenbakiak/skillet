use crate::types::Value;
use crate::error::Error;
use std::collections::HashSet;
use super::{arithmetic, logical, string, array, datetime, financial, statistical};

/// Optimized function dispatch using static hash sets for O(1) category lookup
pub struct FunctionDispatch {
    arithmetic_functions: HashSet<&'static str>,
    logical_functions: HashSet<&'static str>,
    string_functions: HashSet<&'static str>,
    array_functions: HashSet<&'static str>,
    datetime_functions: HashSet<&'static str>,
    financial_functions: HashSet<&'static str>,
    statistical_functions: HashSet<&'static str>,
}

impl FunctionDispatch {
    /// Create new function dispatch with categorized function sets
    pub fn new() -> Self {
        let mut arithmetic_functions = HashSet::new();
        arithmetic_functions.insert("SUM");
        arithmetic_functions.insert("AVG");
        arithmetic_functions.insert("AVERAGE");
        arithmetic_functions.insert("MIN");
        arithmetic_functions.insert("MAX");
        arithmetic_functions.insert("ROUND");
        arithmetic_functions.insert("CEIL");
        arithmetic_functions.insert("CEILING");
        arithmetic_functions.insert("FLOOR");
        arithmetic_functions.insert("ABS");
        arithmetic_functions.insert("SQRT");
        arithmetic_functions.insert("POW");
        arithmetic_functions.insert("POWER");
        arithmetic_functions.insert("MOD");
        arithmetic_functions.insert("INT");
        
        let mut logical_functions = HashSet::new();
        logical_functions.insert("AND");
        logical_functions.insert("OR");
        logical_functions.insert("NOT");
        logical_functions.insert("XOR");
        logical_functions.insert("IF");
        logical_functions.insert("IFS");
        
        let mut string_functions = HashSet::new();
        string_functions.insert("LENGTH");
        string_functions.insert("CONCAT");
        string_functions.insert("UPPER");
        string_functions.insert("LOWER");
        string_functions.insert("TRIM");
        string_functions.insert("SUBSTRING");
        string_functions.insert("SPLIT");
        string_functions.insert("REPLACE");
        // Note: REVERSE is handled in both string and array modules, prioritize array
        string_functions.insert("ISBLANK");
        string_functions.insert("ISNUMBER");
        string_functions.insert("ISTEXT");
        string_functions.insert("INCLUDES");
        string_functions.insert("LEFT");
        string_functions.insert("RIGHT");
        string_functions.insert("MID");
        
        let mut array_functions = HashSet::new();
        array_functions.insert("ARRAY");
        array_functions.insert("FLATTEN");
        array_functions.insert("FIRST");
        array_functions.insert("LAST");
        array_functions.insert("CONTAINS");
        array_functions.insert("IN");
        array_functions.insert("COUNT");
        array_functions.insert("UNIQUE");
        array_functions.insert("SORT");
        array_functions.insert("REVERSE");
        array_functions.insert("JOIN");
        
        let mut datetime_functions = HashSet::new();
        datetime_functions.insert("NOW");
        datetime_functions.insert("DATE");
        datetime_functions.insert("TIME");
        datetime_functions.insert("YEAR");
        datetime_functions.insert("MONTH");
        datetime_functions.insert("DAY");
        datetime_functions.insert("DATEADD");
        datetime_functions.insert("DATEDIFF");
        
        let mut financial_functions = HashSet::new();
        financial_functions.insert("PMT");
        financial_functions.insert("DB");
        financial_functions.insert("FV");
        financial_functions.insert("IPMT");
        
        let mut statistical_functions = HashSet::new();
        statistical_functions.insert("MEDIAN");
        statistical_functions.insert("MODE.SNGL");
        statistical_functions.insert("MODESNGL");
        statistical_functions.insert("MODE_SNGL");
        statistical_functions.insert("STDEV.P");
        statistical_functions.insert("STDEVP");
        statistical_functions.insert("STDEV_P");
        statistical_functions.insert("VAR.P");
        statistical_functions.insert("VARP");
        statistical_functions.insert("VAR_P");
        statistical_functions.insert("PERCENTILE.INC");
        statistical_functions.insert("PERCENTILEINC");
        statistical_functions.insert("PERCENTILE_INC");
        statistical_functions.insert("QUARTILE.INC");
        statistical_functions.insert("QUARTILEINC");
        statistical_functions.insert("QUARTILE_INC");
        
        Self {
            arithmetic_functions,
            logical_functions,
            string_functions,
            array_functions,
            datetime_functions,
            financial_functions,
            statistical_functions,
        }
    }
    
    /// Execute a builtin function using optimized category lookup
    pub fn execute(&self, name: &str, args: &[Value]) -> Result<Value, Error> {
        // O(1) category lookup then direct dispatch - much faster than sequential module tries
        // Check array functions first for functions that exist in multiple modules (like REVERSE)
        if self.array_functions.contains(name) {
            return array::exec_array(name, args);
        }
        
        if self.arithmetic_functions.contains(name) {
            return arithmetic::exec_arithmetic(name, args);
        }
        
        if self.logical_functions.contains(name) {
            return logical::exec_logical(name, args);
        }
        
        if self.string_functions.contains(name) {
            return string::exec_string(name, args);
        }
        
        if self.datetime_functions.contains(name) {
            return datetime::exec_datetime(name, args);
        }
        
        if self.financial_functions.contains(name) {
            return financial::exec_financial(name, args);
        }
        
        if self.statistical_functions.contains(name) {
            return statistical::exec_statistical(name, args);
        }
        
        Err(Error::new(format!("Unknown function: {}", name), None))
    }
    
    /// Check if a function is registered in any category
    pub fn has_function(&self, name: &str) -> bool {
        self.arithmetic_functions.contains(name) ||
        self.logical_functions.contains(name) ||
        self.string_functions.contains(name) ||
        self.array_functions.contains(name) ||
        self.datetime_functions.contains(name) ||
        self.financial_functions.contains(name) ||
        self.statistical_functions.contains(name)
    }
    
    /// Get the total number of registered functions
    pub fn count(&self) -> usize {
        self.arithmetic_functions.len() +
        self.logical_functions.len() +
        self.string_functions.len() +
        self.array_functions.len() +
        self.datetime_functions.len() +
        self.financial_functions.len() +
        self.statistical_functions.len()
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
        assert!(dispatch.arithmetic_functions.contains("SUM"));
        assert!(dispatch.string_functions.contains("UPPER"));
        assert!(dispatch.array_functions.contains("FLATTEN"));
        assert!(dispatch.datetime_functions.contains("NOW"));
        assert!(dispatch.financial_functions.contains("PMT"));
        assert!(dispatch.statistical_functions.contains("MEDIAN"));
    }
    
    #[test]
    fn test_global_dispatch() {
        // Test that global dispatch works
        assert!(has_builtin_function("SUM"));
        assert!(!has_builtin_function("NONEXISTENT"));
        assert!(builtin_function_count() > 50); // Should have many functions registered
    }
}