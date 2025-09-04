use skillet::{evaluate_with_assignments, Value};
use std::collections::HashMap;

#[test]
fn test_safe_navigation_basic() {
    // Test safe navigation with existing property
    let expression = r#":json_obj := {"name": "John", "age": 30}; :json_obj&.name"#;
    let vars = HashMap::new();
    
    let result = evaluate_with_assignments(expression, &vars).unwrap();
    assert_eq!(result, Value::String("John".to_string()));
}

#[test]
fn test_safe_navigation_missing_property() {
    // Test safe navigation with missing property - should return null instead of error
    let expression = r#":json_obj := {"name": "John", "age": 30}; :json_obj&.missing_property"#;
    let vars = HashMap::new();
    
    let result = evaluate_with_assignments(expression, &vars).unwrap();
    assert_eq!(result, Value::Null);
}

#[test]
fn test_safe_navigation_on_null() {
    // Test safe navigation on null value - should return null instead of error
    let expression = r#":null_value := NULL; :null_value&.anything"#;
    let vars = HashMap::new();
    
    let result = evaluate_with_assignments(expression, &vars).unwrap();
    assert_eq!(result, Value::Null);
}

#[test]
fn test_safe_navigation_chained() {
    // Test chained safe navigation
    let expression = r#":json_obj := {"user": {"profile": {"name": "Jane"}}}; :json_obj&.user&.profile&.name"#;
    let vars = HashMap::new();
    
    let result = evaluate_with_assignments(expression, &vars).unwrap();
    assert_eq!(result, Value::String("Jane".to_string()));
}

#[test]
fn test_safe_navigation_chained_with_missing() {
    // Test chained safe navigation where intermediate property is missing
    let expression = r#":json_obj := {"user": {"profile": {"name": "Jane"}}}; :json_obj&.user&.missing&.name"#;
    let vars = HashMap::new();
    
    let result = evaluate_with_assignments(expression, &vars).unwrap();
    assert_eq!(result, Value::Null);
}

#[test]
fn test_regular_vs_safe_navigation() {
    // Compare regular property access vs safe navigation
    let vars = HashMap::new();
    
    // Regular property access should fail with missing property
    let regular_expression = r#":json_obj := {"name": "John"}; :json_obj.missing_property"#;
    let regular_result = evaluate_with_assignments(regular_expression, &vars);
    assert!(regular_result.is_err());
    
    // Safe navigation should return null
    let safe_expression = r#":json_obj := {"name": "John"}; :json_obj&.missing_property"#;
    let safe_result = evaluate_with_assignments(safe_expression, &vars).unwrap();
    assert_eq!(safe_result, Value::Null);
}

#[test]
fn test_safe_navigation_with_nested_json() {
    // Test safe navigation with deeply nested JSON
    let expression = r#":data := {
        "response": {
            "items": {
                "first": {
                    "metadata": "success"
                }
            }
        }
    }; :data&.response&.items&.first&.metadata"#;
    let vars = HashMap::new();
    
    let result = evaluate_with_assignments(expression, &vars).unwrap();
    assert_eq!(result, Value::String("success".to_string()));
}

#[test]
fn test_safe_navigation_mixed_with_regular() {
    // Test mixing safe navigation with regular property access
    let expression = r#":json_obj := {"user": {"name": "Alice"}}; :json_obj.user&.name"#;
    let vars = HashMap::new();
    
    let result = evaluate_with_assignments(expression, &vars).unwrap();
    assert_eq!(result, Value::String("Alice".to_string()));
}