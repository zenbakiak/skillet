//! BL-105: `::Array` on a string splits it into characters, exactly as `.to_a()` does
//! (Unicode scalars, ADR-0034 §5). Every other input keeps its 0.7 result.

use skillet::{evaluate, evaluate_with, Value};
use std::collections::HashMap;

fn strs(items: &[&str]) -> Value {
    Value::Array(items.iter().map(|s| Value::String((*s).into())).collect())
}

#[test]
fn string_cast_splits_into_characters() {
    assert_eq!(
        evaluate("\"hello\"::Array").unwrap(),
        strs(&["h", "e", "l", "l", "o"])
    );
    // By character, not byte: "é" is one item, as `.length()` counts it.
    assert_eq!(
        evaluate("\"héllo\"::Array").unwrap(),
        strs(&["h", "é", "l", "l", "o"])
    );
    assert_eq!(
        evaluate("(\"héllo\"::Array).length()").unwrap(),
        Value::Number(5.0)
    );
    assert_eq!(evaluate("\"\"::Array").unwrap(), Value::Array(vec![]));
}

#[test]
fn string_cast_matches_to_a() {
    for s in ["hello", "héllo", "", " a b ", "1,2,3", "日本語", "e\u{301}"] {
        let lit = format!("\"{s}\"");
        let cast = evaluate(&format!("{lit}::Array")).unwrap();
        let to_a = evaluate(&format!("{lit}.to_a()")).unwrap();
        assert_eq!(cast, to_a, "{lit}");
    }
    // A string variable too (the PRD-001 FR-4 case).
    let mut vars = HashMap::new();
    vars.insert("csv".into(), Value::String("1,2".into()));
    assert_eq!(
        evaluate_with("=:csv::Array", &vars).unwrap(),
        strs(&["1", ",", "2"])
    );
}

#[test]
#[allow(deprecated)] // `Value::Json` until 0.8 (ADR-0030)
fn other_inputs_keep_their_results() {
    let one = |v: Value| Value::Array(vec![v]);
    let cases = [
        ("NULL::Array", one(Value::Null)),
        ("5::Array", one(Value::Number(5.0))),
        ("1.5::Array", one(Value::Number(1.5))),
        ("TRUE::Array", one(Value::Boolean(true))),
        (
            "DATE(2026, 1, 1)::Array",
            one(Value::DateTime(1_767_225_600)),
        ),
        (
            "[1, \"a\"]::Array",
            Value::Array(vec![Value::Number(1.0), "a".into()]),
        ),
        ("[]::Array", Value::Array(vec![])),
        ("{\"a\": 1}::Array", one(Value::Json("{\"a\":1.0}".into()))),
    ];
    for (src, want) in cases {
        assert_eq!(evaluate(src).unwrap(), want, "{src}");
    }
    // NULL differs from `.to_a()` (which gives `[]`) and keeps doing so.
    assert_eq!(evaluate("NULL.to_a()").unwrap(), Value::Array(vec![]));
}
