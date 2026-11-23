//! ADR-0031 (BL-076): a DateTime as text is ISO 8601 UTC. Its aggregate Currency rule (B-13) is
//! superseded by ADR-0033: there is one number type.
//!
//! 1. `::String`, `.to_s()`, `CONCAT`, `&`, `JOIN`, `.join()`, `TEXTJOIN` and `.to_json()` write
//!    a DateTime as `2026-01-01T00:00:00Z`. `::DateTime` reads that text back.
//! 2. `SUM`, `PRODUCT`, `AVERAGE`, `MIN` and `MAX` with `::Currency` values return Numbers.
//!    Lowercase methods are unchanged.

use skillet::{evaluate, evaluate_with, Value};
use std::collections::HashMap;

fn eval(expr: &str) -> Value {
    evaluate(expr).unwrap_or_else(|e| panic!("{expr}: unexpected error {e:?}"))
}

fn string(expr: &str) -> String {
    match eval(expr) {
        Value::String(s) => s,
        other => panic!("{expr}: expected a string, got {other:?}"),
    }
}

fn err(expr: &str) -> String {
    match evaluate(expr) {
        Err(e) => e.message,
        Ok(v) => panic!("{expr}: expected an error, got {v:?}"),
    }
}

const ISO: &str = "2026-01-01T00:00:00Z";

// ---- 1. DateTime as text ----------------------------------------------------------------

#[test]
fn every_text_path_writes_iso_8601_utc() {
    assert_eq!(string("DATE(2026, 1, 1)::String"), ISO);
    assert_eq!(string("DATE(2026, 1, 1).to_s()"), ISO);
    assert_eq!(string("DATE(2026, 1, 1).to_string()"), ISO);
    assert_eq!(string("CONCAT(DATE(2026, 1, 1))"), ISO);
    assert_eq!(string("DATE(2026, 1, 1) & ''"), ISO);
    assert_eq!(
        string("JOIN([DATE(2026, 1, 1), 'x'], '|')"),
        format!("{ISO}|x")
    );
    assert_eq!(
        string("[DATE(2026, 1, 1), 'x'].join('|')"),
        format!("{ISO}|x")
    );
    assert_eq!(
        string("TEXTJOIN(', ', TRUE, DATE(2026, 1, 1), 'x')"),
        format!("{ISO}, x")
    );
}

#[test]
fn time_of_day_and_old_dates() {
    assert_eq!(
        string("(DATE(2026, 1, 1) + 0.5)::String"),
        "2026-01-01T12:00:00Z"
    );
    assert_eq!(
        string("(DATE(2026, 1, 1) + TIME(23, 59, 59))::String"),
        "2026-01-01T23:59:59Z"
    );
    assert_eq!(string("DATE(1900, 1, 1)::String"), "1900-01-01T00:00:00Z");
    assert_eq!(
        string("(DATE(1970, 1, 1) - 1)::String"),
        "1969-12-31T00:00:00Z"
    );
    // Whole seconds only: NOW() is `YYYY-MM-DDTHH:MM:SSZ`.
    let now = string("NOW()::String");
    assert_eq!(now.len(), 20, "{now}");
    assert!(now.ends_with('Z'), "{now}");
}

#[test]
#[allow(deprecated)] // `Value::Json` is deprecated until 0.8 (ADR-0030)
fn arrays_and_json() {
    // An array's text is its JSON text, a date in it a JSON string (ADR-0034 §4).
    assert_eq!(
        string("[DATE(2026, 1, 1), 1].to_s()"),
        format!("[\"{ISO}\",1]")
    );
    assert_eq!(
        string("CONCAT('on ', [DATE(2026, 1, 1)])"),
        format!("on {ISO}")
    );
    // `.to_json()` already wrote a JSON string; that string is now the ISO text.
    assert_eq!(
        eval("DATE(2026, 1, 1).to_json()"),
        Value::Json(format!("\"{ISO}\""))
    );
    assert_eq!(
        eval("[DATE(2026, 1, 1)].to_json()"),
        Value::Json(format!("[\"{ISO}\"]"))
    );
    // The `::Json` cast writes a JSON number; it is not text and is unchanged.
    assert_eq!(
        eval("DATE(2026, 1, 1)::Json"),
        Value::Json("1767225600".into())
    );
}

#[test]
fn string_cast_round_trips() {
    assert_eq!(
        eval("(DATE(2026, 1, 1)::String)::DateTime == DATE(2026, 1, 1)"),
        Value::Boolean(true)
    );
    assert_eq!(
        eval("((DATE(2026, 1, 1) + 0.75)::String)::DateTime"),
        Value::DateTime(1_767_225_600 + 64_800)
    );
    assert_eq!(
        eval("'2026-01-01T10:30:00Z'::DateTime"),
        Value::DateTime(1_767_263_400)
    );
    // Integer text stays Unix seconds.
    assert_eq!(
        eval("'1767225600'::DateTime"),
        Value::DateTime(1_767_225_600)
    );
}

#[test]
fn round_trips_outside_date_range() {
    let mut vars = HashMap::new();
    for ts in [-62_167_219_200_i64, 253_402_300_800, i64::MAX, i64::MIN] {
        vars.insert("t".to_string(), Value::DateTime(ts));
        assert_eq!(
            evaluate_with("(:t::String)::DateTime", &vars).unwrap(),
            Value::DateTime(ts),
            "{ts}"
        );
    }
    vars.insert("t".to_string(), Value::DateTime(253_402_300_800));
    assert_eq!(
        evaluate_with(":t::String", &vars).unwrap(),
        Value::String("+10000-01-01T00:00:00Z".into())
    );
}

#[test]
fn string_cast_is_strict() {
    let msg = "Cannot cast String to DateTime";
    for text in [
        "2026-01-01",
        "2026-01-01T00:00:00",
        "2026-01-01 00:00:00Z",
        "2026-01-01T00:00:00+00:00",
        "2026-01-01T00:00:00.5Z",
        "2026-02-30T00:00:00Z",
        "2026-01-01T24:00:00Z",
        " 2026-01-01T00:00:00Z",
        "1767225600.5",
    ] {
        assert_eq!(err(&format!("'{text}'::DateTime")), msg, "{text}");
    }
}

// ---- 2. Currency in the aggregates: Numbers since ADR-0033 (supersedes ADR-0031 decision 3) ---

#[test]
fn sum_with_currency_is_a_number() {
    assert_eq!(eval("SUM(10::Currency, 5)"), Value::Number(15.0));
    assert_eq!(eval("SUM(10::Currency, 5::Currency)"), Value::Number(15.0));
    assert_eq!(eval("SUM([1, 2::Currency], 3)"), Value::Number(6.0));
    // Ignored values are still ignored.
    assert_eq!(
        eval("SUM([1::Currency, 'x', TRUE, NULL])"),
        Value::Number(1.0)
    );
    assert_eq!(eval("SUM(10, 5)"), Value::Number(15.0));
    assert_eq!(eval("SUM()"), Value::Number(0.0));
    // ADR-0029 amendment 2 (BL-107): direct text converts or is #VALUE!, it is not skipped.
    assert_eq!(
        err("SUM('a', TRUE)"),
        "SUM: cannot convert \"a\" to a number (#VALUE!)"
    );
}

#[test]
fn product_with_currency_is_a_number() {
    assert_eq!(eval("PRODUCT(2::Currency, 3)"), Value::Number(6.0));
    assert_eq!(
        eval("MULTIPLY([2::Currency], 3::Currency)"),
        Value::Number(6.0)
    );
    assert_eq!(eval("PRODUCT(2, 3)"), Value::Number(6.0));
    assert_eq!(eval("PRODUCT()"), Value::Number(1.0));
}

#[test]
fn average_min_max_with_currency_are_numbers() {
    assert_eq!(
        eval("AVERAGE(10::Currency, 20::Currency)"),
        Value::Number(15.0)
    );
    assert_eq!(eval("AVG(10::Currency, 20)"), Value::Number(15.0));
    assert_eq!(eval("AVERAGE(10, 20)"), Value::Number(15.0));
    assert_eq!(eval("MIN(3::Currency, 1::Currency)"), Value::Number(1.0));
    assert_eq!(eval("MAX([3::Currency, 9::Currency])"), Value::Number(9.0));
    assert_eq!(eval("MIN(1, 3::Currency)"), Value::Number(1.0));
    assert_eq!(eval("MAX(1, 3::Currency)"), Value::Number(3.0));
    assert_eq!(eval("MIN(1, 3)"), Value::Number(1.0));
    assert_eq!(eval("MAX()"), Value::Number(0.0));
    // ADR-0029 amendment 2 (BL-107): direct text converts or is #VALUE!; in an array it is
    // still skipped.
    assert_eq!(
        err("AVERAGE('a')"),
        "AVERAGE: cannot convert \"a\" to a number (#VALUE!)"
    );
    assert_eq!(err("AVERAGE(['a'])"), "AVERAGE of no numbers (#DIV/0!)");
}

#[test]
fn methods_are_unchanged() {
    assert_eq!(eval("[1::Currency, 2].sum()"), Value::Number(3.0));
    assert_eq!(eval("[1::Currency, 2::Currency].avg()"), Value::Number(1.5));
    assert_eq!(eval("[1::Currency, 3::Currency].max()"), Value::Number(3.0));
}
