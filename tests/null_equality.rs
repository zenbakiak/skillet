//! ADR-0013 option A (BL-054): `NULL` equals only `NULL`; ordering with `NULL` is an error.

use skillet::{evaluate, evaluate_with, Value};
use std::collections::HashMap;

fn b(src: &str) -> bool {
    match evaluate(src) {
        Ok(Value::Boolean(b)) => b,
        other => panic!("{src}: expected a boolean, got {other:?}"),
    }
}

#[test]
fn null_equals_null() {
    assert!(b("NULL == NULL"));
    assert!(b("NULL = NULL"));
    assert!(!b("NULL != NULL"));
}

#[test]
fn null_is_unequal_to_every_non_null_value() {
    for other in ["0", "''", "FALSE", "TRUE", "1", "'x'", "[]", "[NULL]"] {
        assert!(!b(&format!("NULL == {other}")), "NULL == {other}");
        assert!(!b(&format!("{other} == NULL")), "{other} == NULL");
        assert!(b(&format!("NULL != {other}")), "NULL != {other}");
        assert!(b(&format!("{other} != NULL")), "{other} != NULL");
    }
}

#[test]
fn ordering_with_null_is_an_error() {
    for src in ["NULL < 1", "1 > NULL", "NULL >= NULL", "NULL <= NULL", "NULL < NULL"] {
        let err = evaluate(src).expect_err(src);
        assert!(err.message.contains("incompatible"), "{src}: {}", err.message);
    }
}

#[test]
fn filter_by_null_comparison() {
    assert_eq!(
        evaluate("[NULL, 1, 2].filter(:x != NULL)").unwrap(),
        Value::Array(vec![Value::Number(1.0), Value::Number(2.0)])
    );
    assert_eq!(
        evaluate("[NULL, 1, NULL].filter(:x == NULL).length()").unwrap(),
        Value::Number(2.0)
    );
    assert_eq!(
        evaluate("FILTER([NULL, 1, 2], :x != NULL)").unwrap(),
        Value::Array(vec![Value::Number(1.0), Value::Number(2.0)])
    );
}

#[test]
fn null_variable_compares_equal_to_null() {
    let mut vars = HashMap::new();
    vars.insert("u".to_string(), Value::Null);
    vars.insert("v".to_string(), Value::Number(3.0));
    assert_eq!(evaluate_with(":u == NULL", &vars).unwrap(), Value::Boolean(true));
    assert_eq!(evaluate_with(":v == NULL", &vars).unwrap(), Value::Boolean(false));
    assert_eq!(
        evaluate_with(":u == NULL ? 'missing' : 'set'", &vars).unwrap(),
        Value::String("missing".into())
    );
}
