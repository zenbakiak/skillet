use skillet::{evaluate_with_json, Value};
use std::collections::HashMap;

fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

#[test]
fn test_sum_with_jsonpath() {
    let json_params = r#"{
        "accounts": [
            {
                "id": 1,
                "amount": 300.1
            },
            {
                "id": 4,
                "amount": 890.1
            }
        ]
    }"#;

    // Test SUM with JSONPath
    let result = evaluate_with_json(
        r#"SUM(JQ(:arguments, "$.accounts[*].amount"))"#,
        json_params,
    )
    .unwrap();
    if let Value::Number(sum) = result {
        assert!(approx(sum, 1190.2));
    } else {
        panic!("Expected number result, got: {:?}", result);
    }
}

#[test]
fn test_sum_with_jsonpath_variables() {
    // Test using JSONPath with variables directly
    let mut vars = HashMap::new();
    vars.insert(
        "arguments".to_string(),
        Value::Json(
            r#"{
        "accounts": [
            {"amount": 100.0},
            {"amount": 200.0},
            {"amount": 300.0}
        ]
    }"#
            .to_string(),
        ),
    );

    let result =
        skillet::evaluate_with(r#"SUM(JQ(:arguments, "$.accounts[*].amount"))"#, &vars).unwrap();
    if let Value::Number(sum) = result {
        assert!(approx(sum, 600.0));
    } else {
        panic!("Expected number result, got: {:?}", result);
    }
}

#[test]
fn test_avg_with_jsonpath() {
    let json_params = r#"{
        "scores": [
            {"value": 85},
            {"value": 92},
            {"value": 78},
            {"value": 95}
        ]
    }"#;

    let result =
        evaluate_with_json(r#"AVG(JQ(:arguments, "$.scores[*].value"))"#, json_params).unwrap();
    if let Value::Number(avg) = result {
        assert!(approx(avg, 87.5));
    } else {
        panic!("Expected number result, got: {:?}", result);
    }
}

#[test]
fn test_max_with_jsonpath() {
    let json_params = r#"{
        "temperatures": [
            {"reading": 22.5},
            {"reading": 31.2},
            {"reading": 18.7},
            {"reading": 29.8}
        ]
    }"#;

    let result = evaluate_with_json(
        r#"MAX(JQ(:arguments, "$.temperatures[*].reading"))"#,
        json_params,
    )
    .unwrap();
    if let Value::Number(max_val) = result {
        assert!(approx(max_val, 31.2));
    } else {
        panic!("Expected number result, got: {:?}", result);
    }
}

#[test]
fn test_min_with_jsonpath() {
    let json_params = r#"{
        "prices": [
            {"cost": 15.99},
            {"cost": 8.50},
            {"cost": 23.75},
            {"cost": 12.30}
        ]
    }"#;

    let result =
        evaluate_with_json(r#"MIN(JQ(:arguments, "$.prices[*].cost"))"#, json_params).unwrap();
    if let Value::Number(min_val) = result {
        assert!(approx(min_val, 8.50));
    } else {
        panic!("Expected number result, got: {:?}", result);
    }
}

#[test]
fn test_nested_jsonpath() {
    let json_params = r#"{
        "departments": [
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
        ]
    }"#;

    let result = evaluate_with_json(
        r#"SUM(JQ(:arguments, "$.departments[*].employees[*].salary"))"#,
        json_params,
    )
    .unwrap();
    if let Value::Number(total_salary) = result {
        assert!(approx(total_salary, 330000.0));
    } else {
        panic!("Expected number result, got: {:?}", result);
    }
}

#[test]
fn test_jsonpath_with_filter() {
    let json_params = r#"{
        "products": [
            {"name": "A", "price": 10, "category": "electronics"},
            {"name": "B", "price": 20, "category": "books"},
            {"name": "C", "price": 30, "category": "electronics"},
            {"name": "D", "price": 15, "category": "books"}
        ]
    }"#;

    // Sum prices of electronics only
    let result = evaluate_with_json(
        r#"SUM(JQ(:arguments, "$.products[?(@.category == 'electronics')].price"))"#,
        json_params,
    )
    .unwrap();
    if let Value::Number(electronics_total) = result {
        assert!(approx(electronics_total, 40.0));
    } else {
        panic!("Expected number result, got: {:?}", result);
    }
}

// ADR-0014: the result shape follows the parsed path, never the match count.

fn jq(json: &str, path: &str) -> Value {
    evaluate_with_json(&format!("JQ(:arguments, \"{}\")", path), json).unwrap()
}

fn nums(xs: &[f64]) -> Value {
    Value::Array(xs.iter().map(|&n| Value::Number(n)).collect())
}

#[test]
fn jq_singular_path_returns_the_value() {
    assert_eq!(
        jq(r#"{"u":{"n":"Ann"}}"#, "$.u.n"),
        Value::String("Ann".into())
    );
    assert_eq!(jq(r#"{"a":[3,4]}"#, "$.a[0]"), Value::Number(3.0));
    assert_eq!(jq(r#"{"a":[3,4]}"#, "$['a'][1]"), Value::Number(4.0));
    // A singular path whose value is an array returns that array, not a wrapped one.
    assert_eq!(jq(r#"{"a":[3]}"#, "$.a"), nums(&[3.0]));
}

#[test]
fn jq_singular_path_without_match_returns_null() {
    assert_eq!(jq(r#"{"a":5}"#, "$.b"), Value::Null);
    assert_eq!(jq(r#"{"a":[3]}"#, "$.a[5]"), Value::Null);
    assert_eq!(jq(r#"{"a":{"b":1}}"#, "$.a.x.y"), Value::Null);
}

#[test]
fn jq_wildcard_always_returns_an_array() {
    assert_eq!(jq(r#"{"a":[]}"#, "$.a[*]"), nums(&[]));
    assert_eq!(jq(r#"{"a":[3]}"#, "$.a[*]"), nums(&[3.0]));
    assert_eq!(jq(r#"{"a":[3,4]}"#, "$.a[*]"), nums(&[3.0, 4.0]));
    // Methods work for a single row too.
    assert_eq!(
        evaluate_with_json(r#"JQ(:arguments, "$.a[*]").sum()"#, r#"{"a":[3]}"#).unwrap(),
        Value::Number(3.0)
    );
}

#[test]
fn jq_recursive_descent_returns_an_array() {
    assert_eq!(jq(r#"{"a":[{"p":1}]}"#, "$..p"), nums(&[1.0]));
    assert_eq!(jq(r#"{"a":[{"q":1}]}"#, "$..p"), nums(&[]));
}

#[test]
fn jq_filter_slice_and_union_return_arrays() {
    let rows = r#"{"r":[{"k":"x","v":1},{"k":"y","v":2}]}"#;
    assert_eq!(jq(rows, "$.r[?(@.k == 'x')].v"), nums(&[1.0]));
    assert_eq!(jq(r#"{"a":[3,4]}"#, "$.a[0:1]"), nums(&[3.0]));
    assert_eq!(jq(r#"{"a":[3,4]}"#, "$.a[0,1]"), nums(&[3.0, 4.0]));
}
