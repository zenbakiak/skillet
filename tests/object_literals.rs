use skillet::{evaluate_with_assignments, Value};
use std::collections::HashMap;

#[test]
fn test_simple_object_literal() {
    let vars = HashMap::new();
    let result = evaluate_with_assignments(":obj := {a: 1, b: 2}", &vars).unwrap();
    
    match result {
        Value::Json(json_str) => {
            let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
            assert_eq!(parsed["a"], 1.0);
            assert_eq!(parsed["b"], 2.0);
        }
        _ => panic!("Expected Json value"),
    }
}

#[test]
fn test_nested_object_literal() {
    let vars = HashMap::new();
    let result = evaluate_with_assignments(":obj := {attrs: {a: [1,2,3,4]}}", &vars).unwrap();
    
    match result {
        Value::Json(json_str) => {
            let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
            assert_eq!(parsed["attrs"]["a"], serde_json::json!([1.0,2.0,3.0,4.0]));
        }
        _ => panic!("Expected Json value"),
    }
}

#[test]
fn test_object_with_quoted_keys() {
    let vars = HashMap::new();
    let result = evaluate_with_assignments(r#":obj := {"attrs": {"a": [1,2,3,4]}}"#, &vars).unwrap();
    
    match result {
        Value::Json(json_str) => {
            let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
            assert_eq!(parsed["attrs"]["a"], serde_json::json!([1.0,2.0,3.0,4.0]));
        }
        _ => panic!("Expected Json value"),
    }
}

#[test]
fn test_array_of_objects() {
    let vars = HashMap::new();
    let result = evaluate_with_assignments(":table := [{a: 1, b: 2, c: 3}, {a: 2, b: 3, c: 5}]", &vars).unwrap();
    
    match result {
        Value::Array(arr) => {
            assert_eq!(arr.len(), 2);
            for item in arr {
                match item {
                    Value::Json(_) => {}, // Expected
                    _ => panic!("Expected Json objects in array"),
                }
            }
        }
        _ => panic!("Expected Array value"),
    }
}

#[test]
fn test_object_with_variables() {
    let mut vars = HashMap::new();
    vars.insert("x".to_string(), Value::Number(10.0));
    vars.insert("y".to_string(), Value::Number(20.0));
    
    let result = evaluate_with_assignments(":obj := {sum: :x + :y, product: :x * :y}", &vars).unwrap();
    
    match result {
        Value::Json(json_str) => {
            let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
            assert_eq!(parsed["sum"], 30.0);
            assert_eq!(parsed["product"], 200.0);
        }
        _ => panic!("Expected Json value"),
    }
}

#[test]
fn test_empty_object() {
    let vars = HashMap::new();
    let result = evaluate_with_assignments(":obj := {}", &vars).unwrap();
    
    match result {
        Value::Json(json_str) => {
            let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
            assert!(parsed.is_object());
            assert_eq!(parsed.as_object().unwrap().len(), 0);
        }
        _ => panic!("Expected Json value"),
    }
}

#[test]
fn test_object_property_access() {
    let vars = HashMap::new();
    let result = evaluate_with_assignments(":obj := {name: \"test\", value: 42}; :obj.name", &vars).unwrap();
    
    match result {
        Value::String(s) => assert_eq!(s, "test"),
        _ => panic!("Expected String value"),
    }
}