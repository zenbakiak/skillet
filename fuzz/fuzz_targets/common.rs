//! Shared limits for the evaluating targets.
use std::time::{Duration, Instant};

/// Small budget so one input cannot use much memory or time.
pub const MAX_BYTES: u64 = 1024 * 1024;
pub const TIMEOUT: Duration = Duration::from_millis(500);
/// How far past the deadline an evaluation may run before we call it a bug (one builtin call
/// runs to completion before the next deadline check, ADR-0011).
pub const SLACK: Duration = Duration::from_secs(2);

/// Run `f` under the fuzz limits; panic if it runs past the deadline plus slack.
pub fn limited<R>(what: &str, f: impl FnOnce() -> R) -> R {
    let mut limits = skillet::EvalLimits::default();
    limits.max_bytes = MAX_BYTES;
    limits.timeout = Some(TIMEOUT);
    let start = Instant::now();
    let r = skillet::with_limits(limits, f);
    let took = start.elapsed();
    assert!(
        took <= TIMEOUT + SLACK,
        "{what} ran {took:?}, deadline is {TIMEOUT:?} (+{SLACK:?} slack)"
    );
    r
}
