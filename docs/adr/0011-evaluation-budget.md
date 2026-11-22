# 0011. Per-evaluation budget and server evaluation timeout

> **Amendment at acceptance (maintainer, 2026-11-20):** BL-030 (lambdas share the parent scope, and
> variable reads stop deep-cloning) lands **before** BL-027b. Variable reads are therefore **not
> charged**: the "Variable read" row below is struck, A12 stays a passing formula (it moves to the
> P rows), and the PRD-001 open question about dropping the read charge is closed. Everything else
> is accepted as written, including A11 (quadratic `reduce(MERGE(:acc, [:x]), [])` fails above ~2,800 items).
>
> **Amendment 2 (maintainer, 2026-11-20, after BL-027c):** reads that **copy** a value are charged
> `cost(value)`; reads that only **borrow** (method/property targets, e.g. `:rows.length()`) stay free.
> A12 therefore still passes. Formulas that copy a large variable once per item (e.g.
> `:rows.map(SUM(:rows))` on big data) may now hit the budget; hoist instead:
> `:t := SUM(:rows); :rows.map(:t)`. Every variable read and binary operation also ticks the
> deadline (zero-byte charge). Amplifying conversions (`.to_a()` on strings, `.to_json()`,
> `::String`, `CONCAT`, `MERGE`) are pre-charged with an upper bound, and the HTTP server rejects
> results whose serialized size would exceed the budget. Removing the copy entirely (shared
> values) is tracked as BL-036 and would let the read charge be dropped again.
>
> **Amendment 3 (BL-035, 2026-11-21, no decision needed):** the per-string constant is **48**, not
> 32: `cost(String(s) | Json(s)) = 48 + s.len()` (32 for the `Value` plus 16 for the smallest heap
> block). Other values keep 32, so the shape of the model is unchanged. Measured with the system
> allocator's usable sizes (`malloc_size`, macOS): a 1-char string really holds 48.1 B and was
> charged 33; it is now charged 49. glibc's smallest block is 24 usable bytes (56 B per 1-char
> string, still within 1.25x). C3/C4 move from 36/35 to 52/51 bytes. Two further hardening steps
> need no new rule: `UPPER`/`LOWER`/`.upper()`/`.lower()` check that their output fits the budget
> **before** building it (up to 3x growth, e.g. U+0390; exact count only when the 3x worst case
> does not fit, so the pass/fail boundary does not move), and `SORT`, `UNIQUE`, `JOIN`, `TEXTJOIN`
> and their method forms tick the deadline every 64 items, so one such call no longer runs to
> completion past the deadline. Custom functions that move work to another thread still escape
> the budget and the deadline; that is documented on `CustomFunction` and `with_limits`.
>
> **Amendment 4 (maintainer, 2026-11-21, BL-075; defaults revised on PR #106):** Skillet processes
> large data, so the per-request limits of `sk_http_server` and `sk_server` become
> **configurable**. This replaces "no CLI flag or environment variable" in §2. Deadline:
> `--eval-timeout <seconds>` / `SKILLET_EVAL_TIMEOUT`, default **20 s** (was
> `SERVER_EVAL_TIMEOUT = 10 s`), whole seconds from 1 to 86400 (a deadline too far away to
> represent would silently mean none). Budget: `--max-eval-bytes <bytes>` /
> `SKILLET_MAX_EVAL_BYTES`, default **256 MiB**, the same as the library's `MAX_EVAL_BYTES`
> (unchanged, now configurable); a whole number of bytes or one of the suffixes `K`, `M`, `G`
> (KiB, MiB, GiB). The flag wins over the env var, and `0`, an empty or an invalid value stops
> start-up with exit code 1, as for the tokens and `--max-connections-per-ip` (ADR-0025). The
> banner shows both values. One shared definition (`src/bin/http_server/eval_limits.rs`) serves
> both servers, and the response-size check uses the configured budget. Unchanged: the library's
> `MAX_EVAL_BYTES` and `EvalLimits::default()`, no deadline in the library or `sk` (§2), the 30 s
> request read and 10 s response write deadlines, and the JS per-call cap (ADR-0017 §3: the request
> deadline or 10 s, whichever is sooner), so a looping plugin stops at 10 s, before the 20 s
> request deadline. Worst-case memory: every worker can hold one evaluation at the full budget, so
> the counted values reach `threads × max-eval-bytes` (8 threads × 256 MiB = 2 GiB), and live heap
> up to about twice that (Consequences). The Docker image's default `SK_THREADS` becomes 4:
> 4 × 256 MiB = 1 GiB counted, about 2 GiB live. Operators must size `--threads` and
> `--max-eval-bytes` for the host. The 10 s figures below describe the original decision.

- Status: Accepted (2026-11-20, with amendment below)
- Date: 2026-11-20
- Deciders: Maintainer (accepts), Language Designer (author), Security Reviewer, Rust Engineer
- Related: BL-027 (a/b/c), BL-017 / `src/limits.rs` (`MAX_NESTING_DEPTH`), BL-018 (QuickJS limits),
  BL-030 (shared lambda scope, filed with this ADR), PRD-001 NFR-4…NFR-7, ADR-0004, ADR-0008

## Context
One short expression can make an evaluation allocate memory that grows exponentially, and nothing stops it:

- `[1,…(20×)].reduce([:acc, :acc], 0)` builds a 2^20-leaf tree. Measured: 96 MB peak heap, 65 ms.
  Each extra item doubles it, so 26 items is about 6 GB.
- The same doubling works with flat arrays (`MERGE(:acc, :acc)`), so BL-017's depth limit does not
  catch it. It also works with strings (`CONCAT(:acc, :acc)`), JSON objects (`{"a": :acc, "b": :acc}`)
  and repeated assignment (`:a := MERGE(:a, :a); …`).
- Some single builtins multiply the size of their input:
  `SUBSTITUTE(4000×"a", "a", 4000×"b")` returns a 16 MB string from 8 KB of expression text.
  `SUBSTITUTE`/`SUBSTITUTEM`/`.replace` (including an empty search string), `JOIN`/`.join`
  (separator × count) and `JQ` with `$..*` (about 120× on deep JSON) all do this. There is no
  `REPT`, range or `SEQUENCE` builtin, and no builtin builds an array from a number argument.
- Time: one lambda iteration costs about 54 ns (1M nested `map` iterations take 54 ms). The real time
  sink is work the evaluator does not count: every method call inside a lambda clones the whole
  variable map (`clone_variables`). So `:nums.map(:x.abs()).sum()` over 100k numbers (589 KB of
  JSON, under the 1 MB HTTP cap) takes **14.6 s and allocates 305 GB in total**. A 10k-row
  `:rows.map(:x.price.round(1)).sum()` takes 1.7 s and allocates 10 GB.
- `sk_http_server` and `sk_server` evaluate on a fixed pool of `std::thread` workers (ADR-0008).
  A thread cannot be killed from outside. The only time limits today are the socket timeouts
  (30 s read, 10 s write). Nothing limits how long an evaluation runs once the request has been read.
- The `sk_http_server` result cache (`http_server/cache.rs`) holds 1000 entries. Each entry has no
  size limit: the key includes the `Debug` text of all variables, and the value is the full result.

These are the largest realistic workloads, measured in-process (release build, heap counted by a
wrapping global allocator). The tests in `tests/sk_performance.rs` are much smaller: `SUM` over at
most 1000 numbers, and JSON with 100 items.

| Workload | Time | Total allocated |
|---|---|---|
| `:nums.map(:x*2).map(:x+1).map(:x/3).filter(:x>10).sum()`, 100k nums (589 KB JSON) | 23 ms | 46 MB |
| same, 300k nums (2 MB JSON, above the HTTP cap) | 69 ms | 138 MB |
| `:rows.filter(:x.cat=="a").map(:x.price*:x.qty*(1+:rate)).sum()`, 10k rows (786 KB) | 10 ms | 23 MB |
| `JQ(:rows, "$[*].price").sum()`, 10k rows | 8 ms | 13 MB |
| `:rows.map({"id": :x.id, "total": :x.price*:x.qty}).length()`, 10k rows | 18 ms | 38 MB |

## Decision

### 1. One budget per evaluation, counted in bytes of values created
Each evaluation gets a budget, `MAX_EVAL_BYTES = 256 MiB`. Every `Value` the evaluator creates is
charged against it. The cost of a value is a deterministic estimate of its size:

```
cost(Number | Boolean | Null | Currency | DateTime) = 32          // size_of::<Value>()
cost(String(s) | Json(s))                           = 32 + s.len()   // 48 + s.len() since amendment 3
cost(Array(items))                                  = 32 + Σ cost(item)
```

The total is cumulative: values are charged when they are created, and freeing them gives nothing back.
So the counter measures both memory and work, and it is deterministic. The same expression with the
same variables always passes or always fails, whatever the machine, allocator or load.

**Where values are charged** (the Rust engineer must cover every site listed):

| Site | Charge |
|---|---|
| Results of array literals, function calls, method and safe-method calls, and casts (the sites that already call `check_value_depth`) | `cost(result)` |
| Object literal result, and property / safe-property access result | `cost(result)` |
| ~~Variable read~~ | Not charged (amendment: reads are cheap after BL-030) |
| Assignment `:a := v` (the stored clone) | `cost(v)` |
| Each lambda iteration (`filter/map/find/reduce/SUMIF/AVGIF/COUNTIF`, function and method forms) | `cost(bound item)` |

Some builtins can produce far more than their already-charged inputs, either at once or while they
run. These must **charge before or while they allocate**, not afterwards:

| Builtin | Charge before or while allocating |
|---|---|
| `SUBSTITUTE`, `SUBSTITUTEM`, `.replace` | exact output length, worked out first: `len + matches × (repl.len − find.len)`. An empty `find` matches `chars + 1` times. |
| `JOIN`, `.join` | `Σ part.len + (n − 1) × sep.len`, worked out first |
| `SPLIT`, `.split` | `32 + part.len` (48 since amendment 3) for each part, while collecting |
| `JQ` | `cost` of each match, before it is cloned out of the query result |
| Parsing a `Json` string (property access, `DIG`, `.keys/.values/.dig`, `JQ` input, object literals that embed a `Json`) | `s.len() + 32 × (1 + count of ',' ':' '[' '{' in s)`, a one-pass upper bound on the nodes, charged before `serde_json::from_str` |

Every other builtin produces at most a small constant multiple of its charged inputs
(`MERGE`, `FLATTEN`, `ARRAY`, `CONCAT`, `UPPER`, `SORT`, `UNIQUE`, `.to_s/.to_json`, …). Those are
charged afterwards, when their result reaches a function or method result site.

**Implementation shape.** Replace `check_value_depth` with a single `check_value` that walks the
value once, checks depth (BL-017) and adds up the cost. It stops early when either limit is passed.
Depth already walks every element of an array, so this costs about nothing extra. The counter lives
in a `thread_local!` budget (`used`, `max`, `deadline`, `ticks`). It is not a field on
`EvaluationContext`, because lambdas and method arguments build fresh contexts
(`VariableContext::with_owned`, `eval_with_vars`) and would lose it. Each public `evaluate*` entry
point opens a default scope if none is active. A nested evaluation, such as a custom function calling
`skillet::evaluate`, joins the scope that is already active, so it cannot reset the counter.

**Error text** (same style as `Expression nested too deeply (max 128)`, position `None`):
- `Evaluation budget exceeded (max 256 MB)`. The number shown is the active limit in MiB, or
  `max N bytes` when the limit is below 1 MiB.

### 2. Wall-clock deadline, checked inside the evaluator (servers only by default)
The budget struct has an optional `deadline: Option<Instant>`. Every call to `charge()` increments
`ticks`. On every 256th tick, the evaluator reads the clock (`Instant::now()`, about 20–40 ns). If the
deadline has passed, it returns:
- `Evaluation timed out (max 10 s)`. The number shown is the active limit, in whole seconds, or `N ms`
  when it is not a whole number of seconds.

The error unwinds through the normal `Result` path. The worker thread is then free for the next
request. Nothing is cancelled or killed, and nothing outside the evaluator's own loop is interrupted.
A single builtin that is already running, such as `SORT` on a big array or one `clone_variables`,
finishes before the next check. Its cost is bounded by the budget, or by the size of the variables
until BL-030 lands.

- `sk_http_server` and `sk_server` wrap every evaluation in a deadline of
  `SERVER_EVAL_TIMEOUT = 10 s`. That is 6× the slowest realistic workload measured (1.7 s, caused by
  the env clone that BL-030 removes) and well below the 30 s socket read timeout. **No CLI flag or
  environment variable.** A flag can be added later without breaking anything, if a deployment shows
  it needs one.
- The library and the `sk` CLI have **no deadline by default**. Library results must stay
  deterministic, and embedders own their own threads.
- Both errors are ordinary evaluation errors, so the servers return HTTP **400**, as they do for every
  other evaluation error today. The wire format does not change.

### 3. Constants by default, one scoped override for embedders
New public API, all additive:

```rust
pub const MAX_EVAL_BYTES: u64 = 256 * 1024 * 1024;

#[non_exhaustive]
#[derive(Clone, Copy, Debug)]
pub struct EvalLimits { pub max_bytes: u64, pub timeout: Option<Duration> }
impl Default for EvalLimits { /* MAX_EVAL_BYTES, None */ }

/// Run `f` with these limits for every evaluation it performs on this thread.
pub fn with_limits<R>(limits: EvalLimits, f: impl FnOnce() -> R) -> R;
```

A `with_limits` inside an active scope can only **tighten** the limits: it takes the smaller
`max_bytes`, comparing against what remains of the outer budget, and the earlier deadline. A custom
or plugin function therefore cannot raise the limits from inside an evaluation.

The override exists for two reasons:
1. The servers need some way to set a deadline.
2. Embedders evaluate datasets in process, without the 1 MB HTTP cap. A 10 MB JSON array of numbers
   needs about 45 MB of budget per pass over it.

The signatures of existing functions do not change. `MAX_NESTING_DEPTH` stays a constant with no
override, because it protects the stack, not the heap.

### 4. Server cache
The LRU cache stores an entry only if `key.len() + approx_size(result) + approx_size(variable_context)`
is at most **64 KiB**. `approx_size` is the same cost formula, as a private helper in `cache.rs`.
This caps the cache at about 64 MB in the worst case. Without the cap, 1000 entries could each hold a
budget-sized result or a key built from 1 MB of variables. Errors stay uncached. Budget errors never
reach the cache anyway, and timeouts depend on load, so they must never be cached.

### 5. JS plugins and custom functions
The result of a JS or Rust custom function is charged like any other function result, and its
arguments are charged where they are evaluated. Memory and CPU used *inside* QuickJS are not covered
here. That is BL-018: a QuickJS memory limit plus an interrupt handler. Its interrupt handler should
check the same thread-local deadline (`limits::deadline_passed()`), so one timeout covers both.

## Consequences
- \+ No expression can make one evaluation create more than 256 MiB of values. Live heap per
  evaluation is at most the budget plus the one value that crossed it (≤ ~2× budget), plus the
  uncounted env copies until BL-030. Worst case for a server ≈ `threads × 512 MB`. Operators size
  `--threads` with that in mind. Before this change, there was no bound at all.
- \+ The counter is deterministic, so QA can pin exact boundaries (see the example table).
  It costs one thread-local add per charge site. The depth walk is reused.
- \+ Lambda iterations are metered too. The charged work is capped at roughly 4M iterations
  (≥ 64 B each), about 0.25 s, so the deadline only has to cover work the budget does not count.
- − **Behaviour change (Error where a Value was returned before)**. This affects only quadratic
  patterns at sizes where they already took hundreds of milliseconds or more:
  - Accumulating with `reduce(MERGE(:acc, [:x]), [])` fails above about 2,900 items
    (10k items: 228 ms and 3 GB allocated today). Use `map`/`filter`/`FLATTEN` instead.
  - Reading a large variable inside a lambda, for example
    `:rows.map(:x.price / :rows.length())`, passes at 1k rows (about 110 MB charged) and fails at
    10k rows (3.5 s and 21 GB allocated today). Hoist the read: `:n := :rows.length(); :rows.map(:x.price / :n)`.
  
  No existing test changes. These patterns go in the docs (BL-028 limits section).
- − The budget does not count `clone_variables`, because counting it would reject ordinary
  10k-row formulas. Until **BL-030** (lambdas share the parent scope instead of cloning it) lands,
  the library and CLI have no protection against `:nums.map(:x.abs())`-style CPU blow-ups, and the
  servers rely on the 10 s deadline. BL-030 should land with or right after BL-027b. If BL-030 or a
  later change makes variable reads O(1) (shared values), the read charge can be dropped. That is a
  relaxation, not a breaking change.
- − Some new public API surface (`MAX_EVAL_BYTES`, `EvalLimits`, `with_limits`). It is additive, and
  `#[non_exhaustive]` leaves room for more fields later, such as a step limit.
- Thread-local state means an embedder that moves work to another thread inside a custom function
  does not carry the budget across. That is acceptable, and it is documented.

## Example table (acceptance for BL-027b / QA)
Values were measured on the current build unless the row says *model*. Byte figures in the "Why"
column use the cost formula above. `n×"a"` means a literal string of n `a` characters, and `[1×n]` an
array literal of n ones.

**Must keep passing (unchanged Value)**

| # | Expression (vars) | Expected | Why |
|---|---|---|---|
| P1 | `:nums.map(:x*2).filter(:x>10).sum()` (nums = 0..99999) | `Number(9999899970.0)` | ~20 MB charged |
| P2 | `:nums.map(:x*2).map(:x+1).map(:x/3).filter(:x>10).sum()` (nums = 0..99999) | `Number(3333333258.3333335)` | ~40 MB charged |
| P3 | `:nums.sort("DESC").unique().reverse().length()` (nums = 0..99999) | `Number(100000.0)` | ~13 MB charged |
| P4 | `:rows.filter(:x.cat == "a").map(:x.price * :x.qty * (1 + :rate)).sum()` (10k rows, test fixture) | Number, unchanged | ~15 MB charged (incl. Json parse pre-charge) |
| P5 | `:rows.map({"id": :x.id, "total": :x.price * :x.qty}).length()` (10k rows) | `Number(10000.0)` | ~20 MB charged |
| P6 | `JQ(:j, "$..*").length()` (j = 120 nested objects around a 100 KB string) | `Number(121.0)` | ~12 MB (pre-charged per match) |
| P7 | `[1×1000].map([1×1000].map(:x + 1).length()).sum()` (1M iterations) | `Number(1000000.0)` | ~130 MB charged, 54 ms |
| P8 | `SUBSTITUTE(4000×"a", "a", 4000×"b").length()` | `Number(16000000.0)` | 16 MB pre-charged |
| P9 | `:rows.map(:x.price / :rows.length()).sum()` (1k rows) | Number, unchanged | ~110 MB charged |
| P10 | `[1×20000].reduce(:acc + :x, 0)` | `Number(20000.0)` | ~1.3 MB |
| P11 | `SUM(1, 2, …, 1000)` (largest in `tests/sk_performance.rs`) | `Number(500500.0)` | tiny |
| P12 | `[1×128].reduce([:acc], 0)` and `[1×50000].reduce([:acc], 0)` (security_limits) | unchanged: Value / `Expression nested too deeply (max 128)` | depth limit fires first |

**Must fail with the budget error (`Evaluation budget exceeded (max 256 MB)`)**

| # | Expression | Unbounded today | Under the budget (*model*) |
|---|---|---|---|
| A1 | `[1×26].reduce([:acc, :acc], 0)` | 20 items: 96 MB peak; 26: ~6 GB | fails by step ~21 |
| A2 | `[1×32].reduce(CONCAT(:acc, :acc), "ab")` | 22 items: 8 MB string, 40 MB alloc; 32: 8 GB | fails at step ~25 (64 MB string) |
| A3 | `[1×30].reduce(MERGE(:acc, :acc), [1])` (flat, no depth growth) | 20 items: 80 MB peak | fails at step ~21 |
| A4 | `[1×30].reduce({"a": :acc, "b": :acc}, 0)` | 16 items: 41 MB peak | fails by step ~21 (Json parse pre-charge) |
| A5 | `:a := [1]; :a := MERGE(:a, :a); …(30×); :a.length()` | 20×: 128 MB peak | fails at assignment ~21 |
| A6 | `:s := "ab"; :s := CONCAT(:s, :s); …(32×)` | 22×: 40 MB peak | fails at assignment ~25 |
| A7 | `SUBSTITUTE(20000×"a", "a", 20000×"b")` | 400 MB in one call | fails before allocating (pre-charge 400 MB) |
| A8 | `20000×"a".replace("", 20000×"b")` | ~400 MB in one call | fails before allocating |
| A9 | `JOIN([""×20000], 20000×"b")` and `.join` | ~400 MB in one call | fails before allocating |
| A10 | `[1×1000].map([1×1000].map([1×1000].map(:x).length()).sum())` (1e9 iterations) | ~54 s of CPU | fails after ~8k inner literals (< 0.5 s) |
| A11 | `:nums.reduce(MERGE(:acc, [:x]), []).length()` (nums = 0..9999) | `Number(10000.0)`, 228 ms, 3 GB alloc | **behaviour change**: fails (~3 GB charged); passes at ≤ 2,800 items |
| A12 | `:rows.map(:x.price / :rows.length()).sum()` (10k rows) | Number, 3.5 s, 21 GB alloc | **amended**: must pass (same Number); after BL-030 it should also be fast |

**Timeout (servers only, `Evaluation timed out (max 10 s)`, HTTP 400)**

| # | Expression | Library / `sk` | Server |
|---|---|---|---|
| T1 | `:nums.map(:x.abs()).sum()` (nums = 0..99999, 589 KB JSON) | `Number(4999950000.0)` after 14.6 s (until BL-030; then a few ms) | timeout error after ~10 s, worker freed |
| T2 | any P-row above | same Value | same Value (all < 0.1 s) |

**Cost model boundaries (normative, for `limits.rs` unit tests with `with_limits`)**

| # | Limits | Expression | Expected |
|---|---|---|---|
| C1 | `max_bytes = 160` | `[1, 2, 3, 4]` | `Array([1,2,3,4])` (cost 32 + 4×32 = 160) |
| C2 | `max_bytes = 160` | `[1, 2, 3, 4, 5]` | `Evaluation budget exceeded (max 160 bytes)` |
| C3 | `max_bytes = 36` (52 since amendment 3) | `"abcd".upper()` | `String("ABCD")` (method result 32 + 4, now 48 + 4; literals are not charged) |
| C4 | `max_bytes = 35` (51 since amendment 3) | `"abcd".upper()` | `Evaluation budget exceeded (max 35 bytes)` (now `max 51 bytes`) |
| C5 | outer `max_bytes = 1000`, inner `with_limits(max_bytes = 10 MiB)` | `[1×40]` (1312 B) inside the inner scope | budget error (inner cannot raise outer) |
| C6 | `timeout = 1 ms` | T1 expression | `Evaluation timed out (max 1 ms)` |

## Alternatives considered
- Counting global allocator / `cap` crate. It measures real bytes, but a library must not choose the
  embedder's allocator, and results would depend on the allocator and on concurrency.
- Separate caps on single array length and string length. They do not bound the total: a sequence of
  values each just under the cap can still exhaust memory (`:b := :a; :c := :a; …`), and the
  `CONCAT(:a, :a, …)` argument fan-out has the same problem. A cumulative counter covers both with one number.
- A separate step counter next to the byte budget. It would add a second number to explain and tune.
  Charging each lambda iteration the size of its bound item meters steps in the same unit.
- Charging `clone_variables`. That would reject ordinary 10k-row formulas (10 GB of clones). The fix
  is to remove the clone (BL-030), not to charge for it.
- Running each evaluation on a fresh thread or a watchdog that kills it. Rust cannot kill a thread,
  a process per request is far too slow, and both lose the 16 MB worker-stack setup from BL-017.
- A deadline by default in the library. Results would then depend on machine speed. Embedders opt in
  with `with_limits`.
- A CLI flag or env var for the server timeout or budget now. No deployment has asked for one, and
  adding one later is additive.
