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
| Commit | `21a37cc` (develop after #54, #57–#64, #66) |
| Date | 2026-11-20 |
| Load | 1-min load average 4.2–5.5 during the baseline, `lambda_scope` and `sk_http_bench` runs (11 cores); 6.3–11.1 during the before/after checks below. One other agent (docs) running |

Method: `RUNS=15` samples per case, rounds interleaved (each round runs every case once), and
the bench run 6 times, alternating with `lambda_scope`. Each table cell is the median over the
6 runs of that run's median / p95 (nearest rank). Library samples are the mean of a batch sized
to ~5 ms; CLI and HTTP samples are single evaluations (HTTP: one new connection per request,
`Connection: close`).

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
| `arith` | 0.39 / 0.41 µs | 0.50 / 0.57 µs | 3.65 / 7.59 ms | 3.62 / 5.68 ms | 107 / 208 µs | 113 / 219 µs |
| `string_builtins` | 1.02 / 1.15 µs | 1.77 / 1.96 µs | 3.63 / 4.95 ms | 3.60 / 6.01 ms | 111 / 183 µs | 115 / 177 µs |
| `array_lambdas` | 1.20 / 1.28 µs | 2.05 / 2.28 µs | 3.54 / 5.37 ms | 3.55 / 5.66 ms | 108 / 182 µs | 115 / 181 µs |
| `jsonpath_jq` | 0.42 / 0.47 µs | 5.68 / 6.50 µs | 3.75 / 6.84 ms | 3.67 / 5.55 ms | 136 / 227 µs | 130 / 208 µs |
| `json_vars` | 0.83 / 0.91 µs | 2.92 / 3.37 µs | 3.67 / 5.79 ms | 3.60 / 6.03 ms | 122 / 186 µs | 127 / 199 µs |
| `json_lambda` | 1.11 / 1.25 µs | 3.80 / 4.52 µs | 3.67 / 5.19 ms | 3.67 / 6.10 ms | 131 / 191 µs | 136 / 211 µs |
| `financial` | 0.29 / 0.32 µs | 0.44 / 0.51 µs | 3.72 / 4.92 ms | 3.77 / 6.17 ms | 107 / 190 µs | 111 / 171 µs |
| `assign_sequence` | 0.84 / 0.94 µs | 1.10 / 1.17 µs | 3.53 / 4.74 ms | 3.60 / 5.22 ms | 106 / 173 µs | 115 / 187 µs |
| `method_chain` | 0.42 / 0.46 µs | 0.73 / 0.79 µs | 3.64 / 5.28 ms | 3.69 / 6.25 ms | 108 / 174 µs | 108 / 176 µs |
| `logic_if` | 0.65 / 0.70 µs | 0.95 / 1.02 µs | 3.59 / 5.22 ms | 3.79 / 5.74 ms | 124 / 226 µs | 115 / 196 µs |
| `parse_heavy` | 24.7 / 28.1 µs | 29.7 / 31.8 µs | 3.62 / 5.04 ms | 3.75 / 5.17 ms | 109 / 180 µs | 147 / 243 µs |

Reference points:
- Process-start floor (`/usr/bin/true`): 1.12 / 1.50 ms (per-run medians 0.95–1.70 ms).
- `GET /health` (`curl -w %{time_total}`, 15 requests): 0.29 ms median.
- `sk_http_bench 127.0.0.1:PORT -n 1000` (expression `10*5+sqrt(25)`), 9 runs each, cached and
  `--no-cache` alternating. Each run sweeps 1, 5, 10, 20 and 50 concurrent connections;
  median (min–max) req/s:

  | Concurrent connections | 1 | 5 | 10 | 20 | 50 |
  |---|---|---|---|---|---|
  | Cached | 14071 (10026–15625) | 34929 (25816–39412) | 36944 (28883–39849) | 36897 (30110–39617) | 35835 (34170–39056) |
  | `--no-cache` | 13519 (7555–15264) | 35942 (32950–37507) | 36774 (31374–40419) | 37331 (34582–40451) | 37068 (10088–44346) |

  Every request succeeded, including "Range operations" (fixed in #57). Throughput stops
  growing at ~5 connections: the server runs with `--threads 4`.

### `lambda_scope` (large inputs, library layer)

`RUNS=9 cargo bench --bench lambda_scope`, 6 runs interleaved with the baseline; median over the
runs of median / p95. `:nums` is 100k numbers (700 KB of JSON), `:rows` 10k objects (449 KB).

| Formula | median / p95 |
|---|---|
| `:nums.map(:x * 1).sum()` | 2.10 / 2.44 ms |
| `:nums.map(:x.abs()).sum()` | 4.52 / 5.05 ms |
| `:rows.map(:x.price).sum()` | 2.94 / 3.07 ms |
| `:rows.map(:x.price.round(1)).sum()` | 3.44 / 3.93 ms |
| `:rows.map(:x.price / :rows.length()).sum()` | 3.62 / 3.79 ms |

## Changes since the first baseline

The first baseline was taken on `32dec00` (develop `d917ce1` + benches + BL-029) at load 3.7–3.9.
Before → after, median:

| What | `32dec00` | `21a37cc` | Why |
|---|---|---|---|
| HTTP request, cached (`arith`) | 1.18 ms | 0.107 ms | #54: the accept loop blocks in `accept` instead of sleeping 1 ms when no connection is waiting |
| HTTP request, uncached (`arith`) | 1.29 ms | 0.113 ms | same |
| HTTP, all 11 cases, cached / uncached | 1.18–1.31 / 1.24–1.35 ms | 106–136 / 108–147 µs | same |
| `GET /health` (curl) | 1.08 ms | 0.29 ms | same |
| `sk_http_bench`, 1 connection | 764–779 req/s | 13519–14071 req/s (medians) | same; ~18× |
| `sk_http_bench`, 50 connections | 16090–20933 req/s | 35835–37068 req/s (medians) | same; ~2.2× |
| Library, all cases (parse + eval) | 0.46–30.2 µs | −4% … +6% per case | different sessions, not an A/B; #64's same-session A/B found −4% … +3% |
| CLI `sk` | 2.9 ms | 3.5–3.8 ms in the table; unchanged in a direct A/B | see below |
| `lambda_scope` vs `bf7c20a` (before the evaluation budget) | +5–12% (after BL-027) | −4% (`:x * 1`), +7–8% (the others) | #64 (BL-038) took back part of the BL-027 cost |

The CLI row in the table is higher than the first baseline, but so is the process-start floor
(0.92 → 1.12 ms), and the per-run medians spread from 3.1 to 4.2 ms. A direct check, `sk` built
at `32dec00` and at `21a37cc`, alternated 300 times each with `/usr/bin/true` (Python
`subprocess`, so all three include ~1.3 ms of launcher overhead; load 9.7–11.1), gave
4.81 → 4.94 ms and 4.51 → 4.56 ms in two runs: +1–3%, within noise. The CLI did not get slower;
the machine was busier.

The `lambda_scope` row is a direct check as well: the bench built at `bf7c20a` and at `21a37cc`,
6 alternating rounds of `RUNS=9`, load 6.3: 2.08 → 2.00, 4.13 → 4.42, 2.69 → 2.87,
3.11 → 3.32, 3.31 → 3.59 ms. What is left in the `:rows` cases is mostly the up-front charge for
parsing JSON text (BL-036 would remove it).

## What the numbers say

1. **Library evaluation takes 0.4–6 µs** for typical expressions, and ~30 µs for 2.6 KB of text.
   On short expressions, parsing is 40–80% of the total.
2. **The CLI takes ~3–4 ms per call no matter which expression runs.** About 1 ms of that is
   OS process start; the other ~2–2.5 ms is `sk` start-up. Loading the repo's one JS hook does
   not change it: QuickJS only starts when an expression calls a hook (BL-040). The time goes to
   loading system libraries (CoreFoundation) before `main` runs (BL-055).
3. **HTTP takes ~0.1 ms per request** (new connection each time), 10× less than the first
   baseline. Evaluation is still only 0.5–30 µs of that, so cached and uncached requests cost
   about the same; the cache shows only for text that is expensive to parse (`parse_heavy`: 109 µs cached, 147 µs uncached).
   The connection itself (connect, thread hand-off, write, close) is now the floor.
4. **`sk_http_bench -c` has no effect.** The concurrency sweep is fixed at 1, 5, 10, 20, 50
   (`run_concurrent_tests` in `src/bin/sk_http_bench.rs`); `-c` is only printed in the header.

## The "100x" claim

Earlier versions of `README.md`, `docs/README.md` and `docs/DEVELOPMENT_GUIDE.md` said "~3 ms
evaluation time (100x+ improvement from original 300 ms)". No benchmark defined the "original
300 ms", and ~3 ms is the time of one `sk` process, not of evaluation (0.4–30 µs in-process). The
claim was removed in `fa5ad50` (BL-006). A claim backed by this file reads: "evaluation is
microseconds in-process (`cargo bench --bench baseline`); one CLI call is ~3–4 ms, mostly
process start; one HTTP request is ~0.1 ms."

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

# This baseline: 6 rounds, baseline and lambda_scope alternating (check the port is free first:
#   lsof -nP -iTCP:5873 -sTCP:LISTEN):
for r in 1 2 3 4 5 6; do
  uptime; RUNS=15 PORT=5873 cargo bench --bench baseline; RUNS=9 cargo bench --bench lambda_scope
done

# HTTP throughput, with and without the expression cache (9 runs each, alternating):
SKILLET_HOOKS_DIR=$(mktemp -d) target/release/sk_http_server 5874 --threads 4 &
target/release/sk_http_bench 127.0.0.1:5874 -n 1000
target/release/sk_http_bench 127.0.0.1:5874 -n 1000 --no-cache
for i in $(seq 15); do curl -s -o /dev/null -w '%{time_total}\n' http://127.0.0.1:5874/health; done
kill %1

# Stack per nesting level (BL-029):
cargo bench --bench parser_stack                 # release
cargo bench --bench parser_stack --profile dev   # debug
cargo test --test nesting_stack                  # depth 128 on a 2 MB thread, debug build
```

Record `uptime` before and after each run; numbers from a run with a 1-minute load average above
the core count are not comparable with this table.
