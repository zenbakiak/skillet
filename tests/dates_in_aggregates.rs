//! BL-090: dates in the aggregates, in sorting and in criteria (ADR-0031 amendment 2).
//!
//! One test per group of rows of the amendment's before -> after table, plus the rules of
//! B1-B8 the table does not list (aliases, order of checks, every criteria operator).

use skillet::{evaluate, Value};

fn eval(expr: &str) -> Value {
    evaluate(expr).unwrap_or_else(|e| panic!("{expr}: unexpected error {e:?}"))
}

fn err(expr: &str, message: &str) {
    match evaluate(expr) {
        Err(e) => assert_eq!(e.message, message, "{expr}"),
        Ok(v) => panic!("{expr}: expected an error, got {v:?}"),
    }
}

/// The DateTime of midnight UTC on a date.
fn d(y: i32, m: u32, day: u32) -> Value {
    eval(&format!("DATE({y}, {m}, {day})"))
}

fn n(x: f64) -> Value {
    Value::Number(x)
}

fn check(expr: &str, want: Value) {
    assert_eq!(eval(expr), want, "{expr}");
}

// ---- B1: MIN and MAX ----

#[test]
fn min_and_max_of_dates_are_dates() {
    check(
        "MAX(DATE(2026,1,1), DATE(2027,1,1))",
        Value::DateTime(1_798_761_600),
    );
    check("MIN([DATE(2026,1,1), DATE(2025,6,1)])", d(2025, 6, 1));
    check("MAX([DATE(2026,1,1), [DATE(2027,3,1)]])", d(2027, 3, 1));
    check("MIN(DATE(2027,1,1), [[DATE(2026,1,1)]])", d(2026, 1, 1));
}

#[test]
fn min_and_max_ignore_what_they_ignored_before() {
    check("MAX(DATE(2026,1,1), NULL)", d(2026, 1, 1));
    check(r#"MAX(DATE(2026,1,1), "x", TRUE)"#, d(2026, 1, 1));
    check("MAX(DATE(2026,1,1), [])", d(2026, 1, 1));
    check(r#"MIN(DATE(2026,1,1), "5")"#, d(2026, 1, 1));
}

#[test]
fn min_and_max_reject_dates_with_numbers() {
    err(
        "MAX(DATE(2026,1,1), 5)",
        "MAX cannot compare dates with numbers (#VALUE!)",
    );
    err(
        "MIN(DATE(2026,1,1), 5)",
        "MIN cannot compare dates with numbers (#VALUE!)",
    );
    err(
        "MIN([1, [DATE(2026,1,1)]])",
        "MIN cannot compare dates with numbers (#VALUE!)",
    );
}

#[test]
fn min_and_max_without_dates_are_unchanged() {
    check("MAX()", n(0.0));
    check("MAX([])", n(0.0));
    check("MAX(NULL)", n(0.0));
    check("MAX(1, 5)", n(5.0));
    check("MIN(3, [1, 2])", n(1.0));
}

// ---- B2: the number aggregates ----

#[test]
fn sum_and_product_reject_dates() {
    for e in [
        "SUM(DATE(2026,1,1))",
        "SUM(DATE(2026,1,1), 1)",
        "SUM([1, DATE(2026,1,1)])",
    ] {
        err(e, "SUM does not accept dates (#VALUE!)");
    }
    err(
        "PRODUCT(DATE(2026,1,1))",
        "PRODUCT does not accept dates (#VALUE!)",
    );
    err(
        "MULTIPLY(DATE(2026,1,1))",
        "PRODUCT does not accept dates (#VALUE!)",
    );
    check("SUM(1, [2, 3])", n(6.0));
}

#[test]
fn average_rejects_dates_before_the_no_numbers_error() {
    err(
        "AVERAGE(DATE(2026,1,1), DATE(2026,1,3))",
        "AVERAGE does not accept dates (#VALUE!)",
    );
    err(
        "AVG(DATE(2026,1,1))",
        "AVERAGE does not accept dates (#VALUE!)",
    );
    err("AVERAGE()", "AVERAGE of no numbers (#DIV/0!)");
}

#[test]
fn statistical_builtins_reject_dates_with_the_canonical_name() {
    let cases = [
        ("MEDIAN(DATE(2026,1,1), DATE(2026,1,3))", "MEDIAN"),
        ("MEDIAN(1, DATE(2026,1,3))", "MEDIAN"),
        ("MODE(1, 1, DATE(2026,1,3))", "MODE.SNGL"),
        ("MODE_SNGL(DATE(2026,1,3))", "MODE.SNGL"),
        ("STDEV.P(1, 2, DATE(2026,1,3))", "STDEV.P"),
        ("STDEVP(DATE(2026,1,3))", "STDEV.P"),
        ("VAR.P([DATE(2026,1,3)])", "VAR.P"),
        ("VARP(1, DATE(2026,1,3))", "VAR.P"),
        ("STDEV(DATE(2026,1,1), 1)", "STDEV.S"),
        ("STDEV_S(1, 2, DATE(2026,1,1))", "STDEV.S"),
        ("VAR(DATE(2026,1,1))", "VAR.S"),
        ("VAR.S(1, [DATE(2026,1,1)])", "VAR.S"),
        ("PERCENTILE.INC([DATE(2026,1,1)], 0.5)", "PERCENTILE.INC"),
        ("PERCENTILE([1, DATE(2026,1,1)], 0.5)", "PERCENTILE.INC"),
        ("QUARTILE.INC([DATE(2026,1,1)], 1)", "QUARTILE.INC"),
        ("QUARTILE_INC([1, DATE(2026,1,1)], 1)", "QUARTILE.INC"),
    ];
    for (e, name) in cases {
        err(e, &format!("{name} does not accept dates (#VALUE!)"));
    }
}

#[test]
fn percentile_and_quartile_check_k_before_the_data() {
    err(
        "PERCENTILE.INC([DATE(2026,1,1)], 2)",
        "Percentile must be between 0 and 1 (#NUM!)",
    );
    err(
        r#"PERCENTILE.INC([DATE(2026,1,1)], "a")"#,
        "PERCENTILE.INC percentile must be a number",
    );
    err(
        "QUARTILE.INC([DATE(2026,1,1)], 5)",
        "Quartile must be 0-4 (#NUM!)",
    );
}

#[test]
fn count_and_counta_still_count_dates() {
    check(r#"COUNT(DATE(2026,1,1), 1, "a")"#, n(2.0));
    check("COUNTA(DATE(2026,1,1), NULL)", n(1.0));
}

// ---- B4: SORT ----

#[test]
fn sort_orders_dates_in_the_number_place() {
    check(
        "SORT([DATE(2027,1,1), DATE(2026,1,1)])",
        Value::Array(vec![d(2026, 1, 1), d(2027, 1, 1)]),
    );
    check(
        "SORT([DATE(2026,1,1), DATE(2027,1,1)], -1)",
        Value::Array(vec![d(2027, 1, 1), d(2026, 1, 1)]),
    );
    check(
        r#"SORT([TRUE, "a", DATE(2026,1,1)])"#,
        Value::Array(vec![
            d(2026, 1, 1),
            Value::String("a".into()),
            Value::Boolean(true),
        ]),
    );
    check(
        r#"SORT([DATE(2026,1,1), "a", TRUE], "DESC")"#,
        Value::Array(vec![
            Value::Boolean(true),
            Value::String("a".into()),
            d(2026, 1, 1),
        ]),
    );
}

#[test]
fn sort_rejects_dates_with_numbers_and_names_dates() {
    err(
        "SORT([DATE(2026,1,1), 5])",
        "SORT cannot compare dates with numbers (#VALUE!)",
    );
    err(
        r#"SORT([5, "a", DATE(2026,1,1)])"#,
        "SORT cannot compare dates with numbers (#VALUE!)",
    );
    err(
        "SORT([1, NULL])",
        "SORT expects numbers, dates, text or booleans",
    );
    err(
        "SORT([DATE(2026,1,1), [1]])",
        "SORT expects numbers, dates, text or booleans",
    );
}

// ---- B5: .sort(), .min(), .max() ----

#[test]
fn methods_accept_an_array_of_dates() {
    check(
        "[DATE(2027,1,1), DATE(2026,1,1)].sort()",
        Value::Array(vec![d(2026, 1, 1), d(2027, 1, 1)]),
    );
    check(
        r#"[DATE(2026,1,1), DATE(2027,1,1)].sort("DESC")"#,
        Value::Array(vec![d(2027, 1, 1), d(2026, 1, 1)]),
    );
    check("[DATE(2027,1,1), DATE(2026,1,1)].max()", d(2027, 1, 1));
    check("[DATE(2027,1,1), DATE(2026,1,1)].min()", d(2026, 1, 1));
    check(
        "[3, 1, 2].sort()",
        Value::Array(vec![n(1.0), n(2.0), n(3.0)]),
    );
    check("[3, 1, 2].max()", n(3.0));
}

#[test]
fn methods_reject_anything_else() {
    err(
        "[DATE(2027,1,1), 1].max()",
        "max method expects an array of numbers or of dates",
    );
    err(
        "[1, DATE(2027,1,1)].min()",
        "min method expects an array of numbers or of dates",
    );
    err(
        r#"["b", "a"].sort()"#,
        "sort expects an array of numbers or of dates",
    );
    err(
        "[DATE(2027,1,1), NULL].sort()",
        "sort expects an array of numbers or of dates",
    );
    err(
        "[1, DATE(2027,1,1)].sort()",
        "sort expects an array of numbers or of dates",
    );
}

#[test]
fn methods_keep_their_empty_results_and_sum_avg_messages() {
    check("[].sort()", Value::Array(vec![]));
    check("[].min()", Value::Null);
    check("[].max()", Value::Null);
    err("[DATE(2026,1,1)].sum()", "sum method expects numeric array");
    err("[DATE(2026,1,1)].avg()", "avg method expects numeric array");
}

// ---- B6: BETWEEN ----

#[test]
fn between_accepts_three_dates() {
    check(
        "BETWEEN(DATE(2026,1,1), DATE(2027,1,1), DATE(2026,6,1))",
        Value::Boolean(true),
    );
    check(
        "BETWEEN(DATE(2026,1,1), DATE(2027,1,1), DATE(2027,1,1))",
        Value::Boolean(true),
    );
    check(
        "BETWEEN(DATE(2026,1,1), DATE(2027,1,1), DATE(2028,1,1))",
        Value::Boolean(false),
    );
    err(
        "BETWEEN(DATE(2026,1,1), 10, 5)",
        "BETWEEN min must be a number",
    );
    err(
        "BETWEEN(DATE(2026,1,1), DATE(2027,1,1), 5)",
        "BETWEEN min must be a number",
    );
    check("BETWEEN(1, 10, 5)", Value::Boolean(true));
}

// ---- B7: equality is already by instant ----

#[test]
fn equality_and_membership_are_unchanged() {
    check(
        "UNIQUE([DATE(2026,1,1), DATE(2026,1,1), DATE(2027,1,1)])",
        Value::Array(vec![d(2026, 1, 1), d(2027, 1, 1)]),
    );
    check(
        "CONTAINS([DATE(2026,1,1)], DATE(2026,1,1))",
        Value::Boolean(true),
    );
    check(
        r#"DATE(2026,1,1) == "2026-01-01T00:00:00Z"::DateTime"#,
        Value::Boolean(true),
    );
    check(
        "DATE(2026,1,1) == 1767225600::DateTime",
        Value::Boolean(true),
    );
    check(
        r#"DATE(2026,1,1) == "2026-01-01T00:00:00Z""#,
        Value::Boolean(false),
    );
    check("DATE(2026,1,1) == 1767225600", Value::Boolean(false));
}

// ---- B8: criteria ----

const TWO: &str = "[DATE(2026,1,1), DATE(2027,1,1)]";

fn countif(range: &str, criteria: &str) -> Value {
    eval(&format!("COUNTIF({range}, {criteria})"))
}

#[test]
fn criteria_built_with_a_date_compare_by_instant() {
    assert_eq!(countif(TWO, r#"">" & DATE(2026,6,1)"#), n(1.0));
    assert_eq!(countif(TWO, r#""=" & DATE(2026,1,1)"#), n(1.0));
    assert_eq!(countif(TWO, "DATE(2026,1,1)"), n(1.0));
    assert_eq!(countif(TWO, "DATE(2028,1,1)"), n(0.0));
}

#[test]
fn every_operator_applies_to_iso_text() {
    let cases = [
        (r#"">=2026-01-01T00:00:00Z""#, 2.0),
        (r#""<2027-01-01T00:00:00Z""#, 1.0),
        (r#""<=2027-01-01T00:00:00Z""#, 2.0),
        (r#""2026-01-01T00:00:00Z""#, 1.0),
        (r#""=2026-01-01T00:00:00Z""#, 1.0),
        (r#""<>2026-01-01T00:00:00Z""#, 1.0),
        (r#"">2026-01-01T00:00:00Z""#, 1.0),
    ];
    for (crit, want) in cases {
        assert_eq!(countif(TWO, crit), n(want), "{crit}");
    }
}

#[test]
fn date_only_text_and_patterns_are_not_dates() {
    assert_eq!(countif(TWO, r#"">2026-06-01""#), n(0.0));
    assert_eq!(
        countif("[DATE(2026,1,1)]", r#""2026-01-01T00:00:00*""#),
        n(0.0)
    );
    assert_eq!(
        countif("[DATE(2026,1,1)]", r#"">2026-02-30T00:00:00Z""#),
        n(0.0)
    );
    assert_eq!(
        countif("[DATE(2026,1,1)]", r#"">+10000-01-01T00:00:00Z""#),
        n(0.0)
    );
    assert_eq!(
        countif("[DATE(2026,1,1)]", r#""<+10000-01-01T00:00:00Z""#),
        n(1.0)
    );
}

#[test]
fn a_date_operand_matches_only_dates_and_iso_text_except_ne() {
    let mixed = r#"[DATE(2026,1,1), 5, "a", NULL]"#;
    assert_eq!(countif(mixed, r#"">2025-01-01T00:00:00Z""#), n(1.0));
    assert_eq!(countif(mixed, r#""<>2026-01-01T00:00:00Z""#), n(3.0));
    assert_eq!(countif(mixed, "DATE(2026,1,1)"), n(1.0));
}

#[test]
fn other_operands_never_match_a_date() {
    assert_eq!(countif("[DATE(2026,1,1), 5]", r#"">0""#), n(1.0));
    assert_eq!(countif("[DATE(2026,1,1), 5]", r#"">1767225599""#), n(0.0));
    assert_eq!(countif("[DATE(2026,1,1)]", r#""<>5""#), n(1.0));
    assert_eq!(countif("[DATE(2026,1,1)]", r#""""#), n(0.0));
    assert_eq!(countif("[DATE(2026,1,1)]", r#""<>a*""#), n(1.0));
}

#[test]
fn iso_text_items_compare_as_dates() {
    assert_eq!(
        countif(
            r#"["2026-02-01T00:00:00Z", "2025-02-01T00:00:00Z"]"#,
            r#"">2026-01-01T00:00:00Z""#
        ),
        n(1.0)
    );
    assert_eq!(
        countif(r#"["2026-01-01T00:00:00Z"]"#, "DATE(2026,1,1)"),
        n(1.0)
    );
    assert_eq!(
        countif(r#"["2026-01-01t00:00:00z"]"#, r#""=2026-01-01T00:00:00Z""#),
        n(0.0)
    );
    assert_eq!(countif(r#"["1767225600"]"#, "DATE(2026,1,1)"), n(0.0));
    // Data from JSON holds dates as text.
    let vars = r#"{"dates": ["2026-01-01T00:00:00Z", "2027-01-01T00:00:00Z"]}"#;
    assert_eq!(
        skillet::evaluate_with_json(r#"COUNTIF(:dates, ">" & DATE(2026,6,1))"#, vars).unwrap(),
        n(1.0)
    );
}

#[test]
fn sumif_and_averageif_with_date_criteria() {
    check(
        r#"SUMIF([DATE(2026,1,1), DATE(2027,1,1)], ">2026-06-01T00:00:00Z", [10, 20])"#,
        n(20.0),
    );
    check(
        r#"AVERAGEIF([DATE(2026,1,1), DATE(2027,1,1)], ">2025-06-01T00:00:00Z", [10, 20])"#,
        n(15.0),
    );
}

// ---- B3: SUMIF and AVERAGEIF reject a date they would sum ----

#[test]
fn sumif_and_averageif_reject_a_summed_date() {
    err(
        r#"SUMIF([DATE(2026,1,1), DATE(2027,1,1)], ">2026-06-01T00:00:00Z")"#,
        "SUMIF does not accept dates (#VALUE!)",
    );
    err(
        r#"SUMIF([1, 2], ">0", [DATE(2026,1,1), 5])"#,
        "SUMIF does not accept dates (#VALUE!)",
    );
    check(r#"SUMIF([1, 2], ">1", [DATE(2026,1,1), 5])"#, n(5.0));
    err(
        r#"AVERAGEIF([1, 2], ">0", [DATE(2026,1,1), 5])"#,
        "AVERAGEIF does not accept dates (#VALUE!)",
    );
    err(
        r#"AVGIF([DATE(2026,1,1)], DATE(2026,1,1))"#,
        "AVERAGEIF does not accept dates (#VALUE!)",
    );
    err(
        "SUMIF([1, 2], :x > 0, [DATE(2026,1,1), 5])",
        "SUMIF does not accept dates (#VALUE!)",
    );
    // A date in the criteria range is not summed when a sum range is given.
    check("SUMIF([DATE(2026,1,1), 2], :x == 2, [10, 20])", n(20.0));
}

#[test]
fn lambda_criteria_and_countif_are_unchanged() {
    check(
        "COUNTIF([DATE(2026,1,1), DATE(2027,1,1)], :x > DATE(2026,6,1))",
        n(1.0),
    );
    check(
        r#"COUNTIF([DATE(2026,1,1), DATE(2027,1,1)], ">2026-06-01T00:00:00Z")"#,
        n(1.0),
    );
}
