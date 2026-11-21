# 0015. NaN and infinity in statistical builtins

- Status: Proposed
- Date: 2026-11-20
- Deciders: Language Designer (proposal), Maintainer
- Related: BL-026, BL-003, ADR-0003 (`f64` numbers), `src/runtime/statistical.rs`

## Context
Skillet numbers are `f64` (ADR-0003). `0/0`, `SQRT(-1)` and `10^400 - 10^400` produce `NaN`, and
`1/0` produces `inf`. The `PERCENTILE_INC` range check `p < 0.0 || p > 1.0` lets NaN through.
BL-003 kept that behaviour behind `#[allow(clippy::manual_range_contains)]`. A survey of the module
on the release build found a worse problem: **NaN in the data panics** in three builtins. Their
sort calls `partial_cmp(..).unwrap()`, and nothing in `src/` catches the panic (`catch_unwind`).

```
=PERCENTILE_INC([1,2,3], 0/0)      -> Number(1.0)      (NaN percentile acts as 0)
=PERCENTILE_INC([1,2,3], 1/0)      -> Error: Percentile must be between 0 and 1
=PERCENTILE_INC([], 0/0)           -> Number(0.0)      (empty data is checked before the argument)
=QUARTILE_INC([1,2,3,4], 0/0)      -> Number(1.0)      (NaN as i32 == 0)
=QUARTILE_INC([1,2,3,4], -0.5)     -> Number(1.0)      (truncated to 0 before the range check)
=QUARTILE_INC([1,2,3,4], 4.9)      -> Number(4.0)
=QUARTILE_INC([1,2,3,4], 2.7)      -> Number(2.5)      (truncated to 2, as Excel does)
=MEDIAN([1, 0/0, 3])               -> thread 'main' panicked at src/runtime/statistical.rs:27:50
=PERCENTILE_INC([1, 0/0, 3], 0.5)  -> panicked at statistical.rs:161:50
=QUARTILE_INC([1, 0/0, 3], 2)      -> panicked at statistical.rs:209:50
=MEDIAN([1, 1/0, 3])               -> Number(3.0)      (infinity in data is fine)
=AVG([1, 0/0]) / VAR_P / STDEV_P   -> Number(NaN)      (IEEE propagation)
=MODE_SNGL([0/0, 0/0, 1])          -> Number(NaN)
=MAX([1, 0/0]) / MIN([0/0, 1])     -> Number(1.0)      (NaN silently ignored)
=[3, 0/0, 1].sort()                -> Array([3, NaN, 1])  (left unsorted)
```

Excel: `PERCENTILE.INC` returns `#NUM!` when k < 0 or k > 1, or when the array is empty.
`QUARTILE.INC` returns `#NUM!` when quart < 0 or quart > 4, and truncates a non-integer quart
that is in range. Excel has no NaN; an error value in the data propagates.

## Options

| Expression | Current | A: validate argument | B: A + NaN data errors (order stats) | C: NaN anywhere errors |
|---|---|---|---|---|
| `PERCENTILE_INC([1,2,3], 0/0)` | `1` | Error | Error | Error |
| `PERCENTILE_INC([], 0/0)` | `0` | Error | Error | Error |
| `QUARTILE_INC([1,2,3,4], -0.5)` | `1` | Error | Error | Error |
| `QUARTILE_INC([1,2,3,4], 4.9)` | `4` | Error | Error | Error |
| `QUARTILE_INC([1,2,3,4], 2.7)` | `2.5` | `2.5` | `2.5` | `2.5` |
| `MEDIAN([1, 0/0, 3])` | panic | panic | Error | Error |
| `AVG([1, 0/0])` | `NaN` | `NaN` | `NaN` | Error |
| `MAX([1, 0/0])` | `1` | `1` | `1` | Error |
| `MEDIAN([1, 1/0, 3])` | `3` | `3` | `3` | `3` |

- **A.** Reject a non-finite or out-of-range `p`/`quart` before anything else. Check the quart range
  before truncating, as Excel does. This alone leaves the panics in place.
- **B.** A, plus: order-based builtins (`MEDIAN`, `PERCENTILE_INC`, `QUARTILE_INC`) return
  `Error("... : NaN in data")` instead of panicking. Arithmetic builtins keep IEEE NaN propagation.
- **C.** Any NaN input to any statistical or aggregate builtin is an error. This is the most
  Excel-like, but it changes `AVG`, `VAR_P`, `STDEV_P`, `MODE_SNGL`, `MAX` and `MIN`, and it only
  makes sense together with a decision on whether `0/0` itself should be an error.

## Decision (recommended)
Adopt **B**. The panic is a crash on user input, so it gets fixed whichever option is chosen.
Implementation: `!(0.0..=1.0).contains(&p)` (which rejects NaN), then drop the `allow`. Range-check
`q` as an `f64` in `0.0..=4.0`, then truncate. Use `total_cmp` or a NaN pre-check instead of
`partial_cmp().unwrap()`. Infinity stays a legal data value. The behaviour of `0/0`, and NaN in
`AVG`/`MAX`/`MIN`/`.sort()`, is left to a follow-up ADR (C), because it is a wider question of how
numbers work.

## Compatibility
- Changed: NaN or out-of-range `p`/`quart` now error instead of returning a value. NaN data now errors
  instead of panicking. No in-range result changes.
- Tests: `tests/prd_comprehensive.rs` (L154–216) and `tests/evaluator_parity.rs` (L455–460) use only
  finite, in-range arguments, so they are unaffected. Docs (`README.md`, `docs/reference/api.md`,
  `docs/guide/user-guide.md`) show only valid calls.
- QA: add the table rows for column B as regression tests. One `MEDIAN` NaN case is a panic regression test.

## Consequences
- + No user-reachable panic in `statistical.rs`. Matches Excel `#NUM!` on bad arguments.
- − `MAX`/`MIN` ignoring NaN and `.sort()` leaving NaN unsorted stay inconsistent until the follow-up.
- The same panic exists outside this module: `=SORT([3, 0/0, 1])` panics at `src/runtime/array.rs:101`.
  The B fix should cover it too. Security follow-up: check whether a panicking evaluation kills a
  `sk_server`/`sk_http_server` worker.
