//! BL-107: the aggregates split direct arguments from array items (ADR-0029 amendment 2).
//!
//! A direct argument (a literal, a variable, any sub-expression, one value of a spread) is read
//! as a value: number text and booleans convert, other text is `#VALUE!`, a JSON object is
//! `#VALUE!`, `NULL` is skipped. An array item is range data: only numbers (and dates for
//! `MIN`/`MAX`) are read. A NaN is `#NUM!`. `COUNT` counts and never errors (B3).
//!
//! One test per group of rows of the amendment's before -> after table, then the "unchanged,
//! to be pinned" list, the canonical names, the order of errors and the lambda case.

use skillet::{evaluate_with, Value};
use std::collections::HashMap;

/// `:n` = NaN, `:a` = `"x"`, `:t` = `"2"`, `:nil` = NULL, `:xs` = `[1, "2", TRUE]`.
fn vars() -> HashMap<String, Value> {
    let mut v = HashMap::new();
    v.insert("n".to_string(), Value::Number(f64::NAN));
    v.insert("a".to_string(), Value::String("x".to_string()));
    v.insert("t".to_string(), Value::String("2".to_string()));
    v.insert("nil".to_string(), Value::Null);
    v.insert(
        "xs".to_string(),
        Value::Array(vec![
            Value::Number(1.0),
            Value::String("2".to_string()),
            Value::Boolean(true),
        ]),
    );
    v
}

fn eval(expr: &str) -> Value {
    evaluate_with(expr, &vars()).unwrap_or_else(|e| panic!("{expr}: unexpected error {e:?}"))
}

fn num(expr: &str, want: f64) {
    assert_eq!(eval(expr), Value::Number(want), "{expr}");
}

fn err(expr: &str, message: &str) {
    match evaluate_with(expr, &vars()) {
        Err(e) => assert_eq!(e.message, message, "{expr}"),
        Ok(v) => panic!("{expr}: expected an error, got {v:?}"),
    }
}

fn err_contains(expr: &str, part: &str) {
    match evaluate_with(expr, &vars()) {
        Err(e) => assert!(e.message.contains(part), "{expr}: {}", e.message),
        Ok(v) => panic!("{expr}: expected an error, got {v:?}"),
    }
}

fn cannot_convert(name: &str, text: &str) -> String {
    format!("{name}: cannot convert {text:?} to a number (#VALUE!)")
}

// ---- The before -> after table ----

#[test]
fn sum_converts_direct_number_text_and_booleans() {
    num(r#"SUM(1, "2", TRUE)"#, 4.0);
    num(r#"SUM("2")"#, 2.0);
    num(r#"SUM(" 3 ", "1e2")"#, 103.0);
    num("SUM(1 == 1, 2 == 2)", 2.0);
    num("SUM(:t, 1)", 3.0);
}

#[test]
fn spread_items_are_direct_arguments() {
    num(r#"SUM(...[1, "2", TRUE])"#, 4.0);
    num("SUM(...:xs)", 4.0);
    num(r#"COUNT(...[1, "2"])"#, 2.0);
    // A spread array inside a spread argument is still an array: its values are array items.
    num(r#"SUM(...[[1, "2"]])"#, 1.0);
    err(r#"SUM(...[1, "x"])"#, &cannot_convert("SUM", "x"));
}

#[test]
fn spread_in_an_array_literal_gives_array_items() {
    // BL-106: `[...:xs]` builds an array, so its values are array items and stay skipped.
    num("SUM([...:xs])", 1.0);
    num(r#"SUM([...[1, "2", TRUE]])"#, 1.0);
    num("COUNT([...:xs])", 1.0);
}

#[test]
fn sum_rejects_direct_text_that_is_not_a_number() {
    err(r#"SUM(1, "x")"#, &cannot_convert("SUM", "x"));
    err(r#"SUM(1, "")"#, &cannot_convert("SUM", ""));
    err(r#"SUM("TRUE")"#, &cannot_convert("SUM", "TRUE"));
    err("SUM('a', TRUE)", &cannot_convert("SUM", "a"));
    err(r#"SUM("1,000")"#, &cannot_convert("SUM", "1,000"));
    err(r#"SUM("2026-01-01")"#, &cannot_convert("SUM", "2026-01-01"));
    err("SUM(:a, 1)", &cannot_convert("SUM", "x"));
}

#[test]
fn long_direct_text_is_cut_to_32_characters() {
    let long = "y".repeat(40);
    err(
        &format!("SUM({long:?})"),
        &format!(
            "SUM: cannot convert {:?}… to a number (#VALUE!)",
            "y".repeat(32)
        ),
    );
}

#[test]
fn a_direct_json_object_is_an_error() {
    err(
        r#"SUM({"a": 1}, 2)"#,
        "SUM expects numbers, got json (#VALUE!)",
    );
    err(
        r#"MAX({"a": 1})"#,
        "MAX expects numbers, got json (#VALUE!)",
    );
    err(
        r#"STDEV_P({"a": 1}, 1)"#,
        "STDEV.P expects numbers, got json (#VALUE!)",
    );
    // In an array it is skipped.
    num(r#"SUM([{"a": 1}, 2])"#, 2.0);
}

#[test]
fn nan_is_an_error_direct_or_in_an_array() {
    err("SUM(:n, 1)", "SUM value is not a number (#NUM!)");
    err("SUM([:n])", "SUM value is not a number (#NUM!)");
    err("PRODUCT(:n)", "PRODUCT value is not a number (#NUM!)");
    err("AVERAGE(:n)", "AVERAGE value is not a number (#NUM!)");
    err("MAX(:n, 1)", "MAX value is not a number (#NUM!)");
    err("MIN([:n, 1])", "MIN value is not a number (#NUM!)");
    err(
        "MODE_SNGL(:n, :n)",
        "MODE.SNGL value is not a number (#NUM!)",
    );
    err("STDEV_P(:n, 1)", "STDEV.P value is not a number (#NUM!)");
    err("STDEV_S([1, :n])", "STDEV.S value is not a number (#NUM!)");
    err("VAR_P(:n)", "VAR.P value is not a number (#NUM!)");
    err("VAR_S(1, :n)", "VAR.S value is not a number (#NUM!)");
}

#[test]
fn product_converts_direct_values() {
    num(r#"PRODUCT("2", 3)"#, 6.0);
    num(r#"MULTIPLY("2", 3)"#, 6.0);
    num("PRODUCT(FALSE, 5)", 0.0);
}

#[test]
fn average_converts_direct_values() {
    num(r#"AVERAGE(1, "2")"#, 1.5);
    num(r#"AVG(1, "2")"#, 1.5);
    num("AVERAGE(TRUE)", 1.0);
    err(r#"AVERAGE("x")"#, &cannot_convert("AVERAGE", "x"));
    err("AVERAGE('a')", &cannot_convert("AVERAGE", "a"));
}

#[test]
fn min_and_max_convert_direct_values() {
    num(r#"MAX(-1, "5")"#, 5.0);
    num("MIN(TRUE, 5)", 1.0);
    err(r#"MIN("x")"#, &cannot_convert("MIN", "x"));
}

#[test]
fn min_and_max_mix_converted_values_with_dates() {
    let mixed = "MAX cannot compare dates with numbers (#VALUE!)";
    err("MAX(DATE(2026,1,1), TRUE)", mixed);
    err(r#"MAX(DATE(2026,1,1), "5")"#, mixed);
    err(
        r#"MAX(DATE(2026,1,1), "2027-01-01")"#,
        &cannot_convert("MAX", "2027-01-01"),
    );
}

#[test]
fn count_counts_direct_number_text_and_booleans() {
    num(r#"COUNT(1, "2", TRUE)"#, 3.0);
    num("COUNT(FALSE)", 1.0);
    num(r#"COUNT(" 4 ")"#, 1.0);
    num("COUNT(:t)", 1.0);
    num("COUNT(...:xs)", 3.0);
    // Array items are counted only when they are numbers or dates.
    num("COUNT(:xs)", 1.0);
}

#[test]
fn the_statistics_convert_direct_values() {
    num(r#"MEDIAN(1, "9", "8")"#, 8.0);
    num("MODE_SNGL(TRUE, 1)", 1.0);
    num(r#"STDEV_P("2", "4")"#, 1.0);
    num("VAR_S(TRUE, FALSE)", 0.5);
    err("VAR.S('a', 1)", &cannot_convert("VAR.S", "a"));
    num(r#"PERCENTILE_INC(1, "3", 0.5)"#, 2.0);
    num(r#"QUARTILE_INC(1, "5", 1)"#, 2.0);
}

#[test]
fn a_lambda_calling_an_aggregate_uses_the_same_rule() {
    assert_eq!(
        eval(r#"[["2", 3]].map(SUM(...:x))"#),
        Value::Array(vec![Value::Number(5.0)])
    );
    assert_eq!(
        eval(r#"[["2", 3]].map(SUM(:x))"#),
        Value::Array(vec![Value::Number(3.0)])
    );
    assert_eq!(
        eval(r#"MAP([["2", 3]], SUM(...:x))"#),
        Value::Array(vec![Value::Number(5.0)])
    );
}

// ---- Unchanged, pinned ----

#[test]
fn array_items_stay_skipped() {
    num(r#"SUM([1, "2", TRUE, NULL])"#, 1.0);
    num("SUM(:xs)", 1.0);
    num(r#"SUM(["x"])"#, 0.0);
    num(r#"MIN(["a"])"#, 0.0);
    num(r#"MEDIAN([1, "9", TRUE])"#, 1.0);
    num(r#"SUM([:a, 1])"#, 1.0);
    err(
        r#"AVERAGE(["x", TRUE, NULL])"#,
        "AVERAGE of no numbers (#DIV/0!)",
    );
}

#[test]
fn null_stays_skipped() {
    num("SUM(NULL, 2)", 2.0);
    num("SUM(:nil, 1)", 1.0);
    num("SUM()", 0.0);
    num("MIN()", 0.0);
    num("MAX(NULL)", 0.0);
    err("AVERAGE(NULL)", "AVERAGE of no numbers (#DIV/0!)");
    num("COUNT(NULL)", 0.0);
}

#[test]
fn dates_are_unchanged() {
    err("SUM(DATE(2026,1,1))", "SUM does not accept dates (#VALUE!)");
    assert_eq!(
        eval("MAX(DATE(2026,1,1), DATE(2027,1,1))"),
        eval("DATE(2027,1,1)")
    );
}

#[test]
fn count_skips_what_it_skipped_before() {
    num(r#"COUNT(1, "x")"#, 1.0);
    num(r#"COUNT("a")"#, 0.0);
    num(r#"COUNT({"a": 1})"#, 0.0);
    num(r#"COUNT("2026-01-01")"#, 0.0);
    num(r#"COUNT(1, "hello", 3, 4)"#, 3.0);
    num("COUNT(:n)", 1.0);
    err_contains("COUNT()", "COUNT expects at least 1 argument");
    num(r#"COUNTA(1, "x", NULL, TRUE)"#, 3.0);
}

#[test]
fn order_statistics_keep_their_nan_and_k_errors() {
    err_contains("MEDIAN(:n)", "Cannot sort NaN values");
    err_contains("MEDIAN([1, :n, 3])", "Cannot sort NaN values");
    err_contains("PERCENTILE_INC(:n, 1, 0.5)", "Cannot sort NaN values");
    err_contains("QUARTILE_INC([1, :n], 1)", "Cannot sort NaN values");
    err_contains(r#"STDEV_S("1")"#, "needs at least two numbers");
    err_contains(
        r#"PERCENTILE_INC([1, 2], "0.5")"#,
        "percentile must be a number",
    );
}

#[test]
fn builtins_outside_the_rule_are_unchanged() {
    let npv = eval(r#"NPV(0.1, "100", TRUE, 100)"#);
    assert_eq!(npv, Value::Number(90.9090909090909));
    num(r#"SUMIF(["1", 2], ">0")"#, 2.0);
    err_contains(
        r#"[1, "2", TRUE].sum()"#,
        "sum method expects numeric array",
    );
    assert!(evaluate_with(r#"["2"].max()"#, &vars()).is_err());
    err_contains(r#"AND(...[TRUE, "x"])"#, "#VALUE!");
}

// ---- Names and order ----

#[test]
fn every_alias_names_the_canonical_function() {
    let cases: &[(&[&str], &str, &str)] = &[
        (&["SUM"], "SUM", ""),
        (&["PRODUCT", "MULTIPLY"], "PRODUCT", ""),
        (&["AVERAGE", "AVG"], "AVERAGE", ""),
        (&["MIN"], "MIN", ""),
        (&["MAX"], "MAX", ""),
        (&["MEDIAN"], "MEDIAN", ""),
        (
            &["MODE.SNGL", "MODE_SNGL", "MODESNGL", "MODE"],
            "MODE.SNGL",
            "",
        ),
        (&["STDEV.P", "STDEV_P", "STDEVP"], "STDEV.P", ""),
        (&["STDEV.S", "STDEV_S", "STDEV"], "STDEV.S", ""),
        (&["VAR.P", "VAR_P", "VARP"], "VAR.P", ""),
        (&["VAR.S", "VAR_S", "VAR"], "VAR.S", ""),
        (
            &[
                "PERCENTILE.INC",
                "PERCENTILE_INC",
                "PERCENTILEINC",
                "PERCENTILE",
            ],
            "PERCENTILE.INC",
            ", 0.5",
        ),
        (
            &["QUARTILE.INC", "QUARTILE_INC", "QUARTILEINC", "QUARTILE"],
            "QUARTILE.INC",
            ", 1",
        ),
    ];
    for (aliases, canonical, tail) in cases {
        for alias in *aliases {
            err(
                &format!(r#"{alias}(1, "x"{tail})"#),
                &cannot_convert(canonical, "x"),
            );
            err(
                &format!(r#"{alias}({{"a": 1}}{tail})"#),
                &format!("{canonical} expects numbers, got json (#VALUE!)"),
            );
        }
    }
}

#[test]
fn the_first_error_from_the_left_wins() {
    err(r#"SUM(1, "x", {"a": 1})"#, &cannot_convert("SUM", "x"));
    err(
        r#"SUM({"a": 1}, "x")"#,
        "SUM expects numbers, got json (#VALUE!)",
    );
    err(r#"SUM(:n, "x")"#, "SUM value is not a number (#NUM!)");
    err(r#"SUM("x", :n)"#, &cannot_convert("SUM", "x"));
    err(
        r#"SUM([1, [:n]], "x")"#,
        "SUM value is not a number (#NUM!)",
    );
    err(
        r#"SUM(DATE(2026,1,1), "x")"#,
        "SUM does not accept dates (#VALUE!)",
    );
    err(
        r#"AVERAGE("x", DATE(2026,1,1))"#,
        &cannot_convert("AVERAGE", "x"),
    );
    err(r#"MAX("y", "x")"#, &cannot_convert("MAX", "y"));
}

#[test]
fn k_and_quart_are_checked_before_the_data() {
    err_contains(
        r#"PERCENTILE_INC("x", 2)"#,
        "Percentile must be between 0 and 1",
    );
    err_contains(
        r#"PERCENTILE_INC("x", "0.5")"#,
        "percentile must be a number",
    );
    err_contains(r#"QUARTILE_INC("x", 9)"#, "Quartile must be 0-4");
    err(
        r#"QUARTILE_INC("x", 1)"#,
        &cannot_convert("QUARTILE.INC", "x"),
    );
}
