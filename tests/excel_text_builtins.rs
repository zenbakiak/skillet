//! BL-072: the text, array and financial builtins follow Excel's documented behaviour (ADR-0018).
//!
//! Expected values come from Microsoft's documentation for each function (the examples on the
//! function's support page) and the Excel column of docs/reference/excel-compat.md. Where Excel
//! returns `#VALUE!` or `#NUM!`, Skillet returns an `Error`.

use skillet::{evaluate, Value};

fn eval(expr: &str) -> Value {
    evaluate(expr).unwrap_or_else(|e| panic!("{expr}: unexpected error {e:?}"))
}

fn string(expr: &str) -> String {
    match eval(expr) {
        Value::String(s) => s,
        other => panic!("{expr}: expected a string, got {other:?}"),
    }
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

fn s(v: &str) -> Value {
    Value::String(v.to_string())
}

fn n(v: f64) -> Value {
    Value::Number(v)
}

// ---- LEFT / RIGHT ----

#[test]
fn left_right_microsoft_examples() {
    assert_eq!(string(r#"LEFT("Sale Price", 4)"#), "Sale");
    assert_eq!(string(r#"LEFT("Sweden")"#), "S");
    assert_eq!(string(r#"RIGHT("Sale Price", 5)"#), "Price");
    assert_eq!(string(r#"RIGHT("Stock Number")"#), "r");
}

#[test]
fn left_right_lengths_are_truncated_and_clamped() {
    assert_eq!(string(r#"LEFT("hello", 2.9)"#), "he");
    assert_eq!(string(r#"LEFT("hello", 10)"#), "hello");
    assert_eq!(string(r#"RIGHT("hello", 10)"#), "hello");
    assert_eq!(string(r#"LEFT("hello", 0)"#), "");
    assert_eq!(string(r#"RIGHT("héllo", 4)"#), "éllo");
}

#[test]
fn left_right_negative_length_is_value_error() {
    err(
        r#"LEFT("hello", -1)"#,
        "LEFT num_chars must not be negative (#VALUE!)",
    );
    err(
        r#"RIGHT("hello", -1)"#,
        "RIGHT num_chars must not be negative (#VALUE!)",
    );
    err(r#"LEFT("hello", -0.5)"#, "#VALUE!");
}

// ---- MID ----

#[test]
fn mid_microsoft_examples() {
    assert_eq!(string(r#"MID("Fluid Flow", 1, 5)"#), "Fluid");
    assert_eq!(string(r#"MID("Fluid Flow", 7, 20)"#), "Flow");
    assert_eq!(string(r#"MID("Fluid Flow", 20, 5)"#), "");
}

#[test]
fn mid_start_below_one_or_negative_length_is_value_error() {
    err(
        r#"MID("hello", 0, 2)"#,
        "MID start_num must be at least 1 (#VALUE!)",
    );
    err(r#"MID("hello", -3, 2)"#, "#VALUE!");
    err(r#"MID("hello", 0.5, 2)"#, "#VALUE!");
    err(
        r#"MID("hello", 2, -1)"#,
        "MID num_chars must not be negative (#VALUE!)",
    );
}

#[test]
fn mid_edges() {
    assert_eq!(string(r#"MID("hello", 1.9, 2)"#), "he");
    assert_eq!(string(r#"MID("hello", 2, 0)"#), "");
    assert_eq!(string(r#"MID("hello", 6, 1)"#), "");
    // Leaving out num_chars is a Skillet extension: the rest of the text.
    assert_eq!(string(r#"MID("hello", 2)"#), "ello");
}

// ---- REPLACE ----

#[test]
fn replace_microsoft_examples() {
    assert_eq!(string(r#"REPLACE("abcdefghijk", 6, 5, "*")"#), "abcde*k");
    assert_eq!(string(r#"REPLACE("2009", 3, 2, "10")"#), "2010");
    assert_eq!(string(r#"REPLACE("123456", 1, 3, "@")"#), "@456");
}

#[test]
fn replace_past_the_end_appends_and_clamps() {
    assert_eq!(string(r#"REPLACE("abc", 10, 1, "X")"#), "abcX");
    assert_eq!(string(r#"REPLACE("hello", 4, 10, "X")"#), "helX");
    assert_eq!(string(r#"REPLACE("abc", 1, 0, "X")"#), "Xabc");
}

#[test]
fn replace_start_below_one_or_negative_length_is_value_error() {
    err(
        r#"REPLACE("abcdef", 0, 1, "X")"#,
        "REPLACE start_num must be at least 1 (#VALUE!)",
    );
    err(
        r#"REPLACE("abc", 1, -1, "X")"#,
        "REPLACE num_chars must not be negative (#VALUE!)",
    );
}

// ---- CONCAT and & ----

#[test]
fn concat_microsoft_example() {
    assert_eq!(
        string(r#"CONCAT("Stream population for ", "trout", " species is ", 32, "/mile")"#),
        "Stream population for trout species is 32/mile"
    );
}

#[test]
fn concat_writes_currency_as_its_number() {
    assert_eq!(string(r#"CONCAT("a", 1::Currency)"#), "a1");
    assert_eq!(string(r#"CONCAT("Total: ", 9.5::Currency)"#), "Total: 9.5");
    assert_eq!(string(r#""x" & 2.25::Currency"#), "x2.25");
}

#[test]
fn concat_writes_datetime_as_its_string_cast() {
    // The same text as `::String`: Unix seconds, like Excel's date serial number.
    assert_eq!(
        string(r#"CONCAT("a", 1::Currency, DATE(2026, 1, 1))"#),
        "a11767225600"
    );
    assert_eq!(
        string(r#"CONCAT(DATE(2026, 1, 1))"#),
        string("DATE(2026, 1, 1)::String")
    );
    assert_eq!(string(r#""on " & DATE(2026, 1, 1)"#), "on 1767225600");
    assert_eq!(
        string(r#"CONCAT([1::Currency, DATE(1970, 1, 2)])"#),
        "186400"
    );
}

#[test]
fn concat_other_values_unchanged() {
    assert_eq!(string(r#"CONCAT("a", 1, TRUE, NULL)"#), "a1TRUE");
    assert_eq!(string(r#"CONCAT("a", [1, 2])"#), "a12");
    assert_eq!(string("CONCAT()"), "");
}

// ---- SORT ----

#[test]
fn sort_text_ignores_case() {
    assert_eq!(
        eval(r#"SORT(["banana", "Apple", "cherry"])"#),
        Value::Array(vec![s("Apple"), s("banana"), s("cherry")])
    );
    assert_eq!(
        eval(r#"SORT(["b", "a"])"#),
        Value::Array(vec![s("a"), s("b")])
    );
}

#[test]
fn sort_mixed_types_numbers_then_text_then_booleans() {
    assert_eq!(
        eval(r#"SORT([TRUE, "b", 2, "A", FALSE, 1])"#),
        Value::Array(vec![
            n(1.0),
            n(2.0),
            s("A"),
            s("b"),
            Value::Boolean(false),
            Value::Boolean(true)
        ])
    );
}

#[test]
fn sort_descending_keeps_ties_in_order() {
    assert_eq!(
        eval("SORT([3, 1, 2], -1)"),
        Value::Array(vec![n(3.0), n(2.0), n(1.0)])
    );
    assert_eq!(
        eval(r#"SORT(["a", 1, TRUE, "B"], -1)"#),
        Value::Array(vec![Value::Boolean(true), s("B"), s("a"), n(1.0)])
    );
    // "a" and "A" are equal for SORT, so they keep their input order both ways.
    assert_eq!(
        eval(r#"SORT(["a", "A"])"#),
        Value::Array(vec![s("a"), s("A")])
    );
    assert_eq!(
        eval(r#"SORT(["a", "A"], "DESC")"#),
        Value::Array(vec![s("a"), s("A")])
    );
}

#[test]
fn sort_rejects_values_without_an_excel_order() {
    err("SORT([1, NULL])", "SORT expects numbers, text or booleans");
    err("SORT([[1], 2])", "SORT expects numbers, text or booleans");
    err("SORT([1, 2], 2)", "SORT order must be");
}

// ---- UNIQUE (B-7) ----

#[test]
fn unique_keeps_every_type_in_first_seen_order() {
    assert_eq!(
        eval(r#"UNIQUE(["a", "a", 1])"#),
        Value::Array(vec![s("a"), n(1.0)])
    );
    assert_eq!(
        eval("UNIQUE([3, 1, 3, 2])"),
        Value::Array(vec![n(3.0), n(1.0), n(2.0)])
    );
    assert_eq!(
        eval(r#"UNIQUE([TRUE, NULL, TRUE, NULL, [1], [1], 1::Currency, 1])"#),
        Value::Array(vec![
            Value::Boolean(true),
            Value::Null,
            Value::Array(vec![n(1.0)]),
            Value::Currency(1.0),
            n(1.0)
        ])
    );
    // Text is case-sensitive, like `==` (ADR-0023).
    assert_eq!(
        eval(r#"UNIQUE(["a", "A"])"#),
        Value::Array(vec![s("a"), s("A")])
    );
}

// ---- REVERSE (B-11) ----

#[test]
fn reverse_reverses_strings_and_arrays() {
    assert_eq!(string(r#"REVERSE("abc")"#), "cba");
    assert_eq!(string(r#"REVERSE("héllo")"#), "olléh");
    assert_eq!(
        eval("REVERSE([1, 2, 3])"),
        Value::Array(vec![n(3.0), n(2.0), n(1.0)])
    );
    err("REVERSE(1)", "REVERSE expects array or string");
}

// ---- DB (B-8) ----

fn db(expr: &str, want: f64) {
    match eval(expr) {
        Value::Number(got) => assert!((got - want).abs() < 0.005, "{expr}: want {want}, got {got}"),
        other => panic!("{expr}: expected a number, got {other:?}"),
    }
}

#[test]
fn db_microsoft_example() {
    // cost 1,000,000, salvage 100,000, life 6 years, 7 months in the first year.
    let want = [
        186_083.33, 259_639.42, 176_814.44, 120_410.64, 81_999.64, 55_841.76, 15_845.10,
    ];
    for (period, want) in (1..).zip(want) {
        db(&format!("DB(1000000, 100000, 6, {period}, 7)"), want);
    }
}

#[test]
fn db_full_first_year() {
    // The rate is rounded to 3 decimals: 1 - (1000/10000)^(1/5) = 0.369.
    db("DB(10000, 1000, 5, 1)", 3690.0);
    db("DB(10000, 1000, 5, 2)", 2328.39);
    err(
        "DB(10000, 1000, 5, 6)",
        "DB period must be between 1 and life",
    );
}

#[test]
fn db_out_of_range_arguments_are_num_errors() {
    err("DB(0, 0, 5, 1)", "#NUM!");
    err("DB(10000, 1000, 5, 0)", "#NUM!");
    err("DB(1000000, 100000, 6, 8, 7)", "#NUM!");
    err(
        "DB(10000, 1000, 5, 1, 0)",
        "DB month must be between 1 and 12 (#NUM!)",
    );
    err("DB(10000, 1000, 5, 1, 13)", "#NUM!");
    err("DB(10000, -1, 5, 1)", "#NUM!");
    err("DB(10000, 1000, 0, 1)", "#NUM!");
}

// ---- SUBSTITUTEM (deprecated since 0.7.0) ----

#[test]
fn deprecated_substitutem_still_matches_substitute() {
    assert_eq!(string(r#"SUBSTITUTEM("a-b-c", "-", "+")"#), "a+b+c");
    assert_eq!(
        eval(r#"SUBSTITUTEM("a-b-c", "-", "+")"#),
        eval(r#"SUBSTITUTE("a-b-c", "-", "+")"#)
    );
}
