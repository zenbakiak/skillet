use crate::error::Error;
use crate::types::Value;
use std::collections::HashMap;

/// Trait for implementing custom functions in skillet
/// 
/// # Example
/// ```rust
/// use skillet::custom::CustomFunction;
/// use skillet::{Value, Error};
/// 
/// struct DoubleFunction;
/// 
/// impl CustomFunction for DoubleFunction {
///     fn name(&self) -> &str { "DOUBLE" }
///     fn min_args(&self) -> usize { 1 }
///     fn max_args(&self) -> Option<usize> { Some(1) }
///     
///     fn execute(&self, args: Vec<Value>) -> Result<Value, Error> {
///         let num = args[0].as_number()
///             .ok_or_else(|| Error::new("DOUBLE expects a number", None))?;
///         Ok(Value::Number(num * 2.0))
///     }
/// }
/// ```
pub trait CustomFunction: Send + Sync {
    /// The name of the function (case-insensitive)
    fn name(&self) -> &str;
    
    /// Minimum number of arguments required
    fn min_args(&self) -> usize;
    
    /// Maximum number of arguments allowed (None = unlimited)
    fn max_args(&self) -> Option<usize>;
    
    /// Execute the function with the given arguments
    fn execute(&self, args: Vec<Value>) -> Result<Value, Error>;
    
    /// Optional: Description of the function for documentation
    fn description(&self) -> Option<&str> { None }
    
    /// Optional: Example usage for documentation
    fn example(&self) -> Option<&str> { None }
}

/// Registry for custom functions
#[derive(Default)]
pub struct FunctionRegistry {
    functions: HashMap<String, Box<dyn CustomFunction>>,
}

impl FunctionRegistry {
    /// Create a new empty function registry
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
        }
    }
    
    /// Register a custom function
    pub fn register(&mut self, function: Box<dyn CustomFunction>) -> Result<(), Error> {
        let name = function.name().to_uppercase();
        
        // Validate function definition
        if name.is_empty() {
            return Err(Error::new("Function name cannot be empty", None));
        }
        
        if function.min_args() > function.max_args().unwrap_or(usize::MAX) {
            return Err(Error::new("min_args cannot be greater than max_args", None));
        }
        
        self.functions.insert(name, function);
        Ok(())
    }
    
    /// Get a function by name (case-insensitive)
    pub fn get(&self, name: &str) -> Option<&dyn CustomFunction> {
        self.functions.get(&name.to_uppercase()).map(|f| f.as_ref())
    }
    
    /// List all registered function names
    pub fn list_functions(&self) -> Vec<&str> {
        self.functions.keys().map(|s| s.as_str()).collect()
    }
    
    /// Remove a function by name
    pub fn unregister(&mut self, name: &str) -> bool {
        self.functions.remove(&name.to_uppercase()).is_some()
    }
    
    /// Check if a function is registered
    pub fn has_function(&self, name: &str) -> bool {
        self.functions.contains_key(&name.to_uppercase())
    }
    
    /// Validate and execute a function
    pub fn execute(&self, name: &str, args: Vec<Value>) -> Result<Value, Error> {
        let function = self.get(name)
            .ok_or_else(|| Error::new(format!("Unknown custom function: {}", name), None))?;
        
        // Validate argument count
        let arg_count = args.len();
        if arg_count < function.min_args() {
            return Err(Error::new(
                format!("{} expects at least {} arguments, got {}", 
                    name, function.min_args(), arg_count), 
                None
            ));
        }
        
        if let Some(max_args) = function.max_args() {
            if arg_count > max_args {
                return Err(Error::new(
                    format!("{} expects at most {} arguments, got {}", 
                        name, max_args, arg_count), 
                    None
                ));
            }
        }
        
        // Execute the function
        function.execute(args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    struct TestFunction;
    
    impl CustomFunction for TestFunction {
        fn name(&self) -> &str { "TEST" }
        fn min_args(&self) -> usize { 1 }
        fn max_args(&self) -> Option<usize> { Some(2) }
        
        fn execute(&self, args: Vec<Value>) -> Result<Value, Error> {
            Ok(Value::String(format!("Called with {} args", args.len())))
        }
        
        fn description(&self) -> Option<&str> { Some("A test function") }
        fn example(&self) -> Option<&str> { Some("TEST(1, 2)") }
    }
    
    #[test]
    fn test_function_registry() {
        let mut registry = FunctionRegistry::new();
        
        // Register function
        assert!(registry.register(Box::new(TestFunction)).is_ok());
        
        // Check if registered
        assert!(registry.has_function("TEST"));
        assert!(registry.has_function("test")); // Case insensitive
        
        // Execute function
        let result = registry.execute("TEST", vec![Value::Number(1.0)]).unwrap();
        assert!(matches!(result, Value::String(_)));
        
        // Test argument validation
        assert!(registry.execute("TEST", vec![]).is_err()); // Too few args
        assert!(registry.execute("TEST", vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)]).is_err()); // Too many args
        
        // Unregister
        assert!(registry.unregister("TEST"));
        assert!(!registry.has_function("TEST"));
    }
}