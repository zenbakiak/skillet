//! Size of a result once serialized into a server response (ADR-0011 amendment 2).
//!
//! JSON escaping can make a response up to 6x larger than the value it carries (`\u0001`),
//! and serializing it happens after the evaluation budget has ended. The servers size the
//! response first, without allocating, and return the budget error instead of building a
//! response larger than the budget. Shared by `sk_http_server` and `sk_server`.

use skillet::{Value, MAX_EVAL_BYTES};
use std::fmt::Write;

/// Upper bound on the bytes `value` takes in a JSON response.
///
/// `nested_debug`: arrays nested in an array are sent as their `Debug` text in a JSON string
/// (`sk_server`'s format) instead of as JSON arrays (`sk_http_server`).
pub fn json_size(value: &Value, nested_debug: bool) -> u64 {
    size(value, nested_debug, 0)
}

/// `Err(budget error text)` when a response of `bytes` would exceed the budget.
pub fn check(bytes: u64) -> Result<(), String> {
    if bytes > MAX_EVAL_BYTES {
        Err(format!(
            "Evaluation budget exceeded (max {} MB)",
            MAX_EVAL_BYTES / (1024 * 1024)
        ))
    } else {
        Ok(())
    }
}

fn size(value: &Value, nested_debug: bool, depth: usize) -> u64 {
    match value {
        // Longest f64 / i64 texts, quoted where the servers send a string.
        Value::Number(_) | Value::Currency(_) | Value::DateTime(_) => 26,
        Value::Boolean(_) | Value::Null => 5,
        Value::String(s) => quoted(s),
        // Re-serialized after parsing: at most its text plus 32 per node (an exponent such as
        // `9E15` prints longer), or the quoted text when it does not parse.
        Value::Json(s) => quoted(s).max(s.len() as u64 + 32 * (1 + json_nodes(s))),
        Value::Array(_) if nested_debug && depth > 0 => {
            let mut c = EscapedCounter(2);
            let _ = write!(c, "{:?}", value);
            c.0
        }
        Value::Array(items) => items.iter().fold(2, |acc, v| {
            acc.saturating_add(1 + size(v, nested_debug, depth + 1))
        }),
    }
}

fn quoted(s: &str) -> u64 {
    2 + escaped_len(s.as_bytes())
}

/// Bytes that start or separate JSON nodes (same bound as the evaluator's JSON parse charge).
fn json_nodes(s: &str) -> u64 {
    s.bytes()
        .filter(|b| matches!(b, b',' | b':' | b'[' | b'{'))
        .count() as u64
}

/// Length once escaped as the body of a JSON string, as `serde_json` escapes it.
fn escaped_len(bytes: &[u8]) -> u64 {
    bytes
        .iter()
        .map(|&b| match b {
            b'"' | b'\\' | b'\n' | b'\r' | b'\t' | 0x08 | 0x0c => 2,
            0..=0x1f => 6,
            _ => 1,
        })
        .sum()
}

/// Counts the JSON-escaped length of everything written to it.
struct EscapedCounter(u64);

impl Write for EscapedCounter {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0 = self.0.saturating_add(escaped_len(s.as_bytes()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_match_serde_json_for_strings() {
        for s in ["", "abc", "a\"b\\c", "\u{1}\u{1f}\n\t", "é€😀"] {
            let want = serde_json::to_string(s).unwrap().len() as u64;
            assert_eq!(json_size(&Value::String(s.into()), false), want, "{s:?}");
        }
    }

    #[test]
    fn nested_debug_counts_the_escaped_debug_text() {
        let inner = Value::Array(vec![Value::String("\u{1}\"".into())]);
        let want = serde_json::to_string(&format!("{:?}", inner))
            .unwrap()
            .len() as u64;
        let outer = Value::Array(vec![inner]);
        assert_eq!(json_size(&outer, true), 2 + 1 + want);
    }

    #[test]
    fn check_uses_the_budget_text() {
        assert!(check(MAX_EVAL_BYTES).is_ok());
        assert_eq!(
            check(MAX_EVAL_BYTES + 1).unwrap_err(),
            "Evaluation budget exceeded (max 256 MB)"
        );
    }
}
