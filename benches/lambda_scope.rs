//! BL-030: cost of method/function calls inside lambdas.
//!
//! Run: `cargo bench --bench lambda_scope` (release profile, library layer, no process start).
//! Optional: `RUNS=9 cargo bench --bench lambda_scope`.
//!
//! Inputs are built in-process (no CLI argv limit) the same way `evaluate_with_json_custom`
//! and the servers build them: one variable per top-level JSON key plus `arguments`, the whole
//! JSON text. Each case prints median and p95 (= max for 5 runs) wall time and the result, so a
//! before/after run also shows the result did not change.

use skillet::{evaluate_with_custom, json_to_value, Value};
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[allow(deprecated)] // `Value::Json` is deprecated until 0.8 (ADR-0030)
fn vars_from_json(json: &str) -> HashMap<String, Value> {
    let parsed: serde_json::Value = serde_json::from_str(json).expect("bench JSON is valid");
    let mut vars = HashMap::new();
    vars.insert("arguments".to_string(), Value::Json(json.to_string()));
    if let serde_json::Value::Object(map) = parsed {
        for (k, v) in map {
            vars.insert(k, json_to_value(v).expect("bench JSON converts"));
        }
    }
    vars
}

/// 100k numbers, about 0.7 MB of JSON (ADR-0011 used 589 KB; same item count).
fn nums_json() -> String {
    let items: Vec<String> = (0..100_000)
        .map(|i| format!("{}", (i as f64 * 1.37 - 50_000.0).round() / 10.0))
        .collect();
    format!("{{\"nums\":[{}]}}", items.join(","))
}

/// 10k rows of small objects, about 0.45 MB of JSON.
fn rows_json() -> String {
    let items: Vec<String> = (0..10_000)
        .map(|i| {
            format!(
                "{{\"id\":{},\"cat\":\"{}\",\"price\":{:.2},\"qty\":{}}}",
                i,
                ["a", "b", "c"][i % 3],
                (i % 997) as f64 * 1.13 + 0.5,
                i % 7 + 1
            )
        })
        .collect();
    format!("{{\"rows\":[{}]}}", items.join(","))
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
        "{:<48} median {:>10.2} ms  p95 {:>10.2} ms  => {:?}",
        expr,
        median.as_secs_f64() * 1000.0,
        p95.as_secs_f64() * 1000.0,
        result.expect("at least one run")
    );
}

fn main() {
    let runs: usize = std::env::var("RUNS")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&n| n > 0)
        .unwrap_or(5);

    let nums = nums_json();
    let rows = rows_json();
    println!(
        "BL-030 lambda scope bench: runs={} nums JSON {} B, rows JSON {} B",
        runs,
        nums.len(),
        rows.len()
    );

    let nums_vars = vars_from_json(&nums);
    time_case(":nums.map(:x * 1).sum()", &nums_vars, runs);
    time_case(":nums.map(:x.abs()).sum()", &nums_vars, runs);

    let rows_vars = vars_from_json(&rows);
    time_case(":rows.map(:x.price).sum()", &rows_vars, runs);
    time_case(":rows.map(:x.price.round(1)).sum()", &rows_vars, runs);
    time_case(
        ":rows.map(:x.price / :rows.length()).sum()",
        &rows_vars,
        runs,
    );
}
