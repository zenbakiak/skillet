//! BL-005: performance baseline over a fixed set of representative expressions.
//!
//! Three layers, measured separately (harness/roles/performance-engineer.md):
//! - `lib`:  in-process. `parse` alone, and `evaluate_with_custom` (parse + eval) with the
//!   (or `evaluate_with_assignments` for sequences, as the CLI and servers do) with the
//!   variables prebuilt the way the servers build them (one variable per top-level JSON key
//!   plus `arguments`, the whole JSON text).
//! - `cli`:  one `sk` process per evaluation (process start, JS plugin loading and JSON
//!   variable parsing included), with an empty hooks dir and with the repo's `hooks/`.
//! - `http`: one `POST /eval` per request against a `sk_http_server` this bench starts on
//!   127.0.0.1, new TCP connection per request (`Connection: close`). `cached` repeats the
//!   same body; `uncached` adds a unique `bench_nonce` argument so every request misses the
//!   expression cache (its key includes the variables).
//!
//! Run (release profile): `cargo bench --bench baseline`
//! Options (env): `RUNS` samples per case (default 15), `LAYERS=lib,cli,http` (default all),
//! `PORT` for the HTTP server (default 5099), `CASE=<substring>` to run a subset.
//!
//! Rounds are interleaved: every round runs every case once, so load spikes on a shared
//! machine spread over all cases instead of hitting one. Each `lib` sample is the mean of a
//! batch sized to take ~5 ms; `cli`/`http` samples are single evaluations. Output is median
//! and p95 per case and the result, so a before/after run also shows results did not change.

use skillet::{
    evaluate_with_assignments, evaluate_with_custom, json_to_value, parse, Error, Value,
};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// Shared JSON document for the cases that read variables.
const DOC: &str = r#"{"user":{"name":"alice","profile":{"age":34,"tags":["a","b","c"]}},"items":[{"name":"pen","price":1.5,"qty":10},{"name":"book","price":12.0,"qty":2},{"name":"bag","price":30.25,"qty":1},{"name":"cup","price":4.75,"qty":6}],"x":7,"y":3}"#;

struct Case {
    name: &'static str,
    expr: String,
    json: Option<&'static str>,
}

fn cases() -> Vec<Case> {
    // ~1.8 KB of text (1,796 bytes), 100 call arguments: parse cost dominates, shallow AST (depth 4).
    let long_terms: Vec<String> = (1..=100)
        .map(|i| format!("({} * 2 + {} - 1)", i, i % 7))
        .collect();
    let long = format!("SUM({})", long_terms.join(", "));
    let c = |name, expr: &str, json| Case {
        name,
        expr: format!("={}", expr),
        json,
    };
    vec![
        c("arith", "2 + 3 * 4 - (10 / 5) ^ 2 % 3", None),
        c(
            "string_builtins",
            r#"CONCAT(UPPER("hello"), " ", LOWER("WORLD"), " ", TRIM("  skillet  "), LENGTH("abc"))"#,
            None,
        ),
        c(
            "array_lambdas",
            "[1,2,3,4,5,6,7,8,9,10].map(:x * 2).filter(:x > 10).sum()",
            None,
        ),
        c(
            "jsonpath_jq",
            r#"JQ(:arguments, "$.items[*].price").sum()"#,
            Some(DOC),
        ),
        c(
            "json_vars",
            ":user.profile.age + :items.length() + :user&.profile&.tags.length()",
            Some(DOC),
        ),
        c(
            "json_lambda",
            ":items.map(:x.price * :x.qty).filter(:x > 10).sum().round(2)",
            Some(DOC),
        ),
        c("financial", "PMT(0.05/12, 360, 200000)", None),
        c(
            "assign_sequence",
            ":a := 10; :b := :a * 2; :c := :b + :a; :c * 2",
            None,
        ),
        c(
            "method_chain",
            r#""  Hello World  ".trim().upper().reverse()"#,
            None,
        ),
        c(
            "logic_if",
            r#"IF(AND(:x > 5, :y < 10), "yes", "no")"#,
            Some(DOC),
        ),
        Case {
            name: "parse_heavy",
            expr: format!("={}", long),
            json: None,
        },
    ]
}

/// Same entry point the CLI and the servers pick: sequences/assignments go through
/// `evaluate_with_assignments`, everything else through `evaluate_with_custom`.
fn eval(expr: &str, vars: &HashMap<String, Value>) -> Result<Value, Error> {
    if expr.contains(';') || expr.contains(":=") {
        evaluate_with_assignments(expr, vars)
    } else {
        evaluate_with_custom(expr, vars)
    }
}

#[allow(deprecated)] // `Value::Json` is deprecated until 0.8 (ADR-0030)
fn vars_from_json(json: Option<&str>) -> HashMap<String, Value> {
    let mut vars = HashMap::new();
    let Some(json) = json else { return vars };
    let parsed: serde_json::Value = serde_json::from_str(json).expect("bench JSON is valid");
    vars.insert("arguments".to_string(), Value::Json(json.to_string()));
    if let serde_json::Value::Object(map) = parsed {
        for (k, v) in map {
            vars.insert(k, json_to_value(v).expect("bench JSON converts"));
        }
    }
    vars
}

/// Median and p95 (nearest rank) of a sample set.
fn stats(samples: &mut [Duration]) -> (Duration, Duration) {
    samples.sort();
    let n = samples.len();
    let p95 = samples[((n as f64 * 0.95).ceil() as usize).clamp(1, n) - 1];
    (samples[n / 2], p95)
}

fn fmt(d: Duration) -> String {
    let us = d.as_secs_f64() * 1e6;
    if us < 1000.0 {
        format!("{:.2} us", us)
    } else {
        format!("{:.3} ms", us / 1000.0)
    }
}

fn report(layer: &str, name: &str, samples: &mut [Duration], result: &str) {
    let (median, p95) = stats(samples);
    println!(
        "{:<6} {:<26} median {:>12}  p95 {:>12}  => {}",
        layer,
        name,
        fmt(median),
        fmt(p95),
        result
    );
}

/// Iterations needed for one batch to take about `target`.
fn calibrate(mut f: impl FnMut(), target: Duration) -> u32 {
    let mut n = 1u32;
    loop {
        let start = Instant::now();
        for _ in 0..n {
            f();
        }
        let el = start.elapsed();
        if el >= target / 4 || n >= 1 << 20 {
            let per = el.as_secs_f64() / n as f64;
            return ((target.as_secs_f64() / per).ceil() as u32).max(1);
        }
        n *= 4;
    }
}

fn batch_mean(mut f: impl FnMut(), n: u32) -> Duration {
    let start = Instant::now();
    for _ in 0..n {
        f();
    }
    start.elapsed() / n
}

fn layer_lib(cases: &[Case], runs: usize) {
    let target = Duration::from_millis(5);
    let vars: Vec<_> = cases.iter().map(|c| vars_from_json(c.json)).collect();
    let parse_n: Vec<u32> = cases
        .iter()
        .map(|c| {
            calibrate(
                || {
                    std::hint::black_box(parse(&c.expr)).ok();
                },
                target,
            )
        })
        .collect();
    let eval_n: Vec<u32> = cases
        .iter()
        .zip(&vars)
        .map(|(c, v)| {
            calibrate(
                || {
                    std::hint::black_box(eval(&c.expr, v)).ok();
                },
                target,
            )
        })
        .collect();
    let mut parse_s = vec![Vec::with_capacity(runs); cases.len()];
    let mut eval_s = vec![Vec::with_capacity(runs); cases.len()];
    for _ in 0..runs {
        for (i, c) in cases.iter().enumerate() {
            parse_s[i].push(batch_mean(
                || {
                    std::hint::black_box(parse(&c.expr)).ok();
                },
                parse_n[i],
            ));
            eval_s[i].push(batch_mean(
                || {
                    std::hint::black_box(eval(&c.expr, &vars[i])).ok();
                },
                eval_n[i],
            ));
        }
    }
    for (i, c) in cases.iter().enumerate() {
        report("lib", &format!("{} parse", c.name), &mut parse_s[i], "");
        let result = format!("{:?}", eval(&c.expr, &vars[i]));
        report("lib", &format!("{} eval", c.name), &mut eval_s[i], &result);
    }
}

fn run_sk(c: &Case, hooks: &str) -> String {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sk"));
    cmd.arg(&c.expr).env("SKILLET_HOOKS_DIR", hooks);
    if let Some(j) = c.json {
        cmd.arg("--json").arg(j);
    }
    let out = cmd
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("spawn sk");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn layer_cli(cases: &[Case], runs: usize) {
    let empty = std::env::temp_dir().join("skillet-baseline-empty-hooks");
    std::fs::create_dir_all(&empty).expect("create empty hooks dir");
    let repo_hooks = format!("{}/hooks", env!("CARGO_MANIFEST_DIR"));
    let variants = [
        ("cli", empty.to_string_lossy().to_string()),
        ("cli+js", repo_hooks),
    ];
    for (_, hooks) in &variants {
        for c in cases {
            run_sk(c, hooks); // warm the page cache
        }
    }
    let mut samples = vec![vec![Vec::with_capacity(runs); cases.len()]; variants.len()];
    for _ in 0..runs {
        for (v, (_, hooks)) in variants.iter().enumerate() {
            for (i, c) in cases.iter().enumerate() {
                let start = Instant::now();
                run_sk(c, hooks);
                samples[v][i].push(start.elapsed());
            }
        }
    }
    // Process-start floor for comparison: spawning a binary that does nothing.
    let mut floor = Vec::with_capacity(runs);
    for _ in 0..runs {
        let start = Instant::now();
        Command::new("/usr/bin/true").status().expect("spawn true");
        floor.push(start.elapsed());
    }
    report("cli", "(floor: /usr/bin/true)", &mut floor, "");
    for (v, (label, hooks)) in variants.iter().enumerate() {
        for (i, c) in cases.iter().enumerate() {
            report(label, c.name, &mut samples[v][i], &run_sk(c, hooks));
        }
    }
}

fn http_post(port: u16, body: &str) -> std::io::Result<String> {
    let mut s = TcpStream::connect(("127.0.0.1", port))?;
    s.set_read_timeout(Some(Duration::from_secs(10)))?;
    let req = format!(
        "POST /eval HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    s.write_all(req.as_bytes())?;
    let mut resp = String::new();
    s.read_to_string(&mut resp)?;
    Ok(resp)
}

fn http_body(c: &Case, nonce: Option<u64>) -> String {
    let mut args: serde_json::Map<String, serde_json::Value> = match c.json {
        Some(j) => serde_json::from_str(j).expect("bench JSON is valid"),
        None => serde_json::Map::new(),
    };
    if let Some(n) = nonce {
        args.insert("bench_nonce".into(), n.into());
    }
    let mut body = serde_json::json!({ "expression": c.expr });
    if !args.is_empty() {
        body["arguments"] = serde_json::Value::Object(args);
    }
    body.to_string()
}

struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn layer_http(cases: &[Case], runs: usize) {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(5099);
    let empty = std::env::temp_dir().join("skillet-baseline-empty-hooks");
    std::fs::create_dir_all(&empty).expect("create empty hooks dir");
    let pid = std::env::temp_dir().join(format!("skillet-baseline-{}.pid", port));
    let _server = Server(
        Command::new(env!("CARGO_BIN_EXE_sk_http_server"))
            .arg(port.to_string())
            .args(["--threads", "4", "--pid-file"])
            .arg(&pid)
            .env("SKILLET_HOOKS_DIR", &empty)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sk_http_server"),
    );
    let deadline = Instant::now() + Duration::from_secs(10);
    while TcpStream::connect(("127.0.0.1", port)).is_err() {
        assert!(
            Instant::now() < deadline,
            "server did not start on {}",
            port
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut nonce = 0u64;
    let cached: Vec<String> = cases.iter().map(|c| http_body(c, None)).collect();
    for b in &cached {
        http_post(port, b).expect("warm request"); // fill the cache
    }
    let mut samples = vec![vec![Vec::with_capacity(runs); cases.len()]; 2];
    for _ in 0..runs {
        for (i, c) in cases.iter().enumerate() {
            let start = Instant::now();
            http_post(port, &cached[i]).expect("cached request");
            samples[0][i].push(start.elapsed());
            nonce += 1;
            let body = http_body(c, Some(nonce));
            let start = Instant::now();
            http_post(port, &body).expect("uncached request");
            samples[1][i].push(start.elapsed());
        }
    }
    for (v, label) in ["http", "http-nc"].iter().enumerate() {
        for (i, c) in cases.iter().enumerate() {
            let resp = http_post(port, &cached[i]).unwrap_or_default();
            let result = resp
                .split("\r\n\r\n")
                .nth(1)
                .and_then(|b| serde_json::from_str::<serde_json::Value>(b).ok())
                .map(|j| j["result"].to_string())
                .unwrap_or_else(|| "?".into());
            report(label, c.name, &mut samples[v][i], &result);
        }
    }
}

fn main() {
    let runs: usize = std::env::var("RUNS")
        .ok()
        .and_then(|s| s.parse().ok())
        .filter(|&n| n > 0)
        .unwrap_or(15);
    let layers = std::env::var("LAYERS").unwrap_or_else(|_| "lib,cli,http".into());
    let filter = std::env::var("CASE").unwrap_or_default();
    let cases: Vec<Case> = cases()
        .into_iter()
        .filter(|c| c.name.contains(&filter))
        .collect();
    println!(
        "BL-005 baseline: runs={} layers={} cases={}",
        runs,
        layers,
        cases.len()
    );
    for layer in layers.split(',') {
        match layer.trim() {
            "lib" => layer_lib(&cases, runs),
            "cli" => layer_cli(&cases, runs),
            "http" => layer_http(&cases, runs),
            other => eprintln!("unknown layer {:?} (expected lib, cli, http)", other),
        }
    }
}
