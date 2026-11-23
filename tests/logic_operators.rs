//! ADR-0029 (BL-073): the `NOT` keyword, short-circuit `AND`/`OR` operators, Excel `AND`/`OR`/
//! `NOT`/`XOR` builtins, and the `IF`/`IFS` condition rules of ADR-0018 D-8 (B-18).

use skillet::ast::{BinaryOp, Expr};
use skillet::{evaluate, evaluate_with, parse, Value};
use std::collections::HashMap;
use std::rc::Rc;

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
    assert_eq!(ok("NOT(TRUE).to_s()"), Value::String("FALSE".into()));
    // The keyword's operand includes the postfix chain: NOT ((TRUE).to_s()) is text.
    err_has("NOT (TRUE).to_s()", "#VALUE!");
    err_has("NOT(1, 2)", "NOT expects 1 argument");
    assert!(evaluate("NOT (1, 2)").is_err());
}

fn var(name: &str) -> Rc<Expr> {
    Rc::new(Expr::Variable(name.into()))
}

fn not(arg: Expr) -> Expr {
    Expr::FunctionCall {
        name: "NOT".into(),
        args: vec![arg],
    }
}

fn tree(src: &str) -> Expr {
    parse(src).unwrap_or_else(|e| panic!("{src}: {}", e.message))
}

/// Comparisons > `NOT` > `AND` > `OR`, as in VB and SQL (ADR-0029 §3).
#[test]
fn not_keyword_binds_looser_than_comparisons() {
    // NOT :x > 1  =>  NOT(:x > 1)
    let gt = Expr::Binary(var("x"), BinaryOp::Gt, Rc::new(Expr::Number(1.0)));
    assert_eq!(tree("NOT :x > 1"), not(gt));
    // NOT 1 > 2 AND TRUE  =>  NOT(1 > 2) AND TRUE
    let lt = Expr::Binary(
        Rc::new(Expr::Number(1.0)),
        BinaryOp::Gt,
        Rc::new(Expr::Number(2.0)),
    );
    assert_eq!(
        tree("NOT 1 > 2 AND :t"),
        Expr::Binary(Rc::new(not(lt)), BinaryOp::And, var("t"))
    );
    // NOT :a OR :b  =>  NOT(:a) OR :b
    assert_eq!(
        tree("NOT :a OR :b"),
        Expr::Binary(
            Rc::new(not(Expr::Variable("a".into()))),
            BinaryOp::Or,
            var("b")
        )
    );
    // :a AND NOT :b == :c  =>  :a AND NOT(:b == :c)
    let eq = Expr::Binary(var("b"), BinaryOp::Eq, var("c"));
    assert_eq!(
        tree(":a AND NOT :b == :c"),
        Expr::Binary(var("a"), BinaryOp::And, Rc::new(not(eq)))
    );
    // The same trees as the explicit calls.
    assert_eq!(tree("NOT :x > 1"), tree("NOT(:x > 1)"));
    assert_eq!(tree("NOT 1 > 2 AND TRUE"), tree("NOT(1 > 2) AND TRUE"));
    assert_eq!(tree("NOT :a || :b"), tree("NOT(:a) || :b"));
    assert_eq!(tree("NOT NOT :a"), tree("NOT(NOT(:a))"));
    assert_eq!(tree("NOT :x"), tree("NOT(:x)"));
}

#[test]
fn not_keyword_results() {
    let v = vars(&[("x", Value::Number(5.0))]);
    assert_eq!(
        evaluate_with("NOT :x > 1", &v).unwrap(),
        Value::Boolean(false)
    );
    assert_eq!(
        evaluate_with("NOT :x < 1", &v).unwrap(),
        Value::Boolean(true)
    );
    is("NOT 1 > 2 AND TRUE", true);
    is("NOT 1 > 2 AND FALSE", false);
    is("NOT TRUE OR TRUE", true); // (NOT TRUE) OR TRUE
    is("NOT FALSE AND FALSE", false); // (NOT FALSE) AND FALSE
    is("TRUE AND NOT 1 > 2", true);
    is("FALSE OR NOT 1 > 2", true);
    is("NOT TRUE && TRUE", false);
    is("NOT FALSE || FALSE", true);
    is("NOT 1 + 1 == 2", false);
    is("NOT NOT 1 > 2", false);
    is("IF(NOT 1 > 2, TRUE, FALSE)", true);
    assert_eq!(ok("NOT 1 > 0 ? 'a' : 'b'"), Value::String("b".into()));
}

#[test]
fn bang_stays_a_tight_unary() {
    // `!` binds tighter than comparisons: `!TRUE == FALSE` is `(!TRUE) == FALSE`.
    is("!TRUE == FALSE", true);
    is("NOT TRUE == FALSE", true); // NOT (TRUE == FALSE)
    is("NOT !TRUE", true);
    let v = vars(&[("x", Value::Number(5.0))]);
    assert!(evaluate_with("!:x > 1", &v).is_err());
}

#[test]
fn not_keyword_inside_a_tighter_operator_needs_parentheses() {
    for src in [
        "1 + NOT TRUE",
        "-NOT TRUE",
        "!NOT TRUE",
        "TRUE == NOT FALSE",
    ] {
        err_has(src, "write (NOT x)");
    }
    is("TRUE == (NOT FALSE)", true);
    // With no space, `NOT(` is the call, which is an atom anywhere.
    is("TRUE == NOT(FALSE)", true);
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
fn and_or_operators_take_conditions() {
    // ADR-0029 amendment 1 (BL-089): both sides use the IF condition rule.
    is("1 AND TRUE", true);
    is("TRUE AND 1", true);
    is("TRUE AND 0", false);
    err_has(
        "'x' OR TRUE",
        "OR condition must be TRUE, FALSE or a number, got string (#VALUE!)",
    );
    err_has(
        "TRUE AND 'x'",
        "AND condition must be TRUE, FALSE or a number, got string (#VALUE!)",
    );
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
