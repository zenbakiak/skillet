//! Arbitrary JSON as variables into `evaluate_with_json`.
//! Input: `<expression>\n<json>`; without a newline the whole input is JSON and a fixed set of
//! expressions runs over it. Any `Err` is fine; a panic, abort or a run past the deadline is a bug.
#![no_main]

#[path = "common.rs"]
mod common;

use libfuzzer_sys::fuzz_target;

const EXPRS: &[&str] = &[
    ":a",
    ":a + 1",
    ":a.length()",
    "SUM(:a)",
    ":a&.b&.c",
    ":arguments",
    "JQ(:arguments, '$..*')",
    ":a.map(:x * 2).filter(:x > 1)",
    "CONCAT(:a, :b)",
    ":a == :b",
];

fuzz_target!(|data: &[u8]| {
    let Ok(src) = std::str::from_utf8(data) else {
        return;
    };
    match src.split_once('\n') {
        Some((expr, json)) => {
            let _ = common::limited("json_vars", || skillet::evaluate_with_json(expr, json));
        }
        None => {
            for expr in EXPRS {
                let _ = common::limited("json_vars", || skillet::evaluate_with_json(expr, src));
            }
        }
    }
});
