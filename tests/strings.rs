use skillet::{evaluate, evaluate_with, Value};
use std::collections::HashMap;

fn s(v: Value) -> String { if let Value::String(s) = v { s } else { panic!("expected string") } }
fn n(v: Value) -> f64 { if let Value::Number(n) = v { n } else { panic!("expected number") } }

#[test]
fn string_literals_and_functions() {
    assert_eq!(s(evaluate("\"Hello\" ").unwrap()), "Hello");
    assert_eq!(s(evaluate("CONCAT(\"Hello, \", \"World\")").unwrap()), "Hello, World");
    assert_eq!(s(evaluate("UPPER(\"abc\")").unwrap()), "ABC");
    assert_eq!(s(evaluate("LOWER(\"AbC\")").unwrap()), "abc");
    assert_eq!(s(evaluate("TRIM(\"  hi  \")").unwrap()), "hi");
    assert_eq!(n(evaluate("LENGTH(\"hé\")").unwrap()), 2.0);
    // SPLIT and REPLACE
    match evaluate("SPLIT('a,b,c', ',')").unwrap() { Value::Array(v) => assert_eq!(v, vec![Value::String("a".into()), Value::String("b".into()), Value::String("c".into())]), _ => panic!() }
    assert_eq!(s(evaluate("REPLACE('foo bar foo', 'foo', 'baz')").unwrap()), "baz bar baz");
}

#[test]
fn string_methods_and_chain() {
    assert_eq!(s(evaluate("\"  john  \\t\".trim().upper() ").unwrap()), "JOHN");
    assert_eq!(s(evaluate("\"abc\".reverse() ").unwrap()), "cba");
}

#[test]
fn string_vars() {
    let mut vars = HashMap::new();
    vars.insert("name".to_string(), Value::String("Jane".to_string()));
    assert_eq!(s(evaluate_with("CONCAT(\"Hello, \", :name)", &vars).unwrap()), "Hello, Jane");
}
