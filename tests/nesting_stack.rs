//! BL-029: nesting up to the limit fits a default-sized (2 MB) thread, debug builds included.
//!
//! `cargo test` builds without optimizations, where every local gets its own stack slot, so
//! this is the worst case an embedder meets. Each case parses and evaluates the deepest
//! accepted input of one nesting shape on a spawned 2 MB thread. A stack overflow aborts the
//! whole test binary (it can't be caught), which fails the run; that's why these live in
//! their own file. Measure stack per level with `cargo bench --bench parser_stack
//! --profile dev`.

use skillet::{evaluate, Value, MAX_NESTING_DEPTH};

const STACK: usize = 2 * 1024 * 1024;

fn on_2mb_thread(expr: String) -> Result<Value, skillet::Error> {
    std::thread::Builder::new()
        .stack_size(STACK)
        .spawn(move || evaluate(&expr))
        .expect("spawn 2 MB thread")
        .join()
        .expect("evaluation thread panicked")
}

fn assert_ok(name: &str, expr: String) {
    let result = on_2mb_thread(expr);
    assert!(result.is_ok(), "{}: {:?}", name, result);
}

const D: usize = MAX_NESTING_DEPTH;

#[test]
fn parens_depth_128_on_2mb_thread() {
    let r = on_2mb_thread(format!("{}1{}", "(".repeat(D), ")".repeat(D)));
    assert_eq!(r.unwrap(), Value::Number(1.0));
}

#[test]
fn arrays_depth_128_on_2mb_thread() {
    assert_ok("arrays", format!("{}1{}", "[".repeat(D), "]".repeat(D)));
}

#[test]
fn objects_depth_128_on_2mb_thread() {
    assert_ok("objects", format!("{}1{}", "{a:".repeat(D), "}".repeat(D)));
}

#[test]
fn unary_depth_128_on_2mb_thread() {
    assert_ok("unary minus", format!("{}1", "-".repeat(D)));
    assert_ok("unary not", format!("{}true", "!".repeat(D)));
}

#[test]
fn power_depth_128_on_2mb_thread() {
    let r = on_2mb_thread(format!("{}1", "1^".repeat(D)));
    assert_eq!(r.unwrap(), Value::Number(1.0));
}

#[test]
fn ternary_depth_128_on_2mb_thread() {
    assert_ok(
        "ternary then",
        format!("{}1{}", "true?".repeat(D), ":0".repeat(D)),
    );
    assert_ok("ternary else", format!("{}1", "false?0:".repeat(D)));
}

#[test]
fn operator_and_postfix_chains_depth_128_on_2mb_thread() {
    let r = on_2mb_thread(format!("{}1", "1+".repeat(D)));
    assert_eq!(r.unwrap(), Value::Number((D + 1) as f64));
    assert_ok("method", format!("\"x\"{}", ".upper()".repeat(D)));
}

#[test]
fn cast_depth_128_on_2mb_thread() {
    assert_ok(
        "cast",
        format!("{}1{}", "(".repeat(D), "::string)".repeat(D)),
    );
}

#[test]
fn function_call_depth_128_on_2mb_thread() {
    let r = on_2mb_thread(format!("{}1{}", "ABS(".repeat(D), ")".repeat(D)));
    assert_eq!(r.unwrap(), Value::Number(1.0));
}

#[test]
fn lambda_deepest_accepted_on_2mb_thread() {
    // `[1].map(` adds two levels for the outermost call, so 127 is the deepest accepted.
    let n = D - 1;
    assert_ok(
        "lambda",
        format!("{}:x{}", "[1].map(".repeat(n), ")".repeat(n)),
    );
}

#[test]
fn too_deep_still_rejected_on_2mb_thread() {
    let r = on_2mb_thread(format!("{}1{}", "(".repeat(D + 1), ")".repeat(D + 1)));
    let err = r.unwrap_err();
    assert!(err.message.contains("nested too deeply"), "{:?}", err);
}
