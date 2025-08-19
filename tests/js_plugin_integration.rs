use skillet::{register_function, unregister_function, evaluate_with_custom, JavaScriptFunction, JSPluginLoader, Value, CustomFunction};
use std::collections::HashMap;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_js_function_basic() {
    // Clean up any existing DOUBLE function first
    unregister_function("DOUBLE");
    
    let js_code = r#"
        // @name: DOUBLE
        // @min_args: 1
        // @max_args: 1
        // @description: Doubles a number
        // @example: DOUBLE(5) returns 10
        
        function execute(args) {
            return args[0] * 2;
        }
    "#;

    let js_func = JavaScriptFunction::parse_js_function(js_code).unwrap();
    assert_eq!(js_func.name(), "DOUBLE");
    assert_eq!(js_func.min_args(), 1);
    assert_eq!(js_func.max_args(), Some(1));
    assert_eq!(js_func.description(), Some("Doubles a number"));
    assert_eq!(js_func.example(), Some("DOUBLE(5) returns 10"));

    // Test execution
    let result = js_func.execute(vec![Value::Number(5.0)]).unwrap();
    match result {
        Value::Number(n) => assert!((n - 10.0).abs() < 1e-9),
        _ => panic!("Expected number result"),
    }
}

#[test]
fn test_js_function_string_manipulation() {
    let js_code = r#"
        // @name: REVERSE
        // @min_args: 1
        // @max_args: 1
        
        function execute(args) {
            return args[0].split('').reverse().join('');
        }
    "#;

    let js_func = JavaScriptFunction::parse_js_function(js_code).unwrap();
    let result = js_func.execute(vec![Value::String("hello".to_string())]).unwrap();
    
    match result {
        Value::String(s) => assert_eq!(s, "olleh"),
        _ => panic!("Expected string result"),
    }
}

#[test]
fn test_js_function_array_processing() {
    let js_code = r#"
        // @name: ARRAYSUM
        // @min_args: 1
        // @max_args: 1
        
        function execute(args) {
            const array = args[0];
            return array.reduce((sum, item) => sum + item, 0);
        }
    "#;

    let js_func = JavaScriptFunction::parse_js_function(js_code).unwrap();
    let array = Value::Array(vec![
        Value::Number(1.0),
        Value::Number(2.0),
        Value::Number(3.0),
        Value::Number(4.0),
        Value::Number(5.0),
    ]);
    
    let result = js_func.execute(vec![array]).unwrap();
    match result {
        Value::Number(n) => assert!((n - 15.0).abs() < 1e-9),
        _ => panic!("Expected number result"),
    }
}

#[test]
fn test_js_function_multiple_args() {
    let js_code = r#"
        // @name: MULTIPLY
        // @min_args: 2
        // @max_args: 2
        
        function execute(args) {
            return args[0] * args[1];
        }
    "#;

    let js_func = JavaScriptFunction::parse_js_function(js_code).unwrap();
    let result = js_func.execute(vec![Value::Number(6.0), Value::Number(7.0)]).unwrap();
    
    match result {
        Value::Number(n) => assert!((n - 42.0).abs() < 1e-9),
        _ => panic!("Expected number result"),
    }
}

#[test]
fn test_js_function_unlimited_args() {
    let js_code = r#"
        // @name: SUMALL
        // @min_args: 1
        // @max_args: unlimited
        
        function execute(args) {
            return args.reduce((sum, arg) => sum + arg, 0);
        }
    "#;

    let js_func = JavaScriptFunction::parse_js_function(js_code).unwrap();
    assert_eq!(js_func.max_args(), None);
    
    let result = js_func.execute(vec![
        Value::Number(1.0),
        Value::Number(2.0),
        Value::Number(3.0),
        Value::Number(4.0),
    ]).unwrap();
    
    match result {
        Value::Number(n) => assert!((n - 10.0).abs() < 1e-9),
        _ => panic!("Expected number result"),
    }
}

#[test]
fn test_js_function_with_conditionals() {
    let js_code = r#"
        // @name: FIBONACCI
        // @min_args: 1
        // @max_args: 1
        
        function execute(args) {
            const n = args[0];
            if (n <= 1) return n;
            
            let a = 0, b = 1;
            for (let i = 2; i <= n; i++) {
                let temp = a + b;
                a = b;
                b = temp;
            }
            return b;
        }
    "#;

    let js_func = JavaScriptFunction::parse_js_function(js_code).unwrap();
    
    // Test fibonacci(10) = 55
    let result = js_func.execute(vec![Value::Number(10.0)]).unwrap();
    match result {
        Value::Number(n) => assert!((n - 55.0).abs() < 1e-9),
        _ => panic!("Expected number result"),
    }
}

#[test]
fn test_js_plugin_loader() {
    let temp_dir = TempDir::new().unwrap();
    let hooks_path = temp_dir.path().to_str().unwrap();

    // Create a test JavaScript file
    let js_content = r#"
        // @name: TESTFUNC
        // @min_args: 1
        // @max_args: 1
        // @description: Test function
        
        function execute(args) {
            return args[0] * 3;
        }
    "#;
    
    let js_file_path = temp_dir.path().join("test.js");
    fs::write(&js_file_path, js_content).unwrap();

    // Load functions from directory
    let loader = JSPluginLoader::new(hooks_path.to_string());
    let functions = loader.load_functions().unwrap();
    
    assert_eq!(functions.len(), 1);
    assert_eq!(functions[0].name(), "TESTFUNC");
}

#[test]
fn test_js_function_integration_with_skillet() {
    // Clean up any existing TRIPLE function first
    unregister_function("TRIPLE");
    
    let js_code = r#"
        // @name: TRIPLE
        // @min_args: 1
        // @max_args: 1
        
        function execute(args) {
            return args[0] * 3;
        }
    "#;

    let js_func = JavaScriptFunction::parse_js_function(js_code).unwrap();
    
    // Register the function
    register_function(Box::new(js_func)).unwrap();
    
    // Test integration with skillet evaluation
    let vars = HashMap::new();
    let result = evaluate_with_custom("TRIPLE(7)", &vars).unwrap();
    
    match result {
        Value::Number(n) => assert!((n - 21.0).abs() < 1e-9),
        _ => panic!("Expected number result"),
    }
    
    // Test with variables
    let mut vars = HashMap::new();
    vars.insert("x".to_string(), Value::Number(8.0));
    
    let result = evaluate_with_custom("TRIPLE(:x)", &vars).unwrap();
    match result {
        Value::Number(n) => assert!((n - 24.0).abs() < 1e-9),
        _ => panic!("Expected number result"),
    }
    
    // Clean up
    unregister_function("TRIPLE");
}

#[test]
fn test_object_keys_function() {
    let js_code = r#"
        // @name: OBJECT_KEYS
        // @min_args: 1
        // @max_args: 1
        
        function execute(args) {
            const obj = args[0];
            
            // Handle string input that might be JSON
            if (typeof obj === 'string') {
                try {
                    const parsed = JSON.parse(obj);
                    if (typeof parsed === 'object' && !Array.isArray(parsed) && parsed !== null) {
                        return Object.keys(parsed);
                    }
                } catch (e) {
                    return [];
                }
            }
            
            // Handle direct object input
            if (typeof obj === 'object' && !Array.isArray(obj) && obj !== null) {
                return Object.keys(obj);
            }
            return [];
        }
    "#;

    let js_func = JavaScriptFunction::parse_js_function(js_code).unwrap();
    
    // Test with a JSON string (this is how JSON values are passed to JS functions)
    let obj_str = r#"{"name": "John", "age": 30}"#;
    let obj_value = Value::Json(obj_str.to_string());
    
    let result = js_func.execute(vec![obj_value]).unwrap();
    match result {
        Value::Array(keys) => {
            assert_eq!(keys.len(), 2); // Should have "name" and "age"
            // Check that we have string keys
            for key in keys {
                assert!(matches!(key, Value::String(_)));
            }
        },
        _ => panic!("Expected array result"),
    }
}

#[test]
fn test_array_sort_function() {
    let js_code = r#"
        // @name: ARRAY_SORT
        // @min_args: 1
        // @max_args: 2
        
        function execute(args) {
            const array = args[0];
            const sortMode = args.length > 1 ? args[1] : "asc";
            
            if (!Array.isArray(array)) {
                throw new Error("Expected array");
            }
            
            const sorted = [...array];
            if (sortMode === "desc") {
                return sorted.sort().reverse();
            } else if (sortMode === "numeric") {
                return sorted.sort((a, b) => Number(a) - Number(b));
            }
            return sorted.sort();
        }
    "#;

    let js_func = JavaScriptFunction::parse_js_function(js_code).unwrap();
    
    // Test basic sorting
    let array = Value::Array(vec![
        Value::Number(3.0),
        Value::Number(1.0),
        Value::Number(4.0),
        Value::Number(1.0),
        Value::Number(5.0),
    ]);
    
    let result = js_func.execute(vec![array]).unwrap();
    match result {
        Value::Array(sorted) => {
            assert_eq!(sorted.len(), 5);
            // Check first and last elements
            if let (Value::Number(first), Value::Number(last)) = (&sorted[0], &sorted[4]) {
                assert!(*first <= *last); // Should be sorted ascending
            }
        },
        _ => panic!("Expected array result"),
    }
}

#[test]
fn test_js_function_error_handling() {
    let js_code = r#"
        // @name: ERRORTEST
        // @min_args: 1
        // @max_args: 1
        
        function execute(args) {
            if (args[0] < 0) {
                throw new Error("Negative numbers not allowed");
            }
            return args[0] * 2;
        }
    "#;

    let js_func = JavaScriptFunction::parse_js_function(js_code).unwrap();
    
    // Test normal execution
    let result = js_func.execute(vec![Value::Number(5.0)]).unwrap();
    match result {
        Value::Number(n) => assert!((n - 10.0).abs() < 1e-9),
        _ => panic!("Expected number result"),
    }
    
    // Test error case
    let result = js_func.execute(vec![Value::Number(-1.0)]);
    assert!(result.is_err());
}

#[test]
fn test_js_function_boolean_operations() {
    let js_code = r#"
        // @name: ISPOSITIVE
        // @min_args: 1
        // @max_args: 1
        
        function execute(args) {
            return args[0] > 0;
        }
    "#;

    let js_func = JavaScriptFunction::parse_js_function(js_code).unwrap();
    
    // Test positive number
    let result = js_func.execute(vec![Value::Number(5.0)]).unwrap();
    match result {
        Value::Boolean(b) => assert!(b),
        _ => panic!("Expected boolean result"),
    }
    
    // Test negative number
    let result = js_func.execute(vec![Value::Number(-5.0)]).unwrap();
    match result {
        Value::Boolean(b) => assert!(!b),
        _ => panic!("Expected boolean result"),
    }
}