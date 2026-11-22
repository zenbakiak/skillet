//! BL-069: the whole input must be one expression (or a `;` sequence). Tokens after it are a
//! parse error at the first extra token, on every entry point; before, they were dropped
//! (`=1 2` was `1`).

use skillet::{
    evaluate, evaluate_with, evaluate_with_assignments, evaluate_with_assignments_and_context,
    evaluate_with_custom, evaluate_with_json, evaluate_with_json_custom, parse, Error, Value,
};
use std::collections::HashMap;

const MSG: &str = "Unexpected token after end of expression";

/// `(input, byte position of the first extra token)`.
const TRAILING: &[(&str, usize)] = &[
    ("=1 2", 3),
    ("1 2", 2),
    ("  =  1 2", 7),
    ("1)", 1),
    ("1 ]", 2),
    ("(1) (2)", 4),
    ("SUM(1) SUM(2)", 7),
    ("'a' 'b'", 4),
    (":x :y", 3),
    ("TRUE FALSE", 5),
    ("1 + 2 3", 6),
    ("1 ? 2 : 3 4", 10),
    ("1; 2 3", 5),
    ("1; 2; 3 )", 8),
    (":a := 1 2", 8),
    (":a := 1; :a 2", 12),
    ("=42 as string", 4),
    ("[1, 2] [0]x", 10),
];

fn assert_trailing(name: &str, src: &str, pos: usize, got: Result<impl std::fmt::Debug, Error>) {
    let err = got.expect_err(&format!("{name}({src:?}) should fail"));
    assert_eq!(err.message, MSG, "{name}({src:?})");
    assert_eq!(err.position, Some(pos), "{name}({src:?})");
}

#[test]
fn parse_rejects_trailing_tokens_with_their_position() {
    for &(src, pos) in TRAILING {
        assert_trailing("parse", src, pos, parse(src));
    }
}

#[test]
fn every_text_entry_point_rejects_trailing_tokens() {
    let vars = HashMap::from([
        ("x".to_string(), Value::Number(1.0)),
        ("y".to_string(), Value::Number(2.0)),
    ]);
    for &(src, pos) in TRAILING {
        assert_trailing("evaluate", src, pos, evaluate(src));
        assert_trailing("evaluate_with", src, pos, evaluate_with(src, &vars));
        assert_trailing(
            "evaluate_with_custom",
            src,
            pos,
            evaluate_with_custom(src, &vars),
        );
        assert_trailing(
            "evaluate_with_json",
            src,
            pos,
            evaluate_with_json(src, "{}"),
        );
        assert_trailing(
            "evaluate_with_json_custom",
            src,
            pos,
            evaluate_with_json_custom(src, "{}"),
        );
        assert_trailing(
            "evaluate_with_assignments",
            src,
            pos,
            evaluate_with_assignments(src, &vars),
        );
        assert_trailing(
            "evaluate_with_assignments_and_context",
            src,
            pos,
            evaluate_with_assignments_and_context(src, &vars),
        );
    }
}

#[test]
fn complete_inputs_still_parse() {
    let ok = [
        ("1", 1.0),
        ("=1", 1.0),
        ("  = 1 + 2  ", 3.0),
        ("1;", 1.0),
        ("1; 2", 2.0),
        (":a := 2; :a * 3", 6.0),
        (":a := 2; :a * 3;", 6.0),
        ("(1)", 1.0),
        ("10%", 0.1),
        ("1 // comment", 1.0),
    ];
    for (src, want) in ok {
        assert_eq!(
            evaluate_with_assignments(src, &HashMap::new()).unwrap(),
            Value::Number(want),
            "{src}"
        );
    }
}

/// `AND`/`OR` between operands were dropped with the rest of the input (`TRUE AND FALSE` was
/// `TRUE`); they are now the logical operators, as documented. `AND(...)` stays the builtin.
#[test]
fn and_or_keywords_are_operators_after_an_operand() {
    let cases = [
        ("TRUE AND FALSE", false),
        ("TRUE OR FALSE", true),
        ("FALSE or TRUE", true),
        ("(2 > 1) AND (1 < 2)", true),
        ("(2 > 3) AND (1 < 2)", false),
        ("1 + 2 > 2 AND 3 < 4", true),
        ("TRUE OR FALSE AND FALSE", true), // AND binds tighter than OR
        ("AND(TRUE, FALSE) OR TRUE", true),
        ("TRUE AND OR(FALSE, TRUE)", true),
    ];
    for (src, want) in cases {
        assert_eq!(evaluate(src).unwrap(), Value::Boolean(want), "{src}");
    }
}

#[test]
fn sk_cli_rejects_trailing_tokens() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_sk"))
        .arg("=1 2")
        .env(
            "SKILLET_HOOKS_DIR",
            std::env::temp_dir().join("skillet-eoi-no-hooks"),
        )
        .output()
        .expect("run sk");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("Unexpected token after end of expression at position 3"),
        "{stderr}"
    );
}
