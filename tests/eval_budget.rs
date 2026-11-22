//! ADR-0011 acceptance: per-evaluation byte budget and evaluation deadline (BL-027b).
//!
//! Rows are named after the ADR example table: P = must keep passing with the same Value,
//! A = must fail with the budget error, C = cost-model boundaries via `with_limits`,
//! T = timeout. Attacks fail early by design, so every test here is cheap.

mod common;

use common::{ServerProcess, SLOW};
use skillet::{
    evaluate, evaluate_with_assignments, evaluate_with_custom, evaluate_with_json_custom,
    with_limits, EvalLimits, Value, MAX_EVAL_BYTES,
};
use std::collections::HashMap;
use std::time::Duration;

const BUDGET_ERROR: &str = "Evaluation budget exceeded (max 256 MB)";

fn nums_vars(n: usize) -> HashMap<String, Value> {
    let mut vars = HashMap::new();
    let nums = (0..n).map(|i| Value::Number(i as f64)).collect();
    vars.insert("nums".to_string(), Value::Array(nums));
    vars
}

/// `n` small row objects plus `rate`, as JSON text (fed through `evaluate_with_json_custom`,
/// so `:rows` items are `Json` values and `:arguments` holds the whole text, like the servers).
fn rows_json(n: usize) -> String {
    let rows: Vec<String> = (0..n)
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
    format!("{{\"rate\":0.16,\"rows\":[{}]}}", rows.join(","))
}

/// The rows as plain Rust tuples (id, cat, price, qty), parsed the same way the library does.
fn rows_parsed(json: &str) -> Vec<(String, f64, f64)> {
    let v: serde_json::Value = serde_json::from_str(json).unwrap();
    v["rows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["cat"].as_str().unwrap().to_string(),
                r["price"].as_f64().unwrap(),
                r["qty"].as_f64().unwrap(),
            )
        })
        .collect()
}

fn ones(n: usize) -> String {
    format!("[{}]", vec!["1"; n].join(", "))
}

fn assert_budget_error(name: &str, result: Result<Value, skillet::Error>) {
    match result {
        Err(e) => assert_eq!(e.message, BUDGET_ERROR, "{name}"),
        Ok(v) => panic!("{name}: expected budget error, got {:?}", short(&v)),
    }
}

fn short(v: &Value) -> String {
    let s = format!("{:?}", v);
    s.chars().take(80).collect()
}

fn limits(max_bytes: u64) -> EvalLimits {
    let mut l = EvalLimits::default();
    l.max_bytes = max_bytes;
    l
}

// ---------------------------------------------------------------- P rows: unchanged Values

#[test]
fn p1_p3_large_number_pipelines_pass() {
    let vars = nums_vars(100_000);
    let cases = [
        (
            ":nums.map(:x*2).filter(:x>10).sum()",
            Value::Number(9999899970.0),
        ),
        (
            ":nums.map(:x*2).map(:x+1).map(:x/3).filter(:x>10).sum()",
            Value::Number(3333333258.3333335),
        ),
        (
            ":nums.sort(\"DESC\").unique().reverse().length()",
            Value::Number(100000.0),
        ),
    ];
    for (src, want) in cases {
        assert_eq!(evaluate_with_custom(src, &vars), Ok(want), "{src}");
    }
}

#[test]
fn p4_p5_row_pipelines_pass() {
    let json = rows_json(10_000);
    let rows = rows_parsed(&json);
    let want = rows
        .iter()
        .filter(|(cat, _, _)| cat == "a")
        .fold(0.0, |acc, (_, p, q)| acc + p * q * (1.0 + 0.16));
    assert_eq!(
        evaluate_with_json_custom(
            ":rows.filter(:x.cat == \"a\").map(:x.price * :x.qty * (1 + :rate)).sum()",
            &json
        ),
        Ok(Value::Number(want))
    );
    assert_eq!(
        evaluate_with_json_custom(
            ":rows.map({\"id\": :x.id, \"total\": :x.price * :x.qty}).length()",
            &json
        ),
        Ok(Value::Number(10000.0))
    );
}

#[test]
fn p6_jq_recursive_descent_on_deep_json_passes() {
    let mut j = format!("{{\"s\":\"{}\"}}", "x".repeat(100_000));
    for _ in 0..120 {
        j = format!("{{\"a\":{}}}", j);
    }
    let mut vars = HashMap::new();
    vars.insert("j".to_string(), Value::Json(j));
    assert_eq!(
        evaluate_with_custom("JQ(:j, \"$..*\").length()", &vars),
        Ok(Value::Number(121.0)),
    );
}

#[test]
fn p7_one_million_nested_iterations_pass() {
    let src = format!(
        "{}.map({}.map(:x + 1).length()).sum()",
        ones(1000),
        ones(1000)
    );
    assert_eq!(evaluate(&src), Ok(Value::Number(1_000_000.0)));
}

#[test]
fn p8_substitute_16mb_passes() {
    let src = format!(
        "SUBSTITUTE(\"{}\", \"a\", \"{}\").length()",
        "a".repeat(4000),
        "b".repeat(4000)
    );
    assert_eq!(evaluate(&src), Ok(Value::Number(16_000_000.0)));
}

#[test]
fn p9_a12_reading_a_large_variable_in_a_lambda_passes() {
    // A12 (amended): variable reads are not charged, so 10k rows pass as well as 1k.
    for n in [1_000, 10_000] {
        let json = rows_json(n);
        let want = rows_parsed(&json)
            .iter()
            .fold(0.0, |acc, (_, p, _)| acc + p / n as f64);
        assert_eq!(
            evaluate_with_json_custom(":rows.map(:x.price / :rows.length()).sum()", &json),
            Ok(Value::Number(want)),
            "{n} rows"
        );
    }
}

#[test]
fn p10_p11_small_workloads_pass() {
    assert_eq!(
        evaluate(&format!("{}.reduce(:acc + :x, 0)", ones(20_000))),
        Ok(Value::Number(20000.0))
    );
    let args: Vec<String> = (1..=1000).map(|i| i.to_string()).collect();
    assert_eq!(
        evaluate(&format!("SUM({})", args.join(", "))),
        Ok(Value::Number(500500.0))
    );
}

#[test]
fn p12_depth_limit_fires_before_the_budget() {
    let ok = evaluate(&format!("{}.reduce([:acc], 0)", ones(128)));
    assert!(
        matches!(ok, Ok(Value::Array(_))),
        "{:?}",
        ok.map(|v| short(&v))
    );
    let err = evaluate(&format!("{}.reduce([:acc], 0)", ones(50_000))).unwrap_err();
    assert_eq!(err.message, "Expression nested too deeply (max 128)");
}

#[test]
fn t1_t2_library_has_no_deadline() {
    // T1 in the library returns the Value (fast since BL-030).
    assert_eq!(
        evaluate_with_custom(":nums.map(:x.abs()).sum()", &nums_vars(100_000)),
        Ok(Value::Number(4999950000.0))
    );
}

// ---------------------------------------------------------------- A rows: budget error

#[test]
fn a1_a4_doubling_reduce_fails() {
    let cases = [
        format!("{}.reduce([:acc, :acc], 0)", ones(26)),
        format!("{}.reduce(CONCAT(:acc, :acc), \"ab\")", ones(32)),
        format!("{}.reduce(MERGE(:acc, :acc), [1])", ones(30)),
        format!("{}.reduce({{\"a\": :acc, \"b\": :acc}}, 0)", ones(30)),
    ];
    for src in cases {
        assert_budget_error(&src[src.len() - 40..], evaluate(&src));
    }
}

#[test]
fn a5_a6_doubling_assignments_fail() {
    let merge = format!(
        ":a := [1]; {} :a.length()",
        ":a := MERGE(:a, :a);".repeat(30)
    );
    assert_budget_error("A5", evaluate_with_assignments(&merge, &HashMap::new()));
    let concat = format!(
        ":s := \"ab\"; {} :s.length()",
        ":s := CONCAT(:s, :s);".repeat(32)
    );
    assert_budget_error("A6", evaluate_with_assignments(&concat, &HashMap::new()));
}

#[test]
fn a7_a9_amplifying_builtins_fail_before_allocating() {
    let a = "a".repeat(20_000);
    let b = "b".repeat(20_000);
    let empties = format!("[{}]", vec!["\"\""; 20_000].join(","));
    let cases = [
        ("A7", format!("SUBSTITUTE(\"{a}\", \"a\", \"{b}\")")),
        ("A7m", format!("SUBSTITUTEM(\"{a}\", \"a\", \"{b}\")")),
        ("A8", format!("\"{a}\".replace(\"\", \"{b}\")")),
        ("A9", format!("JOIN({empties}, \"{b}\")")),
        ("A9m", format!("{empties}.join(\"{b}\")")),
    ];
    for (name, src) in cases {
        assert_budget_error(name, evaluate(&src));
    }
}

#[test]
fn split_is_charged_per_part() {
    // 1 MB string split into ~1M one-char parts: 48 + 1 bytes each, ~49 MB, passes.
    let s = "a".repeat(1_000_000);
    assert_eq!(
        evaluate(&format!("SPLIT(\"{s}\", \"\").length()")),
        Ok(Value::Number(1_000_002.0))
    );
    // With a tight budget the split itself fails while collecting.
    let err = with_limits(limits(1024 * 1024), || {
        evaluate(&format!("\"{s}\".split(\"\").length()"))
    })
    .unwrap_err();
    assert_eq!(err.message, "Evaluation budget exceeded (max 1 MB)");
}

#[test]
fn a10_billion_iterations_fail_fast() {
    let src = format!(
        "{}.map({}.map({}.map(:x).length()).sum())",
        ones(1000),
        ones(1000),
        ones(1000)
    );
    assert_budget_error("A10", evaluate(&src));
}

#[test]
fn a11_quadratic_merge_accumulation_behaviour_change() {
    // Behaviour change accepted in ADR-0011: 10k items now fail; small sizes still pass.
    assert_budget_error(
        "A11 10k",
        evaluate_with_custom(
            ":nums.reduce(MERGE(:acc, [:x]), []).length()",
            &nums_vars(10_000),
        ),
    );
    assert_eq!(
        evaluate_with_custom(
            ":nums.reduce(MERGE(:acc, [:x]), []).length()",
            &nums_vars(2_800)
        ),
        Ok(Value::Number(2800.0))
    );
}

#[test]
fn copying_a_large_variable_per_item_fails_hoisting_passes() {
    // ADR-0011 amendment 2: `SUM(:nums)` copies `:nums` (charged 32 + 32 × 5000 bytes) once
    // per item, ~800 MB in total; hoisting the copy out of the lambda makes it one copy.
    let vars = nums_vars(5_000);
    assert_budget_error(
        "per-item copy",
        evaluate_with_custom(":nums.map(SUM(:nums)).sum()", &vars),
    );
    assert_eq!(
        evaluate_with_assignments(":t := SUM(:nums); :nums.map(:t).sum()", &vars),
        Ok(Value::Number(12_497_500.0 * 5_000.0))
    );
    // Borrowed reads stay free: the same size passes as a method target.
    assert_eq!(
        evaluate_with_custom(":nums.map(:nums.sum()).sum()", &vars),
        Ok(Value::Number(12_497_500.0 * 5_000.0))
    );
}

#[test]
fn swallowed_budget_error_is_still_reported() {
    // SUMIF tries its criteria as a static value first and ignores errors from that attempt;
    // with a third argument it then fails with an unrelated message. The entry point reports
    // the budget error that was hit instead.
    let src = format!(
        "SUMIF([1, 2], {}.reduce(MERGE(:acc, :acc), [1]), [1, 2])",
        ones(30)
    );
    assert_budget_error("SUMIF", evaluate(&src));

    // A custom function that catches the error of a nested evaluation cannot hide it either.
    struct Catcher;
    impl skillet::CustomFunction for Catcher {
        fn name(&self) -> &str {
            "BUDGET_CATCHER"
        }
        fn min_args(&self) -> usize {
            0
        }
        fn max_args(&self) -> Option<usize> {
            Some(0)
        }
        fn execute(&self, _args: Vec<Value>) -> Result<Value, skillet::Error> {
            let attack = format!("{}.reduce(MERGE(:acc, :acc), [1])", ones(30));
            Ok(skillet::evaluate(&attack).unwrap_or(Value::Null))
        }
    }
    skillet::register_function(Box::new(Catcher)).unwrap();
    let res = evaluate_with_custom("BUDGET_CATCHER()", &HashMap::new());
    skillet::unregister_function("BUDGET_CATCHER");
    assert_budget_error("custom catch", res);
}

#[test]
fn nested_evaluation_joins_the_active_budget() {
    struct Inner;
    impl skillet::CustomFunction for Inner {
        fn name(&self) -> &str {
            "BUDGET_INNER"
        }
        fn min_args(&self) -> usize {
            0
        }
        fn max_args(&self) -> Option<usize> {
            Some(0)
        }
        fn execute(&self, _args: Vec<Value>) -> Result<Value, skillet::Error> {
            // 1312 bytes inside the custom function count towards the caller's budget.
            skillet::evaluate(&format!("{}.length()", ones(40)))
        }
    }
    skillet::register_function(Box::new(Inner)).unwrap();
    let vars = HashMap::new();
    assert!(with_limits(limits(2000), || evaluate_with_custom(
        "BUDGET_INNER()",
        &vars
    ))
    .is_ok());
    let err = with_limits(limits(1000), || {
        evaluate_with_custom("BUDGET_INNER()", &vars)
    })
    .unwrap_err();
    assert_eq!(err.message, "Evaluation budget exceeded (max 1000 bytes)");
    skillet::unregister_function("BUDGET_INNER");
}

// ---------------------------------------------------------------- C rows: cost model

#[test]
fn c1_c4_cost_model_boundaries() {
    assert_eq!(
        with_limits(limits(160), || evaluate("[1, 2, 3, 4]")),
        Ok(Value::Array(
            (1..=4).map(|i| Value::Number(i as f64)).collect()
        ))
    );
    assert_eq!(
        with_limits(limits(160), || evaluate("[1, 2, 3, 4, 5]"))
            .unwrap_err()
            .message,
        "Evaluation budget exceeded (max 160 bytes)"
    );
    assert_eq!(
        with_limits(limits(52), || evaluate("\"abcd\".upper()")),
        Ok(Value::String("ABCD".into()))
    );
    assert_eq!(
        with_limits(limits(51), || evaluate("\"abcd\".upper()"))
            .unwrap_err()
            .message,
        "Evaluation budget exceeded (max 51 bytes)"
    );
}

#[test]
fn c5_inner_scope_cannot_raise_outer_budget() {
    let err = with_limits(limits(1000), || {
        with_limits(limits(10 * 1024 * 1024), || evaluate(&ones(40)))
    })
    .unwrap_err();
    assert_eq!(err.message, "Evaluation budget exceeded (max 1000 bytes)");
}

#[test]
fn c6_timeout() {
    let mut l = EvalLimits::default();
    l.timeout = Some(Duration::from_millis(1));
    let vars = nums_vars(100_000);
    let err = with_limits(l, || {
        // Several passes so even a fast machine runs well past 1 ms.
        evaluate_with_custom(
            ":nums.map(:x.abs()).map(:x.abs()).map(:x.abs()).map(:x.abs()).sum()",
            &vars,
        )
    })
    .unwrap_err();
    assert_eq!(err.message, "Evaluation timed out (max 1 ms)");
}

#[test]
fn defaults_and_scope_reset() {
    assert_eq!(MAX_EVAL_BYTES, 256 * 1024 * 1024);
    assert_eq!(EvalLimits::default().max_bytes, MAX_EVAL_BYTES);
    assert_eq!(EvalLimits::default().timeout, None);
    // Each top-level evaluation gets a fresh budget: 4 × 81 MB in sequence is fine.
    let src = format!(
        "SUBSTITUTE(\"{}\", \"a\", \"{}\").length()",
        "a".repeat(9_000),
        "b".repeat(9_000)
    );
    for _ in 0..4 {
        assert_eq!(evaluate(&src), Ok(Value::Number(81_000_000.0)));
    }
}

// ---------------------------------------------------------------- servers (127.0.0.1 only)

/// Server flags for the limits the server tests below were written for.
const LIBRARY_LIMITS: [&str; 4] = ["--eval-timeout", "10", "--max-eval-bytes", "256M"];

/// A server binary on its own 127.0.0.1 port, killed on drop (tests/common, BL-061).
struct Server(ServerProcess);

impl Server {
    fn spawn(bin: &str, args: impl Fn(u16) -> Vec<String>) -> Server {
        let hooks = std::env::temp_dir().join("skillet-eval-budget-no-hooks");
        Server(ServerProcess::start(|port| {
            let mut cmd = std::process::Command::new(bin);
            cmd.args(args(port))
                .env("SKILLET_HOOKS_DIR", &hooks)
                .env("SKILLET_BIND_HOST", "127.0.0.1")
                .stdout(std::process::Stdio::null());
            cmd
        }))
    }

    /// `sk_http_server` with one worker, a 10 s deadline and the library's 256 MiB budget
    /// (the server default deadline is 20 s since ADR-0011 amendment 4; the budget default is
    /// the same 256 MiB, set explicitly so the tests do not depend on it).
    fn http() -> Server {
        Server::spawn(env!("CARGO_BIN_EXE_sk_http_server"), |port| {
            let mut args = vec![port.to_string(), "--threads".into(), "1".into()];
            args.extend(LIBRARY_LIMITS.map(String::from));
            args
        })
    }

    /// `sk_server` with one worker and the same limits as [`Server::http`].
    fn line() -> Server {
        Server::spawn(env!("CARGO_BIN_EXE_sk_server"), |port| {
            let mut args = vec![port.to_string(), "1".into()];
            args.extend(LIBRARY_LIMITS.map(String::from));
            args
        })
    }

    /// POST `body` to `/eval`; returns the raw response and the time it took.
    fn post(&self, body: &str) -> (String, Duration) {
        use std::io::{Read, Write};
        let start = std::time::Instant::now();
        let mut s = self.0.connect();
        let req = format!(
            "POST /eval HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        s.write_all(req.as_bytes()).unwrap();
        let mut resp = String::new();
        let _ = s.read_to_string(&mut resp);
        (resp, start.elapsed())
    }

    /// Send one newline-delimited request to `sk_server`; returns the response line.
    fn send_line(&self, body: &str) -> String {
        use std::io::{BufRead, BufReader, Write};
        let mut s = self.0.connect();
        s.write_all(format!("{body}\n").as_bytes()).unwrap();
        let mut line = String::new();
        let _ = BufReader::new(s).read_line(&mut line);
        line
    }
}

fn head(resp: &str) -> &str {
    &resp[..resp.len().min(300)]
}

/// T1 against `sk_http_server`. The ADR's T1 formula (`:nums.map(:x.abs())`) runs in
/// milliseconds since BL-030, and since BL-027d every copying read is charged, so this uses
/// work the budget only ticks for: comparing two 400 KB strings borrows them (no copy, no
/// charge), 1M iterations × 4 comparisons ≫ 10 s.
#[test]
#[ignore = "slow: server timeout (~10 s)"]
fn t1_server_times_out_and_frees_the_worker() {
    let server = Server::http();
    let cmp = "((:s == :t) == (:t == :s)) == ((:s == :t) == (:t == :s))";
    let expr = format!(
        "{}.map({}.map({cmp}).length()).sum()",
        ones(1000),
        ones(1000)
    );
    let text = "x".repeat(400_000);
    let body = format!(
        "{{\"expression\":\"{expr}\",\"arguments\":{{\"s\":\"{text}\",\"t\":\"{text}\"}}}}"
    );
    let (resp, took) = server.post(&body);
    let quick = server.post("{\"expression\":\"2 + 3\"}");

    assert!(resp.starts_with("HTTP/1.1 400"), "{}", head(&resp));
    assert!(
        resp.contains("Evaluation timed out (max 10 s)"),
        "{}",
        head(&resp)
    );
    // The message proves the deadline fired, the lower bound that it did not fire early. The
    // upper bound only guards against a hang (BL-087 QA-8; it was 20 s).
    assert!(took >= Duration::from_secs(10) && took < SLOW, "{took:?}");
    // The single worker is free again.
    assert!(quick.0.starts_with("HTTP/1.1 200"), "{}", quick.0);
    assert!(quick.0.contains('5'), "{}", quick.0);
}

/// F3 (BL-027c): a result whose JSON-escaped response would exceed the budget is rejected
/// with the budget error instead of being serialized. 46 MB of `\x01` is 276 MB escaped
/// (`\u0001`, 6 bytes each), over the 256 MiB budget; the value itself is within it.
#[test]
fn f3_servers_reject_responses_larger_than_the_budget() {
    let ctl = |n: usize| {
        format!(
            "SUBSTITUTE(SUBSTITUTE(\\\"{}\\\", \\\"a\\\", \\\"{}\\\"), \\\"a\\\", \\\"{}\\\")",
            "a".repeat(10),
            "a".repeat(100),
            "\\u0001".repeat(n)
        )
    };
    let big = ctl(46_000); // 1000 × 46,000 chars
    let small = ctl(1); // 1000 chars

    let http = Server::http();
    let (resp, _) = http.post(&format!("{{\"expression\":\"{big}\"}}"));
    assert!(resp.starts_with("HTTP/1.1 400"), "{}", head(&resp));
    assert!(resp.contains(BUDGET_ERROR), "{}", head(&resp));
    // Assigned variables returned with the result count too.
    let (resp, _) = http.post(&format!(
        "{{\"expression\":\":s := {big}; 1\",\"include_variables\":true}}"
    ));
    assert!(resp.starts_with("HTTP/1.1 400"), "{}", head(&resp));
    assert!(resp.contains(BUDGET_ERROR), "{}", head(&resp));
    let (resp, _) = http.post(&format!("{{\"expression\":\"{small}\"}}"));
    assert!(resp.starts_with("HTTP/1.1 200"), "{}", head(&resp));
    assert!(resp.contains(&"\\u0001".repeat(1000)), "{}", head(&resp));
    drop(http);

    let line = Server::line();
    let resp = line.send_line(&format!("{{\"expression\":\"{big}\"}}"));
    assert!(resp.contains("\"success\":false"), "{}", head(&resp));
    assert!(resp.contains(BUDGET_ERROR), "{}", head(&resp));
    let resp = line.send_line(&format!("{{\"expression\":\"{small}\"}}"));
    assert!(resp.contains("\"success\":true"), "{}", head(&resp));
}

// ---------------------------------------------------------------- BL-027c: re-attack
//
// Security review of the budget as built. Each attack is scaled down with `with_limits` so it
// stays fast in debug. Tests marked `#[ignore = "open: …"]` pin a finding that blocks the PR
// and must pass once it is fixed (route: rust-engineer). Heap is measured per thread by the
// counting allocator below (other tests in this binary run on other threads).

mod heap {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        static LIVE: Cell<usize> = const { Cell::new(0) };
        static PEAK: Cell<usize> = const { Cell::new(0) };
        /// Bytes the system allocator really handed out (its usable size, BL-035).
        static REAL: Cell<usize> = const { Cell::new(0) };
    }

    pub struct Counting;

    fn add(n: usize) {
        let _ = LIVE.try_with(|l| {
            let v = l.get().saturating_add(n);
            l.set(v);
            let _ = PEAK.try_with(|p| p.set(p.get().max(v)));
        });
    }

    fn sub(n: usize) {
        let _ = LIVE.try_with(|l| l.set(l.get().saturating_sub(n)));
    }

    /// Usable size of a live block: what the allocator really reserved for a request of
    /// `requested` bytes (rounded up to its size class). Falls back to the request elsewhere.
    unsafe fn usable(ptr: *mut u8, requested: usize) -> usize {
        if ptr.is_null() {
            0
        } else if cfg!(target_vendor = "apple") || cfg!(target_os = "linux") {
            // SAFETY: the caller passes a live, non-null block from the system allocator.
            unsafe { system_usable(ptr) }
        } else {
            requested
        }
    }

    #[cfg(target_vendor = "apple")]
    unsafe fn system_usable(ptr: *mut u8) -> usize {
        // SAFETY: `ptr` is a live block from the system allocator (caller contract).
        unsafe { libc::malloc_size(ptr as *const libc::c_void) }
    }

    #[cfg(target_os = "linux")]
    unsafe fn system_usable(ptr: *mut u8) -> usize {
        // SAFETY: `ptr` is a live block from the system allocator (caller contract).
        unsafe { libc::malloc_usable_size(ptr as *mut libc::c_void) }
    }

    #[cfg(not(any(target_vendor = "apple", target_os = "linux")))]
    unsafe fn system_usable(_ptr: *mut u8) -> usize {
        0
    }

    fn real(add: usize, sub: usize) {
        let _ = REAL.try_with(|r| r.set(r.get().saturating_add(add).saturating_sub(sub)));
    }

    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            add(layout.size());
            // SAFETY: forwards the `GlobalAlloc` contract to `System`; `ptr` is its result.
            unsafe {
                let ptr = System.alloc(layout);
                real(usable(ptr, layout.size()), 0);
                ptr
            }
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            sub(layout.size());
            // SAFETY: forwards the `GlobalAlloc` contract: `ptr` is live and from `System`.
            unsafe {
                real(0, usable(ptr, layout.size()));
                System.dealloc(ptr, layout)
            }
        }
        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            add(new_size);
            sub(layout.size());
            // SAFETY: forwards the `GlobalAlloc` contract: `ptr` is live and from `System`.
            unsafe {
                let old = usable(ptr, layout.size());
                let new = System.realloc(ptr, layout, new_size);
                if !new.is_null() {
                    real(usable(new, new_size), old);
                }
                new
            }
        }
    }

    /// Run `f` and return its result plus the bytes the allocator really holds for it
    /// afterwards (usable sizes, not requested sizes), on this thread.
    pub fn real_held_by<R>(f: impl FnOnce() -> R) -> (R, usize) {
        let start = REAL.with(|r| r.get());
        let r = f();
        (r, REAL.with(|r| r.get()).saturating_sub(start))
    }

    /// Run `f` and return its result plus the peak heap growth (bytes) on this thread.
    pub fn peak_during<R>(f: impl FnOnce() -> R) -> (R, usize) {
        let start = LIVE.with(|l| l.get());
        PEAK.with(|p| p.set(start));
        let r = f();
        (r, PEAK.with(|p| p.get()).saturating_sub(start))
    }
}

#[global_allocator]
static HEAP: heap::Counting = heap::Counting;

const MIB: u64 = 1024 * 1024;

/// Expression for a string of `1000 × reps` copies of `unit` (two nested SUBSTITUTEs).
fn big_string(unit: &str, reps: usize) -> String {
    format!(
        "SUBSTITUTE(SUBSTITUTE(\"{a}\", \"a\", \"{b}\"), \"a\", \"{c}\")",
        a = "a".repeat(10),
        b = "a".repeat(100),
        c = unit.repeat(reps)
    )
}

fn timeout_limits(ms: u64) -> EvalLimits {
    let mut l = EvalLimits::default();
    l.timeout = Some(Duration::from_millis(ms));
    l
}

/// Lambda bodies built only from uncharged reads still meet the deadline: every iteration
/// charges the bound item, and the clock is read every 256 charges.
#[test]
fn bl027c_lambda_over_uncharged_reads_meets_the_deadline() {
    let src = format!(
        ":s := {}; {}.map(:s == :s).length()",
        big_string("a", 1000), // 1 MB
        ones(20_000)
    );
    let start = std::time::Instant::now();
    let r = with_limits(timeout_limits(1), || {
        evaluate_with_assignments(&src, &HashMap::new())
    });
    let took = start.elapsed();
    assert_eq!(
        r.map_err(|e| e.message),
        Err("Evaluation timed out (max 1 ms)".to_string())
    );
    // The error is the proof: with no clock read inside the loop, the evaluation would finish
    // and return Ok. The time bound cannot tell the two apart since the comparisons borrow
    // (BL-027d): the whole loop without a deadline takes ~0.4 s in debug, under the old 2 s
    // bound. So the bound is now only a hang guard (BL-087 QA-8).
    assert!(took < SLOW, "{took:?}");
}

/// The deadline is 1 ms, far below the work done (thousands of 1 MB comparisons), so the
/// test does not depend on how fast the machine is: CI runners finished it under 50 ms.
/// F1 (fixed in BL-027d): a flat list of reads with no charge in between never read the clock.
/// `:s == :s` cloned `:s` twice, and binary operators were not charged, so an array literal
/// of N comparisons did 2N full clones before its single charge. Measured on the release
/// server with an 80 MB `:s` and N = 4000 (42 KB request): 20.4 s, `success: true`, past the
/// 10 s deadline; the time grows linearly with N up to the 1 MB request cap (~8 min).
#[test]
fn bl027c_flat_uncharged_reads_meet_the_deadline() {
    let cmps = vec![":s == :s"; 3_000].join(", ");
    let src = format!(":s := {}; [{}].length()", big_string("a", 1000), cmps);
    let start = std::time::Instant::now();
    let r = with_limits(timeout_limits(1), || {
        evaluate_with_assignments(&src, &HashMap::new())
    });
    let took = start.elapsed();
    assert_eq!(
        r.map_err(|e| e.message),
        Err("Evaluation timed out (max 1 ms)".to_string())
    );
    // As above, the error is the proof (before F1 this returned `success: true`). The whole
    // list without a deadline takes ~60 ms in debug, under the old 500 ms bound, so the bound
    // could not catch F1 coming back and only added flake risk under load (BL-087 QA-8).
    assert!(took < SLOW, "{took:?}");
}

/// F2 (fixed in BL-027d): heap allocated before the budget check must stay within ~2× the
/// budget. Before the fix each case failed with the budget error only after allocating far
/// more than the budget: k uncharged reads in one argument list / literal (CONCAT, MERGE, array and object
/// literals), and builtins that amplify their input and are charged after allocating
/// (`.to_a()` on a string ≈ 48 real bytes per char, `.to_json()` escaping ≤ 6×,
/// `::String` on an array via `Debug` escaping ≤ 5×).
#[test]
fn bl027c_peak_heap_stays_within_twice_the_budget() {
    let budget = 4 * MIB;
    let one_mb = big_string("a", 1000);
    let reads = |n: usize| vec![":s"; n].join(", ");
    let cases = [
        (
            "CONCAT of 40 reads",
            format!(":s := {one_mb}; CONCAT({})", reads(40)),
        ),
        (
            "array literal of 40 reads",
            format!(":s := {one_mb}; [{}]", reads(40)),
        ),
        (
            "object literal of 20 reads",
            format!(
                ":s := {one_mb}; {{{}}}",
                (0..20)
                    .map(|i| format!("\"k{i}\": :s"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        ),
        (
            "MERGE of 40 array reads",
            format!(":a := SPLIT({}, \"a\"); MERGE({})", big_string("a", 30), {
                vec![":a"; 40].join(", ")
            }),
        ),
        ("to_a on a 1 MB string", format!("{one_mb}.to_a()")),
        (
            "to_json on 1 MB of \\x01",
            format!("{}.to_json()", big_string("\u{1}", 1000)),
        ),
        (
            "::String of [1 MB of \\x01]",
            format!("[{}]::String", big_string("\u{1}", 1000)),
        ),
    ];
    let mut failures = Vec::new();
    for (name, src) in &cases {
        let (r, peak) = heap::peak_during(|| {
            with_limits(limits(budget), || {
                evaluate_with_assignments(src, &HashMap::new())
            })
        });
        let ok = matches!(&r, Err(e) if e.message.starts_with("Evaluation budget exceeded"));
        if !ok || peak as u64 > 2 * budget + MIB {
            failures.push(format!(
                "{name}: peak {:.1} MB (budget {} MB), result {:?}",
                peak as f64 / MIB as f64,
                budget / MIB,
                r.map(|v| short(&v)).map_err(|e| e.message)
            ));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

/// Pre-charged builtins stay within ~2× the budget at their peak (these hold today).
#[test]
fn bl027c_pre_charged_builtins_peak_within_twice_the_budget() {
    let budget = 4 * MIB;
    let cases = [
        (
            "SPLIT per char",
            format!("SPLIT({}, \"\")", big_string("a", 1000)),
        ),
        ("SUBSTITUTE blow-up", big_string("a", 10_000)),
        (
            "JOIN separator",
            format!(
                "JOIN(SPLIT({}, \"a\"), \"{}\")",
                big_string("a", 100),
                "x".repeat(100)
            ),
        ),
    ];
    let mut failures = Vec::new();
    for (name, src) in &cases {
        let (r, peak) = heap::peak_during(|| with_limits(limits(budget), || evaluate(src)));
        let ok = matches!(&r, Err(e) if e.message.starts_with("Evaluation budget exceeded"));
        if !ok || peak as u64 > 2 * budget + MIB {
            failures.push(format!(
                "{name}: peak {:.1} MB, result {:?}",
                peak as f64 / MIB as f64,
                r.map(|v| short(&v)).map_err(|e| e.message)
            ));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

// ---------------------------------------------------------------- BL-027e: confirmation pass
//
// Re-checks the BL-027d fixes. A plain variable used as an operator operand or a
// method/property/index/slice target is borrowed (not copied, not charged); every other read
// copies and is charged before the copy. These pin that a borrowed value cannot escape into
// a result without being charged.

/// Run each case under a 4 MiB budget and collect the ones that did not fail with the budget
/// error, or peaked above ~2x the budget before failing.
fn peak_failures(cases: &[(&str, String)]) -> Vec<String> {
    let budget = 4 * MIB;
    let mut failures = Vec::new();
    for (name, src) in cases {
        let (r, peak) = heap::peak_during(|| {
            with_limits(limits(budget), || {
                evaluate_with_assignments(src, &HashMap::new())
            })
        });
        let ok = matches!(&r, Err(e) if e.message.starts_with("Evaluation budget exceeded"));
        if !ok || peak as u64 > 2 * budget + MIB {
            failures.push(format!(
                "{name}: peak {:.1} MB (budget {} MB), result {:?}",
                peak as f64 / MIB as f64,
                budget / MIB,
                r.map(|v| short(&v)).map_err(|e| e.message)
            ));
        }
    }
    failures
}

/// Every way a read can leave the expression as a value (ternary/IF branches, spread, nested
/// assignment, lambda results, reduce accumulator, index/slice/method results) is charged.
#[test]
fn bl027e_reads_that_escape_into_a_result_are_charged() {
    let s = format!(":s := {}; ", big_string("a", 1000)); // 1 MB
    let a = format!("{s}:a := [:s]; ");
    let x8 = |e: &str| [e; 8].join(", ");
    let cases = [
        ("ternary branch", format!("{s}[{}]", x8("true ? :s : 0"))),
        ("IF branch", format!("{s}[{}]", x8("IF(true, :s, 0)"))),
        ("spread into CONCAT", format!("{a}CONCAT({})", x8("...:a"))),
        ("nested assignment", format!("{s}[{}]", x8("(:b := :s)"))),
        ("sequence of assignments", {
            let assigns: Vec<String> = (0..8).map(|i| format!(":v{i} := :s")).collect();
            format!("{s}{}; 1", assigns.join("; "))
        }),
        ("lambda result", format!("{s}{}.map(:s)", ones(8))),
        (
            "reduce accumulator",
            format!("{s}{}.reduce(:s, 0)", ones(8)),
        ),
        ("index of a variable", format!("{a}[{}]", x8(":a[0]"))),
        ("slice of a variable", format!("{a}[{}]", x8(":a[0:1]"))),
        ("method on a variable", format!("{a}[{}]", x8(":a.first()"))),
        ("identity method", format!("{s}[{}]", x8(":s.to_s()"))),
        ("identity cast", format!("{s}[{}]", x8(":s::String"))),
    ];
    let failures = peak_failures(&cases);
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

/// Borrowed operands only produce scalars, and still tick: `:s == :s` and `NOT` over a large
/// variable allocate nothing and return a Boolean.
#[test]
fn bl027e_borrowed_operands_do_not_copy() {
    let src = format!(
        ":s := {}; [{}].length()",
        big_string("a", 1000),
        vec!["(:s == :s) == !(:s != :s)"; 200].join(", ")
    );
    let (r, peak) = heap::peak_during(|| evaluate_with_assignments(&src, &HashMap::new()));
    assert_eq!(r, Ok(Value::Number(200.0)));
    // `:s` (1 MB) plus its stored copy and the SUBSTITUTE intermediates; no per-use copy.
    assert!((peak as u64) < 5 * MIB, "peak {peak}");
}

/// F4 (found in BL-027e, fixed in BL-027f): an object literal serializes its values
/// (JSON escaping, up to 6x for control characters) after the reads are charged and before
/// the result is checked. Release `sk`, `:s` = 20 MB of `\x01`, `{"k0": :s, …}`: 1 key 199 MB,
/// 4 keys 599 MB (sk_http_server 602 MB, sk_server 600 MB), 6 keys 866 MB peak; about
/// +133 MB per key until the reads alone reach the budget (~2 GB). Fixed: the serialized size
/// is pre-charged (same as `.to_json()` on arrays) before `serde_json::to_string`.
#[test]
fn bl027e_object_literal_escaping_is_charged_before_serializing() {
    let s = format!(":s := {}; ", big_string("\u{1}", 200)); // 200 KB of \x01
    let a = format!("{s}:a := [:s]; ");
    // Key counts keep the charged reads under the 4 MiB budget; the escaped text is ~6x.
    let obj = |v: &str, n: usize| {
        let pairs: Vec<String> = (0..n).map(|i| format!("\"k{i}\": {v}")).collect();
        format!("{{{}}}", pairs.join(", "))
    };
    let cases = [
        ("object of 16 string reads", format!("{s}{}", obj(":s", 16))),
        ("object of 12 array reads", format!("{a}{}", obj(":a", 12))),
    ];
    let failures = peak_failures(&cases);
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

/// Same class as F4 (BL-027f): a `JQ` match that is an object becomes `Json` text again, so
/// escaping grows it up to 6x over its parsed size. `$..*` on nested objects returns every
/// level, each re-serialized with the whole string: charged by its text, not its parts.
#[test]
fn bl027f_jq_object_matches_are_charged_as_json_text() {
    let budget = 4 * MIB;
    let mut text = format!("\"{}\"", "\\u0001".repeat(100_000)); // 100 KB of \x01, 600 KB text
    for _ in 0..20 {
        text = format!("{{\"a\": {text}}}");
    }
    let vars = HashMap::from([("j".to_string(), Value::Json(text))]);
    let (r, peak) = heap::peak_during(|| {
        with_limits(limits(budget), || {
            evaluate_with_custom("JQ(:j, \"$..*\").length()", &vars)
        })
    });
    assert!(
        matches!(&r, Err(e) if e.message.starts_with("Evaluation budget exceeded")),
        "{r:?}"
    );
    assert!((peak as u64) < 2 * budget + MIB, "peak {peak}");
}

// ---------------------------------------------------------------- BL-035: hardening

/// The per-string charge matches what a small string really costs: its 32-byte `Value` slot
/// plus the smallest heap block. Measured with the allocator's usable sizes; BL-027c found 33
/// charged against ~48 real for a 1-char string (macOS: 16-byte minimum block, so 48 exactly).
#[test]
fn bl035_tiny_strings_are_charged_what_they_really_cost() {
    const N: usize = 100_000;
    let (strings, held) = heap::real_held_by(|| {
        let mut v: Vec<Value> = Vec::with_capacity(N);
        v.extend((0..N).map(|_| Value::String("x".to_string())));
        v
    });
    let real = held as f64 / N as f64;
    drop(strings);
    // Charged per 1-char string: `"x".upper()` is a 1-char result, 48 + 1 bytes.
    let charged = 49.0;
    assert!(with_limits(limits(49), || evaluate("\"x\".upper()")).is_ok());
    assert!(with_limits(limits(48), || evaluate("\"x\".upper()")).is_err());
    assert!(
        real <= charged * 1.25 && real >= charged * 0.6,
        "real {real:.1} B per 1-char string, charged {charged}"
    );
}

/// Case mapping can grow non-ASCII text 3x (`ΐ`, 2 bytes, upper-cases to `Ϊ́`, 6). The output
/// must fit the budget before it is built; ASCII and text that fits are unchanged.
#[test]
fn bl035_case_mapping_fails_before_growing() {
    let budget = 4 * MIB;
    // 3.6 MB of "ΐ" fits the budget; its 10.8 MB upper-case does not. U+0130 lower-cases to 3
    // bytes (1.5x). Before BL-035 the output was built first: peak 24 MB and 13.7 MB.
    let vars = HashMap::from([
        ("s".to_string(), Value::String("ΐ".repeat(1_800_000))),
        ("t".to_string(), Value::String("İ".repeat(1_800_000))),
    ]);
    let cases = [
        "UPPER(:s)",
        ":s.upper()",
        ":s.upcase()",
        "LOWER(:t)",
        ":t.downcase()",
    ];
    let mut failures = Vec::new();
    for src in cases {
        let (r, peak) =
            heap::peak_during(|| with_limits(limits(budget), || evaluate_with_custom(src, &vars)));
        let ok = matches!(&r, Err(e) if e.message.starts_with("Evaluation budget exceeded"));
        if !ok || peak as u64 > 2 * budget + MIB {
            failures.push(format!(
                "{src}: peak {:.1} MB, result {:?}",
                peak as f64 / MIB as f64,
                r.map(|v| short(&v)).map_err(|e| e.message)
            ));
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
    // Same values as before, and the exact boundary: 48 + 6 bytes for one upper-cased "ΐ".
    assert_eq!(
        evaluate("UPPER(\"ΐé\")"),
        Ok(Value::String("\u{399}\u{308}\u{301}É".into()))
    );
    assert_eq!(evaluate("\"ΣΑΣ\".lower()"), Ok(Value::String("σας".into())));
    assert_eq!(
        with_limits(limits(54), || evaluate("\"ΐ\".upper()")),
        Ok(Value::String("\u{399}\u{308}\u{301}".into()))
    );
    assert_eq!(
        with_limits(limits(53), || evaluate("\"ΐ\".upper()"))
            .unwrap_err()
            .message,
        "Evaluation budget exceeded (max 53 bytes)"
    );
}

/// One long builtin call no longer runs to completion past the deadline: SORT, UNIQUE, the
/// joins and their method forms tick every 64 items. Before BL-035 each of these returned
/// `Ok` after the full call (release, 3M items: 60 ms to 1.1 s) despite a 1 ms deadline.
#[test]
fn bl035_long_builtins_meet_the_deadline() {
    let vars = nums_vars(300_000);
    let exprs = [
        "SORT(:nums).length()",
        "UNIQUE(:nums).length()",
        ":nums.sort().length()",
        ":nums.unique().length()",
        "JOIN(:nums, \",\").length()",
        ":nums.join(\",\").length()",
        "TEXTJOIN(\",\", TRUE, :nums).length()",
    ];
    for e in exprs {
        assert!(evaluate_with_custom(e, &vars).is_ok(), "{e}");
        let r = with_limits(timeout_limits(1), || {
            std::thread::sleep(Duration::from_millis(2));
            evaluate_with_custom(e, &vars)
        });
        assert_eq!(
            r.map_err(|e| e.message),
            Err("Evaluation timed out (max 1 ms)".to_string()),
            "{e}"
        );
    }
}

/// The deadline is honoured in criteria matching (BL-080): COUNTIF/SUMIF/AVERAGEIF with an
/// Excel criterion tick per row, and the wildcard matcher ticks while it scans one long text.
/// Before, the criteria loop never read the clock, so these returned `Ok` past the deadline.
#[test]
fn bl080_deadline_is_honoured_in_criteria_matching() {
    let mut vars = HashMap::new();
    let words = (0..300_000)
        .map(|i| Value::String(format!("w{}x{}", i % 97, i % 13)))
        .collect();
    vars.insert("words".to_string(), Value::Array(words));
    vars.insert("text".to_string(), Value::String("a".repeat(1_000_000)));
    let exprs = [
        "COUNTIF(:words, \"*7?3\")",
        "COUNTIF(:words, \"<>w1*\")",
        "SUMIF(:words, \">w5\")",
        "COUNTIF(:text, \"*b*\")",
        "COUNTIF(:text, \"*a?b*\")",
    ];
    for e in exprs {
        assert!(evaluate_with_custom(e, &vars).is_ok(), "{e}");
        let r = with_limits(timeout_limits(1), || {
            std::thread::sleep(Duration::from_millis(2));
            evaluate_with_custom(e, &vars)
        });
        assert_eq!(
            r.map_err(|e| e.message),
            Err("Evaluation timed out (max 1 ms)".to_string()),
            "{e}"
        );
    }
}
