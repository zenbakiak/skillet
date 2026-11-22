//! Parse, then evaluate with a fixed variable map under a small budget and deadline.
//! Any `Err` is fine; a panic, abort, stack overflow or a run past the deadline is a bug.
#![no_main]

#[path = "common.rs"]
mod common;

use libfuzzer_sys::fuzz_target;
use skillet::Value;
use std::collections::HashMap;
use std::sync::LazyLock;

static VARS: LazyLock<HashMap<String, Value>> = LazyLock::new(|| {
    let mut v = HashMap::new();
    v.insert("a".into(), Value::Number(3.0));
    v.insert("b".into(), Value::Number(-0.5));
    v.insert("s".into(), Value::String("Hello, World".into()));
    v.insert("t".into(), Value::Boolean(true));
    v.insert("n".into(), Value::Null);
    v.insert("c".into(), Value::Currency(12.5));
    v.insert("d".into(), Value::DateTime(1_700_000_000));
    v.insert(
        "arr".into(),
        Value::Array(vec![
            Value::Number(1.0),
            Value::Number(2.0),
            Value::Number(3.0),
            Value::String("x".into()),
            Value::Null,
        ]),
    );
    v.insert(
        "u".into(),
        Value::Json(r#"{"name":"ana","tags":["x","y"],"age":30,"addr":{"city":"Z"}}"#.into()),
    );
    v
});

fuzz_target!(|data: &[u8]| {
    let Ok(src) = std::str::from_utf8(data) else {
        return;
    };
    let Ok(expr) = skillet::parse(src) else {
        return;
    };
    let _ = common::limited("evaluate", || skillet::evaluate_parsed_with(&expr, &VARS));
});
