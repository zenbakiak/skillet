# Backlog

Single source of truth for work. Statuses: `todo` → `ready` (has brief) → `in-progress (<agent>)`
→ `review` → `done`. Priority P1 (now) · P2 (next) · P3 (later). Only the Orchestrator reorders.

Seeded 2026-11-20 from a codebase audit: 153 tracked files, ~16k LOC, all tests green,
12 compiler warnings, 113 clippy warnings, `cargo fmt --check` reports ~970 diffs.

---

### BL-001 · Unify the two evaluators · P1 · `done`
Two parallel implementations exist: `src/runtime/evaluator.rs` (used by `lib.rs` public API) and
`src/runtime/evaluation/` (used by `method_calls/*` when evaluating lambdas). Top-level and
nested evaluation can silently diverge.
- **BL-001a (qa-engineer)**: differential tests — same expressions through `runtime::evaluator::*`
  and `runtime::evaluation::*`; document every divergence found.
- **BL-001b (rust-engineer)**: keep one (likely `evaluation/`, it is already modular), make the
  other a re-export, delete the duplicate. Blocked by BL-001a.

Brief (orchestrator, 2026-11-20):
  Goal: one evaluator implementation; every expression returns the same Value as today via the public API.
  Branch: feature/bl-001-unify-evaluators (001a commits first, then 001b; one PR).
  BL-001a · qa-engineer · `done`
    Scope: new tests/evaluator_parity.rs only.
    Do: run a broad expression corpus (every Expr variant, every builtin category, methods,
        lambdas, assignments, custom functions, errors) through both
        skillet::runtime::evaluator::{eval, eval_with_vars, eval_with_vars_and_custom,
        eval_with_assignments*} and skillet::runtime::evaluation::{same}; assert equal
        (Ok values equal, or both Err). Divergences: keep the test, mark #[ignore = "BL-001 divergence"],
        and list them in the handoff with both outputs.
    Out of scope: src/ changes.
    Acceptance: cargo test --test evaluator_parity green; divergence list in handoff.
  BL-001b · rust-engineer · `done`
    Scope: src/runtime/, src/lib.rs (call sites only).
    Do: REVISED after 001a (orchestrator): keep runtime/evaluator.rs. It is self-contained and
        matches all 64 parity divergences on the contract side; evaluation/ is the incomplete copy
        (no JQ/DIG, broken REDUCE :acc default, ignores param names on the custom path).
        Point all callers (runtime/mod.rs re-exports, method_calls/*) at evaluator.rs; replace
        runtime/evaluation/{core,assignments}.rs with a thin re-export module so pub paths keep
        compiling; move anything evaluator.rs still needs from higher_order.rs, delete the rest.
        Splitting evaluator.rs into modules is a later, separate refactor.
    Contract for divergences: evaluator.rs behavior wins, because tests/ pin it.
        Any divergence where that is unclear goes back to the orchestrator, not decided in code.
    Acceptance: ./harness/check.sh green; parity tests un-ignored and green; clippy count not up;
        `wc -l src/runtime/evaluator.rs` shrinks to a re-export.
    Refs: ADR-0002, PRD-003 FR-1.

Handoff (qa-engineer, Claude Opus 5.5, 2026-11-20) — BL-001a:
  Did: added tests/evaluator_parity.rs: table-driven differential harness, 273 expressions
       (all Expr variants, every function_dispatch category incl. JQ/DIG, FILTER/MAP/REDUCE/FIND/
       SUMIF/AVGIF/COUNTIF + .filter/.map/.reduce/.find with default and custom params, null,
       errors, local-registry custom fns) run through eval / eval_with_vars /
       eval_with_vars_and_custom / eval_with_assignments / eval_with_assignments_and_context of
       both runtime::evaluator and runtime::evaluation (~1190 expression x entry-point pairs).
       Known divergences are skipped by the parity tests and replayed in 5 #[ignore] tests.
  Evidence: cargo test --test evaluator_parity -> 7 passed, 5 ignored; with --ignored the 5
       divergence tests fail as expected (64 divergent pairs). ./harness/check.sh --fast OK.
       New file adds 0 clippy warnings (count 113 vs baseline 95 predates this change).
  Divergences (evaluator.rs result wins per brief; evaluation/ result shown second):
    D1 JQ/DIG missing in evaluation/ (all var entry points): e.g. JQ(:arguments,'$.name') ->
       Ok(String "acme") vs Err "Unknown function: JQ"; DIG(:obj,['user','name']) -> Ok "Ann" vs
       Err "Unknown function: DIG".
    D2 evaluation::eval (no vars) rejects FILTER/MAP/REDUCE/FIND/SUMIF/AVGIF/COUNTIF:
       e.g. MAP([1,2,3], :x * 10) -> Ok [10,20,30] vs Err "MAP requires variable context".
    D3 REDUCE default :acc in evaluation/ non-custom path: REDUCE([1,2,3], :acc + :x, 0) ->
       Ok 6 vs Err "Missing variable: :acc" (get_param_name(None) returns "x" for acc too).
       Same for REDUCE(:arr, :acc * :x, 1) -> 120. Method .reduce() is unaffected.
    D4 evaluation/ *_with_custom higher-order fns ignore param-name args: e.g.
       FILTER([1,2,3,4], :v > 2, 'v') -> Ok [3,4] vs Err "Undefined variable: v" (also MAP/REDUCE/FIND).
    D5 evaluation::eval (no vars) is reduced: 'a' == 'a' -> Ok true vs Err "Comparison on
       non-number"; NULL == NULL -> Ok false (QA: suspected bug, pinned as-is) vs same Err;
       {"a":{"b":2}}.a.b, &.prop, &.method(), "1; 2; 3" -> Ok vs Err "Use eval_with_vars for ...".
    Error-message-only differences (outcome agrees): builtin arg errors in evaluation/ surface as
       "Unknown function: <NAME>" (UPPER(1,2,3), SUBSTRING(), REPLACE('aaa','a','b'),
       ROUND(:money,0)); missing var "Missing variable: :x" vs "Undefined variable: x".
  Risks/Follow-ups: BL-001b must un-ignore the 5 divergence_* tests. NULL == NULL -> false in the
       public API looks wrong; Language Designer should rule (not filed as an item yet).
       REPLACE is Excel-style (4 args), so REPLACE('aaa','a','b') errors in both paths.
  ADR: none

Handoff (rust-engineer, Claude Opus 5.5, 2026-11-20) — BL-001b:
  Did: method_calls/{mod,lambda_methods,string_methods,array_methods}.rs and runtime/mod.rs
       re-exports now call runtime::evaluator. Deleted runtime/evaluation/{core,higher_order,
       assignments,mod}.rs; runtime/evaluation.rs is a 9-line re-export of the five entry points
       (pub paths unchanged). evaluator.rs needed nothing from higher_order.rs (already
       self-contained), so it is unchanged. Parity test file: removed the 5 #[ignore] attributes
       and emptied KNOWN_DIVERGENCES. assert_parity_each kept: the 5 divergence_* tests use it.
  Evidence: ./harness/check.sh (full) OK; cargo test --test evaluator_parity 12 passed, 0 ignored;
       no other test in tests/ changed result. cargo build --lib --no-default-features OK.
       Clippy 113 -> 92. Lines: evaluator.rs 812 -> 812; runtime/evaluation/ 1204 -> 9
       (evaluation.rs); net -1199 in src/ (commit: +17 / -1216).
       Perf not measured (no BL-005 baseline yet). Lambdas/methods now take evaluator.rs's path,
       which borrows vars (Cow) per call, so no new cloning is expected; confirm in BL-005.
  Risks/Follow-ups: behavior change inside method lambdas (contract side wins, per brief): checked,
       [1,2,3].map(DIG({"a": :x}, ["a"])) was Err "Unknown function: DIG", now [1,2,3]
       (same for JQ). [[1,2],[3]].map(REDUCE(:x, :acc + :x, 0)) = [3,3] before and after (the
       lambda path already used the custom branch). No existing test pinned the old errors. The brief's
       acceptance line "wc -l evaluator.rs shrinks to a re-export" is a typo: evaluation/ is the
       part that shrank. method_calls_old.rs (not compiled) still names runtime::evaluation; BL-002
       deletes it. Stale doc comments in evaluator_parity.rs still say the divergence tests are
       #[ignore]d / "expected to FAIL" (QA to tidy). The NULL == NULL question stays in BL-023.
  ADR: none

Review (orchestrator, 2026-11-20): accepted. Full gate green, 12/12 parity tests, clippy 113 → 92,
  -1199 lines. Method lambdas now gain JQ/DIG and the other D1-D5 fixes (intended). Perf unmeasured
  until BL-005. Pre-existing JQ single-match unwrap filed as BL-024.

### BL-002 · Remove dead modules and unused dependencies · P1 · `done` · rust-engineer
`memory_pool.rs`, `traits.rs`, `concurrent_registry.rs` are `pub mod` but unused;
`runtime/method_calls_old.rs` is not even compiled. Deps `string-interner`, `typed-arena`, `beef`,
`dashmap` are only used by dead code. Since the modules are `pub`, removal is a semver-visible
change → mark `#[deprecated]` in 0.7, remove in 0.8 (or ADR to remove now while pre-1.0).

Brief (orchestrator, 2026-11-20): maintainer chose to remove now (ADR-0010, Accepted).
  Goal: no dead modules, files, or dependencies left; gate green.
  Role: rust-engineer · `done` · Branch: feature/bl-002-remove-dead-code
  Part 1 (internal, no API impact):
    - Cargo.toml: drop `string-interner`, `typed-arena`, `beef` (no users at all).
    - Delete src/runtime/method_calls_old.rs (not compiled).
    - src/bin/http_server/cache.rs, multipart.rs: delete the dead items BL-003 marked with
      `#[allow(dead_code)]` + BL-002 notes (ResponsePool, get/return_pooled_response, unused
      CachedResult fields, MultipartField.content_type, get_field) and remove those allows.
      Bins are not public API. If deleting a field changes a JSON response shape, stop and report.
  Part 2 (public API, ADR-0010):
    - Delete src/memory_pool.rs, src/traits.rs, src/concurrent_registry.rs and their `pub mod` lines;
      drop `dashmap`.
    - Delete src/runtime/evaluation.rs and its `pub mod`.
    - tests/evaluator_parity.rs: its second path is `runtime::evaluation::*`. Re-point that side at
      the public API (`skillet::evaluate`, `evaluate_with`, `evaluate_with_custom` analogue via
      `runtime::evaluator::eval_with_vars_and_custom`, `evaluate_with_assignments*`) so the corpus
      still compares two entry layers; keep every expression; update the header comment.
      This is the only test edit allowed.
  Docs: fix AGENTS.md §4 map rows and the module tree in docs/DEVELOPMENT_GUIDE.md (remove the three
        modules and the "string interning"/"memory pooling" claims about them).
  Out of scope: version bump and CHANGELOG (release branch does that; note "Removed" items in the handoff).
  Acceptance: ./harness/check.sh (full) green; `cargo tree -e normal --depth 1` no longer lists the 4 deps;
        `rg -n 'memory_pool|concurrent_registry|runtime::evaluation|dashmap' src tests Cargo.toml` empty;
        no `BL-002` allow comments remain; report lines deleted.

Handoff (rust-engineer, claude-opus-5-5, 2026-11-20):
  Did: - bd15562 Part 1: dropped string-interner, typed-arena, beef; deleted
         runtime/method_calls_old.rs; removed ResponsePool + get/return_pooled_response,
         CachedResult.{execution_time_ms,cache_hit}, MultipartField.content_type (and its parsing)
         and MultipartData::get_field, with their BL-002 allows. None were serialized, so no HTTP
         JSON shape changed (EvalResponse.execution_time_ms is a separate, still-used field).
       - 22fa8a0 Part 2 (ADR-0010): deleted memory_pool.rs, traits.rs, concurrent_registry.rs,
         runtime/evaluation.rs and their `pub mod` lines; dropped dashmap. evaluator_parity.rs now
         compares runtime::evaluator (pre-parsed Expr) against the public API on the source string
         (evaluate / evaluate_with / evaluate_with_custom / evaluate_with_assignments[_and_context]);
         Custom mode uses evaluate_with_custom with TRIPLE/GREET registered once in the global
         registry (own test binary, no race). Corpus unchanged, no new public API.
       - docs commit: AGENTS.md §4 rows, DEVELOPMENT_GUIDE.md module tree, pipeline and lexer/parser
         sections (no interning/arena claims; stale evaluation/ paths now point at evaluator.rs).
  Evidence: ./harness/check.sh (full, strict) green: 234 tests pass (239 before: the 5 removed were
         unit tests inside memory_pool.rs and concurrent_registry.rs, testing only that dead code).
         evaluator_parity 12/12. `cargo tree -e normal --depth 1` lists none of the 4 deps;
         `rg -n 'memory_pool|concurrent_registry|runtime::evaluation|dashmap|BL-002' src tests
         Cargo.toml` is empty. Net -1935 lines excluding Cargo.lock (+78/-2013), -2039 with it.
         Test edits besides evaluator_parity.rs: the http_server cache unit test (src/bin) now
         checks hit/miss via cache stats instead of the deleted `cache_hit` field (same coverage).
  Removed pub paths (for the 0.7.0 changelog "Removed"): skillet::memory_pool,
         skillet::traits, skillet::concurrent_registry, skillet::runtime::evaluation.
  Risks/Follow-ups: README.md:21 still claims "string interning and memory pooling" and
         benches/benchmark_phase1.sh echoes the same; both outside this brief (docs/perf owners,
         fits BL-006). harness/roles/performance-engineer.md:27 cites
         runtime/evaluation/higher_order.rs (now evaluator.rs). DEVELOPMENT_GUIDE §7.2 still has a
         generic "use string interning" tip; left as advice. Cargo.toml `include` still lists
         deleted DAEMON_*.md files (BL-006).
  ADR: none (implements ADR-0010)

Review (orchestrator, 2026-11-20): accepted. 5 modules/files and 4 deps gone, ~1.9k lines removed,
  strict gate green (234 tests; the 5 fewer are unit tests of deleted code). No HTTP response shape
  changed. 0.7.0 changelog "Removed": skillet::{memory_pool, traits, concurrent_registry,
  runtime::evaluation} (BL-021). Leftover false claims moved to BL-006.

### BL-003 · Format + clippy to zero, then make the gate strict · P1 · `done` · rust-engineer
One commit `cargo fmt` (no other changes), then fix clippy (mostly `args.get(0)` → `first()`,
empty `println!("")`). Finish by setting `STRICT=1` default in `harness/check.sh`.

Brief (orchestrator, 2026-11-20):
  Goal: `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` pass (default and
        --no-default-features); the gate enforces it by default.
  Role: rust-engineer · `done` · Branch: feature/bl-003-fmt-clippy
  Scope: whole crate (src/, tests/, hook_examples/ if compiled), harness/check.sh.
  Commits, in order:
    1. style: `cargo fmt` only. Nothing else in that commit, so reviewers can skip it.
    2. fix(lint): mechanical clippy/rustc fixes with no behavior change.
    3. Dead code: delete only if private and unused; unused `pub` items get `#[allow(dead_code)]`
       with a `// BL-002` comment instead (removal is BL-002's job, needs deprecation).
    4. ci(harness): STRICT=1 default in check.sh (clippy -D warnings + fmt --check), keep STRICT=0
       as an opt-out, drop the warning-count baseline text.
  Out of scope: semantic changes, renames of pub items, new deps, clippy.toml/rustfmt.toml
        (defaults are fine), files in src/runtime/method_calls_old.rs (not compiled; BL-002).
  Rule: if a lint fix would change a result or a pub signature, `#[allow(...)]` it locally with a
        one-line reason and list it in the handoff instead.
  Acceptance: `./harness/check.sh` (full) green with STRICT=1 default; no test file expectations
        changed; list of every `#[allow]` added.
  Refs: PRD-003 FR-3.

Handoff (rust-engineer, claude-opus-5-5, 2026-11-20):
  Did: - 7c4646c `cargo fmt` only (65 files). bd24d2a mechanical clippy/rustc fixes.
         a27532f dead code: deleted private unused fields `duration`, `total_time` in
         sk_http_bench.rs; unused pub items in http_server got `#[allow(dead_code)] // BL-002`.
         93ed863 check.sh: STRICT=1 default (clippy -D warnings for all targets and for
         `--lib --no-default-features`, plus fmt --check); STRICT=0 opt-out reports only.
  Evidence: `./harness/check.sh` (full) green with strict default. clippy warnings 92 -> 0;
         rustc build warnings 0. No test expected value changed (test edits are fmt, clippy
         `assert_eq!(x, true)` -> `assert!(x)`, range `contains`, unused import, allow attrs).
  Allows added:
         - tests/{arithmetic,prd_cases,prd_comprehensive}.rs:2 `#![allow(clippy::approx_constant)]`:
           3.14/3.14159 are ROUND() inputs/expectations, not PI (deny-by-default lint).
         - src/runtime/statistical.rs:156 `allow(clippy::manual_range_contains)` (PERCENTILE):
           `!(0.0..=1.0).contains(&p)` would reject NaN, which the `<`/`>` form lets through.
         - src/bin/http_server/cache.rs:14,16 (CachedResult.execution_time_ms, cache_hit),
           :242,:248 (ResponsePool + impl), :276,:282 (get/return_pooled_response);
           src/bin/http_server/multipart.rs:7 (content_type), :18 (get_field). All `dead_code`, BL-002.
  Risks/Follow-ups: binaries do not build with `--no-default-features` (they import
         JSPluginLoader/js_plugin unconditionally); pre-existing, gate only checks `--lib` there.
         Suggest a small item to `#[cfg(feature = "plugins")]` those imports or set
         `required-features` on the bins. PERCENTILE accepting NaN is likely a bug; route to
         language-designer if anyone cares.
  ADR: none

Review (orchestrator, 2026-11-20): accepted. fmt commit verified as pure `cargo fmt` output (re-ran
  on parent, identical tree); 239 tests pass before and after; clippy 92 → 0; strict gate default.
  Follow-ups: BL-025 (bins without plugins), BL-026 (PERCENTILE NaN). `style` type added to ADR-0009 hook.

### BL-004 · CI parity with the harness gate · P1 · `todo` · release-engineer (QA defines checks)
`.github/workflows/rust.yml` only runs build/test on `main`. Run `./harness/check.sh` on push/PR
to `main` and `develop`, add cargo cache, and a `--no-default-features` job.

### BL-005 · Performance baseline · P1 · `todo` · performance-engineer
Pick 10 representative expressions (arithmetic, string, array lambda, JQ, JSON vars, financial)
and record eval latency (lib, CLI, HTTP) in `benches/BASELINE.md`. All perf items depend on this.

### BL-006 · Fix false or broken documentation · P1 · `todo` · docs-engineer
- `Cargo.toml` `include` lists `DAEMON_DEPLOYMENT_GUIDE.md`, `DAEMON_FLAG_GUIDE.md` (deleted).
- `docs/README.md` links `performance-optimization.prd.md` (missing).
- `docs/DEVELOPMENT_GUIDE.md` claims lexer string interning and AST memory pooling — not true in code.
- "100x+ speed increase" claim has no reproducible benchmark (re-measure after BL-005).
- `README.md` and `benches/benchmark_phase1.sh` still claim string interning and memory pooling
  (the code for them was deleted in BL-002).

### BL-007 · Rustdoc for the public API · P2 · `todo` · docs-engineer
Every `pub fn` in `src/lib.rs` gets docs + runnable doctest; fix `parse` docstring ("arithmetic").

### BL-008 · De-duplicate `lib.rs` entry points · P2 · `todo` · rust-engineer
`evaluate_with_json` and `evaluate_with_json_custom` copy the same JSON→vars block. Extract one
private helper. Consider an `Engine`/builder API via ADR (keep old fns as thin wrappers).

### BL-009 · Consolidate docs into `docs/` tree · P2 · `todo` · docs-engineer
11 root-level `.md` files overlap (`ALPINE_*` ×3, `DOCUMENTATION.md` vs `API_REFERENCE.md`
vs `docs/DEVELOPMENT_GUIDE.md`). Move to layout in `harness/roles/docs-engineer.md`.

### BL-010 · Generate the function reference from code · P2 · `todo` · rust-engineer + docs-engineer
Single table of builtins (name, category, arity, summary) in code → drives dispatch *and* a
generated `docs/reference/functions.md`. Removes doc drift and the string-`HashSet` dispatch.

### BL-011 · `Value::Json(String)` → structured value · P2 · `todo` · language-designer (ADR, supersedes ADR-0004) → rust-engineer
Nested objects are re-serialized/re-parsed on every access. Measure first (BL-005).

### BL-012 · Cache parsed ASTs in servers · P2 · `todo` · performance-engineer → rust-engineer
Servers re-parse identical expressions per request; `Rc<Expr>` blocks sharing across threads
(ADR-0002). Evaluate `Arc` or per-thread LRU of `Expr`.

### BL-013 · Restore Windows builds · P3 · `todo` · release-engineer + rust-engineer
Gate `libc` daemon code behind `#[cfg(unix)]` (`src/bin/http_server/daemon.rs`, `sk_server.rs`).
Re-add Windows target to `release-binaries.yml`.

### BL-014 · Toolchain modernization · P3 · `todo` · release-engineer (needs ADR)
Rust 1.81 → current stable, edition 2024, `lazy_static`/`once_cell` → `std::sync::LazyLock`,
`cargo update` + `cargo outdated` review.

### BL-015 · Repo hygiene · P3 · `todo` · docs-engineer
Remove or relocate `test_js_upload.py`, `test_large_request.py`, `performance_results/*.log`,
`SETUP_COMPLETE.md`; check `.DS_Store`, `dist/`, `.env` are ignored.

### BL-016 · Structured error kinds · P3 · `todo` · language-designer (ADR) → rust-engineer
`Error` is `{message, position}` only; callers can't match on error type. Propose an `ErrorKind`
enum without breaking `Error::new`.

### BL-017 · Stack overflow on deeply nested input (DoS) · P1 · `done` · security-reviewer → rust-engineer
`sk "((((…20000×…1"` aborts with `fatal runtime error: stack overflow` (reproduced 2026-11-20).
It's an abort, not a panic, so one HTTP request kills `sk_http_server` for everyone. Add a nesting
depth limit in `parser.rs` (and in evaluator recursion) that returns `Error`. QA adds
`regression_sec_deep_nesting`. Check `[[[[…` and `:a.b.c.…` chains too.

Brief (orchestrator, 2026-11-20):
  Goal: no input (expression text or JSON vars) can abort the process via stack overflow in the
        lexer, parser, evaluator, or AST drop; it returns `Error` instead.
  Branch: bugfix/bl-017-nesting-depth-limit. Two steps, sequential.
  Decision (orchestrator): one constant `MAX_NESTING_DEPTH = 256` (AST depth), no config knob.
        Error message: "Expression nested too deeply (max 256)". Real formulas are far below this.
  BL-017a · security-reviewer (Claude Opus 5.5) · `done`
    Do: enumerate every vector, write `tests/security_limits.rs` that runs the built `sk` binary as a
        subprocess (an abort kills the test process, so in-process tests can't pin the bug) and asserts
        it exits normally and prints an error, not "stack overflow"/signal. Cover at least: deep parens,
        nested arrays `[[[`, nested objects `{"a":{"a":`, unary chains `----1` / `!!!!`, right-assoc
        `2^2^2…`, ternary chains, long left-assoc binary chains `1+1+…` (100k terms, built iteratively but
        evaluated/dropped recursively), postfix chains `:a.b.c…` and `.to_s().to_s()…`, cast chains,
        nested function calls `ABS(ABS(…`, nested lambdas, deep JSON vars via `--json`.
        Also a positive test: depth 200 of each shape still evaluates.
        Mark tests that currently crash `#[ignore = "BL-017 crash"]` so the gate stays green; list them.
    Out of scope: src/ changes.
  REVISED after 017a (orchestrator, 2026-11-20):
    - Limit is 128, not 256: debug parens overflow ~250 on the 8 MB main thread, and 128 matches
      serde_json's existing cap, so text and JSON inputs share one number. Message:
      "Expression nested too deeply (max 128)". Positive tests move from depth 200 to 128
      (objects/property stay at 100); this is a contract decision, not a weakened test.
    - "Depth" = parser recursion levels (parens add no AST node, so AST depth alone isn't enough)
      plus chain length in the iterative binary/postfix loops. Pick the simplest counter that
      bounds both parser stack and resulting AST depth.
    - Server worker threads: build both pools with `threadpool::Builder` and an explicit
      `thread_stack_size` of 16 MB (sk_http_server.rs:165, sk_server.rs:515), so the limit has
      headroom in debug builds on worker threads too.
    - Runtime-built deep values (`.reduce([:acc], 0)`, chained `:a := [:a]` sequences) bypass the
      parser: enforce the same 128 depth when the evaluator constructs an Array value from evaluated
      elements, returning the same error. Keep it O(1) per construction (e.g. track depth, don't
      re-walk), and report the overhead on one array-heavy expression.
  BL-017b · rust-engineer (Claude Opus 5.5) · `done`
    Do: enforce the limit at parse time (depth counter in recursive descent AND in the iterative
        binary/postfix loops so left-deep trees are bounded too). If 017a shows the evaluator or
        `Drop` can still overflow at depth 256 on a 2 MB thread, fix that too. Un-ignore 017a tests.
    Acceptance: ./harness/check.sh green; all security_limits tests pass un-ignored; no existing
        test changes; `sk "$(printf '(%.0s' {1..20000})1"` prints the error.
  Refs: ADR-0008 (servers), security-reviewer threat model.

Handoff (security-reviewer, Claude Opus 5.5, 2026-11-20):
  Did: added `tests/security_limits.rs` (tests only, no src/ changes): 20 hostile-input subprocess
       tests that run `sk` and assert no signal / no "stack overflow" plus "Error" in output, and 17
       positive tests (depth 200, or 100 where serde_json's 128 cap applies). 17 hostile tests are
       `#[ignore = "BL-017 crash"]` (all abort with SIGABRT today, debug AND release): deep_parens,
       deep_arrays, deep_objects, unary_minus_chain, unary_not_chain, power_chain, ternary_then_chain,
       ternary_else_chain, left_assoc_chain, property_chain, safe_nav_chain, method_chain,
       index_chain, cast_chain, nested_function_calls, nested_lambdas, runtime_deep_value.
       Not ignored (safe today, serde_json depth cap 128): deep_json_vars_{arrays,objects,assignment_path}.
  Evidence: `cargo test --test security_limits` 20 passed / 17 ignored; `-- --ignored` 17 failed, all
       "killed by a signal" (status 6). `./harness/check.sh --fast` OK.
       Debug crash depth on 8 MB main thread: parens/arrays/objects/ABS()/cast ~250, lambdas ~312,
       ternary ~925, unary/power/postfix/left-assoc ~1350 (release roughly 10x). Layer: parser for
       atom-recursive shapes (~14 frames per paren level); evaluator for chains; AST Drop ~35k (debug).
  Risks/Follow-ups for BL-017b:
    - 256 is NOT safe on a 2 MB thread (threadpool default in sk_http_server/sk_server) in debug:
      parens overflow at ~65 in debug, ~650 in release. Either cut the frames per level, count depth
      per frame more strictly, run eval on a thread with a larger stack, or use `stacker`.
    - runtime_deep_value: `[1,…].reduce([:acc], 0)` builds a 10k-deep Value from a flat AST, so a
      parse-time limit does not fix it. It needs a value-depth guard or iterative Value Drop/Clone/Debug.
    - Sizes: each argv string kept <= ~120 KB (Linux MAX_ARG_STRLEN 128 KB), so left-assoc uses 60k
      terms instead of the brief's 100k. `sk` has no stdin/file input.
    - Other DoS (not fixed, file as needed): exponential memory via `.reduce([:acc, :acc], 0)` (20
      items -> 1M elements, 175 MB measured; 26 items -> ~11 GB est.); each lambda iteration clones the whole variable
      map (`clone_variables`), so big JSON vars x big arrays costs O(n*m). REPT does not exist.
  ADR: none

Handoff (rust-engineer, Claude Opus 5.5, 2026-11-20) — BL-017b:
  Did: - `src/limits.rs`: `pub const MAX_NESTING_DEPTH = 128` (re-exported from `lib.rs`, additive API)
         and the error "Expression nested too deeply (max 128)".
       - Parser: internal fns return `(Expr, depth)` (AST depth: leaves/empty = 0, node = 1 + deepest
         child) so every accepted AST is <= 128 deep, including left-deep binary chains (one shared
         `parse_binary_level` loop) and postfix chains; plus a recursion counter on `parse_expr` and
         unary/power operands (parens add recursion, no node). Errors carry the parser position.
         Pure refactor otherwise (shared arg-list/suffix helpers); no other test changed.
       - Evaluator: results of array literals, function calls, method/safe-method calls and casts go
         through `check_value_depth` (walk stops after 128 levels; non-arrays cost one match), which
         covers `.reduce([:acc], 0)` and `:a := [:a]; …`.
       - `sk_http_server`/`sk_server` pools: `threadpool::Builder` with 16 MB `thread_stack_size`.
       - `tests/security_limits.rs`: 17 ignores removed; hostile tests assert "nested too deeply"
         (runtime_deep_value too, plus new runtime_deep_value_assignments); positives at 128 (objects/
         property 100; index 64+64). Lambdas use 127 levels: `lambda(n)` has AST depth n+1 because each
         level's `[1]` target sits under the call. Unit tests in `parser.rs` and `limits.rs`.
  Evidence: `./harness/check.sh` (full, strict) OK. `cargo test --test security_limits` 38 passed,
       0 ignored, debug and `--release`. `sk "$(printf '(%.0s' {1..20000})1"` prints
       "Error: Expression nested too deeply (max 128) at position 129" and exits normally (code 2).
       HTTP server (debug, 2 workers): parens/ABS 128 and lambda 127 evaluate; 20k parens -> 400.
       Overhead (release `sk`, whole process, median of 5, macOS; 10k-element array
       `.map([:x,[:x*2,:x]]).map([:x,:x]).map([:x]).filter(:x.length() > 0).length()`):
       before 19.4 / 17.4 ms, after 19.3 / 19.2 ms (two rounds) — within run-to-run noise;
       `.map(:x*2).filter(:x>10).sum()` 7.5 -> 7.1 ms.
       Stack headroom at depth 128: release survives full eval on a 1 MB stack (fails at 512 KB);
       debug needs 2-3 MB for the parser alone (~20 KB per paren level, measured), so 8 MB main
       thread and 16 MB workers are fine, 2 MB debug threads are not.
  Risks/Follow-ups:
       - Debug builds on default 2 MB threads (e.g. an embedder's `std::thread::spawn`, cargo test
         threads) still overflow on ~100+ nested parens. Release is fine. Fix if it matters: fold the
         six binary levels into one precedence-climbing loop and shrink `parse_atom`'s frame (~4 KB).
       - Values passed in through the Rust API (`evaluate_with(vars)`) are not depth-checked until
         wrapped; JSON vars are capped by serde_json (128).
       - Runtime nesting errors have no position (`None`).
       - BL-028 (docs) filed for the new limit/error.
  ADR: none (limit value decided by orchestrator in the revised plan; new pub const is additive).

Review (orchestrator, 2026-11-20): accepted. Verified in release: 20k parens, 50k unary minus and
  30k-term `1+1+…` all return the depth error; precedence spot checks unchanged; only
  tests/security_limits.rs edited; strict clippy clean (both feature sets). Parser refactor is
  bigger than briefed but covered by parity + full suite. Known gap → BL-029.

### BL-018 · QuickJS resource limits + constant-time token compare · P1 · `todo` · security-reviewer → rust-engineer
`src/js_plugin.rs` sets no memory limit or interrupt handler (an uploaded `while(true){}` pins a
worker forever). `src/bin/http_server/auth.rs` compares tokens with `!=`. Write the threat model
`docs/security/THREAT_MODEL.md` as part of this item.

### BL-019 · Fuzzing + `cargo audit` in CI · P2 · `todo` · security-reviewer + release-engineer
`cargo fuzz` target over `parse` and `evaluate_with_json` (PRD-003 FR-8); `cargo audit` job in CI.

### BL-020 · Excel-compatibility audit of shared function names · P2 · `todo` · language-designer
For each builtin that shares an Excel name: argument order and edge cases match, or the difference
is documented. Output is a table QA turns into tests. Also decide the canonical form
(`SUM(:a)` vs `:a.sum()`) and string → DateTime cast formats.

### BL-021 · `CHANGELOG.md` + release checklist dry run · P2 · `todo` · release-engineer
Create root `CHANGELOG.md` (Keep a Changelog) back-filled from git tags, and run
`cargo publish --dry-run` / `cargo package --list` (expected to catch the BL-006 `include` issue).

### BL-022 · GitHub branch protection for `main`/`develop` · P1 · `todo` · release-engineer (human applies)
Client-side hooks (ADR-0009) can be bypassed. Require PR review and a passing CI check on
`main` and `develop`; block force-push. Depends on BL-004 for the CI check.

### BL-023 · `NULL == NULL` evaluates to `false` · P2 · `todo` · language-designer → rust-engineer
Found by BL-001a parity tests (public API). Decide intended semantics (likely `true`; Excel has no
null, SQL says unknown) and the rule for `NULL != NULL`; ADR if it changes results.

### BL-024 · `JQ` unwraps single matches to a scalar · P3 · `todo` · language-designer
`JQ({"a":[3]}, "$.a[*]")` returns `3`, but two matches return an array, so
`.map(JQ(:x, "$.a[*]").sum())` fails on rows with one element. Decide whether wildcard paths
always return arrays; breaking change needs an ADR.

### BL-025 · Binaries don't build with `--no-default-features` · P2 · `todo` · release-engineer + rust-engineer
`src/bin/*` import `JSPluginLoader`/`js_plugin` unconditionally. Gate those imports on
`#[cfg(feature = "plugins")]` (preferred: servers still work without JS) or set
`required-features = ["plugins"]` per bin. Then extend `harness/check.sh` to build all targets without plugins.

### BL-026 · `PERCENTILE` accepts NaN as the percentile · P3 · `todo` · language-designer → rust-engineer
`src/runtime/statistical.rs` range check uses `p < 0.0 || p > 1.0`, which lets NaN through (kept by
BL-003 with `#[allow(clippy::manual_range_contains)]`). Decide: error on NaN (likely), then use
`!(0.0..=1.0).contains(&p)` and drop the allow.

### BL-027 · Exponential value growth and evaluation budget · P1 · `in-progress` · security-reviewer → language-designer → rust-engineer
Found in BL-017a: `[1,…].reduce([:acc, :acc], 0)` doubles per item (20 items → 1M elements, 175 MB;
~26 items → ~11 GB). Needs a total-size/step budget per evaluation (and a server-side timeout).
Language Designer decides the limits and error; Rust implements; Security re-tests.
Related perf note: lambdas clone the whole variable map per item (`clone_variables`) — O(items × vars).

Raised to P1 by maintainer, 2026-11-20 (same DoS class as BL-017: one request can OOM a shared server).
Brief (orchestrator, 2026-11-20):
  Goal: no expression can make one evaluation allocate unbounded memory or run unbounded time;
        it returns an Error instead. Servers additionally bound wall-clock time per request.
  Branch: bugfix/bl-027-evaluation-budget (stacked on bugfix/bl-017-nesting-depth-limit, PR #44;
        retarget to develop after #44 merges).
  BL-027a · language-designer (Claude Opus 5.5) · `done`
    Do: write ADR-0011 (Proposed) + PRD-001 NFR rows defining the evaluation budget:
      - what is counted (e.g. total Value cells created per evaluation incl. string bytes; and/or max
        single array length / string length; lambda iteration steps), with concrete default numbers;
      - error text(s), consistent in style with "Expression nested too deeply (max 128)";
      - per-request wall-clock timeout for sk_http_server/sk_server (default, and whether it is a
        CLI flag/env var — only add a knob if a real deployment needs to tune it);
      - whether library users get a way to change limits (ponytail: constants unless justified).
      Base the numbers on evidence: run realistic formulas from tests/ and docs (largest JSON/array
      workloads in tests/sk_performance.rs, benches) to show they stay far below the limits, and show
      the attacks (`.reduce([:acc, :acc], 0)` on arrays; string doubling via CONCAT/`+` in REDUCE;
      `SUBSTITUTE`-style growth; huge ranges if any builtin creates arrays from numbers) exceed them.
      Do NOT run inputs that would allocate more than ~500 MB on this machine.
      Output: example table (expression → expected Value or Error) for QA/Rust.
    Out of scope: src/ changes.
  Decision (maintainer, 2026-11-20): ADR-0011 accepted with amendment: BL-030 first, then 027b
    without the variable-read charge (A12 must pass). A11 behaviour change accepted.
  BL-027b · rust-engineer · `ready` (BL-030 done)
  BL-027c · security-reviewer · `todo` (re-attack after 027b)

Handoff (language-designer, Claude Opus 5.5, 2026-11-20) — BL-027a:
  Did: - docs/adr/0011-evaluation-budget.md (Proposed) + README row; PRD-001 NFR-4…NFR-7 + open questions.
       - Decision: one deterministic, cumulative per-evaluation budget `MAX_EVAL_BYTES = 256 MiB` of
         Values created (cost = 32 per Value + string/Json bytes), charged at the check_value_depth
         sites plus object literal, property access, variable read (deep clone today), assignment and
         each lambda iteration; pre-charge in SUBSTITUTE(M)/.replace, JOIN/.join, SPLIT/.split, JQ
         matches and Json parsing. Thread-local budget, one walk shared with the depth check.
         Errors: "Evaluation budget exceeded (max 256 MB)", "Evaluation timed out (max 10 s)".
       - Servers: 10 s deadline checked every 256 charges inside the evaluator (no thread kill, no
         CLI flag/env var), HTTP 400 like other eval errors; LRU cache skips entries > 64 KiB.
       - Library: constants by default; additive `EvalLimits` + `with_limits(limits, f)` (nested
         scopes can only tighten) — justified by servers needing a deadline and in-process datasets
         beyond the 1 MB HTTP cap. No default deadline in the library / `sk`.
  Evidence: in-process measurements (release, counting allocator, scratch harness outside the repo):
       realistic max 23 ms / 46 MB allocated (100k nums, 4-step chain, ~40 MB charged by the model);
       10k-row filter/map 10 ms / 23 MB. Attacks: reduce([:acc,:acc]) 20 items 96 MB peak;
       MERGE doubling 20 items 80 MB (flat — depth limit does not catch it); CONCAT doubling 22 items;
       object doubling 16 items 41 MB; SUBSTITUTE 4000×4000 → 16 MB in one call; JQ "$..*" ~120×.
       Example table (P1–P12 pass, A1–A12 fail, T1–T2 timeout, C1–C6 cost-model boundaries) in ADR-0011.
       No src/ changes; no test changes.
  Risks/Follow-ups:
       - Behaviour change (documented in ADR): quadratic patterns at size fail — reduce(MERGE(:acc,[:x]))
         above ~2.9k items, `:rows.map(... :rows.length())` above ~2.3k rows. No existing test affected.
       - Biggest real DoS is NOT memory: lambdas clone the whole variable map per inner method call.
         `:nums.map(:x.abs()).sum()` on 100k nums (589 KB, under the HTTP cap) = 14.6 s, 305 GB
         churn; 10k-row `:rows.map(:x.price.round(1))` = 1.7 s. Budget can't charge it without
         rejecting normal formulas; servers rely on the deadline. Filed BL-030 (P1).
       - Existing: cache keys embed Debug of all vars (≤1 MB each × 1000 entries) — covered by the
         64 KiB entry cap in ADR-0011 §4.
       - BL-018 interrupt handler should reuse the same thread-local deadline.
       - BL-028 (docs) should add the budget, timeout and the two rewrite patterns.
  ADR: ADR-0011 proposed (maintainer to accept before BL-027b).

### BL-028 · Document the nesting depth limit · P3 · `todo` · docs-engineer
BL-017b added `MAX_NESTING_DEPTH = 128` (`skillet::MAX_NESTING_DEPTH`) and the error
"Expression nested too deeply (max 128)". It applies to expression nesting (parens, brackets,
calls, unary/ternary, operator and `.`/`[]` chains) and to arrays built at runtime. Add it to
`DOCUMENTATION.md` / `API_REFERENCE.md` (limits section) and the HTTP error docs.

### BL-029 · Debug-build parser stack use (~20 KB per nesting level) · P3 · `todo` · performance-engineer → rust-engineer
After BL-017, depth 128 still overflows a default 2 MB thread in **debug** builds (release is fine
down to 1 MB). Affects embedders spawning threads in debug and in-process tests. Fix by folding the
binary precedence levels into one precedence-climbing loop and shrinking `parse_atom`'s frame;
measure stack per level before/after.

### BL-030 · Lambdas clone the whole variable map per call (CPU/alloc DoS) · P1 · `done` · rust-engineer (+ performance-engineer for numbers)
Found in BL-027a. Every method/function call inside a lambda runs `context.clone_variables()` and the
lambda helpers `base_vars.cloned()`, so cost is O(items × size of all vars). Measured (release):
`:nums.map(:x.abs()).sum()` over 100k numbers (589 KB JSON) = 14.6 s and 305 GB allocated;
10k rows `:rows.map(:x.price.round(1)).sum()` = 1.7 s / 10 GB (vs ~10 ms without the inner call).
Fix: a layered scope (borrowed parent map + small local bindings for `:x`/`:acc`) or shared values,
so lambda calls do not copy the parent environment. ADR-0011's server deadline covers this until fixed;
the library/CLI have no protection. Acceptance: both formulas above within 2× of their no-inner-call
equivalents; ./harness/check.sh green; no result changes.

Brief (orchestrator, 2026-11-20):
  Goal: evaluating inside lambdas (and method calls inside them) never copies the parent variable
        environment; variable reads are cheap enough that ADR-0011 does not need to charge them.
  Branch: bugfix/bl-030-shared-lambda-scope (stacked: develop ← #44 BL-017 ← bl-027 ADR commits ← this).
  Scope: src/runtime/evaluator.rs, src/runtime/method_calls/*, src/runtime/function_dispatch.rs or
        wherever `clone_variables` / `base_vars.cloned()` live. No public signature changes; no
        `Value` enum changes (that would be an ADR).
  Approach: Rust engineer's choice; simplest that meets the numbers (e.g. a layered scope: borrowed
        parent `&HashMap` + small local map for `:x`/`:acc`/custom param names, looked up local-first).
        Variable reads may still clone the *value* read; if `:rows.length()` per item stays slow
        because of that, avoid the clone for method-call targets (evaluate by reference).
  Acceptance:
    - ./harness/check.sh (full) green; evaluator_parity + security_limits unchanged and green.
    - Release, median of 5, same machine, before → after:
        `:nums.map(:x.abs()).sum()` (100k nums) — 14.6 s → within 2× of `:nums.map(:x * 1).sum()`;
        `:rows.map(:x.price.round(1)).sum()` (10k rows) — 1.7 s → within 2× of `:rows.map(:x.price).sum()`;
        `:rows.map(:x.price / :rows.length()).sum()` (10k rows, ADR-0011 A12) — 3.5 s → report.
      Put the harness that builds these inputs in benches/ (script or #[ignore] test) so BL-005 can reuse it.
    - Peak/total allocation for the first case reported if easy (counting allocator in a scratch bin is fine).

Handoff (rust-engineer, Claude Opus 5.5, 2026-11-20):
  Did: - benches/lambda_scope.rs + `[[bench]] harness = false` (std only, no new dependency):
         `cargo bench --bench lambda_scope` (RUNS=n optional) builds the 100k-nums / 10k-rows
         inputs in-process (vars shaped like evaluate_with_json_custom, incl. `arguments`), prints
         median/p95 and the result of each formula. Library layer; avoids the argv size limit.
       - `runtime::evaluator::Scope`: local bindings (item/acc) over a borrowed `&dyn EvaluationContext`,
         local-first lookup. Used by method lambdas, FILTER/MAP/FIND/REDUCE/SUMIF/AVGIF/COUNTIF and
         plain method arguments (`eval_arg`). Method/property targets that are plain variables are
         borrowed, not cloned (fixes the `:rows.length()` per-item copy). No `clone_variables` call
         left on any evaluation path; the 8 lambda methods (plain + _with_custom) collapse into 4.
       - Custom-registry visibility kept exactly: lambdas in methods see the caller's registry;
         plain method arguments and FILTER()-style builtin lambdas do not (as before).
  Evidence: ./harness/check.sh (full, strict) green; tests/ untouched (evaluator_parity +
         security_limits green). New unit tests in evaluator.rs: a context whose clone_variables()
         panics evaluates lambda/method/builtin cases (fails on the old code); shadowing and
         reduce same-name (acc wins) unchanged.
         Release, median of 5 (p95), MacBook 11 cores, macOS 27.0.1, rustc 1.81, `cargo bench --bench
         lambda_scope`; before = efaeed5, after = fix commit. Results identical before/after.
         | Formula                                        | before            | after          |
         |------------------------------------------------|-------------------|----------------|
         | `:nums.map(:x * 1).sum()` (100k, 700 KB)       | 6.02 (7.44) ms    | 2.82 (3.04) ms |
         | `:nums.map(:x.abs()).sum()`                    | 15444 (15675) ms  | 4.63 (5.24) ms |
         | `:rows.map(:x.price).sum()` (10k, 449 KB)      | 4.13 (4.72) ms    | 2.68 (2.88) ms |
         | `:rows.map(:x.price.round(1)).sum()`           | 2027 (2033) ms    | 3.12 (3.20) ms |
         | `:rows.map(:x.price / :rows.length()).sum()`   | 3996 (3997) ms    | 3.26 (3.38) ms |
         Acceptance: abs case 1.6x of `:x * 1`; round case 1.2x of `:x.price`; A12 now 3.3 ms.
         Total allocation, `:nums.map(:x.abs()).sum()` (counting allocator, scratch bin outside the
         repo): before 390 GB total / 18 MB peak, 15.9 s; after 3.8 MB total / 3.2 MB peak.
  Risks/Follow-ups:
       - `skillet::runtime::method_calls` is a pub module: sub-handler signatures changed
         (`exec_filter/map/find/reduce`, `exec_string_method`, `exec_array_method` now take
         `&dyn EvaluationContext`; `exec_*_with_custom` removed). `exec_method` /
         `exec_method_with_custom`, the `EvaluationContext` trait and everything in src/lib.rs are
         unchanged. Not part of the AGENTS §7 contract, but flag for the changelog.
       - Pre-existing inconsistency, kept as is (Language Designer to decide): custom functions are
         not visible in plain method arguments (`:s.contains(MYFN())`) nor in FILTER()/MAP()-style
         builtin lambdas, but are visible in `.map()`/`.filter()` lambdas.
       - Variable reads elsewhere still clone the value (`SUM(:rows)`, `:rows[0]`); only method and
         property targets are borrowed. ADR-0011 amendment (no read charge) holds for A12.
       - Flaky gate note: tests/sk_concurrency.rs runs `cargo run --release` in parallel; if the release
         `sk` is stale, concurrent rebuilds make 1–2 runs fail. `cargo build --release --bin sk`
         first avoids it (QA item worth filing).
       - BL-027b is unblocked.
  ADR: none

Review (orchestrator, 2026-11-20): accepted. Re-ran `cargo bench --bench lambda_scope` (RUNS=3):
  all five formulas 2.6–5.0 ms (abs 15.4 s → 5 ms; A12 4.0 s → 3.2 ms); results identical; tests/
  untouched; no remaining clone_variables() call sites. Changelog note: `runtime::method_calls`
  handler signatures changed (`&dyn EvaluationContext`), `exec_*_with_custom` removed; lib.rs API
  unchanged. Follow-ups: BL-031, BL-032.

### BL-031 · Custom functions not visible in method args and builtin lambdas · P3 · `todo` · language-designer
Found in BL-030 (pre-existing, preserved): custom functions work inside `.map()/.filter()` lambdas but
not in plain method arguments (`:s.contains(MYFN())`) or in `FILTER()/MAP()`-style builtin lambdas.
Decide whether that is intended; likely not (one evaluator, one function namespace).

### BL-032 · `sk_concurrency` flakes when the release `sk` is stale · P2 · `todo` · qa-engineer
`tests/sk_concurrency.rs` runs `cargo run --release` from parallel tests; if the release binary is out
of date they rebuild concurrently and some fail. Build once up front (or use `CARGO_BIN_EXE_*`).
