// Test JSONPath functionality as it would be used by the HTTP server
use std::collections::HashMap;
use skillet::{Value, evaluate_with_custom};

// This function simulates how the HTTP server processes JSONPath arguments
fn test_http_server_jsonpath_eval(expression: &str, arguments: HashMap<String, serde_json::Value>) -> Result<Value, String> {
    let mut vars = HashMap::new();

    // Add JSON data for JSONPath functions (like the HTTP server does)
    let json_str = serde_json::to_string(&arguments).unwrap_or_default();
    vars.insert("json_data".to_string(), Value::Json(json_str));

    // Convert arguments to Skillet Values (like the HTTP server does)
    for (key, value) in arguments {
        match skillet::json_to_value(value) {
            Ok(v) => {
                // Sanitize key like the HTTP server does
                let sanitized_key = key.chars()
                    .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
                    .collect::<String>();
                vars.insert(sanitized_key, v);
            }
            Err(e) => return Err(format!("Error converting variable '{}': {}", key, e)),
        }
    }

    // Evaluate with custom functions (like the HTTP server does)
    evaluate_with_custom(expression, &vars)
        .map_err(|e| e.to_string())
}

fn approx_eq(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn test_http_server_jsonpath_sum() {
    let mut arguments = HashMap::new();
    arguments.insert("accounts".to_string(), serde_json::json!([
        {"id": 1, "amount": 300.1},
        {"id": 4, "amount": 890.1}
    ]));

    let result = test_http_server_jsonpath_eval("SUM(JQ(:json_data, \"$.accounts[*].amount\"))", arguments).unwrap();

    if let Value::Number(sum) = result {
        assert!(approx_eq(sum, 1190.2));
    } else {
        panic!("Expected number result, got: {:?}", result);
    }
}

#[test]
fn test_http_server_jsonpath_avg() {
    let mut arguments = HashMap::new();
    arguments.insert("scores".to_string(), serde_json::json!([
        {"value": 85},
        {"value": 92},
        {"value": 78},
        {"value": 95}
    ]));

    let result = test_http_server_jsonpath_eval("AVG(JQ(:json_data, \"$.scores[*].value\"))", arguments).unwrap();

    if let Value::Number(avg) = result {
        assert!(approx_eq(avg, 87.5));
    } else {
        panic!("Expected number result, got: {:?}", result);
    }
}

#[test]
fn test_http_server_jsonpath_max() {
    let mut arguments = HashMap::new();
    arguments.insert("temperatures".to_string(), serde_json::json!([
        {"reading": 22.5},
        {"reading": 31.2},
        {"reading": 18.7},
        {"reading": 29.8}
    ]));

    let result = test_http_server_jsonpath_eval("MAX(JQ(:json_data, \"$.temperatures[*].reading\"))", arguments).unwrap();

    if let Value::Number(max_val) = result {
        assert!(approx_eq(max_val, 31.2));
    } else {
        panic!("Expected number result, got: {:?}", result);
    }
}

#[test]
fn test_http_server_jsonpath_min() {
    let mut arguments = HashMap::new();
    arguments.insert("prices".to_string(), serde_json::json!([
        {"cost": 15.99},
        {"cost": 8.50},
        {"cost": 23.75},
        {"cost": 12.30}
    ]));

    let result = test_http_server_jsonpath_eval("MIN(JQ(:json_data, \"$.prices[*].cost\"))", arguments).unwrap();

    if let Value::Number(min_val) = result {
        assert!(approx_eq(min_val, 8.5));
    } else {
        panic!("Expected number result, got: {:?}", result);
    }
}

#[test]
fn test_http_server_jsonpath_nested() {
    let mut arguments = HashMap::new();
    arguments.insert("departments".to_string(), serde_json::json!([
        {
            "name": "Sales",
            "employees": [
                {"salary": 50000},
                {"salary": 55000}
            ]
        },
        {
            "name": "Engineering",
            "employees": [
                {"salary": 75000},
                {"salary": 80000},
                {"salary": 70000}
            ]
        }
    ]));

    let result = test_http_server_jsonpath_eval("SUM(JQ(:json_data, \"$.departments[*].employees[*].salary\"))", arguments).unwrap();

    if let Value::Number(total_salary) = result {
        assert!(approx_eq(total_salary, 330000.0));
    } else {
        panic!("Expected number result, got: {:?}", result);
    }
}

#[test]
fn test_http_server_jsonpath_with_filter() {
    let mut arguments = HashMap::new();
    arguments.insert("products".to_string(), serde_json::json!([
        {"name": "A", "price": 10, "category": "electronics"},
        {"name": "B", "price": 20, "category": "books"},
        {"name": "C", "price": 30, "category": "electronics"},
        {"name": "D", "price": 15, "category": "books"}
    ]));

    let result = test_http_server_jsonpath_eval("SUM(JQ(:json_data, \"$.products[?(@.category == 'electronics')].price\"))", arguments).unwrap();

    if let Value::Number(electronics_total) = result {
        assert!(approx_eq(electronics_total, 40.0));
    } else {
        panic!("Expected number result, got: {:?}", result);
    }
}

#[test]
fn test_http_server_jsonpath_complex_expression() {
    let mut arguments = HashMap::new();
    arguments.insert("sales".to_string(), serde_json::json!([
        {"amount": 100},
        {"amount": 200}
    ]));
    arguments.insert("bonus".to_string(), serde_json::json!(50));

    let result = test_http_server_jsonpath_eval("SUM(JQ(:json_data, \"$.sales[*].amount\")) + :bonus", arguments).unwrap();

    if let Value::Number(total) = result {
        assert!(approx_eq(total, 350.0));
    } else {
        panic!("Expected number result, got: {:?}", result);
    }
}

#[test]
fn test_http_server_jsonpath_error_handling() {
    let mut arguments = HashMap::new();
    arguments.insert("data".to_string(), serde_json::json!({"valid": "data"}));

    let result = test_http_server_jsonpath_eval("SUM(JQ(:json_data, \"$.invalid[*].path\"))", arguments).unwrap();

    // Should still succeed but return 0 for invalid JSONPath that returns empty result
    if let Value::Number(sum) = result {
        assert!(approx_eq(sum, 0.0));
    } else {
        panic!("Expected number result, got: {:?}", result);
    }
}