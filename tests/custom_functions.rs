use skillet::{register_function, unregister_function, evaluate_with_custom, CustomFunction, Value, Error};
use std::collections::HashMap;
use std::sync::Mutex;

// Global test mutex to prevent concurrent access to the global function registry
static TEST_MUTEX: Mutex<()> = Mutex::new(());

/// Example custom function that doubles a number
struct DoubleFunction;

impl CustomFunction for DoubleFunction {
    fn name(&self) -> &str { "DOUBLE" }
    fn min_args(&self) -> usize { 1 }
    fn max_args(&self) -> Option<usize> { Some(1) }
    
    fn execute(&self, args: Vec<Value>) -> Result<Value, Error> {
        let num = args[0].as_number()
            .ok_or_else(|| Error::new("DOUBLE expects a number", None))?;
        Ok(Value::Number(num * 2.0))
    }
    
    fn description(&self) -> Option<&str> { Some("Doubles a number") }
    fn example(&self) -> Option<&str> { Some("DOUBLE(5) returns 10") }
}

/// Example custom function that concatenates text with a prefix
struct PrefixFunction;

impl CustomFunction for PrefixFunction {
    fn name(&self) -> &str { "PREFIX" }
    fn min_args(&self) -> usize { 2 }
    fn max_args(&self) -> Option<usize> { Some(2) }
    
    fn execute(&self, args: Vec<Value>) -> Result<Value, Error> {
        let prefix = match &args[0] {
            Value::String(s) => s,
            _ => return Err(Error::new("PREFIX expects string as first argument", None)),
        };
        let text = match &args[1] {
            Value::String(s) => s,
            _ => return Err(Error::new("PREFIX expects string as second argument", None)),
        };
        Ok(Value::String(format!("{}{}", prefix, text)))
    }
}

#[test]
fn test_custom_function_registration() {
    let _lock = TEST_MUTEX.lock().unwrap();
    
    // Clean up any existing DOUBLE function first
    unregister_function("DOUBLE");
    
    // Register the custom function
    assert!(register_function(Box::new(DoubleFunction)).is_ok());
    
    // Test that it works in an expression
    let vars = HashMap::new();
    let result = evaluate_with_custom("DOUBLE(5)", &vars).unwrap();
    assert!(matches!(result, Value::Number(10.0)));
    
    // Clean up
    unregister_function("DOUBLE");
}

#[test]
fn test_custom_function_with_variables() {
    let _lock = TEST_MUTEX.lock().unwrap();
    
    // Clean up any existing DOUBLE function first
    unregister_function("DOUBLE");
    
    // Register the custom function
    assert!(register_function(Box::new(DoubleFunction)).is_ok());
    
    // Test with variables
    let mut vars = HashMap::new();
    vars.insert("x".to_string(), Value::Number(7.5));
    
    let result = evaluate_with_custom("DOUBLE(:x)", &vars).unwrap();
    assert!(matches!(result, Value::Number(15.0)));
    
    // Clean up
    unregister_function("DOUBLE");
}

#[test]
fn test_custom_function_priority_over_builtin() {
    let _lock = TEST_MUTEX.lock().unwrap();
    
    // Clean up any existing SUM function first
    unregister_function("SUM");
    
    // Register a custom function with same name as builtin
    struct CustomSum;
    impl CustomFunction for CustomSum {
        fn name(&self) -> &str { "SUM" }
        fn min_args(&self) -> usize { 2 }
        fn max_args(&self) -> Option<usize> { Some(2) }
        
        fn execute(&self, args: Vec<Value>) -> Result<Value, Error> {
            // Custom SUM that multiplies instead of adds
            let a = args[0].as_number().ok_or_else(|| Error::new("Expected number", None))?;
            let b = args[1].as_number().ok_or_else(|| Error::new("Expected number", None))?;
            Ok(Value::Number(a * b))
        }
    }
    
    // Register the custom function
    assert!(register_function(Box::new(CustomSum)).is_ok());
    
    // Test that custom function takes priority
    let vars = HashMap::new();
    let result = evaluate_with_custom("SUM(3, 4)", &vars).unwrap();
    // Should be 12 (multiplication) not 7 (addition)
    assert!(matches!(result, Value::Number(12.0)));
    
    // Clean up
    unregister_function("SUM");
}

#[test]
fn test_string_custom_function() {
    let _lock = TEST_MUTEX.lock().unwrap();
    
    // Clean up any existing PREFIX function first
    unregister_function("PREFIX");
    
    // Register the string function
    assert!(register_function(Box::new(PrefixFunction)).is_ok());
    
    // Test with string arguments
    let vars = HashMap::new();
    let result = evaluate_with_custom("PREFIX(\"Hello, \", \"World!\")", &vars).unwrap();
    if let Value::String(s) = result {
        assert_eq!(s, "Hello, World!");
    } else {
        panic!("Expected string result");
    }
    
    // Clean up
    unregister_function("PREFIX");
}

#[test]
fn test_custom_function_error_handling() {
    let _lock = TEST_MUTEX.lock().unwrap();
    
    // Clean up any existing DOUBLE function first
    unregister_function("DOUBLE");
    
    // Register the function
    assert!(register_function(Box::new(DoubleFunction)).is_ok());
    
    // Test with wrong argument type
    let vars = HashMap::new();
    let result = evaluate_with_custom("DOUBLE(\"hello\")", &vars);
    assert!(result.is_err());
    
    // Test with wrong number of arguments
    let result = evaluate_with_custom("DOUBLE(1, 2)", &vars);
    assert!(result.is_err());
    
    // Clean up
    unregister_function("DOUBLE");
}