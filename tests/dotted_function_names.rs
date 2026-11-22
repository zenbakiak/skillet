//! D-4 / X-4 / B-9 (BL-054): Excel dotted function names (`STDEV.P(`) parse as calls, and do
//! not change `:var.method()` or property chains.

use skillet::{evaluate, evaluate_with, parse, Expr, Value};
use std::collections::HashMap;

fn num(src: &str) -> f64 {
    match evaluate(src) {
        Ok(Value::Number(n)) => n,
        other => panic!("{src}: expected a number, got {other:?}"),
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-12
}

const DATA: &str = "2, 4, 4, 4, 5, 5, 7, 9";

#[test]
fn dotted_names_parse_as_function_calls() {
    match parse("STDEV.P(1, 2)").unwrap() {
        Expr::FunctionCall { name, args } => {
            assert_eq!(name, "STDEV.P");
            assert_eq!(args.len(), 2);
        }
        other => panic!("not a call: {other:?}"),
    }
    // Case-insensitive like every builtin name.
    assert!(close(num(&format!("stdev.p({DATA})")), 2.0));
}

#[test]
fn population_functions_match_excel() {
    assert!(close(num(&format!("STDEV.P({DATA})")), 2.0));
    assert!(close(num(&format!("VAR.P({DATA})")), 4.0));
    assert!(close(num("VAR.P(1, 3)"), 1.0));
}

#[test]
fn sample_functions_match_excel() {
    // Excel: STDEV.S(2,4,4,4,5,5,7,9) = 2.1380899352994, VAR.S = 32/7.
    for name in ["STDEV.S", "STDEV_S", "STDEV"] {
        assert!(
            close(num(&format!("{name}({DATA})")), (32.0f64 / 7.0).sqrt()),
            "{name}"
        );
    }
    for name in ["VAR.S", "VAR_S", "VAR"] {
        assert!(close(num(&format!("{name}({DATA})")), 32.0 / 7.0), "{name}");
    }
    assert!(close(num("VAR.S([1, 2, 3, 4])"), 5.0 / 3.0));
    assert!(close(num("STDEV.S(1, [3])"), 2f64.sqrt()));
}

#[test]
fn sample_functions_need_two_numbers() {
    for src in ["STDEV.S()", "STDEV.S(5)", "VAR.S([7])", "VAR.S('a', 1)"] {
        let err = evaluate(src).expect_err(src);
        assert!(
            err.message.contains("at least two numbers"),
            "{src}: {}",
            err.message
        );
    }
}

#[test]
fn other_existing_dotted_names() {
    assert!(close(num("MODE.SNGL(1, 2, 2, 3)"), 2.0));
    assert!(close(num("PERCENTILE.INC([1, 2, 3, 4], 0.25)"), 1.75));
    assert!(close(
        num("QUARTILE.INC([1, 2, 3, 4, 5, 6, 7, 8], 1)"),
        2.75
    ));
}

#[test]
fn dotted_name_requires_touching_parts_and_a_call() {
    for src in [
        "STDEV .P(1, 2)",
        "STDEV. P(1, 2)",
        "STDEV.P",
        "STDEV.P + 1",
        "FOO.(1)",
    ] {
        assert!(evaluate(src).is_err(), "{src} should not parse");
    }
    let err = evaluate("NOPE.X(1)").unwrap_err();
    assert_eq!(err.message, "Unknown function: NOPE.X");
}

#[test]
#[allow(deprecated)] // `Value::Json` is deprecated until 0.8 (ADR-0030)
fn variables_methods_and_results_are_unaffected() {
    let mut vars = HashMap::new();
    vars.insert(
        "P".to_string(),
        Value::Array(vec![Value::Number(1.0), Value::Number(2.0)]),
    );
    vars.insert("u".to_string(), Value::Json(r#"{"name": "ann"}"#.into()));
    assert_eq!(
        evaluate_with(":P.sum()", &vars).unwrap(),
        Value::Number(3.0)
    );
    assert_eq!(
        evaluate_with(":P.SUM()", &vars).unwrap(),
        Value::Number(3.0)
    );
    assert_eq!(
        evaluate_with(":u.name.upper()", &vars).unwrap(),
        Value::String("ANN".into())
    );
    assert_eq!(evaluate("SUM(1, 2).abs()").unwrap(), Value::Number(3.0));
    assert!(close(num("STDEV.P(1, 3).round(0) + 1"), 2.0));
}
