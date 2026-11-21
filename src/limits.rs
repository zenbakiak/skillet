//! Resource limits that protect the process from hostile input (BL-017, ADR-0011).
//!
//! Two limits apply to every evaluation:
//! - [`MAX_NESTING_DEPTH`] bounds expression and `Value` nesting (protects the stack);
//! - a per-evaluation byte budget ([`MAX_EVAL_BYTES`] by default) bounds the total size of the
//!   values an evaluation creates (protects the heap and meters lambda work). An optional
//!   deadline bounds wall-clock time; the servers set one, the library does not by default.
//!
//! The budget lives in a thread-local. Each public `evaluate*` entry point opens a scope if none
//! is active; nested evaluations (a custom function calling `skillet::evaluate`) join the active
//! scope, so they cannot reset the counter. Work moved to another thread inside a custom function
//! does not carry the budget with it.

use crate::error::Error;
use crate::types::Value;
use std::cell::Cell;
use std::time::{Duration, Instant};

/// Maximum nesting depth of an expression and of any `Value` built while evaluating it.
///
/// The parser counts both its own recursion (parentheses, brackets, call arguments, unary
/// operators, …) and the depth of the AST it builds (including left-deep operator chains
/// and postfix chains). The evaluator applies the same bound to arrays it constructs.
/// Exceeding it returns an `Error` instead of overflowing the stack. 128 matches
/// `serde_json`'s recursion limit, so expression text and JSON variables share one number.
pub const MAX_NESTING_DEPTH: usize = 128;

/// Default per-evaluation budget, in bytes of values created (ADR-0011).
///
/// Cost model: every `Value` costs 32 bytes, plus its byte length for `String`/`Json`, plus the
/// cost of its items for `Array`. The count is cumulative (freeing a value gives nothing back)
/// and deterministic: the same expression with the same variables always passes or always fails.
/// Exceeding it returns `Evaluation budget exceeded (max 256 MB)`.
pub const MAX_EVAL_BYTES: u64 = 256 * 1024 * 1024;

/// Size charged for every value (`size_of::<Value>()`).
const VALUE_COST: u64 = 32;
/// The deadline is checked on every `TICKS_PER_CLOCK_READ`th charge.
const TICKS_PER_CLOCK_READ: u32 = 256;
const MIB: u64 = 1024 * 1024;

/// Limits for evaluations run inside [`with_limits`].
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EvalLimits {
    /// Budget in bytes of values created (see [`MAX_EVAL_BYTES`] for the cost model).
    pub max_bytes: u64,
    /// Wall-clock limit per scope; `None` means no deadline.
    pub timeout: Option<Duration>,
}

impl Default for EvalLimits {
    fn default() -> Self {
        Self {
            max_bytes: MAX_EVAL_BYTES,
            timeout: None,
        }
    }
}

/// Run `f` with `limits` applied to every evaluation it performs on this thread.
///
/// Inside an already active scope (for example from a custom function), limits can only be
/// tightened: the budget becomes the smaller of `limits.max_bytes` and what remains of the outer
/// budget, and the deadline the earlier of the two. Charges made inside `f` still count towards
/// the outer scope.
///
/// ```
/// let mut limits = skillet::EvalLimits::default();
/// limits.max_bytes = 160;
/// let err = skillet::with_limits(limits, || skillet::evaluate("[1, 2, 3, 4, 5]")).unwrap_err();
/// assert_eq!(err.message, "Evaluation budget exceeded (max 160 bytes)");
/// ```
pub fn with_limits<R>(limits: EvalLimits, f: impl FnOnce() -> R) -> R {
    let _restore = BUDGET.with(|b| {
        let saved = b.snapshot();
        let now_used = if saved.active { b.used.get() } else { 0 };
        if !saved.active {
            b.used.set(0);
            b.ticks.set(0);
            b.tripped.set(None);
            b.max.set(u64::MAX);
            b.shown_bytes.set(u64::MAX);
            b.deadline.set(None);
        }
        let inner_max = now_used.saturating_add(limits.max_bytes);
        if inner_max < b.max.get() {
            b.max.set(inner_max);
            b.shown_bytes.set(limits.max_bytes);
        }
        if let Some(t) = limits.timeout {
            let d = Instant::now().checked_add(t);
            if let Some(d) = d {
                if !matches!(b.deadline.get(), Some(old) if old <= d) {
                    b.deadline.set(Some(d));
                    b.shown_timeout.set(t);
                }
            }
        }
        b.active.set(true);
        Restore(saved)
    });
    f()
}

/// Run one evaluation entry point: open a default scope if none is active, and return the
/// recorded limit error if one was hit, even when the evaluation itself swallowed it.
pub(crate) fn evaluation<R>(f: impl FnOnce() -> Result<R, Error>) -> Result<R, Error> {
    let active = BUDGET.with(|b| b.active.get());
    let run = || {
        let result = f();
        match BUDGET.with(|b| b.tripped.get()) {
            Some(trip) => Err(BUDGET.with(|b| b.error(trip))),
            None => result,
        }
    };
    if active {
        run()
    } else {
        with_limits(EvalLimits::default(), run)
    }
}

/// Charge `bytes` to the active budget. Never fails outside a scope (there `max` is
/// `u64::MAX` and there is no deadline; `used` is reset when the next outermost scope opens).
#[inline]
pub(crate) fn charge(bytes: u64) -> Result<(), Error> {
    let fast = BUDGET.with(|b| {
        let used = b.used.get().saturating_add(bytes);
        b.used.set(used);
        let ticks = b.ticks.get().wrapping_add(1);
        b.ticks.set(ticks);
        used <= b.max.get() && ticks % TICKS_PER_CLOCK_READ != 0
    });
    if fast {
        Ok(())
    } else {
        charge_slow()
    }
}

/// Over budget, or time to read the clock.
#[cold]
#[inline(never)]
fn charge_slow() -> Result<(), Error> {
    BUDGET.with(|b| {
        if b.used.get() > b.max.get() {
            return Err(b.trip(Trip::Budget));
        }
        match b.deadline.get() {
            Some(deadline) if Instant::now() >= deadline => Err(b.trip(Trip::Timeout)),
            _ => Ok(()),
        }
    })
}

/// True when the active scope has a deadline that has passed (for BL-018's QuickJS interrupt).
#[allow(dead_code)]
pub(crate) fn deadline_passed() -> bool {
    BUDGET.with(|b| b.active.get() && b.deadline.get().is_some_and(|d| Instant::now() >= d))
}

/// Charge the cost of an existing value (lambda item binding, assignment).
#[inline]
pub(crate) fn charge_value(value: &Value) -> Result<(), Error> {
    match value {
        Value::Array(_) => {}
        Value::String(s) | Value::Json(s) => return charge_string(s.len()),
        _ => return charge(VALUE_COST),
    }
    let Value::Array(items) = value else {
        unreachable!("scalars returned above")
    };
    let cap = remaining();
    let mut cost = VALUE_COST;
    match walk_items(items, usize::MAX, &mut cost, cap) {
        Walk::Ok => charge(cost),
        _ => charge(cost.max(cap.saturating_add(1))),
    }
}

/// Pre-charge a parse of the JSON text `s`: `len + 32 × (1 + count of , : [ {)`, a one-pass
/// upper bound on the nodes `serde_json::from_str` builds.
pub(crate) fn charge_json_parse(s: &str) -> Result<(), Error> {
    // Table lookup + sum vectorizes; `filter(matches!).count()` does not.
    let nodes = 1 + s
        .as_bytes()
        .iter()
        .map(|&c| u64::from(NODE_BYTE[c as usize]))
        .sum::<u64>();
    charge((s.len() as u64).saturating_add(VALUE_COST.saturating_mul(nodes)))
}

/// 1 for the bytes that start or separate JSON nodes: `,` `:` `[` `{`.
const NODE_BYTE: [u8; 256] = {
    let mut t = [0u8; 256];
    t[b',' as usize] = 1;
    t[b':' as usize] = 1;
    t[b'[' as usize] = 1;
    t[b'{' as usize] = 1;
    t
};

/// Pre-charge a string result of `len` bytes.
pub(crate) fn charge_string(len: usize) -> Result<(), Error> {
    charge(VALUE_COST.saturating_add(len as u64))
}

/// Pre-charge `text.replace(find, repl)` (`SUBSTITUTE`, `SUBSTITUTEM`, `.replace`) with its exact
/// output length, `len + matches × (repl.len − find.len)`; an empty `find` matches `chars + 1` times.
pub(crate) fn charge_replace(text: &str, find: &str, repl: &str) -> Result<(), Error> {
    let matches = if find.is_empty() {
        text.chars().count() + 1
    } else {
        text.matches(find).count()
    };
    let delta = repl.len() as i128 - find.len() as i128;
    let out = (text.len() as i128 + matches as i128 * delta).max(0);
    charge_string(usize::try_from(out).unwrap_or(usize::MAX))
}

/// Pre-charge `parts.join(sep)` (`JOIN`, `.join`): `Σ part.len + (n − 1) × sep.len`.
pub(crate) fn charge_join<S: AsRef<str>>(parts: &[S], sep: &str) -> Result<(), Error> {
    let total = parts
        .iter()
        .fold(0usize, |acc, p| acc.saturating_add(p.as_ref().len()));
    let seps = parts.len().saturating_sub(1).saturating_mul(sep.len());
    charge_string(total.saturating_add(seps))
}

/// Pre-charge a value about to be built from a JSON node (a `JQ` match): numbers, booleans and
/// null 32; strings 32 + len; arrays 32 + items; objects 32 + Σ (key len + value), since they
/// become a `Json` string of about that size.
pub(crate) fn charge_json_node(node: &serde_json::Value) -> Result<(), Error> {
    let cap = remaining();
    let mut cost = 0;
    if !add_json_cost(node, &mut cost, cap) {
        return charge(cost.max(cap.saturating_add(1)));
    }
    charge(cost)
}

/// A zero-byte charge: counts towards the deadline check (every 256th tick reads the clock)
/// without using budget. Every variable read and binary operation ticks (ADR-0011 amendment 2).
#[inline]
pub(crate) fn tick() -> Result<(), Error> {
    charge(0)
}

/// Pre-charge a `MERGE` result: the cost of every argument, before the merged array is built.
pub(crate) fn charge_merge(args: &[Value]) -> Result<(), Error> {
    args.iter().try_for_each(charge_value)
}

/// Pre-charge the string `write` would produce, counted with no allocation (`CONCAT`, `.to_s`,
/// `::String` on arrays). Costs one extra formatting pass.
pub(crate) fn charge_formatted(write: impl FnOnce(&mut ByteCounter)) -> Result<(), Error> {
    let mut counter = ByteCounter(0);
    write(&mut counter);
    charge_string(usize::try_from(counter.0).unwrap_or(usize::MAX))
}

/// Pre-charge the JSON text of a string (`.to_json()`): quotes plus escaped length.
pub(crate) fn charge_json_string(s: &str) -> Result<(), Error> {
    charge_string(json_escaped_len(s).saturating_add(2))
}

/// Check a value whose cost was pre-charged by the builtin that built it (`MERGE`, `CONCAT`):
/// depth only, plus a deadline tick. Charging it again would halve the budget for them.
pub(crate) fn check_prepaid(value: Value) -> Result<Value, Error> {
    tick()?;
    if let Value::Array(items) = &value {
        if let Walk::TooDeep = walk_items(items, MAX_NESTING_DEPTH - 1, &mut 0, u64::MAX) {
            return Err(nesting_error(None));
        }
    }
    Ok(value)
}

/// Length of `s` once escaped as the body of a JSON string by `serde_json` (`"` `\` and the
/// short escapes take 2 bytes, other control characters 6 as `\u00XX`), without allocating.
pub(crate) fn json_escaped_len(s: &str) -> usize {
    s.as_bytes()
        .iter()
        .map(|&c| usize::from(JSON_ESCAPE_LEN[c as usize]))
        .sum()
}

const JSON_ESCAPE_LEN: [u8; 256] = {
    let mut t = [1u8; 256];
    let mut c = 0;
    while c < 0x20 {
        t[c] = 6;
        c += 1;
    }
    t[b'\n' as usize] = 2;
    t[b'\r' as usize] = 2;
    t[b'\t' as usize] = 2;
    t[0x08] = 2;
    t[0x0c] = 2;
    t[b'"' as usize] = 2;
    t[b'\\' as usize] = 2;
    t
};

/// A `fmt::Write` / `io::Write` sink that only counts bytes, to size output before building it.
pub(crate) struct ByteCounter(pub(crate) u64);

impl std::fmt::Write for ByteCounter {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.0 = self.0.saturating_add(s.len() as u64);
        Ok(())
    }
}

impl std::io::Write for ByteCounter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0 = self.0.saturating_add(buf.len() as u64);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Check a value the evaluator just built: depth within [`MAX_NESTING_DEPTH`] and its cost
/// within the budget, in one walk that stops as soon as either limit is passed.
///
/// Called at every result site (array and object literals, function and method results,
/// property access, casts), so deep values built from a flat AST — e.g.
/// `[1, …].reduce([:acc], 0)` — fail before their recursive `Drop`/`Clone`/`Debug` can
/// overflow the stack, and doubling patterns fail before they exhaust memory.
#[inline]
pub(crate) fn check_value(value: Value) -> Result<Value, Error> {
    // Scalars and strings: no walk, one charge.
    match &value {
        Value::Array(_) => {}
        Value::String(s) | Value::Json(s) => {
            charge_string(s.len())?;
            return Ok(value);
        }
        _ => {
            charge(VALUE_COST)?;
            return Ok(value);
        }
    }
    let Value::Array(items) = &value else {
        unreachable!("scalars returned above")
    };
    let cap = remaining();
    let mut cost = VALUE_COST;
    match walk_items(items, MAX_NESTING_DEPTH - 1, &mut cost, cap) {
        Walk::Ok => charge(cost)?,
        Walk::TooDeep => return Err(nesting_error(None)),
        Walk::TooBig => charge(cost.max(cap.saturating_add(1)))?,
    }
    Ok(value)
}

/// The error returned when [`MAX_NESTING_DEPTH`] is exceeded.
pub(crate) fn nesting_error(position: Option<usize>) -> Error {
    Error::new(
        format!("Expression nested too deeply (max {})", MAX_NESTING_DEPTH),
        position,
    )
}

/// Bytes left in the active budget (`u64::MAX` outside any scope).
fn remaining() -> u64 {
    BUDGET.with(|b| {
        if b.active.get() {
            b.max.get().saturating_sub(b.used.get())
        } else {
            u64::MAX
        }
    })
}

enum Walk {
    Ok,
    TooDeep,
    TooBig,
}

/// Walk the items of an array: depth (same rule as the parser: non-arrays and empty arrays are
/// 0, an array is one more than its deepest element; `depth_left` levels remain for the items)
/// and cost (32 per item plus string bytes, nested arrays recursively). Scalars are handled
/// inline and the cap is checked once per array, so the hot flat-array case is one tight loop.
fn walk_items(items: &[Value], depth_left: usize, cost: &mut u64, cap: u64) -> Walk {
    *cost = cost.saturating_add(VALUE_COST.saturating_mul(items.len() as u64));
    for item in items {
        match item {
            Value::Array(inner) if !inner.is_empty() => {
                if depth_left == 0 {
                    return Walk::TooDeep;
                }
                match walk_items(inner, depth_left - 1, cost, cap) {
                    Walk::Ok => {}
                    stop => return stop,
                }
            }
            Value::String(s) | Value::Json(s) => *cost = cost.saturating_add(s.len() as u64),
            _ => {}
        }
    }
    if *cost > cap {
        Walk::TooBig
    } else {
        Walk::Ok
    }
}

fn add_json_cost(node: &serde_json::Value, cost: &mut u64, cap: u64) -> bool {
    *cost = cost.saturating_add(VALUE_COST);
    match node {
        serde_json::Value::String(s) => *cost = cost.saturating_add(s.len() as u64),
        serde_json::Value::Array(items) => {
            for item in items {
                if !add_json_cost(item, cost, cap) {
                    return false;
                }
            }
        }
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                *cost = cost.saturating_add(k.len() as u64);
                if !add_json_cost(v, cost, cap) {
                    return false;
                }
            }
        }
        _ => {}
    }
    *cost <= cap
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Trip {
    Budget,
    Timeout,
}

/// Per-thread budget state. `max` is absolute (comparable with `used`); `shown_*` are the
/// numbers printed in the error, i.e. the limit the innermost tightening scope asked for.
struct Budget {
    active: Cell<bool>,
    used: Cell<u64>,
    max: Cell<u64>,
    shown_bytes: Cell<u64>,
    deadline: Cell<Option<Instant>>,
    shown_timeout: Cell<Duration>,
    ticks: Cell<u32>,
    tripped: Cell<Option<Trip>>,
}

/// Saved outer state, restored when a [`with_limits`] scope ends, also on unwind. `used` and
/// `ticks` carry over to the outer scope; a trip carries over only if the outer limit is also
/// passed (an inner, tighter scope's error must not fail the outer evaluation by itself).
#[derive(Clone, Copy)]
struct Saved {
    active: bool,
    max: u64,
    shown_bytes: u64,
    deadline: Option<Instant>,
    shown_timeout: Duration,
    tripped: Option<Trip>,
}

struct Restore(Saved);

impl Drop for Restore {
    fn drop(&mut self) {
        let s = self.0;
        BUDGET.with(|b| {
            b.active.set(s.active);
            b.max.set(s.max);
            b.shown_bytes.set(s.shown_bytes);
            b.deadline.set(s.deadline);
            b.shown_timeout.set(s.shown_timeout);
            if s.active {
                let over = (b.used.get() > s.max).then_some(Trip::Budget);
                b.tripped.set(s.tripped.or(over));
            } else {
                b.used.set(0);
                b.ticks.set(0);
                b.tripped.set(None);
            }
        });
    }
}

impl Budget {
    fn snapshot(&self) -> Saved {
        Saved {
            active: self.active.get(),
            max: self.max.get(),
            shown_bytes: self.shown_bytes.get(),
            deadline: self.deadline.get(),
            shown_timeout: self.shown_timeout.get(),
            tripped: self.tripped.get(),
        }
    }

    fn trip(&self, trip: Trip) -> Error {
        if self.tripped.get().is_none() {
            self.tripped.set(Some(trip));
        }
        self.error(trip)
    }

    fn error(&self, trip: Trip) -> Error {
        match trip {
            Trip::Budget => budget_error(self.shown_bytes.get()),
            Trip::Timeout => timeout_error(self.shown_timeout.get()),
        }
    }
}

thread_local! {
    static BUDGET: Budget = const {
        Budget {
            active: Cell::new(false),
            used: Cell::new(0),
            max: Cell::new(u64::MAX),
            shown_bytes: Cell::new(u64::MAX),
            deadline: Cell::new(None),
            shown_timeout: Cell::new(Duration::ZERO),
            ticks: Cell::new(0),
            tripped: Cell::new(None),
        }
    };
}

/// `Evaluation budget exceeded (max 256 MB)`; `max N bytes` below 1 MiB or when not whole MiB.
fn budget_error(max: u64) -> Error {
    let shown = if max >= MIB && max % MIB == 0 {
        format!("{} MB", max / MIB)
    } else {
        format!("{} bytes", max)
    };
    Error::new(format!("Evaluation budget exceeded (max {})", shown), None)
}

/// `Evaluation timed out (max 10 s)`; `N ms` when not a whole number of seconds.
fn timeout_error(timeout: Duration) -> Error {
    let shown = if timeout.subsec_nanos() == 0 {
        format!("{} s", timeout.as_secs())
    } else {
        format!("{} ms", timeout.as_millis())
    };
    Error::new(format!("Evaluation timed out (max {})", shown), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nest(depth: usize) -> Value {
        let mut v = Value::Number(1.0);
        for _ in 0..depth {
            v = Value::Array(vec![v]);
        }
        v
    }

    fn limited(max_bytes: u64) -> EvalLimits {
        EvalLimits {
            max_bytes,
            timeout: None,
        }
    }

    #[test]
    fn value_depth_limit_boundary() {
        assert!(check_value(nest(MAX_NESTING_DEPTH)).is_ok());
        let err = check_value(nest(MAX_NESTING_DEPTH + 1)).expect_err("too deep");
        assert_eq!(err.message, "Expression nested too deeply (max 128)");
        assert!(check_value(Value::Array(vec![])).is_ok());
    }

    #[test]
    fn value_depth_walk_stops_early() {
        // Far deeper than the stack could recurse; the walk must stop at the limit.
        let deep = nest(5_000);
        let mut cost = 0;
        let Value::Array(items) = &deep else {
            unreachable!()
        };
        assert!(matches!(
            walk_items(items, MAX_NESTING_DEPTH - 1, &mut cost, u64::MAX),
            Walk::TooDeep
        ));
        std::mem::forget(deep); // recursive Drop of 5k levels is not what this tests
    }

    #[test]
    fn cost_model() {
        let v = Value::Array(vec![
            Value::Number(1.0),
            Value::String("abcd".into()),
            Value::Array(vec![Value::Json("{}".into())]),
        ]);
        let Value::Array(items) = &v else {
            unreachable!()
        };
        let mut cost = VALUE_COST;
        assert!(matches!(
            walk_items(items, usize::MAX, &mut cost, u64::MAX),
            Walk::Ok
        ));
        assert_eq!(cost, 32 + 32 + (32 + 4) + (32 + 32 + 2));
        assert!(matches!(
            walk_items(items, usize::MAX, &mut 0, 100),
            Walk::TooBig
        ));
    }

    #[test]
    fn charges_are_free_outside_a_scope() {
        assert!(charge(u64::MAX).is_ok());
        assert!(check_value(Value::String("x".repeat(10))).is_ok());
    }

    #[test]
    fn budget_boundary_and_sticky_trip() {
        with_limits(limited(64), || {
            assert!(charge(64).is_ok());
            let err = charge(1).expect_err("over budget");
            assert_eq!(err.message, "Evaluation budget exceeded (max 64 bytes)");
            // A swallowed error is still reported by the entry point.
            let res = evaluation(|| Ok(1));
            assert_eq!(res.unwrap_err().message, err.message);
        });
        // The scope is gone: nothing is charged and nothing is tripped.
        assert!(evaluation(|| charge(1_000)).is_ok());
    }

    #[test]
    fn nested_scope_only_tightens() {
        with_limits(limited(1000), || {
            charge(900).unwrap();
            with_limits(limited(10 * MIB), || {
                let err = charge(101).expect_err("outer still applies");
                assert_eq!(err.message, "Evaluation budget exceeded (max 1000 bytes)");
            });
        });
        with_limits(limited(1000), || {
            with_limits(limited(10), || {
                assert_eq!(
                    charge(11).unwrap_err().message,
                    "Evaluation budget exceeded (max 10 bytes)"
                );
            });
        });
    }

    #[test]
    fn inner_trip_does_not_fail_outer_scope() {
        with_limits(limited(1000), || {
            with_limits(limited(10), || assert!(charge(11).is_err()));
            assert!(evaluation(|| charge(1)).is_ok());
        });
    }

    #[test]
    fn inner_charges_count_towards_outer() {
        with_limits(limited(100), || {
            with_limits(limited(100), || charge(60).unwrap());
            assert!(charge(41).is_err());
        });
    }

    #[test]
    fn deadline_is_checked_every_256_charges() {
        let mut limits = limited(MAX_EVAL_BYTES);
        limits.timeout = Some(Duration::from_millis(1));
        with_limits(limits, || {
            std::thread::sleep(Duration::from_millis(5));
            assert!(deadline_passed());
            let mut last = Ok(());
            for _ in 0..TICKS_PER_CLOCK_READ {
                last = charge(0);
                if last.is_err() {
                    break;
                }
            }
            assert_eq!(last.unwrap_err().message, "Evaluation timed out (max 1 ms)");
        });
        assert!(!deadline_passed());
    }

    #[test]
    fn error_texts() {
        assert_eq!(
            budget_error(MAX_EVAL_BYTES).message,
            "Evaluation budget exceeded (max 256 MB)"
        );
        assert_eq!(
            budget_error(160).message,
            "Evaluation budget exceeded (max 160 bytes)"
        );
        assert_eq!(
            timeout_error(Duration::from_secs(10)).message,
            "Evaluation timed out (max 10 s)"
        );
        assert_eq!(
            timeout_error(Duration::from_millis(1500)).message,
            "Evaluation timed out (max 1500 ms)"
        );
    }
}
