//! ADR-0016 option A (BL-054): custom functions are visible in every sub-expression of a
//! custom-enabled entry point (builtin lambdas, method arguments, `:=` / `;` formulas), and
//! in none of a registry-free one. Uses a Rust `CustomFunction`, so it runs without plugins.

use skillet::{
    evaluate, evaluate_with, evaluate_with_assignments, evaluate_with_assignments_and_context,
    evaluate_with_custom, register_function, CustomFunction, Error, Value,
};
use std::collections::HashMap;
use std::sync::Once;

/// `TWICE(n)` = 2n.
struct Twice;
impl CustomFunction for Twice {
    fn name(&self) -> &str {
        "TWICE"
    }
    fn min_args(&self) -> usize {
        1
    }
    fn max_args(&self) -> Option<usize> {
        Some(1)
    }
    fn execute(&self, args: Vec<Value>) -> Result<Value, Error> {
        let n = args[0]
            .as_number()
            .ok_or_else(|| Error::new("TWICE expects a number", None))?;
        Ok(Value::Number(n * 2.0))
    }
}

/// Shadows the builtin `ABS`: always returns 100.
struct FakeAbs;
impl CustomFunction for FakeAbs {
    fn name(&self) -> &str {
        "ABS"
    }
    fn min_args(&self) -> usize {
        0
    }
    fn max_args(&self) -> Option<usize> {
        None
    }
    fn execute(&self, _args: Vec<Value>) -> Result<Value, Error> {
        Ok(Value::Number(100.0))
    }
}

fn setup() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        register_function(Box::new(Twice)).unwrap();
        register_function(Box::new(FakeAbs)).unwrap();
    });
}

fn custom(src: &str) -> Value {
    setup();
    evaluate_with_custom(src, &HashMap::new()).unwrap_or_else(|e| panic!("{src}: {}", e.message))
}

fn nums(xs: &[f64]) -> Value {
    Value::Array(xs.iter().map(|x| Value::Number(*x)).collect())
}

#[test]
fn builtin_lambdas_see_custom_functions() {
    assert_eq!(custom("MAP([1, 2], TWICE(:x))"), nums(&[2.0, 4.0]));
    assert_eq!(
        custom("FILTER([1, 2, 3], TWICE(:x) > 3)"),
        nums(&[2.0, 3.0])
    );
    assert_eq!(custom("FIND([1, 2, 3], TWICE(:x) > 3)"), Value::Number(2.0));
    assert_eq!(
        custom("REDUCE([1, 2], :acc + TWICE(:x), 0)"),
        Value::Number(6.0)
    );
    assert_eq!(
        custom("SUMIF([1, 2, 3], TWICE(:x) > 3)"),
        Value::Number(5.0)
    );
    assert_eq!(
        custom("AVGIF([1, 2, 3], TWICE(:x) > 3)"),
        Value::Number(2.5)
    );
    assert_eq!(
        custom("COUNTIF([1, 2, 3], TWICE(:x) > 3)"),
        Value::Number(2.0)
    );
}

#[test]
fn method_arguments_see_custom_functions() {
    assert_eq!(custom("(2.555).round(TWICE(1))"), Value::Number(2.56));
    assert_eq!(custom("[1.234].map(:x.round(TWICE(1)))"), nums(&[1.23]));
    assert_eq!(
        custom("[1].map(FILTER([1, 2], TWICE(:x) > 2))"),
        Value::Array(vec![nums(&[2.0])])
    );
    assert_eq!(custom("[1, 2, 3].map(TWICE(:x))"), nums(&[2.0, 4.0, 6.0]));
}

#[test]
fn shadowed_builtin_resolves_to_the_custom_function_everywhere() {
    assert_eq!(custom("ABS(-1)"), Value::Number(100.0));
    assert_eq!(custom("MAP([-1], ABS(:x))"), nums(&[100.0]));
    assert_eq!(custom("FILTER([1, 2], ABS(:x) > 50)"), nums(&[1.0, 2.0]));
    assert_eq!(custom("(2.71828).round(ABS(0) - 98)"), Value::Number(2.72));
}

#[test]
fn assignment_formulas_see_custom_functions() {
    setup();
    let vars = HashMap::new();
    assert_eq!(
        evaluate_with_assignments(":v := TWICE(3); :v + 1", &vars).unwrap(),
        Value::Number(7.0)
    );
    assert_eq!(
        evaluate_with_assignments(":v := 1; TWICE(:v)", &vars).unwrap(),
        Value::Number(2.0)
    );
    let (v, out) =
        evaluate_with_assignments_and_context(":a := TWICE(2); MAP([:a], TWICE(:x))", &vars)
            .unwrap();
    assert_eq!(v, nums(&[8.0]));
    assert_eq!(out.get("a"), Some(&Value::Number(4.0)));
}

#[test]
fn registry_free_entry_points_see_no_custom_functions() {
    setup();
    for src in [
        "TWICE(1)",
        "MAP([1], TWICE(:x))",
        "[1].map(TWICE(:x))",
        "(1.5).round(TWICE(0))",
    ] {
        let err = evaluate(src).expect_err(src);
        assert!(
            err.message.contains("Unknown function"),
            "{src}: {}",
            err.message
        );
        assert!(evaluate_with(src, &HashMap::new()).is_err(), "{src}");
    }
    // The builtin, not the registered shadow.
    assert_eq!(evaluate("MAP([-1], ABS(:x))").unwrap(), nums(&[1.0]));
}
