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
    // SPLIT and SUBSTITUTE/REPLACE
    match evaluate("SPLIT('a,b,c', ',')").unwrap() { Value::Array(v) => assert_eq!(v, vec![Value::String("a".into()), Value::String("b".into()), Value::String("c".into())]), _ => panic!() }
    // SUBSTITUTE replaces all occurrences of a substring
    assert_eq!(s(evaluate("SUBSTITUTE('foo bar foo', 'foo', 'baz')").unwrap()), "baz bar baz");
    // SUBSTITUTEM is an alias that replaces all occurrences
    assert_eq!(s(evaluate("SUBSTITUTEM('a-a-a', '-', '_')").unwrap()), "a_a_a");

    // REPLACE is Excel-style positional replacement (1-based start)
    assert_eq!(s(evaluate("REPLACE('abcdef', 3, 2, 'XY')").unwrap()), "abXYef");
    // Insert without removing when num_chars is 0
    assert_eq!(s(evaluate("REPLACE('abc', 1, 0, 'X')").unwrap()), "Xabc");
    // Replace to end if num exceeds length
    assert_eq!(s(evaluate("REPLACE('hello', 4, 10, 'X')").unwrap()), "helX");

    // Excel-like LEFT/RIGHT/MID
    assert_eq!(s(evaluate("LEFT('Hello', 2)").unwrap()), "He");
    assert_eq!(s(evaluate("LEFT('Hello')").unwrap()), "H");
    assert_eq!(s(evaluate("RIGHT('Hello', 3)").unwrap()), "llo");
    assert_eq!(s(evaluate("RIGHT('Hello')").unwrap()), "o");
    assert_eq!(s(evaluate("MID('Hello', 2, 3)").unwrap()), "ell");
    assert_eq!(s(evaluate("MID('Hello', 2)").unwrap()), "ello");
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
