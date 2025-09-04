use skillet::{evaluate_with_assignments, Value};
use std::collections::HashMap;

#[test]
fn test_string_includes_method_basic() {
    // Test basic includes method functionality
    let expression = r#":text := "hello world"; :text.includes("world")"#;
    let vars = HashMap::new();
    
    let result = evaluate_with_assignments(expression, &vars).unwrap();
    assert_eq!(result, Value::Boolean(true));
}

#[test]
fn test_string_includes_method_not_found() {
    // Test includes method when substring is not found
    let expression = r#":text := "hello world"; :text.includes("xyz")"#;
    let vars = HashMap::new();
    
    let result = evaluate_with_assignments(expression, &vars).unwrap();
    assert_eq!(result, Value::Boolean(false));
}

#[test]
fn test_string_includes_method_empty_string() {
    // Test includes method with empty string (should return true)
    let expression = r#":text := "hello world"; :text.includes("")"#;
    let vars = HashMap::new();
    
    let result = evaluate_with_assignments(expression, &vars).unwrap();
    assert_eq!(result, Value::Boolean(true));
}

#[test]
fn test_string_includes_method_case_sensitive() {
    // Test that includes is case sensitive
    let expression = r#":text := "Hello World"; :text.includes("hello")"#;
    let vars = HashMap::new();
    
    let result = evaluate_with_assignments(expression, &vars).unwrap();
    assert_eq!(result, Value::Boolean(false));
    
    // Test with correct case
    let expression = r#":text := "Hello World"; :text.includes("Hello")"#;
    let result = evaluate_with_assignments(expression, &vars).unwrap();
    assert_eq!(result, Value::Boolean(true));
}

#[test]
fn test_string_includes_method_partial_match() {
    // Test includes with partial matches
    let expression = r#":text := "programming"; :text.includes("gram")"#;
    let vars = HashMap::new();
    
    let result = evaluate_with_assignments(expression, &vars).unwrap();
    assert_eq!(result, Value::Boolean(true));
}

#[test]
fn test_includes_function_basic() {
    // Test basic INCLUDES function functionality
    let expression = r#"INCLUDES("hello world", "world")"#;
    let vars = HashMap::new();
    
    let result = evaluate_with_assignments(expression, &vars).unwrap();
    assert_eq!(result, Value::Boolean(true));
}

#[test]
fn test_includes_function_not_found() {
    // Test INCLUDES function when substring is not found
    let expression = r#"INCLUDES("hello world", "xyz")"#;
    let vars = HashMap::new();
    
    let result = evaluate_with_assignments(expression, &vars).unwrap();
    assert_eq!(result, Value::Boolean(false));
}

#[test]
fn test_includes_function_empty_string() {
    // Test INCLUDES function with empty string
    let expression = r#"INCLUDES("hello world", "")"#;
    let vars = HashMap::new();
    
    let result = evaluate_with_assignments(expression, &vars).unwrap();
    assert_eq!(result, Value::Boolean(true));
}

#[test]
fn test_includes_function_case_sensitive() {
    // Test that INCLUDES is case sensitive
    let expression = r#"INCLUDES("Hello World", "hello")"#;
    let vars = HashMap::new();
    
    let result = evaluate_with_assignments(expression, &vars).unwrap();
    assert_eq!(result, Value::Boolean(false));
    
    // Test with correct case
    let expression = r#"INCLUDES("Hello World", "Hello")"#;
    let result = evaluate_with_assignments(expression, &vars).unwrap();
    assert_eq!(result, Value::Boolean(true));
}

#[test]
fn test_includes_function_with_variables() {
    // Test INCLUDES function with variables
    let expression = r#":text := "hello world"; :substring := "world"; INCLUDES(:text, :substring)"#;
    let vars = HashMap::new();
    
    let result = evaluate_with_assignments(expression, &vars).unwrap();
    assert_eq!(result, Value::Boolean(true));
}

#[test]
fn test_includes_function_invalid_arguments() {
    // Test INCLUDES function with invalid arguments
    let expression = r#"INCLUDES(123, "world")"#;
    let vars = HashMap::new();
    
    let result = evaluate_with_assignments(expression, &vars);
    assert!(result.is_err());
}

#[test]
fn test_includes_method_invalid_receiver() {
    // Test includes method with invalid receiver (non-string)
    let expression = r#":num := 123; :num.includes("1")"#;
    let vars = HashMap::new();
    
    let result = evaluate_with_assignments(expression, &vars);
    assert!(result.is_err());
}

#[test]
fn test_includes_method_invalid_argument() {
    // Test includes method with invalid argument (non-string)
    let expression = r#":text := "hello"; :text.includes(123)"#;
    let vars = HashMap::new();
    
    let result = evaluate_with_assignments(expression, &vars);
    assert!(result.is_err());
}

#[test]
fn test_includes_function_wrong_argument_count() {
    // Test INCLUDES function with wrong number of arguments
    let expression = r#"INCLUDES("hello")"#;
    let vars = HashMap::new();
    
    let result = evaluate_with_assignments(expression, &vars);
    assert!(result.is_err());
    
    // Test with too many arguments
    let expression = r#"INCLUDES("hello", "world", "extra")"#;
    let result = evaluate_with_assignments(expression, &vars);
    assert!(result.is_err());
}

#[test]
fn test_includes_chaining() {
    // Test chaining includes with other string methods
    let expression = r#":text := "Hello World"; :text.lower().includes("hello")"#;
    let vars = HashMap::new();
    
    let result = evaluate_with_assignments(expression, &vars).unwrap();
    assert_eq!(result, Value::Boolean(true));
}

#[test]
fn test_includes_in_conditional() {
    // Test includes method in conditional expressions
    let expression = r#":text := "programming"; :text.includes("gram") ? "found" : "not found""#;
    let vars = HashMap::new();
    
    let result = evaluate_with_assignments(expression, &vars).unwrap();
    assert_eq!(result, Value::String("found".to_string()));
}