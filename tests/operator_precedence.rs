//! D-10 / X-6 (BL-054): Excel operator precedence.
//! Negation > `%` (percent) > `^` > `* /` > `+ -` > `&` > comparisons, `^` left-associative.

use skillet::{evaluate, evaluate_with, Value};
use std::collections::HashMap;

fn num(src: &str) -> f64 {
    match evaluate(src) {
        Ok(Value::Number(n)) => n,
        other => panic!("{src}: expected a number, got {other:?}"),
    }
}

fn close(src: &str, want: f64) {
    let got = num(src);
    assert!((got - want).abs() < 1e-12, "{src}: {got} != {want}");
}

fn text(src: &str) -> String {
    match evaluate(src) {
        Ok(Value::String(s)) => s,
        other => panic!("{src}: expected a string, got {other:?}"),
    }
}

#[test]
fn negation_binds_tighter_than_power() {
    close("=-2^2", 4.0);
    close("-3 ^ 2", 9.0);
    close("-(2^2)", -4.0);
    close("0 - 2^2", -4.0);
    close("2^-1", 0.5);
    let mut vars = HashMap::new();
    vars.insert("a".to_string(), Value::Number(3.0));
    assert_eq!(evaluate_with("-:a^2", &vars).unwrap(), Value::Number(9.0));
}

#[test]
fn power_is_left_associative() {
    close("=2^3^2", 64.0);
    close("2^(3^2)", 512.0);
    close("2^-1^2", 0.25);
    close("4^0.5^2", 4.0);
}

#[test]
fn power_binds_tighter_than_multiplication() {
    close("2 * 3^2", 18.0);
    close("2^3 * 2", 16.0);
    close("-2^2 * 3", 12.0);
    close("10 - 2^2", 6.0);
}

#[test]
fn percent_suffix() {
    close("=10%", 0.1);
    close("50% * 2", 1.0);
    close("200 * 15%", 30.0);
    close("-10%", -0.1);
    close("50%%", 0.005);
    close("(25)%", 0.25);
    close("SUM(10%, 20%)", 0.3);
    close("[10%][0]", 0.1);
    close("TRUE ? 10% : 2", 0.1);
    close("FALSE ? 2 : 10%", 0.1);
}

#[test]
fn percent_binds_tighter_than_power() {
    close("2^50%", 2f64.sqrt());
    close("10%^2", 0.01);
    close("1 + 10%", 1.1);
}

#[test]
fn percent_followed_by_an_operand_is_still_modulo() {
    close("10 % 3", 1.0);
    close("10%3", 1.0);
    close("-10 % 3", -1.0);
    close("10 % -3", 1.0);
    close("7 % (2 + 2)", 3.0);
    assert_eq!(
        evaluate("FILTER([1, 2, 3, 4], :x % 2 == 0)").unwrap(),
        Value::Array(vec![Value::Number(2.0), Value::Number(4.0)])
    );
    let mut vars = HashMap::new();
    vars.insert("a".to_string(), Value::Number(9.0));
    vars.insert("b".to_string(), Value::Number(4.0));
    assert_eq!(evaluate_with(":a % :b", &vars).unwrap(), Value::Number(1.0));
}

#[test]
fn ampersand_joins_text() {
    assert_eq!(text("='a' & 'b'"), "ab");
    assert_eq!(text("'a' & 1 & TRUE"), "a1TRUE");
    assert_eq!(text("'x' & NULL"), "x");
    assert_eq!(text("1 & 2"), "12");
    assert_eq!(text("'n=' & 2.5"), "n=2.5");
}

#[test]
fn ampersand_binds_looser_than_arithmetic_and_tighter_than_comparison() {
    assert_eq!(text("1 + 2 & 3"), "33");
    assert_eq!(text("2 * 3 & 4 ^ 2"), "616");
    assert_eq!(evaluate("1 & 2 == '12'").unwrap(), Value::Boolean(true));
    assert_eq!(
        evaluate("'a' & 'b' != 'ab'").unwrap(),
        Value::Boolean(false)
    );
}

#[test]
fn ampersand_does_not_break_safe_navigation_or_logical_and() {
    let mut vars = HashMap::new();
    vars.insert("u".to_string(), Value::Null);
    vars.insert("s".to_string(), Value::String("hi".into()));
    assert_eq!(evaluate_with(":u&.name", &vars).unwrap(), Value::Null);
    assert_eq!(
        evaluate_with(":s&.upper() & '!'", &vars).unwrap(),
        Value::String("HI!".into())
    );
    assert_eq!(evaluate("TRUE && FALSE").unwrap(), Value::Boolean(false));
    assert!(evaluate("'a' &").is_err());
}

#[test]
fn unchanged_rows() {
    close("2 + 3 * 4", 14.0);
    close("(2 + 3) * 4", 20.0);
    close("10 - 4 - 3", 3.0);
    close("2^3", 8.0);
    assert_eq!(
        evaluate("1 + 1 == 2 && !FALSE").unwrap(),
        Value::Boolean(true)
    );
}
