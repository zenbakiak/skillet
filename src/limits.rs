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
/// The limit is fixed; [`with_limits`] does not change it.
///
/// ```rust
/// assert_eq!(skillet::MAX_NESTING_DEPTH, 128);
///
/// let ok = format!("{}1{}", "(".repeat(128), ")".repeat(128));
/// assert_eq!(skillet::evaluate(&ok).unwrap(), skillet::Value::Number(1.0));
///
/// let deep = format!("{}1{}", "(".repeat(129), ")".repeat(129));
/// let err = skillet::evaluate(&deep).unwrap_err();
/// assert_eq!(err.message, "Expression nested too deeply (max 128)");
/// assert_eq!(err.position, Some(129));
/// ```
pub const MAX_NESTING_DEPTH: usize = 128;

/// Default per-evaluation budget, in bytes of values created (ADR-0011).
///
/// Cost model: every `Value` costs 32 bytes, a `String`/`Json` 48 bytes plus its byte length, and
/// an `Array` 32 bytes plus the cost of its items. The count is cumulative (freeing a value gives nothing back)
/// and deterministic: the same expression with the same variables always passes or always fails.
/// Exceeding it returns `Evaluation budget exceeded (max 256 MB)`.
///
/// ```rust
/// assert_eq!(skillet::MAX_EVAL_BYTES, 268_435_456); // 256 MiB
///
/// // Doubling an array 26 times creates more than 256 MiB of values.
/// let ones = vec!["1"; 26].join(",");
/// let err = skillet::evaluate(&format!("[{ones}].reduce([:acc, :acc], 0)")).unwrap_err();
/// assert_eq!(err.message, "Evaluation budget exceeded (max 256 MB)");
/// ```
pub const MAX_EVAL_BYTES: u64 = 256 * 1024 * 1024;

/// Size charged for every value (`size_of::<Value>()`).
const VALUE_COST: u64 = 32;
/// Size charged for a `String`/`Json` before its bytes: the `Value` plus the smallest heap block
/// (16 bytes on the system allocators measured in BL-035), so tiny strings are not undercounted.
pub(crate) const STRING_COST: u64 = VALUE_COST + 16;
/// The deadline is checked on every `TICKS_PER_CLOCK_READ`th charge.
const TICKS_PER_CLOCK_READ: u32 = 256;
const MIB: u64 = 1024 * 1024;

/// Limits for evaluations run inside [`with_limits`].
///
/// The struct is `#[non_exhaustive]`: start from [`EvalLimits::default`] (budget
/// [`MAX_EVAL_BYTES`], no deadline) and set the fields you need.
///
/// ```rust
/// use std::time::Duration;
/// use skillet::{EvalLimits, MAX_EVAL_BYTES};
///
/// let mut limits = EvalLimits::default();
/// assert_eq!(limits.max_bytes, MAX_EVAL_BYTES);
/// assert_eq!(limits.timeout, None);
///
/// limits.timeout = Some(Duration::from_secs(10)); // what the servers use
/// let r = skillet::with_limits(limits, || skillet::evaluate("SUM(1, 2, 3)"));
/// assert_eq!(r.unwrap(), skillet::Value::Number(6.0));
/// ```
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
/// All evaluations inside one call share one budget. The budget and the deadline are
/// thread-local: work that a custom function moves to another thread is not charged and is not
/// stopped by the deadline, and an evaluation started on that other thread gets a fresh default
/// scope. The deadline is checked between operations, so one long builtin call (or one custom
/// function call) runs to completion before the next check.
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
    if charge_fast(bytes) {
        Ok(())
    } else {
        charge_slow()
    }
}

/// Out of line on purpose: here `LocalKey::with` inlines, so a charge from the large `eval`
/// match is one call plus the thread-local lookup, not three nested calls.
#[inline(never)]
fn charge_fast(bytes: u64) -> bool {
    BUDGET.with(|b| b.add(bytes))
}

/// Over budget, or time to read the clock.
#[cold]
#[inline(never)]
fn charge_slow() -> Result<(), Error> {
    BUDGET.with(Budget::slow)
}

/// This thread's budget, borrowed for a loop that charges once per item (lambda methods), so
/// each item costs a few inline instructions instead of a thread-local lookup. Charges made
/// through it are the same charges, in the same order, as the free functions make.
#[derive(Clone, Copy)]
pub(crate) struct Meter<'a>(&'a Budget);

/// Run `f` with a [`Meter`] on this thread's budget.
#[inline]
pub(crate) fn with_meter<R>(f: impl FnOnce(Meter<'_>) -> R) -> R {
    BUDGET.with(|b| f(Meter(b)))
}

impl Meter<'_> {
    /// Same as [`charge_value`].
    #[inline]
    pub(crate) fn charge_value(self, value: &Value) -> Result<(), Error> {
        self.0.charge_value(value)
    }
}

/// True when the active scope has a deadline that has passed (read by the QuickJS interrupt
/// handler, ADR-0017 §3).
#[cfg_attr(not(feature = "plugins"), allow(dead_code))]
pub(crate) fn deadline_passed() -> bool {
    BUDGET.with(|b| b.active.get() && b.deadline.get().is_some_and(|d| Instant::now() >= d))
}

/// Record that code outside the evaluator (a JS plugin stopped by its interrupt handler) found
/// the deadline passed, and return the ADR-0011 timeout error. The trip is sticky like any other:
/// the evaluation entry point reports it even if a formula swallows the returned error.
#[cfg_attr(not(feature = "plugins"), allow(dead_code))]
pub(crate) fn trip_deadline() -> Error {
    BUDGET.with(|b| b.trip(Trip::Timeout))
}

/// `10 s`, or `1500 ms` when not a whole number of seconds (the form used in limit errors).
pub(crate) fn format_duration(d: Duration) -> String {
    if d.subsec_nanos() == 0 {
        format!("{} s", d.as_secs())
    } else {
        format!("{} ms", d.as_millis())
    }
}

/// Charge the cost of an existing value (lambda item binding, assignment).
#[inline]
pub(crate) fn charge_value(value: &Value) -> Result<(), Error> {
    match value {
        Value::Array(_) => {}
        Value::String(s) | Value::Json(s) => return charge_string(s.len()),
        _ => return charge(VALUE_COST),
    }
    BUDGET.with(|b| b.charge_value(value))
}

/// Pre-charge a parse of the JSON text `s`: `len + 32 × (1 + count of , : [ {)`, a one-pass
/// upper bound on the nodes `serde_json::from_str` builds.
pub(crate) fn charge_json_parse(s: &str) -> Result<(), Error> {
    let nodes = 1 + count_node_bytes(s.as_bytes());
    charge((s.len() as u64).saturating_add(VALUE_COST.saturating_mul(nodes)))
}

/// Count of the bytes in [`NODE_BYTE`]: 8 bytes per step as one `u64` (SWAR), the remaining
/// 0–7 bytes by table lookup.
fn count_node_bytes(bytes: &[u8]) -> u64 {
    let mut words = bytes.chunks_exact(8);
    let mut total = 0u64;
    for w in words.by_ref() {
        total += node_bytes_in_word(u64::from_le_bytes(w.try_into().expect("8 bytes")));
    }
    let tail = words.remainder().iter();
    total + tail.map(|&c| u64::from(NODE_BYTE[c as usize])).sum::<u64>()
}

/// Number of bytes of `w` equal to `,` `:` `[` or `{`. `[` is `{` without bit 5, so
/// `w | 0x20…` turns both into `{`. `nonzero` sets the high bit of each byte that is not 0,
/// exactly (no carries cross bytes), so a byte matches when one of the XORs leaves it 0.
#[inline(always)]
fn node_bytes_in_word(w: u64) -> u64 {
    const ONES: u64 = 0x0101_0101_0101_0101;
    const LOW7: u64 = 0x7f7f_7f7f_7f7f_7f7f;
    let nonzero = |x: u64| ((x & LOW7) + LOW7) | x;
    let comma = nonzero(w ^ (ONES * u64::from(b',')));
    let colon = nonzero(w ^ (ONES * u64::from(b':')));
    let open = nonzero((w | (ONES * 0x20)) ^ (ONES * u64::from(b'{')));
    let hits = !(comma & colon & open) & !LOW7;
    (hits >> 7).wrapping_mul(ONES) >> 56
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
    charge(STRING_COST.saturating_add(len as u64))
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
/// null 32; strings 48 + len; arrays 32 + items; objects 32 + Σ (key len + value), or 48 + their
/// serialized size when that is larger (escapes), since they become a `Json` string.
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

/// `s` upper- or lower-cased (`UPPER`, `LOWER`, `.upper()`, `.lower()`). Case mapping can make
/// non-ASCII text up to [`CASE_GROWTH`]x longer (`ΐ` is 2 bytes, `Ϊ́` 6), so the output must fit
/// what is left of the budget before it is built. When even the worst case fits (the usual case)
/// nothing is counted; otherwise the exact output size is counted, without allocating, so the
/// pass/fail boundary does not move. Nothing is charged here: the result site charges the string.
pub(crate) fn case_mapped(s: &str, upper: bool) -> Result<String, Error> {
    let worst = (s.len() as u64).saturating_mul(CASE_GROWTH);
    if !s.is_ascii() && !has_room(STRING_COST.saturating_add(worst)) {
        let len: usize = if upper {
            s.chars()
                .flat_map(char::to_uppercase)
                .map(char::len_utf8)
                .sum()
        } else {
            s.chars()
                .flat_map(char::to_lowercase)
                .map(char::len_utf8)
                .sum()
        };
        if !has_room(STRING_COST.saturating_add(len as u64)) {
            return Err(BUDGET.with(|b| b.trip(Trip::Budget)));
        }
    }
    Ok(if upper {
        s.to_uppercase()
    } else {
        s.to_lowercase()
    })
}

/// Largest growth in UTF-8 bytes of one character under `to_uppercase`/`to_lowercase` (U+0390).
const CASE_GROWTH: u64 = 3;

/// True when charging `bytes` now would stay within the budget.
fn has_room(bytes: u64) -> bool {
    BUDGET.with(|b| b.used.get().saturating_add(bytes) <= b.max.get())
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

/// Pre-charge the JSON text of `json` (object literals, `.to_json()` on arrays): the exact
/// `serde_json` output size, counted with no allocation. Escaping grows control characters 6x
/// (`\u0001`), so the size of the values alone is not an upper bound.
pub(crate) fn charge_json_text(json: &serde_json::Value) -> Result<(), Error> {
    let mut counter = ByteCounter(0);
    serde_json::to_writer(&mut counter, json)
        .map_err(|e| Error::new(format!("Failed to serialize JSON: {}", e), None))?;
    charge_string(usize::try_from(counter.0).unwrap_or(usize::MAX))
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
#[inline(always)]
pub(crate) fn check_value(value: Value) -> Result<Value, Error> {
    // Scalars and strings: no walk, one charge, decided in one out-of-line call.
    match charge_scalar(&value) {
        Some(true) => Ok(value),
        Some(false) => charge_slow().map(|()| value),
        None => check_array(value),
    }
}

/// The charge of a non-array value (32, or 48 plus the length of a string): `Some(true)` when done,
/// `Some(false)` when charged but the slow path must run, `None` for an array (not charged).
#[inline(never)]
fn charge_scalar(value: &Value) -> Option<bool> {
    let bytes = match value {
        Value::Array(_) => return None,
        Value::String(s) | Value::Json(s) => STRING_COST.saturating_add(s.len() as u64),
        _ => VALUE_COST,
    };
    Some(BUDGET.with(|b| b.add(bytes)))
}

#[inline(never)]
fn check_array(value: Value) -> Result<Value, Error> {
    let Value::Array(items) = &value else {
        unreachable!("only arrays reach check_array")
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
    BUDGET.with(Budget::remaining)
}

enum Walk {
    Ok,
    TooDeep,
    TooBig,
}

/// Walk the items of an array: depth (same rule as the parser: non-arrays and empty arrays are
/// 0, an array is one more than its deepest element; `depth_left` levels remain for the items)
/// and cost (32 per item, 48 plus their bytes for strings, nested arrays recursively). Scalars
/// are handled inline and the cap is checked once per array, so the hot flat-array case is one
/// tight loop.
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
            Value::String(s) | Value::Json(s) => {
                *cost = cost.saturating_add(STRING_COST - VALUE_COST + s.len() as u64)
            }
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
    add_json_node_cost(node, cost, cap, false)
}

/// `in_object`: the node is part of an object that is already charged as JSON text.
fn add_json_node_cost(node: &serde_json::Value, cost: &mut u64, cap: u64, in_object: bool) -> bool {
    let start = *cost;
    *cost = cost.saturating_add(VALUE_COST);
    match node {
        serde_json::Value::String(s) => {
            *cost = cost.saturating_add(STRING_COST - VALUE_COST + s.len() as u64)
        }
        serde_json::Value::Array(items) => {
            for item in items {
                if !add_json_node_cost(item, cost, cap, in_object) {
                    return false;
                }
            }
        }
        serde_json::Value::Object(map) => {
            for (k, v) in map {
                *cost = cost.saturating_add(k.len() as u64);
                if !add_json_node_cost(v, cost, cap, true) {
                    return false;
                }
            }
            // An object becomes `Json` text, and escaping can make that text larger than its
            // parts (6x for control characters): charge at least its exact serialized size.
            if !in_object {
                let mut text = ByteCounter(0);
                if serde_json::to_writer(&mut text, node).is_ok() {
                    let as_text = start.saturating_add(STRING_COST).saturating_add(text.0);
                    *cost = (*cost).max(as_text);
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
    /// The fast path of every charge: count `bytes` and the tick; false when the slow path
    /// must run (over budget, or every [`TICKS_PER_CLOCK_READ`]th tick, to read the clock).
    #[inline(always)]
    fn add(&self, bytes: u64) -> bool {
        let used = self.used.get().saturating_add(bytes);
        self.used.set(used);
        let ticks = self.ticks.get().wrapping_add(1);
        self.ticks.set(ticks);
        used <= self.max.get() && ticks % TICKS_PER_CLOCK_READ != 0
    }

    #[inline(always)]
    fn charge(&self, bytes: u64) -> Result<(), Error> {
        if self.add(bytes) {
            Ok(())
        } else {
            self.slow()
        }
    }

    #[cold]
    #[inline(never)]
    fn slow(&self) -> Result<(), Error> {
        if self.used.get() > self.max.get() {
            return Err(self.trip(Trip::Budget));
        }
        match self.deadline.get() {
            Some(deadline) if Instant::now() >= deadline => Err(self.trip(Trip::Timeout)),
            _ => Ok(()),
        }
    }

    fn remaining(&self) -> u64 {
        if self.active.get() {
            self.max.get().saturating_sub(self.used.get())
        } else {
            u64::MAX
        }
    }

    /// See [`charge_value`]: scalars inline, arrays walked out of line.
    #[inline(always)]
    fn charge_value(&self, value: &Value) -> Result<(), Error> {
        match value {
            Value::Array(items) => self.charge_items(items),
            Value::String(s) | Value::Json(s) => {
                self.charge(STRING_COST.saturating_add(s.len() as u64))
            }
            _ => self.charge(VALUE_COST),
        }
    }

    #[inline(never)]
    fn charge_items(&self, items: &[Value]) -> Result<(), Error> {
        let cap = self.remaining();
        let mut cost = VALUE_COST;
        match walk_items(items, usize::MAX, &mut cost, cap) {
            Walk::Ok => self.charge(cost),
            _ => self.charge(cost.max(cap.saturating_add(1))),
        }
    }

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
    Error::new(
        format!("Evaluation timed out (max {})", format_duration(timeout)),
        None,
    )
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
        assert_eq!(cost, 32 + 32 + (48 + 4) + (32 + 48 + 2));
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
    fn node_byte_count_matches_the_byte_set() {
        let slow = |b: &[u8]| {
            b.iter()
                .filter(|&&c| matches!(c, b',' | b':' | b'[' | b'{'))
                .count() as u64
        };
        let every_byte: Vec<u8> = (0..=255u8).cycle().take(256 * 3 + 7).collect();
        let row = br#"{"a":[1,{"b":"c"}],"d":2}"#.repeat(3);
        for len in 0..row.len() {
            assert_eq!(
                count_node_bytes(&row[..len]),
                slow(&row[..len]),
                "len {len}"
            );
        }
        assert_eq!(count_node_bytes(&every_byte), slow(&every_byte));
        assert_eq!(count_node_bytes(&[b'{'; 16 * 600 + 9]), 16 * 600 + 9);
    }

    #[test]
    fn case_growth_bounds_every_char() {
        for c in (0..=0x10_FFFF).filter_map(char::from_u32) {
            let grown = |it: &mut dyn Iterator<Item = char>| it.map(char::len_utf8).sum::<usize>();
            let max = c.len_utf8() as u64 * CASE_GROWTH;
            assert!(grown(&mut c.to_uppercase()) as u64 <= max, "{c:?}");
            assert!(grown(&mut c.to_lowercase()) as u64 <= max, "{c:?}");
        }
    }

    #[test]
    fn case_mapping_checks_room_exactly() {
        // "ΐ" upper-cases to 6 bytes: 48 + 6 fits a 54-byte budget, not a 53-byte one.
        with_limits(limited(54), || assert!(case_mapped("ΐ", true).is_ok()));
        with_limits(limited(53), || {
            let err = case_mapped("ΐ", true).expect_err("no room");
            assert_eq!(err.message, "Evaluation budget exceeded (max 53 bytes)");
        });
        // Nothing is charged: the result site does that.
        with_limits(limited(54), || {
            case_mapped("ΐ", true).unwrap();
            assert!(charge(54).is_ok());
        });
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
