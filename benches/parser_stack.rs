//! BL-029: stack used per nesting level by `parse` and by parse + eval.
//!
//! For each nesting shape (the same shapes as `tests/security_limits.rs`) this finds, by
//! binary search, the smallest thread stack on which parsing (and parse + eval) of the shape
//! at depth 32 and at the deepest accepted depth (<= `MAX_NESTING_DEPTH`) succeeds. A stack
//! overflow aborts the process, so every probe runs in a child process (this binary
//! re-executed with `--probe`). Bytes per level = (min stack at deep - min stack at 32) /
//! (deep - 32), which cancels the fixed cost of the thread and the entry point.
//!
//! Run (release): `cargo bench --bench parser_stack`
//! Run (debug):   `cargo bench --bench parser_stack --profile dev`
//! Options (env): `MODE=parse|eval|both` (default both), `SHAPE=<substring>`.

use std::process::{Command, Stdio};

const LOW: usize = 32;
const GRANULE: usize = 4 * 1024;

type Shape = (&'static str, fn(usize) -> String);

fn shapes() -> Vec<Shape> {
    vec![
        ("parens", |n| format!("{}1{}", "(".repeat(n), ")".repeat(n))),
        ("arrays", |n| format!("{}1{}", "[".repeat(n), "]".repeat(n))),
        ("objects", |n| {
            format!("{}1{}", "{a:".repeat(n), "}".repeat(n))
        }),
        ("unary_minus", |n| format!("{}1", "-".repeat(n))),
        ("unary_not", |n| format!("{}true", "!".repeat(n))),
        ("power", |n| format!("{}1", "1^".repeat(n))),
        ("ternary_then", |n| {
            format!("{}1{}", "true?".repeat(n), ":0".repeat(n))
        }),
        ("ternary_else", |n| format!("{}1", "false?0:".repeat(n))),
        ("left_add", |n| format!("{}1", "1+".repeat(n))),
        ("method", |n| format!("\"x\"{}", ".upper()".repeat(n))),
        ("index", |n| format!("[1]{}", "[0]".repeat(n))),
        ("cast", |n| {
            format!("{}1{}", "(".repeat(n), "::string)".repeat(n))
        }),
        ("func", |n| {
            format!("{}1{}", "ABS(".repeat(n), ")".repeat(n))
        }),
        ("lambda", |n| {
            format!("{}:x{}", "[1].map(".repeat(n), ")".repeat(n))
        }),
    ]
}

/// Child: run one probe on a thread with `stack` bytes; exit 0 on completion, abort on overflow.
fn probe(mode: &str, expr: String, stack: usize) {
    let mode = mode.to_string();
    let h = std::thread::Builder::new()
        .stack_size(stack)
        .spawn(move || match mode.as_str() {
            "parse" => skillet::parse(&expr).is_ok(),
            _ => skillet::evaluate(&expr).is_ok(),
        })
        .expect("spawn probe thread");
    let ok = h.join().expect("probe thread panicked");
    std::process::exit(if ok { 0 } else { 2 });
}

/// Ok(true) = completed, Ok(false) = crashed (stack overflow), Err = expression rejected.
fn run_probe(mode: &str, shape: &str, depth: usize, stack: usize) -> Result<bool, ()> {
    let status = Command::new(std::env::current_exe().expect("current exe"))
        .args([
            "--probe",
            mode,
            shape,
            &depth.to_string(),
            &stack.to_string(),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("spawn probe");
    match status.code() {
        Some(0) => Ok(true),
        Some(2) => Err(()),
        _ => Ok(false),
    }
}

/// Smallest stack (multiple of GRANULE) on which the probe completes.
fn min_stack(mode: &str, shape: &str, depth: usize) -> usize {
    let (mut lo, mut hi) = (GRANULE, 64 * 1024 * 1024);
    while hi - lo > GRANULE {
        let mid = (lo + hi) / 2 / GRANULE * GRANULE;
        if run_probe(mode, shape, depth, mid) == Ok(true) {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    hi
}

/// Deepest depth <= MAX_NESTING_DEPTH that the shape accepts (in `mode`), on a big stack.
fn deepest(mode: &str, shape: &str) -> usize {
    let mut d = skillet::MAX_NESTING_DEPTH;
    while d > LOW && run_probe(mode, shape, d, 256 * 1024 * 1024) != Ok(true) {
        d -= 1;
    }
    d
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() == 6 && args[1] == "--probe" {
        let make = shapes()
            .into_iter()
            .find(|(n, _)| *n == args[3])
            .expect("known shape")
            .1;
        probe(
            &args[2],
            make(args[4].parse().expect("depth")),
            args[5].parse().expect("stack"),
        );
        return;
    }
    let modes = match std::env::var("MODE").as_deref() {
        Ok("parse") => vec!["parse"],
        Ok("eval") => vec!["eval"],
        _ => vec!["parse", "eval"],
    };
    let filter = std::env::var("SHAPE").unwrap_or_default();
    println!(
        "BL-029 parser stack: debug_assertions={} (debug build if true)",
        cfg!(debug_assertions)
    );
    println!(
        "{:<6} {:<14} {:>5} {:>12} {:>14} {:>12}",
        "mode", "shape", "deep", "min@32 KB", "min@deep KB", "B/level"
    );
    for mode in &modes {
        for (shape, _) in shapes() {
            if !shape.contains(&filter) {
                continue;
            }
            if run_probe(mode, shape, LOW, 256 * 1024 * 1024) != Ok(true) {
                println!("{:<6} {:<14}   n/a (fails to evaluate)", mode, shape);
                continue;
            }
            let deep = deepest(mode, shape);
            let low = min_stack(mode, shape, LOW);
            let high = min_stack(mode, shape, deep);
            let per = high.saturating_sub(low) / (deep - LOW).max(1);
            println!(
                "{:<6} {:<14} {:>5} {:>12} {:>14} {:>12}",
                mode,
                shape,
                deep,
                low / 1024,
                high / 1024,
                per
            );
        }
    }
}
