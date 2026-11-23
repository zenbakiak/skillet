//! BL-054: existing `UPPERCASE()` builtins follow Excel's documented behaviour.
//!
//! Expected values come from Microsoft's function documentation (the Excel column of
//! docs/reference/excel-compat.md). Where Excel returns `#DIV/0!`, `#NUM!`, `#VALUE!` or `#N/A`,
//! Skillet returns an `Error`. Lower-case methods keep their Ruby semantics.

use skillet::{evaluate, evaluate_with_json, Value};

fn num(expr: &str) -> f64 {
    match evaluate(expr) {
        Ok(Value::Number(n)) => n,
        other => panic!("{expr}: expected a number, got {other:?}"),
    }
}

fn close(expr: &str, want: f64) {
    let got = num(expr);
    assert!((got - want).abs() < 1e-9, "{expr}: want {want}, got {got}");
}

fn err(expr: &str, needle: &str) {
    match evaluate(expr) {
        Err(e) => assert!(
            e.message.contains(needle),
            "{expr}: error {:?} should contain {needle:?}",
            e.message
        ),
        Ok(v) => panic!("{expr}: expected an error, got {v:?}"),
    }
}

fn string(expr: &str) -> String {
    match evaluate(expr) {
        Ok(Value::String(s)) => s,
        other => panic!("{expr}: expected a string, got {other:?}"),
    }
}

// ---- D-2: TIME, IPMT, COUNTIF (B-1, B-2, B-3) ----

#[test]
fn time_is_a_fraction_of_a_day_with_excel_truncation_and_wrap() {
    let day = 86400.0;
    close("=TIME(14, 30, 0)", 52200.0 / day); // 0.6041666...
    close("=TIME(12, 0, 0)", 0.5);
    close("=TIME(25, 0, 0)", 3600.0 / day); // Excel: TIME(25,0,0) = TIME(1,0,0)
    close("=TIME(0, 750, 0)", 0.520833333333333); // Microsoft: 750 minutes = 12:30
    close("=TIME(0, 0, 2000)", 2000.0 / day); // Microsoft: 0:33:20
    close("=TIME(1, -30, 0)", 1800.0 / day); // a negative part is fine while the total is not
    close("=TIME(1.9, 0, 0)", 3600.0 / day); // parts are truncated
    close("=TIME(48, 0, 1)", 1.0 / day);
    let now = num("=TIME()");
    assert!((0.0..1.0).contains(&now), "TIME() = {now}");
    err("=TIME(0, -1, 0)", "#NUM!");
    err("=TIME(32768, 0, 0)", "#NUM!");
    err("=TIME(-1, 0, 0)", "#NUM!");
}

#[test]
fn ipmt_follows_excel_sign_convention() {
    // Microsoft's IPMT examples.
    close("=ROUND(IPMT(0.1 / 12, 1, 36, 8000), 2)", -66.67);
    close("=ROUND(IPMT(0.1, 3, 3, 8000), 2)", -292.45);
    // Excel's documented formula, period by period.
    close("=ROUND(IPMT(0.1, 1, 3, 1000), 2)", -100.0);
    close("=ROUND(IPMT(0.1, 2, 3, 1000), 2)", -69.79);
    close("=ROUND(IPMT(0.1, 3, 3, 1000), 2)", -36.56);
    close("=IPMT(0.1, 1, 3, 1000, 0, 1)", 0.0);
    close("=ROUND(IPMT(0.1, 2, 3, 1000, 0, 1), 2)", -63.44);
    close("=IPMT(0, 1, 3, 1000)", 0.0);
    err("=IPMT(0.1, 4, 3, 1000)", "between 1 and nper");
}

#[test]
fn countif_accepts_excel_criteria() {
    close("=COUNTIF([1, 2, 3, 4], \">2\")", 2.0);
    close("=COUNTIF([1, 2, 3], 2)", 1.0);
    close("=COUNTIF([1, 2, 3], \"2\")", 1.0);
    close("=COUNTIF([1, 2, 3], \"=2\")", 1.0);
    close("=COUNTIF([1, 2, 3], \"<>2\")", 2.0);
    close("=COUNTIF([1, 2, 3], \">=2\")", 2.0);
    close("=COUNTIF([1, 2, 3], \"<=2\")", 2.0);
    close("=COUNTIF([1, 2, 3], \"<2\")", 1.0);
    close("=COUNTIF([\"A\", \"b\", \"a\"], \"a\")", 2.0); // case-insensitive
    close("=COUNTIF([\"x\", \"y\", 1], \"<>x\")", 2.0);
    close("=COUNTIF([\"x\", \"y\"], \"=X\")", 1.0);
    close("=COUNTIF([\"apple\", \"Avocado\", \"kiwi\"], \"a*\")", 2.0);
    close("=COUNTIF([\"abc\", \"abbc\", \"ac\"], \"a?c\")", 1.0);
    close("=COUNTIF([\"a*\", \"ab\"], \"a~*\")", 1.0);
    close("=COUNTIF([\"5\", 5, \"x\"], 5)", 2.0); // numeric text matches a number
    close("=COUNTIF([TRUE, FALSE, TRUE], \"TRUE\")", 2.0);
    close("=COUNTIF([NULL, \"\", 0], \"\")", 2.0); // blanks
    close("=COUNTIF([NULL, \"\", 0], \"<>\")", 1.0); // non-blanks
}

#[test]
fn countif_takes_a_scalar_or_string_range() {
    close("=COUNTIF(5, \">2\")", 1.0);
    close("=COUNTIF(1, \">2\")", 0.0);
    close("=COUNTIF(\"Apple\", \"a*\")", 1.0);
}

#[test]
fn countif_lambda_returns_a_condition() {
    close("=COUNTIF([1, 2, 3, 4], :x % 2 == 0)", 2.0);
    // NULL from null-safe navigation counts as FALSE (ADR-0006).
    let vars = r#"{"items": [{"a": {"ok": true}}, {"a": null}]}"#;
    let r = evaluate_with_json("=COUNTIF(:items, :x.a&.ok)", vars).unwrap();
    assert_eq!(r, Value::Number(1.0));
    // B-17, ADR-0029 amendment 1 (BL-089): a number is a condition, text is #VALUE!.
    close("=COUNTIF([0, 1, 2], :x)", 2.0);
    close("=SUMIF([1, 2, 3], :x - 2)", 4.0);
    close("=AVGIF([1, 2, 3], :x - 2)", 2.0);
    err(
        "=COUNTIF([\"a\"], :x)",
        "COUNTIF condition must be TRUE, FALSE or a number, got string (#VALUE!)",
    );
    err("=SUMIF([1], :x & \"\")", "SUMIF condition must be TRUE");
    err("=AVGIF([1], :x & \"\")", "AVERAGEIF condition must be TRUE");
}

// ---- D-7: SUMIF and AVERAGEIF share COUNTIF's criteria ----

#[test]
fn sumif_is_case_insensitive_with_wildcards() {
    close("=SUMIF([\"A\", \"b\"], \"a\", [1, 2])", 1.0);
    close(
        "=SUMIF([\"apple\", \"avocado\", \"kiwi\"], \"a*\", [1, 2, 4])",
        3.0,
    );
    close("=SUMIF([1, 2, 3, 4], \">2\")", 7.0);
    close("=SUMIF([1, 2, 3], \">=2\", [10, 20, 30])", 50.0);
    close("=SUMIF([1, 2, 3], \"<>2\")", 4.0);
    close("=SUMIF([1, 2, 3], \">9\")", 0.0); // no match is 0 in Excel
    close("=SUMIF([1, 2, 3], :x > 1, [10, 20, 30])", 50.0);
}

#[test]
fn averageif_is_added_and_errors_on_no_match() {
    close("=AVERAGEIF([1, 2, 3, 4], \">2\")", 3.5);
    close(
        "=AVERAGEIF([\"a\", \"b\", \"A\"], \"a\", [10, 20, 30])",
        20.0,
    );
    close("=AVERAGEIF([1, 3, 5, -1], :x > 0)", 3.0);
    close("=AVGIF([1, 2, 3, 4], :x > 2)", 3.5);
    close("=AVGIF([1, 2, 3, 4], \">2\")", 3.5);
    err("=AVERAGEIF([1, 2], \">5\")", "#DIV/0!");
    err("=AVGIF([1, 2], :x > 5)", "#DIV/0!");
}

// ---- D-3: COUNT and COUNTA ----

#[test]
fn count_is_variadic_and_counts_numbers_only() {
    close("=COUNT([1, \"a\", TRUE, NULL])", 1.0);
    close("=COUNT(1, 2, 3)", 3.0);
    close("=COUNT([1, [2, 3]], 4)", 4.0);
    close("=COUNT([])", 0.0);
    close("=COUNT(NULL)", 0.0);
    close("=COUNT(\"a\")", 0.0);
    err("=COUNT()", "at least 1 argument");
}

#[test]
fn counta_counts_non_null_values() {
    close("=COUNTA([1, \"a\", TRUE, NULL])", 3.0);
    close("=COUNTA(1, \"\", FALSE)", 3.0); // Excel counts empty text
    close("=COUNTA([], NULL)", 0.0);
    err("=COUNTA()", "at least 1 argument");
}

// ---- D-1: errors where Excel errors, Excel's value where it has one ----

#[test]
fn impossible_math_is_an_error() {
    err("=SQRT(-1)", "#NUM!");
    err("=MOD(5, 0)", "#DIV/0!");
    err("=POWER(0, -1)", "#DIV/0!");
    err("=POWER(-8, 1/3)", "#NUM!");
    err("=POWER(0, 0)", "#NUM!");
    err("=FLOOR(3, 0)", "#DIV/0!");
    err("=CEILING(2.5, -2)", "#NUM!");
    err("=FLOOR(2.5, -2)", "#NUM!");
}

#[test]
fn wrong_type_or_missing_arguments_are_errors() {
    // BL-066: text that is not a number is #VALUE!; numeric text converts (`FLOOR("3.7")` = 3).
    err(
        "=ABS(\"x\")",
        "ABS: cannot convert \"x\" to a number (#VALUE!)",
    );
    err("=SQRT(\"x\")", "SQRT: cannot convert");
    err("=FLOOR(\"3.7x\")", "FLOOR: cannot convert");
    err("=INT(\"x\")", "INT: cannot convert");
    err("=ABS([1])", "expects number");
    err("=ROUND(2.567, \"x\")", "ROUND num_digits: cannot convert");
    err("=ABS()", "ABS expects");
    err("=MOD(10)", "MOD expects");
    err("=POWER(2)", "POWER expects");
    err("=PMT(0.05, 10, 1000, \"x\")", "PMT fv: cannot convert");
    err("=FV(0.05, 10, -100, \"x\")", "FV pv: cannot convert");
    err("=SORT([3, 1, 2], \"sideways\")", "SORT order");
    err("=SUBSTITUTE(\"a-b\", \"-\", \"+\", 0)", "instance_num");
}

#[test]
fn empty_aggregates_match_excel() {
    err("=AVERAGE()", "#DIV/0!");
    err("=AVERAGE([\"x\", TRUE, NULL])", "#DIV/0!");
    err("=MEDIAN()", "#NUM!");
    err("=STDEV_P()", "#DIV/0!");
    err("=VAR_P([])", "#DIV/0!");
    err("=MODE_SNGL()", "#N/A");
    err("=MODE_SNGL(1, 2, 3)", "#N/A"); // no repeated value
    err("=PERCENTILE_INC([], 0.5)", "#NUM!");
    err("=QUARTILE_INC([], 1)", "#NUM!");
    // Excel returns a value here, so Skillet does too.
    close("=MAX()", 0.0);
    close("=MIN([\"a\"])", 0.0);
    close("=SUM()", 0.0);
    close("=MODE_SNGL(3, 1, 1, 3)", 3.0); // first value seen wins a tie
}

// ---- ADR-0015 option B ----

#[test]
fn percentile_and_quartile_reject_nan_and_out_of_range() {
    let nan_err = |expr: &str, needle: &str| {
        let mut vars = std::collections::HashMap::new();
        vars.insert("nan".to_string(), Value::Number(f64::NAN));
        vars.insert("inf".to_string(), Value::Number(f64::INFINITY));
        match skillet::evaluate_with(expr, &vars) {
            Err(e) => assert!(e.message.contains(needle), "{expr}: {e:?}"),
            Ok(v) => panic!("{expr}: expected an error, got {v:?}"),
        }
    };
    nan_err("=PERCENTILE_INC([1, 2, 3], :nan)", "between 0 and 1");
    nan_err("=PERCENTILE_INC([], :nan)", "between 0 and 1");
    err("=PERCENTILE_INC([1, 2, 3], 1.5)", "between 0 and 1");
    nan_err("=QUARTILE_INC([1, 2, 3, 4], :nan)", "0-4");
    err("=QUARTILE_INC([1, 2, 3, 4], -0.5)", "0-4");
    err("=QUARTILE_INC([1, 2, 3, 4], 4.9)", "0-4");
    close("=QUARTILE_INC([1, 2, 3, 4], 2.7)", 2.5); // truncated after the range check
    let mut vars = std::collections::HashMap::new();
    vars.insert("inf".to_string(), Value::Number(f64::INFINITY));
    assert_eq!(
        skillet::evaluate_with("=MEDIAN([1, :inf, 3])", &vars).unwrap(),
        Value::Number(3.0)
    ); // infinity from the host is legal data
    nan_err("=MEDIAN([1, :nan, 3])", "NaN");
}

// ---- D-7 and the rest: MOD, TRIM, ROUND, INT, CEILING, FLOOR ----

#[test]
fn mod_takes_the_sign_of_the_divisor() {
    close("=MOD(10, 3)", 1.0);
    close("=MOD(-10, 3)", 2.0);
    close("=MOD(10, -3)", -2.0);
    close("=MOD(-10, -3)", -1.0);
    close("=MOD(5.5, 2)", 1.5);
    close("=MOD(3, 2)", 1.0); // Microsoft example
    close("=MOD(-3, 2)", 1.0);
    close("=MOD(3, -2)", -1.0);
    close("=MOD(-3, -2)", -1.0);
}

#[test]
fn trim_function_collapses_inner_spaces_but_method_does_not() {
    assert_eq!(string("=TRIM(\"  a   b  \")"), "a b");
    assert_eq!(
        string("=TRIM(\" First Quarter     Earnings \")"),
        "First Quarter Earnings"
    ); // Microsoft example
    assert_eq!(string("=TRIM(\"\")"), "");
    assert_eq!(string("=\"  a   b  \".trim()"), "a   b");
}

#[test]
fn round_supports_negative_digits_and_decimal_halves() {
    close("=ROUND(1234.567, -2)", 1200.0);
    close("=ROUND(15, -1)", 20.0);
    close("=ROUND(-50.55, -2)", -100.0); // Microsoft example
    close("=ROUND(626.3, -3)", 1000.0);
    close("=ROUND(1.98, -1)", 0.0);
    close("=ROUND(2.15, 1)", 2.2); // Microsoft example
    close("=ROUND(2.149, 1)", 2.1);
    close("=ROUND(-1.475, 2)", -1.48);
    close("=ROUND(21.5, -1)", 20.0);
    close("=ROUND(1.005, 2)", 1.01);
    close("=ROUND(2.5)", 3.0);
    close("=ROUND(-2.5)", -3.0);
    close("=ROUND(2.567, 1.9)", 2.6); // digits are truncated
                                      // ADR-0033: `::Currency` gives a Number, so ROUND returns a Number.
    assert_eq!(
        evaluate("=ROUND(10.26::Currency, 1)").unwrap(),
        Value::Number(10.3)
    );
}

/// BL-088 (maintainer decision): `.round([digits])` is `ROUND`, half away from zero on the
/// 15-digit decimal form. Before, it scaled with `f64`, so `1.005.round(2)` was 1.0.
#[test]
fn round_method_is_round() {
    for (n, d) in [
        ("1.005", "2"),
        ("0.285", "2"),
        ("1.015", "2"),
        ("2.15", "1"),
        ("-1.475", "2"),
        ("1234.567", "-2"),
        ("2.5", "0"),
        ("-2.5", "0"),
        ("0.49999999999999994", "0"),
        ("2.567", "1.9"),
        ("1e307", "5"),
        ("(0.1 + 0.2)", "16"),
    ] {
        let method = evaluate(&format!("=({n}).round({d})")).unwrap();
        let function = evaluate(&format!("=ROUND({n}, {d})")).unwrap();
        assert_eq!(method, function, "({n}).round({d})");
    }
    assert_eq!(evaluate("=1.005.round(2)").unwrap(), Value::Number(1.01));
    assert_eq!(evaluate("=0.285.round(2)").unwrap(), Value::Number(0.29));
    assert_eq!(evaluate("=1e307.round(5)").unwrap(), Value::Number(1e307));
    assert_eq!(evaluate("=(2.5).round(NULL)").unwrap(), Value::Number(3.0));
    assert_eq!(evaluate("=(2.5).round(\"1\")").unwrap(), Value::Number(2.5));
    err("=(2.5).round(\"x\")", "round num_digits: cannot convert");
    err("=(2.5).round(1, 2)", "round expects ([num_digits])");
}

#[test]
fn int_rounds_down() {
    close("=INT(8.9)", 8.0); // Microsoft example
    close("=INT(-8.9)", -9.0); // Microsoft example
    close("=INT(-3.9)", -4.0);
}

#[test]
fn ceiling_and_floor_honour_significance() {
    close("=CEILING(2.5, 1)", 3.0); // Microsoft examples
    close("=CEILING(-2.5, -2)", -4.0);
    close("=CEILING(-2.5, 2)", -2.0);
    close("=CEILING(1.5, 0.1)", 1.5);
    close("=CEILING(0.234, 0.01)", 0.24);
    close("=CEILING(2.1, 0.5)", 2.5);
    close("=CEILING(7, 5)", 10.0);
    close("=CEILING(2.1, 0.1)", 2.1);
    close("=CEILING(5, 0)", 0.0);
    close("=CEILING(2.1)", 3.0);
    close("=CEIL(2.1, 0.5)", 2.5);
    close("=FLOOR(3.7, 2)", 2.0); // Microsoft examples
    close("=FLOOR(-2.5, -2)", -2.0);
    close("=FLOOR(1.58, 0.1)", 1.5);
    close("=FLOOR(0.234, 0.01)", 0.23);
    close("=FLOOR(-2.5, 2)", -4.0);
    close("=FLOOR(7, 5)", 5.0);
    close("=FLOOR(-2.1)", -3.0);
}

#[test]
fn substitute_honours_instance_num() {
    assert_eq!(string("=SUBSTITUTE(\"a-b-c\", \"-\", \"+\", 2)"), "a-b+c");
    assert_eq!(string("=SUBSTITUTE(\"a-b-c\", \"-\", \"+\", 5)"), "a-b-c");
    assert_eq!(string("=SUBSTITUTE(\"a-b-c\", \"-\", \"+\")"), "a+b+c");
    assert_eq!(string("=SUBSTITUTE(\"abc\", \"\", \"x\")"), "abc");
}

#[test]
fn sort_accepts_excel_sort_order() {
    let r = evaluate("=SORT([3, 1, 2], -1)").unwrap();
    assert_eq!(
        r,
        Value::Array(vec![
            Value::Number(3.0),
            Value::Number(2.0),
            Value::Number(1.0)
        ])
    );
    let r = evaluate("=SORT([3, 1, 2], 1)").unwrap();
    assert_eq!(
        r,
        Value::Array(vec![
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(3.0)
        ])
    );
}

// ---- D-1 for operators (ADR-0018) ----

#[test]
fn operators_error_instead_of_nan_or_infinity() {
    err("=1 / 0", "Division by zero");
    err("=0 / 0", "Division by zero");
    err("=5 % 0", "Division by zero");
    err("=0 ^ (-1)", "Division by zero");
    err("=0 ^ 0", "#NUM!");
    err("=10 ^ 400", "#NUM!");
    err("=10 ^ 308 * 10", "#NUM!");
    err("=(-8) ^ (1 / 3)", "#NUM!");
    close("=7 / 2", 3.5);
    close("=-10 % 3", -1.0); // % keeps the left operand's sign; MOD is Excel's
    close("=2 ^ -1", 0.5);
}
