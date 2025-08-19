use skillet::{evaluate, evaluate_with, Value};
use std::collections::HashMap;

fn approxv(v: Value, b: f64) -> bool { matches!(v, Value::Number(a) if (a - b).abs() < 1e-9) }

#[test]
fn precedence_and_parentheses() {
    assert!(approxv(evaluate("2 + 3 * 4").unwrap(), 14.0));
    assert!(approxv(evaluate("(2 + 3) * 4").unwrap(), 20.0));
}

#[test]
fn exponent_right_associative() {
    assert!(approxv(evaluate("2 ^ 3 ^ 2").unwrap(), 512.0));
}

#[test]
fn unary_and_equals_prefix() {
    assert!(approxv(evaluate("-3 ^ 2").unwrap(), -9.0));
    assert!(approxv(evaluate("(-3) ^ 2").unwrap(), 9.0));
    assert!(approxv(evaluate("= 10 + 20 * 3").unwrap(), 70.0));
}

#[test]
fn variables_and_sum_function() {
    let mut vars = HashMap::new();
    vars.insert("x".to_string(), Value::Number(3.0));
    vars.insert("y".to_string(), Value::Number(10.0));
    assert!(approxv(evaluate_with("=SUM(:x, 5, 7)", &vars).unwrap(), 15.0));
    assert!(approxv(evaluate_with(":y + SUM(1, 2) * 3", &vars).unwrap(), 19.0));
    assert!(approxv(evaluate_with("SUM()", &vars).unwrap(), 0.0));
}

#[test]
fn arrays_and_sum_arrays() {
    assert!(approxv(evaluate("SUM([1, 2, 3])").unwrap(), 6.0));
    let mut vars = HashMap::new();
    vars.insert("nums".to_string(), Value::Array(vec![Value::Number(1.0), Value::Number(4.0)]));
    assert!(approxv(evaluate_with("SUM(:nums, [5, 10])", &vars).unwrap(), 20.0));
}

#[test]
fn math_builtins() {
    assert!(approxv(evaluate("AVG(1, 2, 3, 4)").unwrap(), 2.5));
    assert!(approxv(evaluate("MIN([3, 5, 1, 9])").unwrap(), 1.0));
    assert!(approxv(evaluate("MAX(3, 5, 1, 9)").unwrap(), 9.0));
    assert!(approxv(evaluate("AVG([2, 4, 6])").unwrap(), 4.0));
    assert!(approxv(evaluate("SUM([2, 4, 6])").unwrap(), 12.0));
    assert!(approxv(evaluate("ROUND(3.14159, 2)").unwrap(), 3.14));
    assert!(approxv(evaluate("ABS(-10)").unwrap(), 10.0));
    assert!(approxv(evaluate("SQRT(9)").unwrap(), 3.0));
    assert!(approxv(evaluate("POW(2, 8)").unwrap(), 256.0));
    // LENGTH for arrays
    assert!(approxv(evaluate("LENGTH([1,2,3,4])").unwrap(), 4.0));
}

#[test]
fn comparisons_logical_ternary() {
    // Comparisons
    match evaluate("2 > 1").unwrap() { Value::Boolean(true) => {}, _ => panic!("expected true") }
    match evaluate("2 == 2").unwrap() { Value::Boolean(true) => {}, _ => panic!("expected true") }
    match evaluate("2 < 1").unwrap() { Value::Boolean(false) => {}, _ => panic!("expected false") }
    // Logical AND/OR
    match evaluate("(2 > 1) AND (1 < 2)").unwrap() { Value::Boolean(true) => {}, _ => panic!("expected true") }
    match evaluate("(2 > 3) || (1 < 2)").unwrap() { Value::Boolean(true) => {}, _ => panic!("expected true") }
    // NOT
    match evaluate("! (2 < 1)").unwrap() { Value::Boolean(true) => {}, _ => panic!("expected true") }
    // Ternary
    assert!(approxv(evaluate("1 < 2 ? 10 : 20").unwrap(), 10.0));
}

#[test]
fn chaining_methods_numeric_and_array() {
    // Numeric chain
    assert!(approxv(evaluate("(-3.7).abs().round(1)").unwrap(), 3.7));
    // Array accessors
    match evaluate("[1,2,3,4].length().positive?").unwrap() { Value::Boolean(true) => {}, _ => panic!("expected true") }
    assert!(approxv(evaluate("[1,2,2,3].unique().sort().sum() ").unwrap(), 6.0));
    assert!(approxv(evaluate("[5,1,9].min()").unwrap(), 1.0));
    assert!(approxv(evaluate("[5,1,9].max()").unwrap(), 9.0));
}

#[test]
fn array_index_and_slice() {
    use Value::*;
    assert!(approxv(evaluate("[10,20,30][0]").unwrap(), 10.0));
    assert!(approxv(evaluate("[10,20,30][-1]").unwrap(), 30.0));
    match evaluate("[1,2,3,4,5][1:3]").unwrap() { Value::Array(v) => assert_eq!(v, vec![Number(2.0), Number(3.0)]), _ => panic!() }
    match evaluate("[1,2,3,4][:2]").unwrap() { Value::Array(v) => assert_eq!(v, vec![Number(1.0), Number(2.0)]), _ => panic!() }
}

#[test]
fn type_casting_minimal() {
    use Value::*;
    assert!(approxv(evaluate("'42'::Integer").unwrap(), 42.0));
    assert!(matches!(evaluate("TRUE::Float").unwrap(), Number(1.0)));
    assert!(matches!(evaluate("0::Boolean").unwrap(), Value::Boolean(false)));
    match evaluate("123::String").unwrap() { Value::String(s) => assert_eq!(s, "123"), _ => panic!("expected string") }
}
