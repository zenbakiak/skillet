//! JSON variables are charged to the evaluation budget (ADR-0011 amendment 5, BL-086): the
//! library's `*_json*` entry points, in the same scope as the evaluation. The servers are
//! covered in `server_eval_limits.rs`.
//!
//! Costs: `:nums` = 32 + 32 per number; `:arguments` = 48 + the length of the JSON text.

use skillet::{
    charge_variables, evaluate_with, evaluate_with_json, evaluate_with_json_custom, with_limits,
    EvalLimits, Value,
};
use std::collections::HashMap;

fn limits(max_bytes: u64) -> EvalLimits {
    let mut l = EvalLimits::default();
    l.max_bytes = max_bytes;
    l
}

/// `{"nums":[0,0,…]}` with `n` zeros.
fn zeros_json(n: usize) -> String {
    format!("{{\"nums\":[{}]}}", vec!["0"; n].join(","))
}

/// What the variables of [`zeros_json`] cost: `:nums` plus `:arguments`.
fn zeros_cost(n: usize) -> u64 {
    let text = zeros_json(n).len() as u64;
    (32 + 32 * n as u64) + (48 + text)
}

#[test]
fn json_variables_just_under_and_over_the_budget() {
    let n = 1000;
    let json = zeros_json(n);
    // `1` creates nothing, so the variables are all that is charged.
    let fits = zeros_cost(n);
    for eval in [evaluate_with_json, evaluate_with_json_custom] {
        let ok = with_limits(limits(fits), || eval("1", &json));
        assert_eq!(ok.unwrap(), Value::Number(1.0));
        let err = with_limits(limits(fits - 1), || eval("1", &json)).unwrap_err();
        assert_eq!(
            err.message,
            format!("Evaluation budget exceeded (max {} bytes)", fits - 1)
        );
    }
}

/// The variables and the evaluation share one budget: what the variables use is not available
/// to the formula, and a borrowed read (`.length()`) does not charge them again.
#[test]
fn variables_and_evaluation_share_the_scope() {
    let n = 1000;
    let json = zeros_json(n);
    let fits = zeros_cost(n) + 32; // + the Number result
    let ok = with_limits(limits(fits), || evaluate_with_json(":nums.length()", &json));
    assert_eq!(ok.unwrap(), Value::Number(n as f64));
    let err = with_limits(limits(fits - 1), || {
        evaluate_with_json(":nums.length()", &json)
    });
    assert!(err
        .unwrap_err()
        .message
        .starts_with("Evaluation budget exceeded"));
}

/// Without an outer scope the default 256 MiB budget applies: 1M numbers (~32 MB) pass,
/// and the result is the same as before the change.
#[test]
fn large_json_variables_pass_the_default_budget() {
    let json = zeros_json(1_000_000);
    assert_eq!(
        evaluate_with_json("SUM(:nums) + :nums.length()", &json).unwrap(),
        Value::Number(1_000_000.0)
    );
}

/// A map passed directly is not charged by `evaluate_with`; a host can charge it itself.
#[test]
fn hashmap_variables_are_charged_only_on_request() {
    let mut vars = HashMap::new();
    let nums = vec![Value::Number(0.0); 1000];
    vars.insert("nums".to_string(), Value::Array(nums));
    let budget = limits(100);
    let ok = with_limits(budget, || evaluate_with(":nums.length()", &vars));
    assert_eq!(ok.unwrap(), Value::Number(1000.0));
    let err = with_limits(budget, || {
        charge_variables(&vars)?;
        evaluate_with(":nums.length()", &vars)
    });
    assert_eq!(
        err.unwrap_err().message,
        "Evaluation budget exceeded (max 100 bytes)"
    );
}

/// Errors that come before the charge are unchanged.
#[test]
fn invalid_json_errors_are_unchanged() {
    let tiny = limits(1);
    let err = with_limits(tiny, || evaluate_with_json("1", "[1]")).unwrap_err();
    assert_eq!(err.message, "JSON must be an object with key-value pairs");
    let err = with_limits(tiny, || evaluate_with_json("1", "{")).unwrap_err();
    assert!(err.message.starts_with("Invalid JSON"), "{}", err.message);
}
