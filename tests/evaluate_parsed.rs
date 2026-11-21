//! BL-012 / ADR-0021: `evaluate_parsed*` gives the same results and the same errors as the
//! matching `evaluate*` function, under the same limits (nesting depth, budget, deadline).

use skillet::{
    evaluate, evaluate_parsed, evaluate_parsed_with, evaluate_parsed_with_assignments,
    evaluate_parsed_with_assignments_and_context, evaluate_parsed_with_custom, evaluate_with,
    evaluate_with_assignments, evaluate_with_assignments_and_context, evaluate_with_custom, parse,
    with_limits, CustomFunction, Error, EvalLimits, Value,
};
use std::collections::HashMap;
use std::time::Duration;

/// Formulas covering literals, operators, builtins, methods, lambdas, JSON access, casts,
/// assignments, and errors raised by the parser and by the evaluator.
const CASES: &[&str] = &[
    "=2 + 3 * 4 - (10 / 5) ^ 2 % 3",
    "CONCAT(UPPER('hello'), ' ', LOWER('WORLD'))",
    "[1,2,3,4,5,6,7,8,9,10].map(:x * 2).filter(:x > 10).sum()",
    ":nums.map(:x * :n).sum()",
    ":user.name.upper()",
    ":user&.missing&.deep",
    "IF(:n > 2, 'big', 'small')",
    ":nums.reduce(:acc + :x, 0)",
    "'12'::Integer + 1",
    "PMT(0.05/12, 360, 200000)",
    ":a := 10; :b := :a * :n; :b + 1",
    ":t := [1, 2]; :t.length()",
    "DOUBLE_IT(:n) + 1",
    "1 / 0",
    ":missing + 1",
    "NOPE(1)",
    "2 +",
    "[1, 2",
    "",
    "[1].reduce([:acc], 0)",
];

fn vars() -> HashMap<String, Value> {
    let mut v = HashMap::new();
    v.insert("n".to_string(), Value::Number(3.0));
    v.insert(
        "nums".to_string(),
        Value::Array(vec![1.0, 2.0, 3.0].into_iter().map(Value::Number).collect()),
    );
    v.insert(
        "user".to_string(),
        Value::Json(r#"{"name": "ada", "age": 36}"#.to_string()),
    );
    v
}

struct DoubleIt;

impl CustomFunction for DoubleIt {
    fn name(&self) -> &str {
        "DOUBLE_IT"
    }
    fn min_args(&self) -> usize {
        1
    }
    fn max_args(&self) -> Option<usize> {
        Some(1)
    }
    fn execute(&self, args: Vec<Value>) -> Result<Value, Error> {
        let n = args[0]
            .as_number()
            .ok_or_else(|| Error::new("DOUBLE_IT expects a number", None))?;
        Ok(Value::Number(n * 2.0))
    }
}

/// `Error` has no `PartialEq`; compare what callers see.
fn key<T: std::fmt::Debug>(r: &Result<T, Error>) -> String {
    match r {
        Ok(v) => format!("Ok({v:?})"),
        Err(e) => format!("Err({:?} at {:?})", e.message, e.position),
    }
}

/// Parse with `parse`, then run `eval` on the tree; a parse error is the result.
fn via_parse<T>(text: &str, eval: impl Fn(&skillet::Expr) -> Result<T, Error>) -> Result<T, Error> {
    parse(text).and_then(|expr| eval(&expr))
}

#[test]
fn every_variant_matches_its_text_twin() {
    skillet::register_function(Box::new(DoubleIt)).unwrap();
    let vars = vars();
    for text in CASES {
        let pairs = [
            (key(&evaluate(text)), key(&via_parse(text, evaluate_parsed))),
            (
                key(&evaluate_with(text, &vars)),
                key(&via_parse(text, |e| evaluate_parsed_with(e, &vars))),
            ),
            (
                key(&evaluate_with_custom(text, &vars)),
                key(&via_parse(text, |e| evaluate_parsed_with_custom(e, &vars))),
            ),
            (
                key(&evaluate_with_assignments(text, &vars)),
                key(&via_parse(text, |e| {
                    evaluate_parsed_with_assignments(e, &vars)
                })),
            ),
            (
                key(&evaluate_with_assignments_and_context(text, &vars)),
                key(&via_parse(text, |e| {
                    evaluate_parsed_with_assignments_and_context(e, &vars)
                })),
            ),
        ];
        for (i, (want, got)) in pairs.iter().enumerate() {
            assert_eq!(want, got, "variant {i}, formula {text:?}");
        }
    }
}

#[test]
fn one_tree_evaluates_with_different_variables() {
    let expr = parse(":nums.map(:x * :n).sum()").unwrap();
    for n in 0..5 {
        let mut v = vars();
        v.insert("n".to_string(), Value::Number(n as f64));
        assert_eq!(
            evaluate_parsed_with(&expr, &v),
            Ok(Value::Number(6.0 * n as f64))
        );
    }
}

#[test]
fn nesting_limit_is_a_parse_error() {
    let deep = format!("{}1{}", "(".repeat(129), ")".repeat(129));
    let err = parse(&deep).unwrap_err();
    assert_eq!(err.message, "Expression nested too deeply (max 128)");
    assert_eq!(key(&evaluate(&deep)), key::<Value>(&Err(err)));

    let ok = format!("{}1{}", "(".repeat(128), ")".repeat(128));
    assert_eq!(
        evaluate_parsed(&parse(&ok).unwrap()),
        Ok(Value::Number(1.0))
    );
}

#[test]
fn values_built_too_deep_fail_the_same_way() {
    let ones = vec!["1"; 130].join(",");
    let text = format!("[{ones}].reduce([:acc], 0)");
    let want = evaluate(&text).unwrap_err();
    assert_eq!(want.message, "Expression nested too deeply (max 128)");
    let got = evaluate_parsed(&parse(&text).unwrap()).unwrap_err();
    assert_eq!((got.message, got.position), (want.message, want.position));
}

#[test]
fn default_budget_applies() {
    let ones = vec!["1"; 26].join(",");
    let expr = parse(&format!("[{ones}].reduce([:acc, :acc], 0)")).unwrap();
    let err = evaluate_parsed(&expr).unwrap_err();
    assert_eq!(err.message, "Evaluation budget exceeded (max 256 MB)");
}

#[test]
fn each_call_gets_a_fresh_budget() {
    // 81 MB per call: four calls in a row only pass if each starts from zero.
    let src = format!(
        "SUBSTITUTE(\"{}\", \"a\", \"{}\").length()",
        "a".repeat(9_000),
        "b".repeat(9_000)
    );
    let expr = parse(&src).unwrap();
    for _ in 0..4 {
        assert_eq!(evaluate_parsed(&expr), Ok(Value::Number(81_000_000.0)));
    }
}

#[test]
fn with_limits_budget_applies() {
    let mut l = EvalLimits::default();
    l.max_bytes = 160;
    let expr = parse("[1, 2, 3, 4, 5]").unwrap();
    let err = with_limits(l, || evaluate_parsed(&expr)).unwrap_err();
    assert_eq!(err.message, "Evaluation budget exceeded (max 160 bytes)");
}

#[test]
fn with_limits_deadline_applies() {
    let mut l = EvalLimits::default();
    l.timeout = Some(Duration::from_millis(1));
    let mut v = HashMap::new();
    v.insert(
        "nums".to_string(),
        Value::Array((0..100_000).map(|i| Value::Number(i as f64)).collect()),
    );
    let expr =
        parse(":nums.map(:x.abs()).map(:x.abs()).map(:x.abs()).map(:x.abs()).sum()").unwrap();
    let err = with_limits(l, || evaluate_parsed_with_custom(&expr, &v)).unwrap_err();
    assert_eq!(err.message, "Evaluation timed out (max 1 ms)");
}
