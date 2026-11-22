//! ADR-0034 acceptance: one text form per value type (BL-091).
//!
//! One test per row of the ADR's behaviour-change table, plus the `&` operator rules (§6), the
//! date fast path (§7) and the budget charge.

use skillet::ast::{BinaryOp, Expr};
use skillet::{evaluate, evaluate_with, parse, with_limits, EvalLimits, Value};
use std::collections::HashMap;

fn eval(expr: &str) -> Value {
    evaluate(expr).unwrap_or_else(|e| panic!("{expr}: {}", e.message))
}

fn text(expr: &str) -> String {
    match eval(expr) {
        Value::String(s) => s,
        other => panic!("{expr}: expected a String, got {other:?}"),
    }
}

fn err(expr: &str) -> String {
    match evaluate(expr) {
        Err(e) => e.message,
        Ok(v) => panic!("{expr}: expected an error, got {v:?}"),
    }
}

/// Every expression in `exprs` evaluates to the String `want`.
fn all_text(exprs: &[&str], want: &str) {
    for e in exprs {
        assert_eq!(text(e), want, "{e}");
    }
}

#[test]
fn every_surface_rounds_to_15_digits() {
    all_text(
        &[
            "(0.1 + 0.2)::String",
            "(0.1 + 0.2).to_s()",
            "(0.1 + 0.2).to_string()",
            "\"\" & (0.1 + 0.2)",
            "CONCAT(0.1 + 0.2)",
            "JOIN([0.1 + 0.2])",
            "[0.1 + 0.2].join()",
            "TEXTJOIN(\",\", TRUE, 0.1 + 0.2)",
        ],
        "0.3",
    );
    assert_eq!(text("\"Total: \" & 3 * 0.1"), "Total: 0.3");
}

#[test]
fn long_fractions() {
    assert_eq!(text("(1/3)::String"), "0.333333333333333");
    assert_eq!(text("SQRT(2) & \"\""), "1.4142135623731");
}

#[test]
fn large_and_small_numbers_use_e_notation() {
    assert_eq!(text("1e21::String"), "1E+21");
    assert_eq!(text("1e100.to_s()"), "1E+100");
    assert_eq!(text("12345678901234567890 & \"\""), "1.23456789012346E+19");
    assert_eq!(text("1.5e-8 & \"\""), "1.5E-08");
    assert_eq!(text("1e-7 & \"\""), "0.0000001");
    // The text reads back as a number.
    assert_eq!(eval("\"1E+21\"::Float"), Value::Number(1e21));
}

#[test]
fn exact_integers_and_short_numbers_are_unchanged() {
    assert_eq!(text("5::String"), "5");
    assert_eq!(text("1e15 & \"\""), "1000000000000000");
    assert_eq!(text("1234567890123456 & \"\""), "1234567890123456");
    assert_eq!(text("123456.789 & \"\""), "123456.789");
    assert_eq!(text("(-42).to_s()"), "-42");
}

#[test]
fn negative_zero_is_zero() {
    assert_eq!(text("(0 - 0.0) * -1 & \"\""), "0");
    assert_eq!(text("((0 - 0.0) * -1)::String"), "0");
    assert_eq!(eval("LEN((0 - 0.0) * -1)"), Value::Number(1.0));
}

#[test]
fn len_counts_the_text_form() {
    assert_eq!(eval("LEN(0.1 + 0.2)"), Value::Number(3.0));
    assert_eq!(eval("LEN(1/3)"), Value::Number(17.0));
    assert_eq!(eval("FIND(\"3\", 0.1 + 0.2)"), Value::Number(3.0));
}

#[test]
fn booleans_are_upper_case_everywhere() {
    assert_eq!(text("TRUE.to_s()"), "TRUE");
    assert_eq!(text("FALSE.to_string()"), "FALSE");
    assert_eq!(text("[TRUE, 1].join()"), "TRUE,1");
    assert_eq!(text("JOIN([TRUE, 1])"), "TRUE,1");
}

#[test]
fn array_text_is_json() {
    let want = r#"["a",1,true,null]"#;
    assert_eq!(text(r#"["a", 1, TRUE, NULL]::String"#), want);
    assert_eq!(text(r#"["a", 1, TRUE, NULL].to_s()"#), want);
    let date = r#"["2026-01-01T00:00:00Z"]"#;
    assert_eq!(text("[DATE(2026,1,1)]::String"), date);
    assert_eq!(text("[DATE(2026,1,1)].to_s()"), date);
    assert_eq!(text("[[1, 2], 3].to_s()"), "[[1,2],3]");
    assert_eq!(text("[]::String"), "[]");
    // Strings are quoted and escaped, so items with commas are not ambiguous.
    assert_eq!(text(r#"["a, b", "c\"d"]::String"#), r#"["a, b","c\"d"]"#);
}

#[test]
fn numbers_in_array_text_keep_full_precision() {
    assert_eq!(
        text("[0.1 + 0.2, 1e21]::String"),
        "[0.30000000000000004,1e+21]"
    );
    assert_eq!(text("[(0 - 0.0) * -1, 1e-7]::String"), "[-0.0,1e-7]");
}

#[test]
fn objects_in_array_text() {
    assert_eq!(text(r#"[{"a":1}]::String"#), r#"[{"a":1.0}]"#);
    assert_eq!(text(r#"[{"a":1}].to_s()"#), r#"[{"a":1.0}]"#);
}

#[test]
fn array_text_reads_back_with_json_cast() {
    let back = eval(r#"(["a", 1, TRUE, NULL]::String)::Json"#);
    assert_eq!(back, Value::Json(r#"["a",1,true,null]"#.into()));
}

#[test]
fn to_json_writes_integral_numbers_as_integers() {
    assert_eq!(eval("[1.5, 5].to_json()"), Value::Json("[1.5,5]".into()));
    assert_eq!(eval("5.to_json()"), Value::Json("5".into()));
    assert_eq!(
        eval("(0.1 + 0.2).to_json()"),
        Value::Json("0.30000000000000004".into())
    );
}

#[test]
fn objects_join_as_their_json_text() {
    let want = r#"x{"a":1.0}"#;
    assert_eq!(text(r#"CONCAT("x", {"a":1})"#), want);
    assert_eq!(text(r#""x" & {"a":1}"#), want);
    assert_eq!(text(r#"[{"a":1}].join()"#), r#"{"a":1.0}"#);
    assert_eq!(text(r#"TEXTJOIN(",", TRUE, {"a":1})"#), r#"{"a":1.0}"#);
}

#[test]
fn string_length_counts_characters() {
    assert_eq!(eval("\"héllo\".length()"), Value::Number(5.0));
    assert_eq!(eval("\"héllo\".len()"), Value::Number(5.0));
    assert_eq!(eval("LEN(\"héllo\")"), Value::Number(5.0));
}

#[test]
fn concat_internal_name_is_gone() {
    let unknown = "Unknown function: __CONCAT__";
    assert_eq!(err("__CONCAT__(\"a\", 1)"), unknown);
    assert_eq!(err("__CONCAT__(\"a\")"), unknown);
    assert!(err("__concat__(\"a\", 1)").starts_with("Unknown function"));
}

#[test]
fn ampersand_parses_to_a_binary_node() {
    let expr = parse("\"a\" & 1").unwrap();
    assert_eq!(
        expr,
        Expr::Binary(
            Expr::StringLit("a".into()).into(),
            BinaryOp::Concat,
            Expr::Number(1.0).into()
        )
    );
}

#[test]
fn unchanged_rows() {
    assert_eq!(text("\"a\" & 1 & TRUE"), "a1TRUE");
    assert_eq!(text("CONCAT(\"a\", [1, 2])"), "a12");
    assert_eq!(text("\"a\" & NULL"), "a");
    assert_eq!(text("DATE(2026,1,1) & \"\""), "2026-01-01T00:00:00Z");
    assert_eq!(err("JOIN([[1],2])"), "JOIN does not flatten nested arrays");
}

#[test]
fn ampersand_precedence_and_errors() {
    assert_eq!(text("\"a\" & 1 + 2"), "a3");
    assert_eq!(eval("1 & 2 == \"12\""), Value::Boolean(true));
    assert_eq!(text("\"a\" & [1, [2, 3]]"), "a123");
    assert_eq!(err("1 & (1/0)"), "Division by zero (#DIV/0!)");
    let parse_error = evaluate("=\"a\" &").unwrap_err();
    assert_eq!(
        (parse_error.message.as_str(), parse_error.position),
        ("Unexpected token: Eof", Some(5))
    );
}

#[test]
fn ampersand_borrows_variables_and_is_charged() {
    let big = "x".repeat(1000);
    let vars = HashMap::from([("s".to_string(), Value::String(big.clone()))]);
    let joined = evaluate_with(":s & :s", &vars).unwrap();
    assert_eq!(joined, Value::String(big.repeat(2)));
    // The 2000-byte result does not fit a budget smaller than it.
    let mut limits = EvalLimits::default();
    limits.max_bytes = 1500;
    let e = with_limits(limits, || evaluate_with(":s & :s", &vars)).unwrap_err();
    assert!(
        e.message.starts_with("Evaluation budget exceeded"),
        "{}",
        e.message
    );
}

#[test]
fn dates_across_the_fast_path_edges() {
    assert_eq!(text("DATE(9999, 12, 31)::String"), "9999-12-31T00:00:00Z");
    assert_eq!(text("DATE(2024, 2, 29).to_s()"), "2024-02-29T00:00:00Z");
    assert_eq!(
        text("(DATE(1970, 1, 1) - 1)::String"),
        "1969-12-31T00:00:00Z"
    );
    let vars = HashMap::from([
        ("first".to_string(), Value::DateTime(-62_167_219_200)),
        ("lo".to_string(), Value::DateTime(-62_167_219_201)),
        ("hi".to_string(), Value::DateTime(253_402_300_800)),
    ]);
    let s = |e: &str| evaluate_with(e, &vars).unwrap();
    assert_eq!(
        s(":first::String"),
        Value::String("0000-01-01T00:00:00Z".into())
    );
    assert_eq!(
        s(":lo::String"),
        Value::String("-0001-12-31T23:59:59Z".into())
    );
    assert_eq!(
        s(":hi::String"),
        Value::String("+10000-01-01T00:00:00Z".into())
    );
    assert_eq!(
        s("[:lo, :hi].join(\" \")"),
        Value::String("-0001-12-31T23:59:59Z +10000-01-01T00:00:00Z".into())
    );
}
