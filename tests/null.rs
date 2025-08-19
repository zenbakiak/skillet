use skillet::{evaluate, evaluate_with, Value};
use std::collections::HashMap;

fn b(v: Value) -> bool { if let Value::Boolean(b) = v { b } else { panic!("expected bool") } }
fn n(v: Value) -> f64 { if let Value::Number(n) = v { n } else { panic!("expected number") } }
fn s(v: Value) -> String { if let Value::String(s) = v { s } else { panic!("expected string") } }

#[test]
fn null_literal_and_predicates() {
    assert!(b(evaluate("NULL.nil?").unwrap()));
    assert!(b(evaluate("NULL.blank?").unwrap()));
    assert!(!b(evaluate("\"\".present?").unwrap()));
    assert!(b(evaluate("\"hi\".present?").unwrap()));
}

#[test]
fn compact_and_length() {
    assert_eq!(n(evaluate("[1, NULL, 2, NULL].compact().length()").unwrap()), 2.0);
    assert_eq!(n(evaluate("LENGTH(NULL)").unwrap()), 0.0);
}

#[test]
fn concat_skips_null_and_equals_prefix() {
    assert_eq!(s(evaluate("=CONCAT('a', NULL, 'b')").unwrap()), "ab");
    let mut vars = HashMap::new();
    vars.insert("name".to_string(), Value::Null);
    assert_eq!(s(evaluate_with("=:name.blank? ? 'Anonymous' : :name.upper()", &vars).unwrap()), "Anonymous");
}
