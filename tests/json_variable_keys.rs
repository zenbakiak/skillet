//! BL-077 (threat model 2.2): variables built from JSON keys, in the library `*_json*` entry
//! points and `sk --json` (both its plain and its `:=`/`;` path). The whole object is the
//! reserved `:arguments`; a key named `arguments` does not replace it and is read as
//! `:arguments.arguments`. Two keys that sanitize to the same variable (`a-b`, `a_b`) are an
//! error instead of one silently replacing the other.

use skillet::{evaluate_with_json, evaluate_with_json_custom, Value};
use std::process::Command;

const SK_BIN: &str = env!("CARGO_BIN_EXE_sk");

/// Runs `sk EXPR --json JSON`; returns (stdout, stderr, exit code).
fn sk_json(expr: &str, json: &str) -> (String, String, i32) {
    let out = Command::new(SK_BIN)
        .args([expr, "--json", json])
        .env("SKILLET_HOOKS_DIR", "skillet-json-keys-no-hooks")
        .output()
        .unwrap();
    (
        String::from_utf8_lossy(&out.stdout).trim().to_string(),
        String::from_utf8_lossy(&out.stderr).trim().to_string(),
        out.status.code().unwrap_or(-1),
    )
}

const WITH_ARGUMENTS_KEY: &str = r#"{"x": 1, "arguments": {"a": 2}}"#;

#[test]
fn library_keeps_the_reserved_arguments_variable() {
    for eval in [evaluate_with_json, evaluate_with_json_custom] {
        let whole = eval(":arguments", WITH_ARGUMENTS_KEY).unwrap();
        let Value::Json(text) = whole else {
            panic!("expected Json, got {whole:?}")
        };
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed, serde_json::json!({"x": 1, "arguments": {"a": 2}}));
        assert_eq!(
            eval(":arguments.arguments.a", WITH_ARGUMENTS_KEY).unwrap(),
            Value::Number(2.0)
        );
        assert_eq!(
            eval("JQ(:arguments, '$.arguments.a')", WITH_ARGUMENTS_KEY).unwrap(),
            Value::Number(2.0)
        );
        assert_eq!(eval(":x", WITH_ARGUMENTS_KEY).unwrap(), Value::Number(1.0));
    }
}

#[test]
fn library_arguments_key_with_a_scalar_value_does_not_replace_the_object() {
    let json = r#"{"arguments": 5, "y": 2}"#;
    assert_eq!(
        evaluate_with_json(":arguments.y + :arguments.arguments", json).unwrap(),
        Value::Number(7.0)
    );
}

#[test]
fn sk_json_keeps_the_reserved_arguments_variable() {
    // Without `:=`/`;` sk uses the library entry point.
    let (out, err, code) = sk_json("=:arguments.arguments.a", WITH_ARGUMENTS_KEY);
    assert_eq!((out.as_str(), code), ("Number(2.0)", 0), "{err}");
    // With `:=`, sk builds the variables itself.
    let (out, err, code) = sk_json("=:v := :arguments.arguments.a; :v + :x", WITH_ARGUMENTS_KEY);
    assert_eq!((out.as_str(), code), ("Number(3.0)", 0), "{err}");
}

const KEY_COLLISION: &str = r#"JSON keys "a-b" and "a_b" both map to variable :a_b"#;

#[test]
fn library_rejects_keys_that_map_to_the_same_variable() {
    for eval in [evaluate_with_json, evaluate_with_json_custom] {
        let err = eval(":a_b", r#"{"a_b": 1, "a-b": 2}"#).unwrap_err();
        assert_eq!(err.message, KEY_COLLISION);
        assert_eq!(err.position, None);
        let err = eval("1", r#"{"x.y": 1, "x y": 2}"#).unwrap_err();
        assert_eq!(
            err.message,
            r#"JSON keys "x y" and "x.y" both map to variable :x_y"#
        );
    }
}

#[test]
fn library_keys_that_stay_distinct_after_sanitizing_still_work() {
    let json = r#"{"first-name": "Ada", "last_name": "L", "arguments": 1}"#;
    assert_eq!(
        evaluate_with_json(":first_name & :last_name", json).unwrap(),
        Value::String("AdaL".into())
    );
}

#[test]
fn sk_json_rejects_keys_that_map_to_the_same_variable() {
    let json = r#"{"a_b": 1, "a-b": 2}"#;
    // BL-096: a `--json` error exits 1 on both paths (the plain one used to exit 2).
    for expr in ["=:a_b", "=:v := :a_b; :v"] {
        let (out, err, code) = sk_json(expr, json);
        assert_eq!((out.as_str(), code), ("", 1), "{expr}: {err}");
        assert_eq!(err, format!("Error: {KEY_COLLISION}"), "{expr}");
    }
}

/// BL-096: invalid JSON and a non-object exit 1 with the same message on both paths; an
/// evaluation error still exits 2.
#[test]
fn sk_json_errors_exit_1_on_both_paths() {
    for expr in ["=1", "=:v := 1; :v"] {
        let (out, err, code) = sk_json(expr, "{not json");
        assert_eq!((out.as_str(), code), ("", 1), "{expr}: {err}");
        assert!(err.starts_with("Error: Invalid JSON: "), "{expr}: {err}");
        let (out, err, code) = sk_json(expr, "[1, 2]");
        assert_eq!((out.as_str(), code), ("", 1), "{expr}: {err}");
        assert_eq!(
            err, "Error: JSON must be an object with key-value pairs",
            "{expr}"
        );
    }
    for expr in ["=:missing", "=:v := :missing; :v"] {
        let (_, err, code) = sk_json(expr, r#"{"x": 1}"#);
        assert_eq!(code, 2, "{expr}: {err}");
    }
}
