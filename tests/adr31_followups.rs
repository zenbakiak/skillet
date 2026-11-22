//! ADR-0031 amendment 1 (BL-078): every example in the amendment.
//!
//! - A1: casts chain left to right (`atom link* cast*`), with positioned cast errors.
//! - A2: `.to_json()` on a date is the ISO text; `::Json` stays a number.
//! - A3: `.join()` writes `NULL` as `JOIN` does; other items keep Ruby text. A `::Currency`
//!   item is a Number since ADR-0033.
//! - A4: `LEN` and `FIND` on a date stay errors.
//! - A5 (superseded by ADR-0033): `SUMIF`/`AVERAGEIF` with `::Currency` values return Numbers.

use skillet::{evaluate, evaluate_with_json, Error, Value};

fn eval(expr: &str) -> Value {
    evaluate(expr).unwrap_or_else(|e| panic!("{expr}: unexpected error {e:?}"))
}

fn err(expr: &str) -> Error {
    match evaluate(expr) {
        Err(e) => e,
        Ok(v) => panic!("{expr}: expected an error, got {v:?}"),
    }
}

fn s(text: &str) -> Value {
    Value::String(text.to_string())
}

fn assert_err_at(expr: &str, message: &str, position: usize) {
    let e = err(expr);
    assert_eq!(e.message, message, "{expr}");
    assert_eq!(e.position, Some(position), "{expr}");
}

// ---- A1. Chained casts ---------------------------------------------------------------------

#[test]
fn casts_chain_left_to_right() {
    assert_eq!(
        eval("DATE(2026,1,1)::String::DateTime"),
        Value::DateTime(1767225600)
    );
    assert_eq!(
        eval("DATE(2026,1,1)::String::DateTime == DATE(2026,1,1)"),
        Value::Boolean(true)
    );
    assert_eq!(eval("1::String::Integer + 1"), Value::Number(2.0));
    assert_eq!(eval("\"2\"::Integer::String"), s("2"));
    // ADR-0033 §6: `::Currency` gives a Number, so its text is the number's text.
    assert_eq!(eval("1.5::Currency::String"), s("1.5"));
    assert_eq!(
        eval("1.5::Currency::String"),
        eval("(1.5::Currency)::String")
    );
    assert_eq!(eval("NULL::String::Integer"), Value::Number(0.0));
    assert_eq!(eval("1::String::Integer::Float"), Value::Number(1.0));
}

#[test]
fn prefix_ops_power_and_not_take_the_whole_cast_chain() {
    assert_eq!(eval("-\"2\"::String::Integer"), Value::Number(-2.0));
    assert_eq!(eval("2^\"3\"::String::Integer"), Value::Number(8.0));
    assert_eq!(eval("NOT 1::String::Boolean"), Value::Boolean(false));
    assert_eq!(eval("NOT 1::String::Boolean"), eval("NOT (\"1\"::Boolean)"));
}

#[test]
fn casts_apply_to_the_whole_link_chain() {
    let v = evaluate_with_json(":u&.name::String::Integer", r#"{"u": null}"#)
        .unwrap_or_else(|e| panic!("unexpected error {e:?}"));
    assert_eq!(v, Value::Number(0.0));
    assert_eq!(eval("(1::String).length()"), Value::Number(1.0));
}

#[test]
fn links_and_percent_after_a_cast_stay_errors() {
    let msg = "Unexpected token after end of expression";
    assert_err_at("=1::String.length()", msg, 10);
    assert_err_at("1::String.length()", msg, 9);
    assert_err_at("1::String::Integer[0]", msg, 18);
    assert_eq!(err("10%::String").message, msg);
}

#[test]
fn cast_errors_point_at_the_token_after_the_colons() {
    assert_err_at("=1::String::Foo", "Unknown cast type", 12);
    assert_err_at("  =1::String::Foo", "Unknown cast type", 14);
    assert_err_at("=1::Foo", "Unknown cast type", 4);
    assert_err_at("=1::String::5", "Expected type name after '::'", 12);
    // End of input: the start of the last token, as `=1 +` reports.
    assert_err_at("=1::String::", "Expected type name after '::'", 10);
    assert_eq!(err("=1 +").position, Some(3));
}

#[test]
fn every_cast_counts_against_the_nesting_limit() {
    let casts = |n: usize| format!("1{}", "::String".repeat(n));
    assert_eq!(eval(&casts(128)), s("1"));
    let e = err(&casts(129));
    assert_eq!(e.message, "Expression nested too deeply (max 128)");
    assert!(e.position.is_some());

    // Same limit as method links.
    let links = |n: usize| format!("\"a\"{}", ".upper()".repeat(n));
    assert_eq!(eval(&links(128)), s("A"));
    assert_eq!(err(&links(129)).message, e.message);
}

// ---- A2. `.to_json()` on a date --------------------------------------------------------------

#[test]
fn to_json_on_a_date_is_iso_text_and_json_cast_a_number() {
    assert_eq!(
        eval("DATE(2026,1,1).to_json()"),
        Value::Json("\"2026-01-01T00:00:00Z\"".to_string())
    );
    assert_eq!(
        eval("DATE(2026,1,1)::Json"),
        Value::Json("1767225600".to_string())
    );
}

// ---- A3. `.join()` with Currency (a Number since ADR-0033) and NULL ------------------------

#[test]
fn join_method_writes_currency_and_null_like_join() {
    assert_eq!(eval("[2.5::Currency].join(\"-\")"), s("2.5"));
    assert_eq!(
        eval("[1::Currency, -2.25::Currency].join(\"; \")"),
        s("1; -2.25")
    );
    assert_eq!(eval("[1, 2.5::Currency, NULL].join(\",\")"), s("1,2.5,"));
    assert_eq!(
        eval("[1, 2.5::Currency, NULL].join(\",\")"),
        eval("JOIN([1, 2.5::Currency, NULL], \",\")")
    );
    assert_eq!(eval("[1, NULL, 2].join(\"-\")"), s("1--2"));
    assert_eq!(eval("[NULL].join()"), s(""));
}

#[test]
fn join_method_keeps_its_other_behaviour() {
    assert_eq!(eval("[TRUE, 1].join()"), s("true,1"));
    assert_eq!(eval("JOIN([TRUE, 1])"), s("TRUE,1"));
    assert_eq!(
        err("[[1,2],3].join()").message,
        "join method cannot convert value to string"
    );
    assert_eq!(eval("[].join()"), s(""));
    assert_eq!(eval("[1.5, \"a\"].join()"), s("1.5,a"));
    assert_eq!(eval("[DATE(2026,1,1)].join()"), s("2026-01-01T00:00:00Z"));
}

// ---- A4. `LEN` and `FIND` on a date ----------------------------------------------------------

#[test]
fn len_and_find_on_a_date_stay_errors() {
    assert_eq!(err("LEN(DATE(2026,1,1))").message, "LEN expects text");
    assert_eq!(
        err("FIND(\"2\", DATE(2026,1,1))").message,
        "FIND expects within_text as text"
    );
    assert_eq!(eval("LEN(DATE(2026,1,1)::String)"), Value::Number(20.0));
}

// ---- A5. SUMIF / AVERAGEIF: Numbers since ADR-0033 (A5 superseded) -------------------------

#[test]
fn sumif_with_currency_values_is_a_number() {
    let c = Value::Number;
    assert_eq!(eval("SUMIF([1,2::Currency,3], \">1\")"), c(5.0));
    assert_eq!(eval("SUMIF([1::Currency,2::Currency], \">0\")"), c(3.0));
    assert_eq!(
        eval("SUMIF([1,2,3], \">1\", [10, 20::Currency, 30])"),
        c(50.0)
    );
    assert_eq!(eval("SUMIF([1, 2], :x > 1, [10, 20::Currency])"), c(20.0));
    assert_eq!(eval("SUMIF([1::Currency, \"a\", 2], \">0\")"), c(3.0));
}

#[test]
fn sumif_stays_a_number_when_no_summed_value_is_currency() {
    let n = Value::Number;
    // The Currency is not matched.
    assert_eq!(eval("SUMIF([1::Currency,2,3], \">1\")"), n(5.0));
    assert_eq!(eval("SUMIF([1,2,3], \">1\")"), n(5.0));
    // 10 is not summed.
    assert_eq!(
        eval("SUMIF([1,2,3], \">1\", [10::Currency, 20, 30])"),
        n(50.0)
    );
    // Currency only in the criteria range.
    assert_eq!(
        eval("SUMIF([1::Currency,2,3], \">0\", [10, 20, 30])"),
        n(60.0)
    );
    assert_eq!(eval("SUMIF([1::Currency], \">5\")"), n(0.0));
    assert_eq!(eval("SUMIF([], \">0\")"), n(0.0));
}

#[test]
fn averageif_follows_the_same_rule() {
    let (c, n) = (Value::Number, Value::Number);
    assert_eq!(eval("AVERAGEIF([1,2::Currency,3], \">1\")"), c(2.5));
    assert_eq!(eval("AVERAGEIF([1::Currency,2::Currency], \">0\")"), c(1.5));
    assert_eq!(eval("AVERAGEIF([1::Currency,2,3], \">1\")"), n(2.5));
    assert_eq!(
        eval("AVERAGEIF([1,2,3], \">1\", [10::Currency, 20, 30])"),
        n(25.0)
    );
    assert_eq!(
        eval("AVERAGEIF([1,2,3], \">1\", [10, 20::Currency, 30])"),
        c(25.0)
    );
    assert_eq!(
        err("AVERAGEIF([1,2,3], \">5\", [1::Currency,2,3])").message,
        "AVERAGEIF: no matching numbers (#DIV/0!)"
    );
}

#[test]
fn countif_is_always_a_number() {
    assert_eq!(
        eval("COUNTIF([1::Currency,2::Currency], \">1\")"),
        Value::Number(1.0)
    );
}
