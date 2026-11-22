//! BL-088: per-item cost of `ROUND`/`.round()` and `AND`/`OR`/`XOR` on 100k numbers.
//!
//! Run: `cargo bench --bench per_item` (release profile, library layer, no process start).
//! Optional: `RUNS=9 cargo bench --bench per_item`; `CASE=ROUND cargo bench --bench per_item`
//! runs only the cases whose expression contains `ROUND`.
//!
//! Each case prints the median and p95 wall time and the result, so a before/after run also
//! shows that the result did not change. The `:x * 1` and `&&` cases are the controls: they do
//! not touch the code BL-088 changes.

use skillet::{evaluate_with_custom, json_to_value, Value};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// 100k numbers: one decimal, from -5000 to 8700 (the `lambda_scope` set).
fn nums_vars() -> HashMap<String, Value> {
    let items: Vec<String> = (0..100_000)
        .map(|i| format!("{}", (i as f64 * 1.37 - 50_000.0).round() / 10.0))
        .collect();
    let json = format!("{{\"nums\":[{}]}}", items.join(","));
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("bench JSON is valid");
    let mut vars = HashMap::new();
    if let serde_json::Value::Object(map) = parsed {
        for (k, v) in map {
            vars.insert(k, json_to_value(v).expect("bench JSON converts"));
        }
    }
    vars
}

fn time_case(expr: &str, vars: &HashMap<String, Value>, runs: usize) {
    let mut times: Vec<Duration> = Vec::with_capacity(runs);
    let mut result = None;
    for _ in 0..runs {
        let start = Instant::now();
        let r = evaluate_with_custom(expr, vars);
        times.push(start.elapsed());
        result = Some(r);
    }
    times.sort();
    let median = times[runs / 2];
    let p95 = times[((runs as f64 * 0.95).ceil() as usize).saturating_sub(1)];
    println!(
        "{:<56} median {:>8.2} ms  p95 {:>8.2} ms  => {:?}",
        expr,
        median.as_secs_f64() * 1000.0,
        p95.as_secs_f64() * 1000.0,
        result.expect("at least one run")
    );
}

const CASES: &[&str] = &[
    ":nums.map(:x * 1).sum()",
    ":nums.map(ROUND(:x, 1)).sum()",
    ":nums.map(ROUND(:x * 1.37, 2)).sum()",
    ":nums.map(:x.round(1)).sum()",
    ":nums.map((:x * 1.37).round(2)).sum()",
    ":nums.filter(:x > 0 && :x < 1000).length()",
    ":nums.filter(AND(:x > 0, :x < 1000)).length()",
    ":nums.filter(OR(:x < 0, :x > 1000)).length()",
    ":nums.filter(XOR(:x < 0, :x > 1000)).length()",
];

fn main() {
    let runs: usize = std::env::var("RUNS")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&n| n > 0)
        .unwrap_or(5);
    let only = std::env::var("CASE").unwrap_or_default();
    let vars = nums_vars();
    println!("BL-088 per-item bench: runs={runs}, 100k numbers");
    for expr in CASES.iter().filter(|e| e.contains(only.as_str())) {
        time_case(expr, &vars, runs);
    }
}
