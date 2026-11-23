//! BL-081: a lambda criterion in `COUNTIF`/`SUMIF`/`AVERAGEIF` is always a per-item predicate
//! over `:x`, even when the caller also defines `x`. Before the fix the criterion was evaluated
//! once in the caller's scope first, so `COUNTIF([5, 5, 1], :x)` with `x = 5` counted the 5s
//! instead of failing like it does without `x`.

use skillet::{evaluate_with, Value};
use std::collections::HashMap;
use std::process::Command;

const SK_BIN: &str = env!("CARGO_BIN_EXE_sk");

fn eval(expr: &str, vars: &[(&str, Value)]) -> Result<Value, String> {
    let vars: HashMap<String, Value> = vars
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect();
    evaluate_with(expr, &vars).map_err(|e| e.message)
}

/// Same result with no outer `x`, and with `x` set to a number and to a string.
fn same_with_outer_x(expr: &str, want: Result<Value, String>) {
    assert_eq!(eval(expr, &[]), want, "{expr} without x");
    for x in [
        Value::Number(5.0),
        Value::Number(3.0),
        Value::String("a*".into()),
    ] {
        assert_eq!(
            eval(expr, &[("x", x.clone())]),
            want,
            "{expr} with x = {x:?}"
        );
    }
}

#[test]
fn lambda_criteria_ignore_an_outer_x() {
    let n = |v: f64| Ok(Value::Number(v));
    same_with_outer_x("COUNTIF([5, 5, 1], :x == 5)", n(2.0));
    same_with_outer_x("COUNTIF([1, 2, 3, 4], :x % 2 == 0)", n(2.0));
    same_with_outer_x("COUNTIF([\"ab\", \"b\"], :x == \"b\")", n(1.0));
    same_with_outer_x("COUNTIF([1, 2, 3], IF(:x == 2, NULL, TRUE))", n(2.0));
    same_with_outer_x("SUMIF([1, 2, 3, 4], :x > 2)", n(7.0));
    same_with_outer_x("SUMIF([1, 2, 3], :x > 1, [10, 20, 30])", n(50.0));
    same_with_outer_x("AVERAGEIF([2, 4, 6], :x >= 4)", n(5.0));
    same_with_outer_x("AVGIF([2, 4, 6], :x >= 4, [1, 2, 3])", n(2.5));
}

#[test]
fn lambda_criteria_that_return_a_number_ignore_an_outer_x() {
    // A number is a condition, non-zero TRUE (ADR-0029 amendment 1, BL-089).
    let n = |v: f64| Ok(Value::Number(v));
    same_with_outer_x("COUNTIF([5, 0, 1], :x)", n(2.0));
    same_with_outer_x("COUNTIF([1, 2, 3, 4], :x % 2)", n(2.0));
    same_with_outer_x("SUMIF([5, 0, 1], :x)", n(6.0));
    same_with_outer_x("AVERAGEIF([5, 0, 1], :x * 1)", n(3.0));
}

#[test]
fn lambda_criteria_that_return_text_still_fail_with_an_outer_x() {
    let err = |f: &str| {
        Err(format!(
            "{f} condition must be TRUE, FALSE or a number, got string (#VALUE!)"
        ))
    };
    same_with_outer_x("COUNTIF([5, 5, 1], :x & '')", err("COUNTIF"));
    same_with_outer_x("SUMIF([5, 5, 1], :x & '')", err("SUMIF"));
    same_with_outer_x("AVERAGEIF([5, 5, 1], :x & '')", err("AVERAGEIF"));
}

#[test]
fn criteria_without_the_item_are_still_evaluated_once() {
    let lst = Value::Array(vec![
        Value::Number(1.0),
        Value::Number(2.0),
        Value::Number(3.0),
    ]);
    let crit = Value::String(">1".into());
    // A variable holding an Excel criterion, with or without an outer `x`.
    for vars in [
        vec![("c", crit.clone())],
        vec![("c", crit), ("x", Value::Number(9.0))],
    ] {
        assert_eq!(
            eval("COUNTIF([1, 2, 3], :c)", &vars),
            Ok(Value::Number(2.0))
        );
    }
    // `:x` bound by a nested lambda does not make the criterion a lambda: the count of
    // items > 1 in `:lst` is 2, and one item of the range equals 2.
    for vars in [
        vec![("lst", lst.clone())],
        vec![("lst", lst), ("x", Value::Number(9.0))],
    ] {
        assert_eq!(
            eval("COUNTIF([1, 2, 3], :lst.filter(:x > 1).length())", &vars),
            Ok(Value::Number(1.0))
        );
        assert_eq!(
            eval("COUNTIF([1, 2, 3], FILTER(:lst, :x > 1).length())", &vars),
            Ok(Value::Number(1.0))
        );
    }
}

fn sk_json(expr: &str, json: &str) -> String {
    let out = Command::new(SK_BIN)
        .args([expr, "--json", json])
        .output()
        .expect("sk runs");
    let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&out.stderr);
    let err = stderr.lines().find(|l| l.starts_with("Error"));
    match err {
        Some(e) if stdout.is_empty() => e.to_string(),
        _ => stdout,
    }
}

#[test]
fn sk_json_lambda_criteria_ignore_an_outer_x() {
    let cases = [
        ("=COUNTIF([5, 5, 1], :x == 5)", "Number(2.0)"),
        ("=SUMIF([1, 2, 3, 4], :x > 2)", "Number(7.0)"),
        ("=AVERAGEIF([2, 4, 6], :x >= 4)", "Number(5.0)"),
    ];
    for (expr, want) in cases {
        for json in [r#"{}"#, r#"{"x": 5}"#, r#"{"x": "a*"}"#] {
            assert_eq!(sk_json(expr, json), want, "{expr} with {json}");
        }
    }
    let without = sk_json("=COUNTIF([5, 0, 1], :x)", "{}");
    assert_eq!(without, "Number(2.0)");
    assert_eq!(sk_json("=COUNTIF([5, 0, 1], :x)", r#"{"x": 5}"#), without);
}
