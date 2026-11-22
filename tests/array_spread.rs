//! BL-106 acceptance: spread `...` in array literals (ADR-0005 amendment 1).
//!
//! `[0, ...:xs, 4]` follows the argument-list spread rules: the operand is one full expression,
//! an array appends its items (one level, no flattening), any other value is the error
//! `Spread expects array`. Rows are named after the amendment's acceptance table (E1-E27) and
//! run through every public entry point, with a custom registry, and through the runtime
//! evaluator with an explicit registry.

use skillet::runtime::evaluator;
use skillet::{
    evaluate_parsed_with_custom, evaluate_with, evaluate_with_assignments,
    evaluate_with_assignments_and_context, evaluate_with_custom, parse, CustomFunction, Error,
    FunctionRegistry, Value,
};
use std::collections::HashMap;
use std::sync::{Arc, Once, RwLock};

fn n(x: f64) -> Value {
    Value::Number(x)
}
fn nums(xs: &[f64]) -> Value {
    Value::Array(xs.iter().map(|x| n(*x)).collect())
}
fn st(s: &str) -> Value {
    Value::String(s.to_string())
}

/// The amendment's variables: `:xs = [1, 2, 3]`, `:nested = [[1, 2], [3]]`, `:none = NULL`,
/// `:s = "ab"`.
fn vars() -> HashMap<String, Value> {
    let mut v = HashMap::new();
    v.insert("xs".into(), nums(&[1.0, 2.0, 3.0]));
    v.insert(
        "nested".into(),
        Value::Array(vec![nums(&[1.0, 2.0]), nums(&[3.0])]),
    );
    v.insert("none".into(), Value::Null);
    v.insert("s".into(), st("ab"));
    v
}

/// `COUNTARGS(...)`: the number of arguments it receives, so a test can see what a spread
/// passed to a custom function.
struct CountArgs;
impl CustomFunction for CountArgs {
    fn name(&self) -> &str {
        "COUNTARGS"
    }
    fn min_args(&self) -> usize {
        0
    }
    fn max_args(&self) -> Option<usize> {
        None
    }
    fn execute(&self, args: Vec<Value>) -> Result<Value, Error> {
        Ok(Value::Number(args.len() as f64))
    }
}

/// `PAIR(a, b)`: `[a, b]`, a custom function whose result can be spread.
struct Pair;
impl CustomFunction for Pair {
    fn name(&self) -> &str {
        "PAIR"
    }
    fn min_args(&self) -> usize {
        2
    }
    fn max_args(&self) -> Option<usize> {
        Some(2)
    }
    fn execute(&self, args: Vec<Value>) -> Result<Value, Error> {
        Ok(Value::Array(args))
    }
}

fn ensure_global_registry() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        skillet::register_function(Box::new(CountArgs)).unwrap();
        skillet::register_function(Box::new(Pair)).unwrap();
    });
}

fn local_registry() -> Arc<RwLock<FunctionRegistry>> {
    let mut r = FunctionRegistry::new();
    r.register(Box::new(CountArgs)).unwrap();
    r.register(Box::new(Pair)).unwrap();
    Arc::new(RwLock::new(r))
}

/// Evaluate `src` through every entry point that takes variables and return the outcomes,
/// labelled. All must agree; `check` compares them with the expected outcome.
fn outcomes(src: &str) -> Vec<(&'static str, Result<Value, String>)> {
    ensure_global_registry();
    let v = vars();
    let msg = |r: Result<Value, Error>| r.map_err(|e| e.to_string());
    let expr = parse(src).map_err(|e| e.to_string());
    let mut out = vec![
        ("evaluate_with", msg(evaluate_with(src, &v))),
        ("evaluate_with_custom", msg(evaluate_with_custom(src, &v))),
        (
            "evaluate_with_assignments",
            msg(evaluate_with_assignments(src, &v)),
        ),
        (
            "evaluate_with_assignments_and_context",
            msg(evaluate_with_assignments_and_context(src, &v).map(|(r, _)| r)),
        ),
    ];
    match expr {
        Ok(expr) => {
            out.push((
                "evaluate_parsed_with_custom",
                msg(evaluate_parsed_with_custom(&expr, &v)),
            ));
            out.push((
                "runtime eval_with_vars_and_custom",
                msg(evaluator::eval_with_vars_and_custom(
                    &expr,
                    &v,
                    &local_registry(),
                )),
            ));
        }
        Err(e) => out.push(("parse", Err(e))),
    }
    out
}

fn check(src: &str, want: Result<Value, &str>) {
    let want = want.map_err(str::to_string);
    for (entry, got) in outcomes(src) {
        assert_eq!(got, want, "{entry}: {src}");
    }
}

fn ok(src: &str, want: Value) {
    check(src, Ok(want));
}

fn err(src: &str, message: &str) {
    check(src, Err(message));
}

// ------------------------------------------------------------------ E1-E19: values

#[test]
fn e1_e5_literal_and_variable_operands() {
    ok("[0, ...[1, 2, 3], 4]", nums(&[0.0, 1.0, 2.0, 3.0, 4.0]));
    ok("[0, ...:xs, 4]", nums(&[0.0, 1.0, 2.0, 3.0, 4.0]));
    ok("[...:xs]", nums(&[1.0, 2.0, 3.0]));
    ok("[...[]]", Value::Array(vec![]));
    ok("[0, ...[], 1]", nums(&[0.0, 1.0]));
}

#[test]
fn e6_e10_repeats_one_level_and_nesting() {
    ok("[...:xs, ...:xs]", nums(&[1.0, 2.0, 3.0, 1.0, 2.0, 3.0]));
    ok(
        "[...[1, [2, 3]]]",
        Value::Array(vec![n(1.0), nums(&[2.0, 3.0])]),
    );
    ok(
        "[...:nested]",
        Value::Array(vec![nums(&[1.0, 2.0]), nums(&[3.0])]),
    );
    ok("[...[...:xs]]", nums(&[1.0, 2.0, 3.0]));
    ok(
        "[[...:xs], ...[[4]]]",
        Value::Array(vec![nums(&[1.0, 2.0, 3.0]), nums(&[4.0])]),
    );
}

#[test]
fn e11_e13_operand_is_a_full_expression() {
    ok("[...:xs.map(:x * 2), 0]", nums(&[2.0, 4.0, 6.0, 0.0]));
    ok("[...TRUE ? [1] : [2], 3]", nums(&[1.0, 3.0]));
    ok(
        "[1, 'a', ...[NULL, TRUE]]",
        Value::Array(vec![n(1.0), st("a"), Value::Null, Value::Boolean(true)]),
    );
}

#[test]
fn e14_e17_spread_arrays_in_larger_expressions() {
    ok("[...:xs].sum()", n(6.0));
    ok("SUM([...:xs, 4])", n(10.0));
    ok("[...:xs][0]", n(1.0));
    ok(
        "[1, 2].map([...[:x], 0])",
        Value::Array(vec![nums(&[1.0, 0.0]), nums(&[2.0, 0.0])]),
    );
}

#[test]
fn e18_e19_explicit_to_a() {
    ok("[...:none.to_a(), 1]", nums(&[1.0]));
    ok("[...:s.to_a()]", Value::Array(vec![st("a"), st("b")]));
}

// ------------------------------------------------------------------ E20-E26: errors

#[test]
fn e20_e24_non_array_operands_are_an_error() {
    for src in [
        "[...:none]",
        "[...NULL]",
        "[...5]",
        "[...TRUE]",
        "[...DATE(2026, 1, 1)]",
        "[...:s]",
        "[...\"\"]",
        "[...{a: 1}]",
    ] {
        err(src, "Spread expects array");
    }
}

#[test]
fn spread_error_has_no_position() {
    let e = skillet::evaluate("[...5]").unwrap_err();
    assert_eq!(e.message, "Spread expects array");
    assert_eq!(e.position, None);
}

#[test]
fn e25_e26_operand_errors_are_returned_unchanged() {
    err("[1, ...:missing]", "Missing variable: :missing");
    err("[...[1], ...1/0]", "Division by zero (#DIV/0!)");
}

#[test]
fn evaluation_stops_at_the_first_error() {
    err("[...:missing, ...1/0]", "Missing variable: :missing");
    err("[...5, ...:missing]", "Spread expects array");
}

// ------------------------------------------------------------------ E27: same rule as argument lists

#[test]
fn e27_array_literal_and_argument_list_agree() {
    ok("MERGE(...[[0], :xs])", nums(&[0.0, 1.0, 2.0, 3.0]));
    ok("[0, ...:xs]", nums(&[0.0, 1.0, 2.0, 3.0]));
    for operand in [":none", "5", ":s", "{a: 1}"] {
        err(&format!("MERGE(...{operand})"), "Spread expects array");
        err(&format!("[...{operand}]"), "Spread expects array");
    }
}

/// Through the entry points that see a custom registry (the global one, or an explicit one).
fn custom(src: &str, want: Result<Value, &str>) {
    ensure_global_registry();
    let v = vars();
    let want = want.map_err(str::to_string);
    let expr = parse(src).unwrap();
    for (entry, got) in [
        ("evaluate_with_custom", evaluate_with_custom(src, &v)),
        (
            "evaluate_parsed_with_custom",
            evaluate_parsed_with_custom(&expr, &v),
        ),
        (
            "runtime eval_with_vars_and_custom",
            evaluator::eval_with_vars_and_custom(&expr, &v, &local_registry()),
        ),
    ] {
        assert_eq!(got.map_err(|e| e.to_string()), want, "{entry}: {src}");
    }
}

#[test]
fn custom_functions_and_spread_arrays() {
    // A spread array passed whole, and spread into the arguments: unchanged argument-list rule.
    custom("COUNTARGS([...:xs, 4])", Ok(n(1.0)));
    custom("COUNTARGS(...[...:xs, 4])", Ok(n(4.0)));
    // A custom function's result spread into an array literal.
    custom("[0, ...PAIR(1, 2), 3]", Ok(nums(&[0.0, 1.0, 2.0, 3.0])));
    custom("[...COUNTARGS()]", Err("Spread expects array"));
    custom("COUNTARGS(...5)", Err("Spread expects array"));
}

#[test]
fn assignments_store_the_spread_array() {
    let (r, ctx) =
        evaluate_with_assignments_and_context(":ys := [0, ...:xs]; :ys.length()", &vars()).unwrap();
    assert_eq!(r, n(4.0));
    assert_eq!(ctx.get("ys"), Some(&nums(&[0.0, 1.0, 2.0, 3.0])));
}

#[test]
fn the_ast_keeps_spread_items_in_the_array() {
    let expr = parse("[0, ...:xs]").unwrap();
    match expr {
        skillet::Expr::Array(items) => {
            assert_eq!(items.len(), 2);
            assert!(matches!(items[1], skillet::Expr::Spread(_)));
        }
        other => panic!("expected an array literal, got {other:?}"),
    }
}

// ------------------------------------------------------------------ parse errors (S4)

fn parse_error(src: &str) -> String {
    match parse(src) {
        Err(e) => e.to_string(),
        Ok(expr) => panic!("{src} parsed: {expr:?}"),
    }
}

#[test]
fn malformed_array_spreads_are_parse_errors() {
    for (src, want) in [
        ("=[...]", "Unexpected token: RBracket at position 5"),
        ("=[1, ...]", "Unexpected token: RBracket at position 8"),
        ("=[... ...[1]]", "Unexpected token: Ellipsis at position 6"),
        (
            "=[...[1] ...[2]]",
            "Expected ',' or ']' in array at position 9",
        ),
        ("=[1, ...[2],]", "Unexpected token: RBracket at position 12"),
    ] {
        assert_eq!(parse_error(src), want, "{src}");
    }
}

#[test]
fn spread_outside_argument_lists_and_arrays_stays_a_parse_error() {
    for (src, want) in [
        ("=...[1]", "Unexpected token: Ellipsis at position 1"),
        ("=(...[1])", "Unexpected token: Ellipsis at position 2"),
        ("=1 + ...[1]", "Unexpected token: Ellipsis at position 5"),
        ("=:a := ...[1]", "Unexpected token: Ellipsis at position 7"),
        ("={a: ...[1]}", "Unexpected token: Ellipsis at position 5"),
    ] {
        assert_eq!(parse_error(src), want, "{src}");
    }
}

// ------------------------------------------------------------------ scope (S6)

#[test]
fn method_argument_spread_is_unchanged() {
    // Out of scope for amendment 1: stays a runtime error where the method reads its arguments.
    assert_eq!(
        skillet::evaluate("[3, 1].sort(...[\"desc\"])")
            .unwrap_err()
            .message,
        "Spread not allowed here"
    );
}

#[test]
fn spread_of_a_volatile_call_is_volatile() {
    assert!(skillet::is_volatile(&parse("[...[NOW()]]").unwrap()));
    assert!(!skillet::is_volatile(&parse("[...[1, 2]]").unwrap()));
}
