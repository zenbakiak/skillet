//! BL-017: deep-nesting / stack-exhaustion regression tests.
//!
//! A Rust stack overflow is an *abort* (SIGABRT), not a panic: it cannot be caught and it
//! takes the whole process down. In `sk_http_server` one hostile request would kill the
//! server for every client. An in-process test can't pin that (the abort would kill the
//! test harness), so every hostile input here runs the compiled `sk` binary as a
//! subprocess and asserts that it exits normally and never reports a stack overflow.
//!
//! Hostile sizes are well above the crash threshold of both debug and release builds on an
//! 8 MB main thread where argv allows (measured 2026-11-20). Each argument stays under
//! ~120 KB: Linux caps a single argv string at 128 KB (MAX_ARG_STRLEN), and `sk` has no
//! file/stdin input.
//!
//! Tests marked `#[ignore = "BL-017 crash"]` abort today. BL-017b removes the `ignore`.
//! Exact error wording is decided by BL-017b and is not asserted here.
//!
//! The positive tests (`*_depth_200` / `*_depth_100`) pin that realistic nesting keeps
//! working under the `MAX_NESTING_DEPTH = 256` limit.

use std::process::{Command, Output};

/// Run `sk` with the given args. Hooks are pointed at an empty dir so JS plugin loading
/// doesn't add noise or depend on the working tree.
fn run_sk(args: &[String]) -> Output {
    let hooks = std::env::temp_dir().join("skillet-security-limits-no-hooks");
    Command::new(env!("CARGO_BIN_EXE_sk"))
        .args(args)
        .env("SKILLET_HOOKS_DIR", hooks)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("failed to spawn sk")
}

fn expr(s: String) -> Vec<String> {
    vec![format!("={}", s)]
}

fn with_json(s: String, json: String) -> Vec<String> {
    vec![format!("={}", s), "--json".to_string(), json]
}

fn combined(out: &Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// The process must exit by itself (not be killed by a signal) and must not report a
/// stack overflow.
fn assert_survives(name: &str, out: &Output) {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(
            out.status.signal(),
            None,
            "{name}: sk was killed by a signal (status {:?})",
            out.status
        );
    }
    assert!(
        out.status.code().is_some(),
        "{name}: sk did not exit normally (status {:?})",
        out.status
    );
    let text = combined(out);
    assert!(
        !text.contains("stack overflow") && !text.contains("overflowed its stack"),
        "{name}: sk reported a stack overflow"
    );
}

/// Hostile input: must survive and must report an error (any wording).
fn assert_rejected(name: &str, args: Vec<String>) {
    let out = run_sk(&args);
    assert_survives(name, &out);
    let text = combined(&out);
    assert!(
        text.contains("Error"),
        "{name}: expected an error, got: {}",
        text.chars().take(200).collect::<String>()
    );
}

/// Benign input: must survive, exit 0, and print `expected` exactly.
fn assert_value(name: &str, args: Vec<String>, expected: &str) {
    let out = run_sk(&args);
    assert_survives(name, &out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "{name}: exit {:?}, output: {}",
        out.status,
        combined(&out).chars().take(300).collect::<String>()
    );
    assert_eq!(stdout.trim(), expected, "{name}");
}

/// Benign input: must survive and exit 0; returns stdout for shape checks.
fn assert_ok_stdout(name: &str, args: Vec<String>) -> String {
    let out = run_sk(&args);
    assert_survives(name, &out);
    assert!(
        out.status.success(),
        "{name}: exit {:?}, output: {}",
        out.status,
        combined(&out).chars().take(300).collect::<String>()
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn parens(n: usize) -> String {
    format!("{}1{}", "(".repeat(n), ")".repeat(n))
}
fn arrays(n: usize) -> String {
    format!("{}1{}", "[".repeat(n), "]".repeat(n))
}
fn objects(n: usize) -> String {
    format!("{}1{}", "{a:".repeat(n), "}".repeat(n))
}
fn unary_minus(n: usize) -> String {
    format!("{}1", "-".repeat(n))
}
fn unary_not(n: usize) -> String {
    format!("{}true", "!".repeat(n))
}
fn power(n: usize) -> String {
    format!("{}1", "1^".repeat(n))
}
fn ternary_then(n: usize) -> String {
    format!("{}1{}", "true?".repeat(n), ":0".repeat(n))
}
fn ternary_else(n: usize) -> String {
    format!("{}1", "false?0:".repeat(n))
}
/// `1+1+…+1` with `n` operators: left-deep tree built iteratively by the parser, but
/// evaluated and dropped recursively.
fn left_add(n: usize) -> String {
    format!("{}1", "1+".repeat(n))
}
fn property(n: usize) -> String {
    format!(":a{}", ".b".repeat(n))
}
fn safe_nav(n: usize) -> String {
    format!(":a{}", "&.b".repeat(n))
}
fn method(n: usize) -> String {
    format!("\"x\"{}", ".upper()".repeat(n))
}
fn index_on(base: String, n: usize) -> String {
    format!("{}{}", base, "[0]".repeat(n))
}
fn cast(n: usize) -> String {
    format!("{}1{}", "(".repeat(n), "::string)".repeat(n))
}
fn func(n: usize) -> String {
    format!("{}1{}", "ABS(".repeat(n), ")".repeat(n))
}
fn lambda(n: usize) -> String {
    format!("{}:x{}", "[1].map(".repeat(n), ")".repeat(n))
}
fn json_arrays(n: usize) -> String {
    format!("{{\"a\":{}{}}}", "[".repeat(n), "]".repeat(n))
}
fn json_objects(n: usize, leaf: &str) -> String {
    format!("{{\"a\":{}{}{}}}", "{\"b\":".repeat(n), leaf, "}".repeat(n))
}
/// `[1,1,…].reduce([:acc], 0)`: tiny AST, but the *runtime Value* is `n` arrays deep.
fn reduce_wrap(n: usize) -> String {
    format!("[{}].reduce([:acc], 0)", vec!["1"; n].join(","))
}

// ---------------------------------------------------------------------------------------
// Hostile inputs. Crash thresholds (debug / release, 8 MB main thread) are in comments.
// ---------------------------------------------------------------------------------------

/// Crashes at ~250 (debug) / ~2_500 (release): parser recursion, 14 frames per level.
#[test]
#[ignore = "BL-017 crash"]
fn regression_sec_deep_parens() {
    assert_rejected("deep_parens", expr(parens(20_000)));
}

/// Crashes at ~250 / ~2_500: parser.
#[test]
#[ignore = "BL-017 crash"]
fn regression_sec_deep_arrays() {
    assert_rejected("deep_arrays", expr(arrays(10_000)));
}

/// Crashes at ~250 / ~2_500: parser.
#[test]
#[ignore = "BL-017 crash"]
fn regression_sec_deep_objects() {
    assert_rejected("deep_objects", expr(objects(10_000)));
}

/// Crashes at ~1_350 / ~11_600: evaluator (parser alone survives ~4_200 in debug).
#[test]
#[ignore = "BL-017 crash"]
fn regression_sec_unary_minus_chain() {
    assert_rejected("unary_minus_chain", expr(unary_minus(50_000)));
}

/// Crashes at ~1_350 / ~11_600: evaluator.
#[test]
#[ignore = "BL-017 crash"]
fn regression_sec_unary_not_chain() {
    assert_rejected("unary_not_chain", expr(unary_not(50_000)));
}

/// Right-assoc `1^1^…`. Crashes at ~1_350 / ~11_600: evaluator (parser ~2_700 debug).
#[test]
#[ignore = "BL-017 crash"]
fn regression_sec_power_chain() {
    assert_rejected("power_chain", expr(power(50_000)));
}

/// Crashes at ~925 / ~16_000: evaluator (parser ~2_600 debug).
#[test]
#[ignore = "BL-017 crash"]
fn regression_sec_ternary_then_chain() {
    assert_rejected("ternary_then_chain", expr(ternary_then(15_000)));
}

/// Crashes at ~925 / ~16_800: evaluator.
#[test]
#[ignore = "BL-017 crash"]
fn regression_sec_ternary_else_chain() {
    assert_rejected("ternary_else_chain", expr(ternary_else(14_000)));
}

/// 60k-term left-assoc chain (brief asked 100k; 200 KB breaks Linux MAX_ARG_STRLEN). Parser is iterative (fine at 200k); crashes at ~1_350 /
/// ~11_600 in the evaluator; `Drop` of the AST alone overflows at ~35_000 (debug).
#[test]
#[ignore = "BL-017 crash"]
fn regression_sec_left_assoc_chain() {
    assert_rejected("left_assoc_chain", expr(left_add(60_000)));
}

/// Crashes at ~1_350 / ~11_600: evaluator (iterative postfix loop in parser).
#[test]
#[ignore = "BL-017 crash"]
fn regression_sec_property_chain() {
    assert_rejected(
        "property_chain",
        with_json(property(50_000), r#"{"a":{}}"#.to_string()),
    );
}

/// Crashes at ~1_350 / ~11_600: evaluator.
#[test]
#[ignore = "BL-017 crash"]
fn regression_sec_safe_nav_chain() {
    assert_rejected(
        "safe_nav_chain",
        with_json(safe_nav(35_000), r#"{"a":null}"#.to_string()),
    );
}

/// `.upper().upper()…`. Crashes at ~1_350 / ~11_600: evaluator.
#[test]
#[ignore = "BL-017 crash"]
fn regression_sec_method_chain() {
    assert_rejected("method_chain", expr(method(12_000)));
}

/// `[1][0][0]…`. Crashes at ~1_350 / ~11_600: evaluator.
#[test]
#[ignore = "BL-017 crash"]
fn regression_sec_index_chain() {
    assert_rejected("index_chain", expr(index_on("[1]".to_string(), 35_000)));
}

/// `((1::string)::string)…` (a bare `1::a::b` is a syntax error). Crashes at ~250 / ~2_500.
#[test]
#[ignore = "BL-017 crash"]
fn regression_sec_cast_chain() {
    assert_rejected("cast_chain", expr(cast(10_000)));
}

/// `ABS(ABS(…))`. Crashes at ~250 / ~2_500: parser.
#[test]
#[ignore = "BL-017 crash"]
fn regression_sec_nested_function_calls() {
    assert_rejected("nested_function_calls", expr(func(10_000)));
}

/// `[1].map([1].map(…))`. Crashes at ~312 / ~2_500: parser.
#[test]
#[ignore = "BL-017 crash"]
fn regression_sec_nested_lambdas() {
    assert_rejected("nested_lambdas", expr(lambda(5_000)));
}

/// Deep value built at *runtime* from a flat AST: `[1,…].reduce([:acc], 0)` over 50k items.
/// Crashes between 7_000 and 10_000 items (debug) in Value drop/clone/Debug recursion.
/// A parse-time AST depth limit does NOT cover this; BL-017b needs a value-depth guard
/// (or iterative Drop) for it. Only survival is asserted (an error OR a value is fine).
#[test]
#[ignore = "BL-017 crash"]
fn regression_sec_runtime_deep_value() {
    let out = run_sk(&expr(format!("{}.length()", reduce_wrap(50_000))));
    assert_survives("runtime_deep_value", &out);
}

/// Deep JSON arrays via `--json`. Safe today: serde_json's recursion limit (128) rejects it.
#[test]
fn regression_sec_deep_json_vars_arrays() {
    assert_rejected(
        "deep_json_vars_arrays",
        with_json(":a".to_string(), json_arrays(50_000)),
    );
}

/// Deep JSON objects via `--json`. Safe today: serde_json's recursion limit (128).
#[test]
fn regression_sec_deep_json_vars_objects() {
    assert_rejected(
        "deep_json_vars_objects",
        with_json(":a".to_string(), json_objects(15_000, "1")),
    );
}

/// Deep JSON through the assignment path of `sk` (`;` / `:=` switches code path).
#[test]
fn regression_sec_deep_json_vars_assignment_path() {
    assert_rejected(
        "deep_json_vars_assignment_path",
        with_json(":z := 1; :a".to_string(), json_arrays(50_000)),
    );
}

// ---------------------------------------------------------------------------------------
// Positive tests: realistic depth keeps working (must stay green after BL-017b).
// ---------------------------------------------------------------------------------------

#[test]
fn regression_sec_parens_depth_200() {
    assert_value("parens_200", expr(parens(200)), "Number(1.0)");
}

#[test]
fn regression_sec_arrays_depth_200() {
    let out = assert_ok_stdout("arrays_200", expr(arrays(200)));
    assert_eq!(out.matches("Array(").count(), 200, "arrays_200");
    assert!(out.contains("Number(1.0)"));
}

/// Object literals round-trip through serde_json, which caps depth at 128, so 100 is the
/// deepest realistic positive case (pre-existing limit, not BL-017).
#[test]
fn regression_sec_objects_depth_100() {
    let out = assert_ok_stdout("objects_100", expr(objects(100)));
    assert!(out.starts_with("Json("), "objects_100: {out}");
}

#[test]
fn regression_sec_unary_minus_depth_200() {
    assert_value("unary_minus_200", expr(unary_minus(200)), "Number(1.0)");
}

#[test]
fn regression_sec_unary_not_depth_200() {
    assert_value("unary_not_200", expr(unary_not(200)), "Boolean(true)");
}

#[test]
fn regression_sec_power_depth_200() {
    assert_value("power_200", expr(power(200)), "Number(1.0)");
}

#[test]
fn regression_sec_ternary_then_depth_200() {
    assert_value("ternary_then_200", expr(ternary_then(200)), "Number(1.0)");
}

#[test]
fn regression_sec_ternary_else_depth_200() {
    assert_value("ternary_else_200", expr(ternary_else(200)), "Number(1.0)");
}

#[test]
fn regression_sec_left_assoc_depth_200() {
    assert_value("left_assoc_200", expr(left_add(200)), "Number(201.0)");
}

/// JSON vars are capped at depth 128 by serde_json, so the property chain uses 100.
#[test]
fn regression_sec_property_depth_100() {
    assert_value(
        "property_100",
        with_json(property(100), json_objects(100, "7")),
        "Number(7.0)",
    );
}

#[test]
fn regression_sec_safe_nav_depth_200() {
    assert_value(
        "safe_nav_200",
        with_json(safe_nav(200), r#"{"a":null}"#.to_string()),
        "Null",
    );
}

#[test]
fn regression_sec_method_depth_200() {
    assert_value("method_200", expr(method(200)), "String(\"X\")");
}

/// 100 nested arrays indexed 100 times: total AST depth 200.
#[test]
fn regression_sec_index_depth_200() {
    assert_value("index_200", expr(index_on(arrays(100), 100)), "Number(1.0)");
}

#[test]
fn regression_sec_cast_depth_200() {
    assert_value("cast_200", expr(cast(200)), "String(\"1\")");
}

#[test]
fn regression_sec_function_calls_depth_200() {
    assert_value("function_calls_200", expr(func(200)), "Number(1.0)");
}

#[test]
fn regression_sec_lambdas_depth_200() {
    let out = assert_ok_stdout("lambdas_200", expr(lambda(200)));
    assert_eq!(out.matches("Array(").count(), 200, "lambdas_200");
    assert!(out.contains("Number(1.0)"));
}

#[test]
fn regression_sec_runtime_value_depth_200() {
    assert_value(
        "runtime_value_200",
        expr(format!("{}.length()", reduce_wrap(200))),
        "Number(1.0)",
    );
}
