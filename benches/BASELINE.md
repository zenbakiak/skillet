# Performance baseline (BL-005)

Numbers for the fixed expression set in [`baseline.rs`](baseline.rs), on three layers measured
separately (library, CLI process, HTTP). All numbers come from the commands under
[Reproduce](#reproduce); nothing here is estimated.

## Environment

| | |
|---|---|
| Machine | Apple M3 Pro, 11 cores, 36 GB RAM |
| OS | macOS 27.0.1 (26A434) |
| Toolchain | rustc 1.89.0 / cargo 1.89.0, `bench` profile (= release) |
| Commit | `32dec00` (`feature/lane3-perf-baseline`: develop `d917ce1` + benches + BL-029 parser change) |
| Date | 2026-11-20 |
| Load | Shared machine, other builds/tests running. 1-min load average 3.7–3.9 during the baseline run, 7.3–8.6 during the BL-029 before/after run |

Method: `RUNS=15` samples per case, rounds interleaved (each round runs every case once), and
the bench run 3 times. Each table cell is the median over the 3 runs of that run's median / p95
(nearest rank). Library samples are the mean of a batch sized to ~5 ms; CLI and HTTP samples are
single evaluations.

## The set

| Case | Expression | Variables |
|---|---|---|
| `arith` | `=2 + 3 * 4 - (10 / 5) ^ 2 % 3` | — |
| `string_builtins` | `=CONCAT(UPPER("hello"), " ", LOWER("WORLD"), " ", TRIM("  skillet  "), LENGTH("abc"))` | — |
| `array_lambdas` | `=[1,2,3,4,5,6,7,8,9,10].map(:x * 2).filter(:x > 10).sum()` | — |
| `jsonpath_jq` | `=JQ(:arguments, "$.items[*].price").sum()` | doc |
| `json_vars` | `=:user.profile.age + :items.length() + :user&.profile&.tags.length()` | doc |
| `json_lambda` | `=:items.map(:x.price * :x.qty).filter(:x > 10).sum().round(2)` | doc |
| `financial` | `=PMT(0.05/12, 360, 200000)` | — |
| `assign_sequence` | `=:a := 10; :b := :a * 2; :c := :b + :a; :c * 2` | — |
| `method_chain` | `="  Hello World  ".trim().upper().reverse()` | — |
| `logic_if` | `=IF(AND(:x > 5, :y < 10), "yes", "no")` | doc |
| `parse_heavy` | `=SUM((1 * 2 + 1 - 1), …)`: 100 arguments, 2.6 KB of text | — |

"doc" is a 330-byte JSON object (`user`, 4 `items`, `x`, `y`), defined as `DOC` in `baseline.rs`.

## Results

Times are median / p95. µs = microseconds, ms = milliseconds.

| Case | lib `parse` | lib parse + eval | CLI `sk` | CLI `sk` + repo `hooks/` | HTTP cached | HTTP uncached |
|---|---|---|---|---|---|---|
| `arith` | 0.38 / 0.38 µs | 0.51 / 0.53 µs | 2.97 / 3.46 ms | 2.89 / 3.20 ms | 1.18 / 1.25 ms | 1.29 / 1.34 ms |
| `string_builtins` | 0.95 / 0.98 µs | 1.79 / 1.89 µs | 2.91 / 4.16 ms | 2.88 / 3.53 ms | 1.27 / 1.43 ms | 1.30 / 1.38 ms |
| `array_lambdas` | 1.13 / 1.18 µs | 2.00 / 2.13 µs | 2.90 / 3.51 ms | 2.97 / 3.37 ms | 1.26 / 1.38 ms | 1.29 / 1.34 ms |
| `jsonpath_jq` | 0.40 / 0.42 µs | 5.36 / 5.60 µs | 2.96 / 3.60 ms | 2.94 / 3.23 ms | 1.31 / 1.53 ms | 1.24 / 1.34 ms |
| `json_vars` | 0.78 / 0.83 µs | 2.80 / 2.92 µs | 2.88 / 3.85 ms | 2.90 / 3.57 ms | 1.25 / 1.60 ms | 1.27 / 1.32 ms |
| `json_lambda` | 1.02 / 1.08 µs | 3.60 / 3.82 µs | 2.89 / 3.84 ms | 2.94 / 3.70 ms | 1.27 / 1.36 ms | 1.28 / 1.39 ms |
| `financial` | 0.27 / 0.29 µs | 0.46 / 0.50 µs | 2.90 / 3.87 ms | 2.90 / 3.68 ms | 1.23 / 1.30 ms | 1.29 / 1.44 ms |
| `assign_sequence` | 0.80 / 0.83 µs | 1.04 / 1.16 µs | 2.92 / 3.61 ms | 2.98 / 3.72 ms | 1.25 / 1.38 ms | 1.29 / 1.35 ms |
| `method_chain` | 0.39 / 0.41 µs | 0.69 / 0.73 µs | 2.91 / 3.17 ms | 2.92 / 3.53 ms | 1.26 / 1.37 ms | 1.29 / 1.36 ms |
| `logic_if` | 0.62 / 0.64 µs | 0.93 / 1.03 µs | 2.89 / 3.50 ms | 2.93 / 3.29 ms | 1.29 / 1.38 ms | 1.26 / 1.46 ms |
| `parse_heavy` | 23.6 / 24.5 µs | 30.2 / 31.1 µs | 2.97 / 3.51 ms | 2.94 / 3.43 ms | 1.25 / 1.34 ms | 1.35 / 1.43 ms |

Reference points:
- Process-start floor (`/usr/bin/true`): 0.92 / 1.24 ms.
- `GET /health` (`curl -w %{time_total}`, 15 requests): 1.08 ms median, so HTTP has the same
  ~1.1–1.3 ms floor without evaluating anything.
- `sk_http_bench 127.0.0.1:PORT -n 1000` (expression `10*5+sqrt(25)`), 2 runs each:

  | Concurrent connections | 1 | 5 | 10 | 20 | 50 |
  |---|---|---|---|---|---|
  | Cached, req/s | 770–777 | 3710–3875 | 6312–6776 | 8055–13464 | 16090–17332 |
  | `--no-cache`, req/s | 764–779 | 3658–3750 | 5884–7065 | 8511–13739 | 16620–20933 |

  The cached/uncached ranges overlap: under this load, the cache makes no measurable difference
  for this expression.

## What the numbers say

1. **Library evaluation takes 0.5–6 µs** for typical expressions, and ~30 µs for 2.6 KB of text.
   On short expressions, parsing is 40–75% of the total.
2. **The CLI takes ~2.9 ms per call no matter which expression runs.** About 0.9 ms of that is
   OS process start; the other ~2 ms is `sk` start-up. Loading the repo's one JS hook does not
   change it measurably, so the JS plugin file itself is not the cost. Not profiled yet;
   QuickJS runtime creation is the main suspect.
3. **HTTP takes ~1.25 ms per request no matter which expression runs, cached or not.** `/health`
   costs the same, so the time goes to the server's accept loop, not to evaluation: the listener
   is non-blocking and sleeps 1 ms whenever `accept` would block (`src/bin/sk_http_server.rs`,
   accept loop). With evaluation at 1–30 µs, the expression cache cannot show a benefit until
   that floor is gone.
4. **One `sk_http_bench` case is broken**: "Range operations" (`range(1,50).sum()`) fails on
   every request, because there is no `RANGE` function.

## The "100x" claim

`README.md`, `docs/README.md` and `docs/DEVELOPMENT_GUIDE.md` say "~3 ms evaluation time (100x+
improvement from original 300 ms)". **It can't be reproduced as written:**
- No benchmark, commit or script in the repo defines the "original 300 ms", so the 100x ratio has
  nothing to be checked against.
- "~3 ms" is not evaluation time. In-process evaluation of this set is 0.5–30 µs, about 100–6000×
  less than 3 ms. ~3 ms is what one `sk` **process** takes (2.9 ms here), and almost all of it
  is process start.

A claim backed by this file would read: "evaluation is microseconds in-process
(`cargo bench --bench baseline`); one CLI call is ~3 ms, mostly process start."

## BL-029: parser stack and parse speed

The parser change in `32dec00` folds the six binary precedence levels into one
precedence-climbing loop, merges postfix/cast/power into one function, and moves branches a
nesting level does not always take into `#[inline(never)]` helpers.

Stack per nesting level, from `benches/parser_stack.rs` (`eval` mode = parse + eval, bytes). For
the shapes the change targets, the parser's peak is higher than the evaluator's, so these rows
equal the parse-only numbers. For unary and ternary, the evaluator's peak is the higher one, and
the evaluator did not change:

| Shape | debug before | debug after | release before | release after |
|---|---|---|---|---|
| `((((1))))` parens / cast | 24064 | 5290 | 3754 | 1536 |
| `[[[1]]]` arrays | 25429 | 5802 | 3754 | 1536 |
| `{a:{a:1}}` objects | 25941 | 6485 | 3754 | 1536 |
| `ABS(ABS(1))` function calls | 25941 | 6826 | 4266 | 1706 |
| `[1].map([1].map(:x))` lambdas | 23799 | 10520 | 4311 | 2414 |
| `1^1^1` power | 3413 | 4096 | 469 | 810 |
| `true?…` / `false?0:…` ternary | 5973 | 5973 | 1024 | 640 |
| `---1` unary | 3413 | 3413 | 469 | 469 |

Smallest stack for the deepest accepted input (128, or 127 for lambdas), worst shape: debug
3.25 MB before (objects, function calls) → 1.30 MB after (lambdas); release 532 KB → 292 KB. `tests/nesting_stack.rs` parses and
evaluates every shape at its deepest accepted depth on a spawned 2 MB thread in the debug test
build; before the change it aborts with a stack overflow. Power chains use more stack than before
(+0.7 KB/level in debug, +0.3 KB in release): each level now goes through `parse_power_rest`.
That is 516 KB at depth 128 in debug, well inside 2 MB.

Parse speed, library layer, before (`d917ce1` parser) vs after (`32dec00`). 6 interleaved
before/after pairs of `RUNS=15 LAYERS=lib`; median of the per-run medians, load 7.3–8.6:

| Case | parse before → after (µs) | Δ | parse + eval before → after (µs) | Δ |
|---|---|---|---|---|
| `arith` | 0.45 → 0.38 | −15% | 0.59 → 0.52 | −13% |
| `string_builtins` | 1.46 → 0.98 | −33% | 2.32 → 1.79 | −23% |
| `array_lambdas` | 1.62 → 1.17 | −28% | 2.52 → 2.06 | −18% |
| `jsonpath_jq` | 0.53 → 0.41 | −22% | 5.74 → 5.54 | −4% |
| `json_vars` | 0.83 → 0.80 | −4% | 2.92 → 2.92 | 0% |
| `json_lambda` | 1.21 → 1.08 | −12% | 3.95 → 3.73 | −6% |
| `financial` | 0.43 → 0.28 | −35% | 0.59 → 0.45 | −24% |
| `assign_sequence` | 0.96 → 0.81 | −16% | 1.21 → 1.06 | −12% |
| `method_chain` | 0.43 → 0.40 | −8% | 0.76 → 0.70 | −7% |
| `logic_if` | 0.90 → 0.64 | −29% | 1.21 → 0.94 | −22% |
| `parse_heavy` | 31.4 → 23.8 | −24% | 38.4 → 30.3 | −21% |

No case got slower. Results are identical: every case prints the same result before and after,
and a differential run over 264k generated expressions printed identical `parse` output (AST or
error message and position).

## Reproduce

```bash
cargo build --release --bin sk --bin sk_http_server --bin sk_http_bench

# All three layers (starts its own sk_http_server on 127.0.0.1:$PORT, default 5099):
RUNS=15 cargo bench --bench baseline
# One layer / some cases:
RUNS=15 LAYERS=lib cargo bench --bench baseline
LAYERS=cli,http CASE=json cargo bench --bench baseline

# Before/after a change, same machine, interleaved: build the bench on each commit, copy
#   target/release/deps/baseline-<hash> aside, then alternate the two binaries:
for r in 1 2 3 4 5 6; do RUNS=15 LAYERS=lib ./baseline_before; RUNS=15 LAYERS=lib ./baseline_after; done

# HTTP throughput, with and without the expression cache:
SKILLET_HOOKS_DIR=$(mktemp -d) target/release/sk_http_server 5872 --threads 4 &
target/release/sk_http_bench 127.0.0.1:5872 -n 1000
target/release/sk_http_bench 127.0.0.1:5872 -n 1000 --no-cache
kill %1

# Stack per nesting level (BL-029):
cargo bench --bench parser_stack                 # release
cargo bench --bench parser_stack --profile dev   # debug
cargo test --test nesting_stack                  # depth 128 on a 2 MB thread, debug build
```

Record `uptime` before and after each run; numbers from a run with a 1-minute load average above
the core count are not comparable with this table.
