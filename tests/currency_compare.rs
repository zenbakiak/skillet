//! BL-082, ADR-0027 Amendment 1: `==`, `!=`, `<`, `<=`, `>`, `>=` compare a Currency with a
//! Number by amount, in either order. Every other cross-type comparison keeps its ADR-0023
//! result. One test per row of the amendment's table, plus the operators inside lambdas.

use skillet::{evaluate, Value};

fn v(src: &str) -> Value {
    evaluate(src).unwrap_or_else(|e| panic!("{src}: {e:?}"))
}

fn b(src: &str) -> bool {
    match v(src) {
        Value::Boolean(b) => b,
        other => panic!("{src}: expected a boolean, got {other:?}"),
    }
}

fn all(cases: &[(&str, bool)]) {
    for (src, want) in cases {
        assert_eq!(b(src), *want, "{src}");
    }
}

fn incompatible(src: &str) {
    let err = evaluate(src).expect_err(src);
    assert!(
        err.message.contains("Comparison of incompatible types"),
        "{src}: {}",
        err.message
    );
}

/// The amount of a Number or Currency result (its type is checked elsewhere).
fn amount(src: &str) -> f64 {
    match v(src) {
        Value::Number(n) | Value::Currency(n) => n,
        other => panic!("{src}: expected a number, got {other:?}"),
    }
}

#[test]
fn aggregate_results_compare_with_numbers() {
    all(&[
        ("SUM(10::Currency, 5) > 10", true),
        ("SUM(10::Currency, 5) == 15", true),
        ("SUM(10::Currency,5) == 15", true),
        ("15 == SUM(10::Currency, 5)", true),
        ("MAX(1, 3::Currency) == 3", true),
        ("(10::Currency * 2) > 15", true),
        ("AVERAGE(10::Currency, 20) <= 15", true),
    ]);
    assert_eq!(
        v(r#"IF(SUM(1::Currency, 2) >= 3, "over", "under")"#),
        Value::String("over".into())
    );
}

#[test]
fn equality_by_amount_in_both_orders() {
    all(&[
        ("15 == 15::Currency", true),
        ("15::Currency == 15", true),
        ("15 == 16::Currency", false),
        ("0.5::Currency == 0.5", true),
        ("1 != 1::Currency", false),
        ("1::Currency != 1", false),
        ("1 != 2::Currency", true),
        ("2::Currency != 1", true),
    ]);
}

#[test]
fn ordering_by_amount_in_both_orders() {
    all(&[
        ("1::Currency < 2", true),
        ("1 < 2::Currency", true),
        ("2::Currency < 1", false),
        ("2 <= 2::Currency", true),
        ("2::Currency <= 2", true),
        ("3::Currency > 2", true),
        ("3 > 2::Currency", true),
        ("2 >= 2::Currency", true),
        ("2::Currency >= 3", false),
        ("-1::Currency < 0", true),
    ]);
}

#[test]
fn arrays_compare_items_by_amount() {
    all(&[
        ("[1::Currency, 2] == [1, 2::Currency]", true),
        ("[1::Currency, 2] != [1, 2::Currency]", false),
        ("[1::Currency] == [2]", false),
        ("[[1::Currency]] == [[1]]", true),
    ]);
}

#[test]
fn lambdas_use_the_same_comparison() {
    assert_eq!(
        v("[10::Currency, 5].filter(:x > 6)"),
        Value::Array(vec![Value::Currency(10.0)])
    );
    assert_eq!(
        v("FILTER([1::Currency, 3], :x >= 2)"),
        Value::Array(vec![Value::Number(3.0)])
    );
    assert_eq!(v("[5::Currency, 1].find(:x < 2)"), Value::Number(1.0));
    assert_eq!(v("[5, 1::Currency].find(:x == 1)"), Value::Currency(1.0));
    assert_eq!(
        v("[1, 2::Currency, 3].map(:x > 1)"),
        v("[FALSE, TRUE, TRUE]")
    );
    // Lambda criteria in the conditional aggregates.
    assert_eq!(amount("SUMIF([1, 2::Currency], :x > 1)"), 2.0);
    assert_eq!(amount("COUNTIF([1, 2::Currency], :x > 1)"), 1.0);
    assert_eq!(amount("COUNTIF([1, 2::Currency, 3], :x == 2)"), 1.0);
    assert_eq!(amount("AVERAGEIF([1::Currency, 3, 5], :x >= 3)"), 4.0);
}

#[test]
fn text_criteria_are_unchanged() {
    assert_eq!(amount(r#"COUNTIF([10::Currency, 5], ">6")"#), 1.0);
    assert_eq!(amount(r#"SUMIF([10::Currency, 5], ">6")"#), 10.0);
}

#[test]
fn min_max_and_sort_are_unchanged() {
    assert_eq!(v("MIN(2::Currency, 1)"), Value::Currency(1.0));
    assert_eq!(v("MAX(1, 3::Currency)"), Value::Currency(3.0));
    assert_eq!(
        v("SORT([3, 1::Currency, 2])"),
        Value::Array(vec![
            Value::Currency(1.0),
            Value::Number(2.0),
            Value::Number(3.0)
        ])
    );
    // Methods stay strict (ADR-0027 decision 3).
    assert!(evaluate("[1::Currency, 2].sort()").is_err());
}

#[test]
fn currency_against_non_numbers_is_unchanged() {
    for other in [r#""15""#, "TRUE", "NULL", "DATE(2026,1,1)", "[15]"] {
        assert!(
            !b(&format!("15::Currency == {other}")),
            "15::Currency == {other}"
        );
        assert!(
            !b(&format!("{other} == 15::Currency")),
            "{other} == 15::Currency"
        );
        assert!(
            b(&format!("15::Currency != {other}")),
            "15::Currency != {other}"
        );
        incompatible(&format!("15::Currency < {other}"));
        incompatible(&format!("{other} >= 15::Currency"));
    }
    assert!(!b("0::Currency == NULL"));
    assert!(!b("1::Currency == TRUE"));
}

#[test]
fn membership_and_unique_are_unchanged() {
    assert!(!b("CONTAINS([1::Currency], 1)"));
    assert!(!b("IN([1::Currency], 1)"));
    assert!(!b("[1::Currency].includes(1)"));
    assert_eq!(
        v("UNIQUE([1, 1::Currency])"),
        Value::Array(vec![Value::Number(1.0), Value::Currency(1.0)])
    );
}

#[test]
fn nan_currency_behaves_like_nan() {
    // NaN only reaches Skillet from a host variable.
    let mut vars = std::collections::HashMap::new();
    vars.insert("c".to_string(), Value::Currency(f64::NAN));
    let eval = |src: &str| skillet::evaluate_with(src, &vars);
    assert_eq!(eval(":c == 1").unwrap(), Value::Boolean(false));
    assert_eq!(eval("1 != :c").unwrap(), Value::Boolean(true));
    assert!(eval(":c < 1").is_err());
}
