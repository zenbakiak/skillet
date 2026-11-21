//! NaN in data must not panic the evaluator. Sorting needs an order and NaN has none, so every
//! builtin or method that sorts numbers returns the error "Cannot sort NaN values" instead.
//! `0/0` evaluates to NaN; these tests build NaN data from it or pass it in as a variable.
//!
//! Before the fix, `partial_cmp(..).unwrap()` panicked in MEDIAN, PERCENTILE_INC, QUARTILE_INC
//! and SORT. `.sort()` returned unsorted data for short arrays and panicked ("does not correctly
//! implement a total order") for arrays longer than 20.

use skillet::{evaluate, evaluate_with, Value};
use std::collections::HashMap;

const NAN_ERROR: &str = "Cannot sort NaN values";

fn assert_nan_error(expr: &str) {
    match evaluate(expr) {
        Err(e) => assert!(
            e.message.contains(NAN_ERROR),
            "{expr}: unexpected error {e:?}"
        ),
        Ok(v) => panic!("{expr}: expected NaN sort error, got {v:?}"),
    }
}

fn nums(v: Value) -> Vec<f64> {
    match v {
        Value::Array(items) => items
            .into_iter()
            .map(|i| match i {
                Value::Number(n) => n,
                other => panic!("expected number, got {other:?}"),
            })
            .collect(),
        other => panic!("expected array, got {other:?}"),
    }
}

/// 25 numbers with five NaNs; long enough for the std sort to detect a broken total order.
fn long_nan_array() -> String {
    let items: Vec<String> = (1..=25)
        .map(|i| {
            if i % 5 == 0 {
                "0/0".to_string()
            } else {
                i.to_string()
            }
        })
        .collect();
    format!("[{}]", items.join(", "))
}

#[test]
fn regression_sec_nan_sort_median() {
    assert_nan_error("=MEDIAN([1, 0/0, 3])");
    assert_nan_error("=MEDIAN(1, 0/0, 3)");
}

#[test]
fn regression_sec_nan_sort_percentile_inc() {
    assert_nan_error("=PERCENTILE_INC([1, 0/0, 3], 0.5)");
    assert_nan_error("=PERCENTILEINC([1, 0/0, 3], 0.5)");
}

#[test]
fn regression_sec_nan_sort_quartile_inc() {
    assert_nan_error("=QUARTILE_INC([1, 0/0, 3], 2)");
    assert_nan_error("=QUARTILEINC([1, 0/0, 3], 2)");
}

#[test]
fn regression_sec_nan_sort_sort_function() {
    assert_nan_error("=SORT([3, 0/0, 1])");
    assert_nan_error("=SORT([3, 0/0, 1], \"DESC\")");
    assert_nan_error(&format!("=SORT({})", long_nan_array()));
}

#[test]
fn regression_sec_nan_sort_sort_method() {
    assert_nan_error("=[3, 0/0, 1].sort()");
    assert_nan_error("=[3, 0/0, 1].sort(\"desc\")");
    assert_nan_error(&format!("={}.sort()", long_nan_array()));
    assert_nan_error(&format!("={}.sort(\"desc\")", long_nan_array()));
}

/// Method call inside a lambda body (goes through the method/lambda evaluator).
#[test]
fn regression_sec_nan_sort_sort_method_in_lambda() {
    match evaluate("=[[3, 0/0, 1]].map(:x.sort())") {
        Err(e) => assert!(e.message.contains(NAN_ERROR), "unexpected error {e:?}"),
        Ok(v) => panic!("expected NaN sort error, got {v:?}"),
    }
}

/// NaN arriving through a variable rather than an expression literal.
#[test]
fn regression_sec_nan_sort_from_variable() {
    let mut vars = HashMap::new();
    vars.insert(
        "data".to_string(),
        Value::Array(vec![
            Value::Number(3.0),
            Value::Number(f64::NAN),
            Value::Number(1.0),
        ]),
    );
    for expr in [
        "=MEDIAN(:data)",
        "=PERCENTILE_INC(:data, 0.5)",
        "=QUARTILE_INC(:data, 2)",
        "=SORT(:data)",
        "=:data.sort()",
    ] {
        match evaluate_with(expr, &vars) {
            Err(e) => assert!(
                e.message.contains(NAN_ERROR),
                "{expr}: unexpected error {e:?}"
            ),
            Ok(v) => panic!("{expr}: expected NaN sort error, got {v:?}"),
        }
    }
}

/// NaN-free data sorts exactly as before.
#[test]
fn nan_sort_fix_keeps_nan_free_results() {
    assert_eq!(
        nums(evaluate("=SORT([3, 1, 2])").unwrap()),
        vec![1.0, 2.0, 3.0]
    );
    assert_eq!(
        nums(evaluate("=SORT([3, 1, 2], \"DESC\")").unwrap()),
        vec![3.0, 2.0, 1.0]
    );
    assert_eq!(
        nums(evaluate("=[3, 1, 2].sort()").unwrap()),
        vec![1.0, 2.0, 3.0]
    );
    assert_eq!(
        nums(evaluate("=[3, 1, 2].sort(\"desc\")").unwrap()),
        vec![3.0, 2.0, 1.0]
    );
    assert_eq!(
        evaluate("=MEDIAN([5, 1, 3, 2])").unwrap(),
        Value::Number(2.5)
    );
    assert_eq!(
        evaluate("=PERCENTILE_INC([4, 1, 3, 2], 0.5)").unwrap(),
        Value::Number(2.5)
    );
    assert_eq!(
        evaluate("=QUARTILE_INC([4, 1, 3, 2], 1)").unwrap(),
        Value::Number(1.75)
    );
    // Infinities are ordered and still sort.
    assert_eq!(
        nums(evaluate("=SORT([1/0, 1, -1/0])").unwrap()),
        vec![f64::NEG_INFINITY, 1.0, f64::INFINITY]
    );
}
