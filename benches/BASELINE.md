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
| Commit | `1c49f34` (develop after #138; the 0.7.0 code) |
| Date | 2026-11-22 |
| Load | 1-min load average 4.4–5.0 during the baseline and `lambda_scope` runs, 4.1–4.3 during `sk_http_bench` (11 cores); 5.4–16 during the before/after checks below (other agents running) |

Method: `RUNS=15` samples per case, rounds interleaved (each round runs every case once), and
the bench run 6 times, alternating with `RUNS=9` of `lambda_scope`. Each table cell is the
median over the 6 runs of that run's median / p95 (nearest rank). Library samples are the mean
of a batch sized to ~5 ms; CLI and HTTP samples are single evaluations (HTTP: one new connection
per request, `Connection: close`). Every case printed the same result as at `21a37cc`.

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
| `parse_heavy` | `=SUM((1 * 2 + 1 - 1), …)`: 100 arguments, 1.8 KB of text (1,796 bytes) | — |

"doc" is a 330-byte JSON object (`user`, 4 `items`, `x`, `y`), defined as `DOC` in `baseline.rs`.

## Results

Times are median / p95. µs = microseconds, ms = milliseconds.

| Case | lib `parse` | lib parse + eval | CLI `sk` | CLI `sk` + repo `hooks/` | HTTP cached | HTTP uncached |
|---|---|---|---|---|---|---|
| `arith` | 0.41 / 0.45 µs | 0.54 / 0.59 µs | 1.94 / 2.29 ms | 1.92 / 2.41 ms | 91 / 245 µs | 93 / 231 µs |
| `string_builtins` | 0.96 / 1.05 µs | 1.63 / 1.78 µs | 1.94 / 2.35 ms | 1.94 / 2.51 ms | 96 / 230 µs | 108 / 318 µs |
| `array_lambdas` | 1.18 / 1.33 µs | 2.04 / 2.28 µs | 1.93 / 2.37 ms | 1.95 / 2.33 ms | 93 / 220 µs | 101 / 245 µs |
| `jsonpath_jq` | 0.40 / 0.44 µs | 5.45 / 5.87 µs | 1.99 / 2.46 ms | 1.99 / 2.42 ms | 115 / 254 µs | 118 / 272 µs |
| `json_vars` | 0.77 / 0.84 µs | 2.79 / 3.00 µs | 1.93 / 2.36 ms | 1.97 / 2.41 ms | 112 / 251 µs | 104 / 248 µs |
| `json_lambda` | 1.02 / 1.09 µs | 3.59 / 3.99 µs | 1.92 / 2.29 ms | 1.96 / 2.53 ms | 106 / 274 µs | 103 / 280 µs |
| `financial` | 0.30 / 0.32 µs | 0.42 / 0.45 µs | 1.92 / 2.51 ms | 1.96 / 2.46 ms | 92 / 248 µs | 95 / 223 µs |
| `assign_sequence` | 0.80 / 0.87 µs | 1.08 / 1.14 µs | 1.89 / 2.46 ms | 1.94 / 2.25 ms | 94 / 209 µs | 92 / 235 µs |
| `method_chain` | 0.38 / 0.40 µs | 0.70 / 0.75 µs | 1.89 / 2.32 ms | 1.91 / 2.21 ms | 87 / 244 µs | 89 / 229 µs |
| `logic_if` | 0.63 / 0.69 µs | 0.85 / 0.93 µs | 1.93 / 2.14 ms | 1.97 / 2.16 ms | 101 / 231 µs | 100 / 242 µs |
| `parse_heavy` | 24.6 / 26.8 µs | 29.4 / 31.6 µs | 1.94 / 2.49 ms | 1.98 / 2.46 ms | 90 / 220 µs | 99 / 282 µs |

Reference points:
- Process-start floor (`/usr/bin/true`): 0.91 / 1.12 ms (per-run medians 0.89–1.28 ms).
- `GET /health` (`curl -w %{time_total}`, 15 requests): 0.32 ms median.
- `sk_http_bench 127.0.0.1:PORT -n 1000` (expression `10*5+sqrt(25)`), 9 runs each, cached and
  `--no-cache` alternating. Each run sweeps 1, 5, 10, 20 and 50 concurrent connections;
  median (min–max) req/s:

  | Concurrent connections | 1 | 5 | 10 | 20 | 50 |
  |---|---|---|---|---|---|
  | Cached | 11227 (5785–13304) | 24979 (23204–26826) | 25716 (23152–28387) | 25409 (22617–26878) | 24317 (11018–27238) |
  | `--no-cache` | 10644 (7815–14045) | 24882 (11133–26664) | 25238 (21555–26819) | 25304 (23800–25916) | 25249 (23021–27055) |

  Every request succeeded. Throughput stops growing at ~5 connections: the server runs with
  `--threads 4`. These req/s are ~30% below the `21a37cc` table (14071 / ~36900), but the
  `21a37cc` server measured in the same session gives the same ~26k (see
  [Since `21a37cc`](#since-21a37cc)): the machine is slower today, the server is not.

### `lambda_scope` (large inputs, library layer)

`RUNS=9 cargo bench --bench lambda_scope`, 6 runs interleaved with the baseline; median over the
runs of median / p95. `:nums` is 100k numbers (700 KB of JSON), `:rows` 10k objects (449 KB).

| Formula | median / p95 |
|---|---|
| `:nums.map(:x * 1).sum()` | 2.18 / 2.31 ms |
| `:nums.map(:x.abs()).sum()` | 4.56 / 4.90 ms |
| `:rows.map(:x.price).sum()` | 2.99 / 3.27 ms |
| `:rows.map(:x.price.round(1)).sum()` | 3.30 / 3.40 ms |
| `:rows.map(:x.price / :rows.length()).sum()` | 3.53 / 3.69 ms |

## 0.6.3 → 0.7.0

The last published crate, `crate-v0.6.3` (`aebdb34`), built in a temporary worktree with this
commit's `benches/baseline.rs` and `benches/lambda_scope.rs` copied in (both use only API that
0.6.3 already had). Three builds of the two benches (0.6.3, `21a37cc`, `1c49f34`) alternated in
6 rounds: each round runs `RUNS=15 LAYERS=lib` of the baseline and `RUNS=9` of `lambda_scope`
for each build in turn. Load 5.9–16 (other agents running), so compare the columns with each
other, not with the table above. Cells are the median over the 6 runs of the per-run median.
Every case printed the same result in all three builds.

| Case | parse 0.6.3 → 0.7.0 (µs) | Δ | parse + eval 0.6.3 → 0.7.0 (µs) | Δ |
|---|---|---|---|---|
| `arith` | 0.43 → 0.42 | −2% | 0.53 → 0.54 | +3% |
| `string_builtins` | 1.36 → 0.97 | −29% | 2.17 → 1.64 | −24% |
| `array_lambdas` | 1.58 → 1.17 | −26% | 3.11 → 2.03 | −35% |
| `jsonpath_jq` | 0.49 → 0.40 | −18% | 5.88 → 5.39 | −8% |
| `json_vars` | 0.82 → 0.77 | −7% | 3.47 → 2.78 | −20% |
| `json_lambda` | 1.16 → 1.01 | −13% | 5.88 → 3.72 | −37% |
| `financial` | 0.41 → 0.29 | −29% | 0.57 → 0.43 | −25% |
| `assign_sequence` | 0.95 → 0.82 | −14% | 1.17 → 1.08 | −8% |
| `method_chain` | 0.44 → 0.39 | −11% | 0.72 → 0.70 | −3% |
| `logic_if` | 0.84 → 0.63 | −25% | 1.14 → 0.86 | −25% |
| `parse_heavy` | 30.0 → 24.8 | −17% | 34.9 → 29.5 | −15% |

| `lambda_scope` formula | 0.6.3 → 0.7.0 (ms) | Δ |
|---|---|---|
| `:nums.map(:x * 1).sum()` | 5.77 → 2.15 | −63% |
| `:nums.map(:x.abs()).sum()` | 7.95 → 4.56 | −43% |
| `:rows.map(:x.price).sum()` | 4.36 → 2.78 | −36% |
| `:rows.map(:x.price.round(1)).sum()` | 4.73 → 3.38 | −29% |
| `:rows.map(:x.price / :rows.length()).sum()` | 2151 → 3.61 | ~600× faster |

The last row is quadratic in 0.6.3: each lambda call copied the whole scope, outer `:rows`
(10k objects) included; BL-030 (`bf7c20a`, ADR-0011) made lambdas share the parent scope. `arith` is the
only case not faster: 0.6.3 has neither the BL-029 parser speed-up nor the Excel precedence
levels (#82), and the two roughly cancel.

CLI, same session (`python3 -I benches/cli_startup.py /usr/bin/true sk_063 sk_21a sk_070`, 5
rounds × 300 calls, empty directory, load 9–11; includes Python launcher overhead): `/usr/bin/true`
1.99 ms, 0.6.3 4.23 ms, `21a37cc` 4.20 ms, 0.7.0 3.07 ms (round medians 2.83–3.24). Above the
floor: 2.24 → 1.09 ms, −1.15 ms per call, from BL-055 (#81, no CoreFoundation). 0.6.3's HTTP
server was not measured: its accept loop slept 1 ms per idle poll (fixed in #54), so it is
~10× slower per request for a reason unrelated to evaluation.

## Since `21a37cc`

Same sessions as above, `21a37cc` vs `1c49f34`. Library and `lambda_scope`: the three-build run
in [0.6.3 → 0.7.0](#063--070). CLI and HTTP: 6 rounds of `RUNS=15 LAYERS=cli,http`, the two bench
binaries alternating (each starts its own build's `sk_http_server`), load 5.4–6.2.
`sk_http_bench`: 9 rounds, a fresh server of each build per round, cached and `--no-cache`, load
7.0–7.9.

| What | `21a37cc` → `1c49f34` | Δ | Why |
|---|---|---|---|
| lib `arith` parse / parse + eval | 0.38 → 0.42 / 0.49 → 0.54 µs | +11% / +11% | #82 Excel precedence, accepted (below) |
| lib, the other 10 cases, parse | — | −6% … +4% | within noise |
| lib, the other 10 cases, parse + eval | — | −9% (`logic_if`) … +3% | within noise; `logic_if` −9% from #102 (short-circuit `AND`) and #127 (allocation-free `AND`) |
| `lambda_scope`, `:nums` cases | 1.99 → 2.15, 4.32 → 4.56 ms | +8%, +6% | below the 10% line; not bisected |
| `lambda_scope`, `:rows` cases | 2.80 → 2.78, 3.29 → 3.38, 3.50 → 3.61 ms | −1% … +3% | within noise |
| CLI `sk`, all cases | 2.79–2.96 → 1.87–1.96 ms | −33% | #81 (BL-055) |
| CLI floor (`/usr/bin/true`) | 0.93 → 0.93 ms | 0% | same session |
| HTTP per request, cached | 104–138 → 100–143 µs | −14% … +4% | within noise |
| HTTP `parse_heavy`, uncached | 152 → 110 µs | −27% | #80 (BL-012) per-worker parse cache |
| HTTP, the other uncached cases | 112–139 → 99–130 µs | −13% … +3% | within noise |
| `sk_http_bench` cached, 1 / 50 connections | 11098 → 11822 / 26372 → 27101 req/s | +7% / +3% | within noise |
| `sk_http_bench --no-cache`, 1 / 50 connections | 11377 → 11628 / 26172 → 26753 req/s | +2% / +2% | within noise |

No case regressed by more than 10% except `arith`, which is the accepted #82 cost.

## Changes from the first baseline to `21a37cc`

The first baseline was taken on `32dec00` (develop `d917ce1` + benches + BL-029) at load 3.7–3.9;
the second on `21a37cc` (2026-11-20, load 4.2–5.5; its table is in git history, `cab2054`).
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

1. **Library evaluation takes 0.4–6 µs** for typical expressions, and ~30 µs for 1.8 KB of text.
   On short expressions, parsing is 40–80% of the total.
2. **The CLI takes ~1.9 ms per call no matter which expression runs.** About 0.9 ms of that is
   OS process start; the other ~1 ms is `sk` start-up. Loading the repo's one JS hook does
   not change it: QuickJS only starts when an expression calls a hook (BL-040). Until BL-055
   another ~1.2 ms went to loading CoreFoundation before `main` (below); at `21a37cc` the CLI
   took 3.5–3.8 ms.
3. **HTTP takes ~0.1 ms per request** (new connection each time), 10× less than the first
   baseline. Evaluation is still only 0.5–30 µs of that, so cached and uncached requests cost
   about the same. Since BL-012 the per-worker parse cache also serves "uncached" requests (new
   variables, same text), so `parse_heavy` no longer stands out (90 µs cached, 99 µs uncached;
   109 / 147 µs at `21a37cc`). The connection itself (connect, thread hand-off, write, close) is
   the floor.
4. **`sk_http_bench -c` has no effect.** The concurrency sweep is fixed at 1, 5, 10, 20, 50
   (`run_concurrent_tests` in `src/bin/sk_http_bench.rs`); `-c` is only printed in the header.

### BL-055: CoreFoundation removed from `sk`

`sk` on macOS linked `CoreFoundation.framework` through `chrono` (`clock` feature) →
`iana-time-zone` → `core-foundation-sys`. Two crates turned `clock` on: `skillet` itself (for
`chrono::Local` in `DATE()` and `TIME()`) and `tera` (through `scalar-doc`, only for the `/docs`
page of `sk_http_server`). Cargo unifies features, so both had to go. ADR-0020
([0020](../docs/adr/0020-drop-corefoundation.md)) reads the local offset from libc instead and
serves `/docs` as static HTML.

First session (exploration, prototypes built from `3f59ae1`; load ~20):

| Build | `otool -L target/release/sk` | `sk` size | `sk "=1+1"` median (range of 5 round medians) | p95 |
|---|---|---|---|---|
| `3f59ae1` as is | CoreFoundation, libSystem | 4 382 000 B | 5.84 ms (4.38–6.32) | 11.7 ms |
| `skillet` chrono without `clock`, `Local` → `Utc` | CoreFoundation, libSystem | 4 340 400 B | 5.95 ms (4.38–6.54) | 11.9 ms |
| same, and `scalar-doc` removed | libSystem | 4 340 096 B | 4.31 ms (3.28–4.76) | 8.7 ms |
| `/usr/bin/true` (floor) | | | 2.94 ms (2.12–3.46) | 6.6 ms |

Changing only `skillet`'s chrono features saves nothing while `tera` keeps `clock` on.

Second session, before → after on the merged change (load 20–27):

| Build | `otool -L target/release/sk` | `sk` size | `sk "=1+1"` median (range of 5 round medians) | p95 |
|---|---|---|---|---|
| `3f59ae1` (before) | CoreFoundation, libSystem | 4 382 000 B | 4.55 ms (4.29–5.68) | 6.88 ms |
| prototype, libSystem only | libSystem | 4 340 096 B | 3.35 ms (3.15–4.09) | 4.63 ms |
| ADR-0020 (after) | libSystem, libiconv | 4 340 480 B | 3.29 ms (3.09–4.16) | 4.90 ms |
| `/usr/bin/true` (floor) | | | 2.14 ms (2.01–2.74) | 3.23 ms |

After vs. before: −1.26 ms per call (per-round deltas 1.29, 1.25, 1.31, 1.19, 1.52 ms), about
half of the time `sk` spent above the process-start floor. `libiconv` comes from the `libc`
crate's `#[link(name = "iconv")]` now that the library calls `localtime_r`; it costs nothing
measurable (prototype vs. after). `sk_http_server` shrank from 7 989 680 B to 4 833 568 B (no
`tera`). `sk_server` and `sk_http_server` still load Foundation through `ctrlc` → `dispatch2`,
as before; they are long-running, so start-up does not matter there.

Method: 5 rounds × 300 calls of each binary, interleaved, Python `subprocess` (so every number
includes launcher overhead; compare rows within a session, not with the table above), empty
working directory, release build, Apple M3 Pro, macOS 27.0.1, rustc 1.89.0, other agents
compiling. Command (from an empty directory, with each build copied aside):
`python3 -I benches/cli_startup.py /usr/bin/true sk_before sk_after`.

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

## BL-012: parse cache

Both servers keep a per-worker-thread cache of parsed expressions (ADR-0021). Measured on the
same machine (Apple M3 Pro, macOS 27.0.1), release builds of `sk_http_server` at `3f59ae1`
(before) and at the BL-012 branch (after), 2026-11-21. `sk_http_server --threads 4`, then
`sk_http_bench 127.0.0.1:PORT -n 1000 --no-cache` (a unique variable per request, so the result
cache never hits and every request evaluates). 11 rounds, before and after alternating, a fresh
server per run; load average 8.7–10.1 on 11 cores (other agents running), so compare the two
columns with each other, not with the tables above. Cells: median / p95 (nearest rank) over the 11
runs of the per-run average latency, and median (min–max) of req/s. The bench prints latency at
10 µs resolution.

Small formula: `10*5+sqrt(25)` (the default sweep expression):

| Connections | latency before → after | req/s before → after |
|---|---|---|
| 1 | 80 / 120 → 80 / 110 µs | 11803 (8546–13287) → 11781 (9149–14936), 0% |
| 5 | 170 / 200 → 170 / 190 µs | 29657 (24245–34127) → 29748 (26607–34695), 0% |
| 10 | 310 / 370 → 320 / 400 µs | 32255 (26546–35741) → 31260 (24728–37497), −3% |
| 20 | 600 / 720 → 620 / 680 µs | 32661 (27327–33959) → 31902 (28646–36567), −2% |
| 50 | 1510 / 1960 → 1500 / 1830 µs | 32308 (24746–34289) → 32379 (26549–36722), 0% |

`parse_heavy` (1,796 bytes, `--expr`):

| Connections | latency before → after | req/s before → after |
|---|---|---|
| 1 | 190 / 240 → 140 / 180 µs | 5129 (4127–6688) → 7245 (5410–7897), +41% |
| 5 | 260 / 330 → 210 / 250 µs | 19286 (14981–22098) → 23263 (19612–27218), +21% |
| 10 | 400 / 680 → 340 / 470 µs | 24332 (14532–28823) → 28334 (21139–29726), +16% |
| 20 | 660 / 1000 → 640 / 2020 µs | 29633 (19466–30971) → 30516 (9808–32068), +3% |
| 50 | 1680 / 3950 → 1640 / 4030 µs | 28204 (12317–32543) → 29137 (12122–31736), +3% |

Every request succeeded in every run. For the small formula the parse (~0.4 µs) is lost in the
~80 µs request, so the change is within noise, as the prototype found. For `parse_heavy` it saves
~50 µs per request at one connection; from 20 connections on, both builds are limited by the 4
workers and the connection handling, and the gain shrinks to ~3%.

## Accepted: parse cost of Excel precedence (#82, PERF-6)

The Excel precedence levels (ADR-0018 D-10, PR #82, commit `6127224`) made short expressions
slower to parse and evaluate. Measured in the 2026-11-22 audit (library layer, medians of 6
rounds, `3f59ae1` → `9b36d9b`; min/max ranges do not overlap):

| Case | Before → after | Change |
|---|---|---|
| `arith` parse | 0.39 → 0.46 µs | +18% |
| `arith` eval | 0.50 → 0.58 µs | +16% |
| `financial` eval | 0.43 → 0.48 µs | +12% |
| `parse_heavy` parse | 24.5 → 27.0 µs | +10% |
| `parse_heavy` eval | 29.7 → 33.2 µs | +12% |

A per-commit bisect puts the whole step at `6127224` (`arith` parse 0.38 → 0.45 µs); it stayed
flat at every later commit, and the other cases are within ±5%. **Accepted** (maintainer,
2026-11-22): parsing is a one-time cost per expression, the servers' per-worker parse cache
(ADR-0021) hides it on repeated requests, and Excel precedence is the language contract. This
crosses the ">10% on the baseline set" rule, so it is recorded here instead of blocking.

Re-measured on the 0.7.0 code (`21a37cc`, before #82 → `1c49f34`, the three-build session in
[0.6.3 → 0.7.0](#063--070)): `arith` parse +11%, `arith` eval +11%, `financial` eval +1%,
`parse_heavy` parse +4%, eval +3%. The `arith` cost is still there and has not grown; for
`financial` and `parse_heavy` it is now within ±5% (not bisected).

## Reproduce 0.6.3 → 0.7.0

```bash
git worktree add --detach /tmp/sk063 crate-v0.6.3
cp benches/baseline.rs benches/lambda_scope.rs /tmp/sk063/benches/
printf '\n[[bench]]\nname = "baseline"\nharness = false\n\n[[bench]]\nname = "lambda_scope"\nharness = false\n' >> /tmp/sk063/Cargo.toml
(cd /tmp/sk063 && cargo bench --no-run --bench baseline --bench lambda_scope)
# copy target/release/deps/{baseline,lambda_scope}-<hash> of each build aside as bl_<v>, ls_<v>, then:
for r in 1 2 3 4 5 6; do for v in 063 21a 070; do
  RUNS=15 LAYERS=lib ./bl_$v; RUNS=9 ./ls_$v
done; done
git worktree remove --force /tmp/sk063
```

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

# Parse cache before/after (BL-012): build sk_http_server on each commit, copy aside, then per
# round start each build with --threads 4 and run (HEAVY = the parse_heavy text from baseline.rs):
target/release/sk_http_bench 127.0.0.1:PORT -n 1000 --no-cache
target/release/sk_http_bench 127.0.0.1:PORT -n 1000 --no-cache --expr "$HEAVY"

# Stack per nesting level (BL-029):
cargo bench --bench parser_stack                 # release
cargo bench --bench parser_stack --profile dev   # debug
cargo test --test nesting_stack                  # depth 128 on a 2 MB thread, debug build
```

Record `uptime` before and after each run; numbers from a run with a 1-minute load average above
the core count are not comparable with this table.
