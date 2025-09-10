use skillet::{evaluate, evaluate_with_assignments, Value};
use std::collections::HashMap;

fn s(v: Value) -> String { if let Value::String(s) = v { s } else { panic!("expected string") } }

#[test]
fn dig_function_basic() {
    // Build an object and use DIG to traverse
    let expr = r#":obj := {
        "user": { "name": "Jane", "posts": [{"title": "First"}, {"title": "Second"}] },
        "meta": { "active": true }
    }; DIG(:obj, ['user', 'posts', 1, 'title'])"#;
    let vars = HashMap::new();
    let result = evaluate_with_assignments(expr, &vars).unwrap();
    assert_eq!(result, Value::String("Second".to_string()));
}

#[test]
fn dig_function_default_when_missing() {
    let expr = r#":obj := {"a": {"b": 1}}; DIG(:obj, ['a','x'], 'fallback')"#;
    let vars = HashMap::new();
    let result = evaluate_with_assignments(expr, &vars).unwrap();
    assert_eq!(result, Value::String("fallback".to_string()));
}

#[test]
fn dig_method_and_safe_nav() {
    // Method form and safe navigation on null
    let expr = r#":obj := {"a": {"arr": [ {"v": "ok"} ] }}; 
        :obj.dig(['a','arr',0,'v'])"#;
    let vars = HashMap::new();
    let result = evaluate_with_assignments(expr, &vars).unwrap();
    assert_eq!(s(result), "ok");

    // Safe navigation when receiver is NULL
    let expr2 = r#":obj := NULL; :obj&.dig(['a','b'], 'def')"#;
    let result2 = evaluate_with_assignments(expr2, &vars).unwrap();
    assert_eq!(result2, Value::Null); // safe call short-circuits to NULL
}

