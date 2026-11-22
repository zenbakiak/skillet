//! ADR-0029 (BL-073): the `NOT` keyword, short-circuit `AND`/`OR` operators, Excel `AND`/`OR`/
//! `NOT`/`XOR` builtins, and the `IF`/`IFS` condition rules of ADR-0018 D-8 (B-18).

use skillet::{evaluate, evaluate_with, parse, Value};
use std::collections::HashMap;

fn ok(src: &str) -> Value {
    evaluate(src).unwrap_or_else(|e| panic!("{src}: {}", e.message))
}

fn is(src: &str, want: bool) {
    assert_eq!(ok(src), Value::Boolean(want), "{src}");
}

fn err(src: &str) -> String {
    match evaluate(src) {
        Ok(v) => panic!("{src}: expected an error, got {v:?}"),
        Err(e) => e.message,
    }
}

fn err_has(src: &str, part: &str) {
    let msg = err(src);
    assert!(
        msg.contains(part),
        "{src}: {msg:?} does not contain {part:?}"
    );
}

fn vars(pairs: &[(&str, Value)]) -> HashMap<String, Value> {
    pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.clone()))
        .collect()
}

// --- NOT keyword -------------------------------------------------------------------------

#[test]
fn not_keyword_in_any_case() {
    is("NOT TRUE", false);
    is("NOT FALSE", true);
    is("not true", false);
    is("Not FALSE", true);
    is("not (0)", true);
    is("NOT (1)", false);
    is("NOT NOT TRUE", true);
    is("NOT NULL", true);
}

#[test]
fn not_keyword_matches_the_builtin() {
    for operand in ["TRUE", "FALSE", "0", "2", "NULL", "(1 > 2)"] {
        assert_eq!(
            evaluate(&format!("NOT {operand}")).map_err(|e| e.message),
            evaluate(&format!("NOT({operand})")).map_err(|e| e.message),
            "NOT {operand}"
        );
    }
    let text = err("NOT 'x'");
    assert_eq!(text, err("NOT('x')"));
    assert!(text.contains("#VALUE!"), "{text}");
}

#[test]
fn not_followed_directly_by_paren_is_the_call() {
    // The call is an atom, so a method applies to its result.
    assert_eq!(ok("NOT(TRUE).to_s()"), Value::String("false".into()));
    // The keyword takes the postfix chain as its operand: NOT ((TRUE).to_s()) is text.
    err_has("NOT (TRUE).to_s()", "#VALUE!");
    err_has("NOT(1, 2)", "NOT expects 1 argument");
    assert!(evaluate("NOT (1, 2)").is_err());
}

#[test]
fn not_keyword_has_the_precedence_of_bang() {
    is("NOT TRUE AND FALSE", false); // (NOT TRUE) AND FALSE
    is("NOT FALSE AND TRUE", true);
    is("TRUE AND NOT FALSE", true);
    is("FALSE OR NOT FALSE", true);
    is("NOT (1 > 2)", true);
    // Like `!:x > 1`, `NOT :x > 1` is `(NOT :x) > 1`.
    let v = vars(&[("x", Value::Number(5.0))]);
    let keyword = evaluate_with("NOT :x > 1", &v).map_err(|e| e.message);
    assert!(keyword.is_err(), "{keyword:?}");
    assert_eq!(
        evaluate_with("NOT (:x > 1)", &v).unwrap(),
        Value::Boolean(false)
    );
    // The AST is the same as for the call.
    assert_eq!(parse("NOT :x").unwrap(), parse("NOT(:x)").unwrap());
}

#[test]
fn not_needs_an_operand() {
    assert!(evaluate("NOT").is_err());
    assert!(evaluate("TRUE AND NOT").is_err());
}

// --- short-circuit operators ---------------------------------------------------------------

#[test]
fn and_or_operators_short_circuit() {
    is("FALSE AND 1/0", false);
    is("TRUE OR 1/0", true);
    is("FALSE && 1/0", false);
    is("TRUE || 1/0", true);
    is("false and UPPER(1)", false);
    is("TRUE or FIRST([])", true);
    // A missing variable on the side not evaluated is not an error.
    is("FALSE AND :undefined", false);
    let v = vars(&[("n", Value::Number(0.0))]);
    assert_eq!(
        evaluate_with(":n == 0 OR 10 / :n > 1", &v).unwrap(),
        Value::Boolean(true)
    );
}

#[test]
fn and_or_operators_evaluate_the_right_side_when_needed() {
    err_has("TRUE AND 1/0", "#DIV/0!");
    err_has("FALSE OR 1/0", "#DIV/0!");
    is("TRUE AND FALSE", false);
    is("FALSE OR TRUE", true);
    is("TRUE && TRUE", true);
    is("FALSE || FALSE", false);
}

#[test]
fn and_or_operators_still_take_booleans() {
    err_has("1 AND TRUE", "Logical op on non-boolean");
    err_has("TRUE AND 1", "Logical op on non-boolean");
    err_has("'x' OR TRUE", "Logical op on non-boolean");
}

// --- AND / OR / XOR builtins ---------------------------------------------------------------

#[test]
fn and_or_builtins_follow_excel() {
    is("AND(TRUE, 1)", true);
    is("AND(TRUE, 1, 0)", false);
    is("OR(FALSE, 0)", false);
    is("OR(FALSE, 2)", true);
    is("AND([TRUE, TRUE], TRUE)", true);
    is("OR([FALSE, [FALSE, TRUE]])", true);
    is("AND(1::Currency)", true);
}

#[test]
fn builtins_evaluate_every_argument() {
    err_has("AND(FALSE, 1/0)", "#DIV/0!");
    err_has("OR(TRUE, 1/0)", "#DIV/0!");
    // Excel checks every argument, so a later text argument is still #VALUE!.
    err_has("AND(FALSE, 'x')", "#VALUE!");
    err_has("OR(TRUE, 'x')", "#VALUE!");
}

#[test]
fn text_in_arrays_is_ignored_and_direct_text_is_an_error() {
    is("AND([TRUE, 'x'])", true);
    is("OR([FALSE, 'TRUE'])", false);
    is("AND([TRUE, NULL, 'x'], TRUE)", true);
    is("XOR([TRUE, 'x', FALSE])", true);
    err_has(
        "AND(TRUE, 'x')",
        "AND expects logical values, got string (#VALUE!)",
    );
    err_has("OR(FALSE, 'TRUE')", "#VALUE!");
    err_has("XOR(TRUE, 'x')", "#VALUE!");
    err_has("AND(DATE(2026, 1, 1))", "#VALUE!");
}

#[test]
fn no_logical_values_is_an_error() {
    for src in [
        "AND()",
        "OR()",
        "XOR()",
        "AND([])",
        "OR(['x', 'y'])",
        "AND(NULL)",
        "XOR([NULL])",
    ] {
        err_has(src, "no logical values (#VALUE!)");
    }
}

#[test]
fn null_arguments_are_ignored_like_blank_cells() {
    is("AND(TRUE, NULL)", true);
    is("OR(FALSE, NULL)", false);
    is("OR(NULL, TRUE)", true);
}

#[test]
fn xor_counts_true_values() {
    is("XOR(TRUE)", true);
    is("XOR(FALSE)", false);
    is("XOR(TRUE, FALSE)", true);
    is("XOR(TRUE, TRUE)", false);
    is("XOR(TRUE, TRUE, TRUE)", true);
    is("XOR(TRUE, TRUE, TRUE, TRUE)", false);
    is("XOR(1, 0, 3)", false);
    is("XOR([TRUE, TRUE], TRUE)", true);
}

#[test]
fn not_builtin_follows_excel() {
    is("NOT(0)", true);
    is("NOT(-1)", false);
    is("NOT(NULL)", true);
    err_has(
        "NOT('x')",
        "NOT expects a logical value, got string (#VALUE!)",
    );
    err_has("NOT([TRUE])", "#VALUE!");
    err_has("NOT()", "NOT expects 1 argument");
}

// --- IF / IFS conditions (B-18) ------------------------------------------------------------

#[test]
fn if_condition_rules() {
    assert_eq!(ok("IF(2, 'a', 'b')"), Value::String("a".into()));
    assert_eq!(ok("IF(0, 'a', 'b')"), Value::String("b".into()));
    assert_eq!(ok("IF(NULL, 'a', 'b')"), Value::String("b".into()));
    assert_eq!(ok("IF(1::Currency, 'a', 'b')"), Value::String("a".into()));
    err_has(
        "IF('x', 1, 2)",
        "IF condition must be TRUE, FALSE or a number, got string (#VALUE!)",
    );
    err_has("IF('TRUE', 1, 2)", "#VALUE!");
    err_has("IF([TRUE], 1, 2)", "got array (#VALUE!)");
    err_has("IF(DATE(2026, 1, 1), 1, 2)", "got datetime (#VALUE!)");
    // The branches are not evaluated when the condition is an error.
    err_has("IF('x', UPPER(1), 2)", "#VALUE!");
}

#[test]
fn ifs_condition_rules() {
    assert_eq!(ok("IFS(0, 'a', 3, 'b')"), Value::String("b".into()));
    assert_eq!(ok("IFS(NULL, 'a', TRUE, 'b')"), Value::String("b".into()));
    err_has("IFS(FALSE, 1)", "IFS: no condition is TRUE (#N/A)");
    err_has("IFS(FALSE, 1, 0, 2, NULL, 3)", "#N/A");
    err_has("IFS('x', 1, TRUE, 2)", "IFS condition must be");
    // Conditions after the first TRUE one are not evaluated.
    assert_eq!(ok("IFS(TRUE, 1, 'x', 2)"), Value::Number(1.0));
}
