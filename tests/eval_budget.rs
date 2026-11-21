//! ADR-0011 acceptance: per-evaluation byte budget and evaluation deadline (BL-027b).
//!
//! Rows are named after the ADR example table: P = must keep passing with the same Value,
//! A = must fail with the budget error, C = cost-model boundaries via `with_limits`,
//! T = timeout. Attacks fail early by design, so every test here is cheap.

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
    // 1 MB string split into ~1M one-char parts: 32 + 1 bytes each, ~33 MB, passes.
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
        with_limits(limits(36), || evaluate("\"abcd\".upper()")),
        Ok(Value::String("ABCD".into()))
    );
    assert_eq!(
        with_limits(limits(35), || evaluate("\"abcd\".upper()"))
            .unwrap_err()
            .message,
        "Evaluation budget exceeded (max 35 bytes)"
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

// ---------------------------------------------------------------- T1: server deadline

/// T1 against `sk_http_server`. Since BL-030 the ADR's T1 formula (`:nums.map(:x.abs())`) runs
/// in milliseconds, so this uses work the budget does not count instead: every `SUM(:nums)`
/// clones the 100k-item variable (reads are not charged), ~100k × clone ≫ 10 s.
#[test]
#[ignore = "slow: server timeout (~10 s)"]
fn t1_server_times_out_and_frees_the_worker() {
    use std::io::{Read, Write};
    use std::net::{TcpListener, TcpStream};

    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let hooks = std::env::temp_dir().join("skillet-eval-budget-no-hooks");
    let mut server = std::process::Command::new(env!("CARGO_BIN_EXE_sk_http_server"))
        .args([port.to_string(), "--threads".into(), "1".into()])
        .env("SKILLET_HOOKS_DIR", hooks)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn sk_http_server");

    let post = |body: String| -> (String, Duration) {
        let start = std::time::Instant::now();
        let mut s = TcpStream::connect(("127.0.0.1", port)).expect("connect");
        s.set_read_timeout(Some(Duration::from_secs(60))).unwrap();
        let req = format!(
            "POST /eval HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        s.write_all(req.as_bytes()).unwrap();
        let mut resp = String::new();
        let _ = s.read_to_string(&mut resp);
        (resp, start.elapsed())
    };

    // Wait for the listener.
    for _ in 0..100 {
        if TcpStream::connect(("127.0.0.1", port)).is_ok() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let nums: Vec<String> = (0..100_000).map(|i| i.to_string()).collect();
    let body = format!(
        "{{\"expression\":\":nums.map(SUM(:nums)).sum()\",\"arguments\":{{\"nums\":[{}]}}}}",
        nums.join(",")
    );
    let (resp, took) = post(body);
    let quick = post("{\"expression\":\"2 + 3\"}".to_string());
    let _ = server.kill();
    let _ = server.wait();

    assert!(
        resp.starts_with("HTTP/1.1 400"),
        "{}",
        &resp[..resp.len().min(200)]
    );
    assert!(
        resp.contains("Evaluation timed out (max 10 s)"),
        "{}",
        &resp[..resp.len().min(400)]
    );
    assert!(
        took >= Duration::from_secs(10) && took < Duration::from_secs(20),
        "{took:?}"
    );
    // The single worker is free again.
    assert!(quick.0.starts_with("HTTP/1.1 200"), "{}", quick.0);
    assert!(quick.0.contains('5'), "{}", quick.0);
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

    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            add(layout.size());
            System.alloc(layout)
        }
        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            sub(layout.size());
            System.dealloc(ptr, layout)
        }
        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            add(new_size);
            sub(layout.size());
            System.realloc(ptr, layout, new_size)
        }
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
    let r = with_limits(timeout_limits(50), || {
        evaluate_with_assignments(&src, &HashMap::new())
    });
    let took = start.elapsed();
    assert_eq!(
        r.map_err(|e| e.message),
        Err("Evaluation timed out (max 50 ms)".to_string())
    );
    assert!(took < Duration::from_secs(2), "{took:?}");
}

/// F1 (blocks): a flat list of reads with no charge in between never reads the clock.
/// `:s == :s` clones `:s` twice, and binary operators are not charged, so an array literal
/// of N comparisons does 2N full clones before its single charge. Measured on the release
/// server with an 80 MB `:s` and N = 4000 (42 KB request): 20.4 s, `success: true`, past the
/// 10 s deadline; the time grows linearly with N up to the 1 MB request cap (~8 min).
#[test]
#[ignore = "open: BL-027d F1, uncharged reads never tick the deadline"]
fn bl027c_flat_uncharged_reads_meet_the_deadline() {
    let cmps = vec![":s == :s"; 3_000].join(", ");
    let src = format!(":s := {}; [{}].length()", big_string("a", 1000), cmps);
    let start = std::time::Instant::now();
    let r = with_limits(timeout_limits(50), || {
        evaluate_with_assignments(&src, &HashMap::new())
    });
    let took = start.elapsed();
    assert_eq!(
        r.map_err(|e| e.message),
        Err("Evaluation timed out (max 50 ms)".to_string())
    );
    assert!(took < Duration::from_millis(500), "{took:?}");
}

/// F2 (blocks): heap allocated before the budget check must stay within ~2× the budget.
/// Each case fails with the budget error today, but only after allocating far more than the
/// budget: k uncharged reads in one argument list / literal (CONCAT, MERGE, array and object
/// literals), and builtins that amplify their input and are charged after allocating
/// (`.to_a()` on a string ≈ 48 real bytes per char, `.to_json()` escaping ≤ 6×,
/// `::String` on an array via `Debug` escaping ≤ 5×).
#[test]
#[ignore = "open: BL-027d F2, charge-after sites allocate past 2x the budget"]
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
