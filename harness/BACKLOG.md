# Backlog

Single source of truth for work. Statuses: `todo` → `ready` (has brief) → `in-progress (<agent>)`
→ `review` → `done`, or `closed` (won't fix). Priority P1 (now) · P2 (next) · P3 (later).
Only the Orchestrator reorders.

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

### BL-004 · CI parity with the harness gate · P1 · `done` (PR #50)· release-engineer (QA defines checks)
`.github/workflows/rust.yml` only runs build/test on `main`. Run `./harness/check.sh` on push/PR
to `main` and `develop`, add cargo cache, and a `--no-default-features` job.

### BL-005 · Performance baseline · P1 · `done` (PR #48) · performance-engineer
Pick 10 representative expressions (arithmetic, string, array lambda, JQ, JSON vars, financial)
and record eval latency (lib, CLI, HTTP) in `benches/BASELINE.md`. All perf items depend on this.

### BL-006 · Fix false or broken documentation · P1 · `done` (PR #49) · docs-engineer
- `Cargo.toml` `include` lists `DAEMON_DEPLOYMENT_GUIDE.md`, `DAEMON_FLAG_GUIDE.md` (deleted).
- `docs/README.md` links `performance-optimization.prd.md` (missing).
- `docs/DEVELOPMENT_GUIDE.md` claims lexer string interning and AST memory pooling — not true in code.
- "100x+ speed increase" claim has no reproducible benchmark (re-measure after BL-005).
- `README.md` and `benches/benchmark_phase1.sh` still claim string interning and memory pooling
  (the code for them was deleted in BL-002).

### BL-007 · Rustdoc for the public API · P2 · `done` (PR #60)· docs-engineer
Every `pub fn` in `src/lib.rs` gets docs + runnable doctest; fix `parse` docstring ("arithmetic").

### BL-008 · De-duplicate `lib.rs` entry points · P2 · `done` (PR #47) · rust-engineer
`evaluate_with_json` and `evaluate_with_json_custom` copy the same JSON→vars block. Extract one
private helper. Consider an `Engine`/builder API via ADR (keep old fns as thin wrappers).

### BL-009 · Consolidate docs into `docs/` tree · P2 · `done` (PR #49) · docs-engineer
11 root-level `.md` files overlap (`ALPINE_*` ×3, `DOCUMENTATION.md` vs `API_REFERENCE.md`
vs `docs/DEVELOPMENT_GUIDE.md`). Move to layout in `harness/roles/docs-engineer.md`.

### BL-010 · Generate the function reference from code · P2 · `done` (PR #58)· rust-engineer + docs-engineer
Single table of builtins (name, category, arity, summary) in code → drives dispatch *and* a
generated `docs/reference/functions.md`. Removes doc drift and the string-`HashSet` dispatch.

### BL-011 · `Value::Json(String)` → structured value · P2 · `ready` (ADR-0030 Accepted, PR #104; implement after the current queue, in its 6–7 steps, with BL-036) · language-designer (ADR, supersedes ADR-0004) → rust-engineer
Nested objects are re-serialized/re-parsed on every access. Measure first (BL-005).

### BL-012 · Cache parsed ASTs in servers · P2 · `done` (PR #80, ADR-0021 Accepted; prototype measured < 2% except on 1.8 KB expressions, +41% req/s there on 1 connection)· performance-engineer → rust-engineer
Servers re-parse identical expressions per request; `Rc<Expr>` blocks sharing across threads
(ADR-0002). Evaluate `Arc` or per-thread LRU of `Expr`.

### BL-013 · Restore Windows builds · P3 · `todo` · release-engineer + rust-engineer
Gate `libc` daemon code behind `#[cfg(unix)]` (`src/bin/http_server/daemon.rs`, `sk_server.rs`).
Re-add Windows target to `release.yml`.

### BL-014 · Toolchain modernization · P3 · `done` (PR #110, ADR-0032: edition 2024, `LazyLock`, MSRV 1.85 kept) · release-engineer (needs ADR)
Rust 1.81 → current stable, edition 2024, `lazy_static`/`once_cell` → `std::sync::LazyLock`,
`cargo update` + `cargo outdated` review.

### BL-015 · Repo hygiene · P3 · `done` (PR #49) · docs-engineer
Remove or relocate `test_js_upload.py`, `test_large_request.py`, `performance_results/*.log`,
`SETUP_COMPLETE.md`; check `.DS_Store`, `dist/`, `.env` are ignored.

### BL-016 · Structured error kinds · P3 · `ready` (planned after the current lanes) · language-designer (ADR) → rust-engineer
Audit 2026-11-22 adds: Excel error tags (`#VALUE!`, `#NUM!`, `#DIV/0!`) are typed by hand into
~150 messages and applied unevenly (LANG-9); no `IFERROR`/`ISERROR` exists; ~250 test
assertions pin exact message text in two styles (QA-6). The ADR should cover all three.

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

### BL-018 · QuickJS resource limits + constant-time token compare · P1 · `done` (tokens PR #51; threat model and ADR-0017 Accepted PR #73; QuickJS limits PR #79)· security-reviewer → rust-engineer
`src/js_plugin.rs` sets no memory limit or interrupt handler (an uploaded `while(true){}` pins a
worker forever). `src/bin/http_server/auth.rs` compares tokens with `!=`. Write the threat model
`docs/security/THREAT_MODEL.md` as part of this item.

### BL-019 · Fuzzing + `cargo audit` in CI · P2 · `done` (cargo audit PR #50; fuzzing PR #98, ADR-0028: `fuzz/` crate, weekly `fuzz.yml`, findings F-1 and F-2 fixed) · security-reviewer + release-engineer
`cargo fuzz` target over `parse` and `evaluate_with_json` (PRD-003 FR-8); `cargo audit` job in CI.

### BL-020 · Excel-compatibility audit of shared function names · P2 · `done` (audit in PR #56, docs/reference/excel-compat.md; decisions D-1…D-10 open)· language-designer
For each builtin that shares an Excel name: argument order and edge cases match, or the difference
is documented. Output is a table QA turns into tests. Also decide the canonical form
(`SUM(:a)` vs `:a.sum()`) and string → DateTime cast formats.

### BL-021 · `CHANGELOG.md` + release checklist dry run · P2 · `done` (PR #50)· release-engineer
Create root `CHANGELOG.md` (Keep a Changelog) back-filled from git tags, and run
`cargo publish --dry-run` / `cargo package --list` (expected to catch the BL-006 `include` issue).

### BL-022 · GitHub branch protection for `main`/`develop` · P1 · `done` (ruleset "protected branchews", 2026-11-21: `main`, `develop`, `release/*`; PR required, no force-push or deletion, no bypass; required checks: harness gate, cargo audit, MSRV build, branch must be up to date; plus CodeQL, code quality, Copilot review) · release-engineer (human applies)
Client-side hooks (ADR-0009) can be bypassed. Require PR review and a passing CI check on
`main` and `develop`; block force-push. Depends on BL-004 for the CI check.

### BL-023 · `NULL == NULL` evaluates to `false` · P2 · `done` (ADR-0013 Accepted, option A, PR #77; implemented in PR #82)· language-designer → rust-engineer
Found by BL-001a parity tests (public API). Decide intended semantics (likely `true`; Excel has no
null, SQL says unknown) and the rule for `NULL != NULL`; ADR if it changes results.

### BL-024 · `JQ` unwraps single matches to a scalar · P3 · `done` (ADR-0014 Accepted, option B, PR #77; implemented in PR #76)· language-designer
`JQ({"a":[3]}, "$.a[*]")` returns `3`, but two matches return an array, so
`.map(JQ(:x, "$.a[*]").sum())` fails on rows with one element. Decide whether wildcard paths
always return arrays; breaking change needs an ADR.

### BL-025 · Binaries don't build with `--no-default-features` · P2 · `done` (PR #50)· release-engineer + rust-engineer
`src/bin/*` import `JSPluginLoader`/`js_plugin` unconditionally. Gate those imports on
`#[cfg(feature = "plugins")]` (preferred: servers still work without JS) or set
`required-features = ["plugins"]` per bin. Then extend `harness/check.sh` to build all targets without plugins.

### BL-026 · `PERCENTILE` accepts NaN as the percentile · P3 · `done` (ADR-0015 Accepted, option B, PR #77; implemented in PR #84; NaN crash fixed in PR #59)· language-designer → rust-engineer
`src/runtime/statistical.rs` range check uses `p < 0.0 || p > 1.0`, which lets NaN through (kept by
BL-003 with `#[allow(clippy::manual_range_contains)]`). Decide: error on NaN (likely), then use
`!(0.0..=1.0).contains(&p)` and drop the allow.

### BL-027 · Exponential value growth and evaluation budget · P1 · `done` · security-reviewer → language-designer → rust-engineer
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
  BL-027b · rust-engineer (Claude Opus 5.5) · `done` · Branch: bugfix/bl-027-budget-impl (from develop after #45)
    Spec: docs/adr/0011-evaluation-budget.md as accepted, INCLUDING the amendment at the top
          (variable reads are not charged; A12 must pass). The ADR's site table, pre-charge list,
          error texts, deadline mechanism, cache cap and `with_limits` API are the contract.
    Tests: new tests/eval_budget.rs implementing the ADR example table (P rows pass with exact
          Values, A rows return the budget error, C rows via `with_limits`). Attacks must stay cheap
          in the test (they fail early by design); keep each test < 1 s in debug.
          Server timeout (T1): one test against sk_http_server is enough; if it is slow, gate it with
          `#[ignore = "slow: server timeout"]` and list it.
    Perf: re-run `cargo bench --bench lambda_scope` before → after; overhead target ≤ 10%.
    Acceptance: ./harness/check.sh green; existing tests unchanged; bench table in handoff.
  BL-027c · security-reviewer (Claude Opus 5.5) · `done` · same branch (bugfix/bl-027-budget-impl)
    Do: re-attack the budget as built. Start from 027b's own leads (uncharged-read n² CPU in
        library/CLI, ~2× budget peak via CONCAT/MERGE post-charge, uncharged index/slice, thread
        handoff in custom fns, single slow builtins like SORT), then look for anything else
        (string methods, casts, JSON round-trips, cache, HTTP path, sk_server path).
        For each finding: severity, repro, measured time/peak (never > ~1 GB on this machine).
        Add regression tests for anything that is fixed or must stay fixed to tests/eval_budget.rs
        or tests/security_limits.rs; do not edit expectations of existing tests.
    Decide per finding: fix-now (blocks this PR; route back to rust-engineer) vs backlog item.
        Rule: anything that lets one HTTP request exceed ~2× MAX_EVAL_BYTES or run past the
        server deadline by more than ~2 s blocks the PR.

  BL-027d · rust-engineer (Claude Opus 5.5) · `done` · same branch
    Decision (maintainer, 2026-11-20): option A, recorded as ADR-0011 Amendment 2 — charge reads that
    copy, keep borrowed method/property targets free; tick the deadline on every read and binary op;
    pre-charge .to_a()/.to_json()/::String/CONCAT/MERGE; reject oversized serialized HTTP results.
    Acceptance: both ignored bl027c tests un-ignored and green; add a server test for F3; every P row
    in tests/eval_budget.rs still passes (A12 included); add a test pinning the new
    `:rows.map(SUM(:rows))` budget error on big input and the hoisted form passing;
    ./harness/check.sh green; lambda_scope bench overhead vs develop ≤ 10%.
    Found in BL-027c; BLOCKS this PR (brief rule: > ~2x MAX_EVAL_BYTES or > ~2 s past the deadline).
    F1 Reads never tick the deadline. `:s := <80 MB>; [:s == :s, … ×4000]` (42 KB request): binary
       ops and variable reads charge nothing, so 8000 full clones run before the one array charge.
       Release sk_http_server and sk_server: 20.4 s / 20.7 s, `success: true` (deadline 10 s);
       linear in N up to the 1 MB request cap (~8 min). Fix sketch: tick (charge 0 + clock-read
       counter) on every variable read and binary op, or make reads borrow (`Rc`/`Cow`).
    F2 Heap allocated before the budget check grows with expression length. Uncharged read clones
       feed charge-after sites: CONCAT(:s ×k) with `:s` = 80 MB measured 314/467/619/772 MB peak
       RSS for k = 1..4 (~+153 MB per arg, unbounded in k); `[:s ×k]`, `{…: :s ×k}`, MERGE likewise.
       Single amplifiers charged after allocating: `.to_a()` on a string (~47 real bytes per char:
       15 MB string -> 711 MB peak; 128 MB max input -> ~6 GB), `.to_json()` on control chars
       (6x: 80 MB -> 619 MB), `[s]::String` (Debug escaping 5x). Fix sketch: charge reads (or
       their size) when they are moved into an argument list/literal, and pre-charge `.to_a()`,
       `.to_json()`, `::String`, CONCAT, MERGE (upper bound before allocating).
    F3 HTTP response serialization is outside the budget. A 40 MB result of `\x01` chars becomes a
       240 MB JSON body (`\u0001`); sk_http_server peak RSS 1061 MB for one request (plain 20 MB
       string: 374 MB). Max in-budget result (~128 MB) -> ~3 GB. Fix sketch: cap the serialized
       response (e.g. stream or reject results whose escaped size > N MB) or count it in the budget.
    Tests (ignored until fixed, un-ignore when done): tests/eval_budget.rs
       `bl027c_flat_uncharged_reads_meet_the_deadline` (F1),
       `bl027c_peak_heap_stays_within_twice_the_budget` (F2, per-thread counting allocator, 4 MiB
       budget: CONCAT 40 reads 131.6 MB, array literal 40.1, object literal 66.8, MERGE 75.2,
       to_a 47.2, to_json 13.9, ::String 13.0). F3 needs a server test (none yet; add with the fix).

  BL-027e · security-reviewer · `done`
  BL-027f · rust-engineer (Claude Opus 5.5) · `done` · same branch · Found in BL-027e; BLOCKS this PR (same rule).
    F4 Object literals serialize their values after the reads are charged and before the result
       is checked; JSON escaping grows control characters 6x. `:s := <20 MB \x01>; {"k0": :s, …}`
       release: 1 key 199 MB, 4 keys 599 MB (sk_http_server 602 MB, sk_server 600 MB, 120 KB
       request), 6 keys 866 MB; ~+133 MB per key until the reads reach the budget (~2 GB).
       Arrays as values (`{"k": [:s]}`) likewise. Fix sketch: pre-charge the escaped size of the
       object (count with `serde_json::to_writer` into `ByteCounter`, as `.to_json()` on arrays)
       before `serde_json::to_string` in `Expr::ObjectLiteral`.
       Test: tests/eval_budget.rs `bl027e_object_literal_escaping_is_charged_before_serializing`
       (ignored, un-ignore when fixed; today 51.5 MB / 26.9 MB peak on a 4 MiB budget).
    F5 sk_server response for nested arrays: inner arrays are sent as their `Debug` text, built
       in full and then JSON-escaped by `serde_json::to_string` (growing buffer). The F3 sizer
       accepts it (bound is correct) but memory is ~2.8x: `[[<40 MB \x01>]]` 657 MB,
       `[[<44 MB \x01>]]` 722 MB (sk_http_server same value: 353 MB). Bounded (sizer caps it),
       but over the ~2x rule. Fix sketch: build sk_server's response with `to_json_exact` and
       write the Debug text straight into the escaper (or count it lower). No in-process test
       possible (bin code); repro with the release server, 1 worker.

Handoff (rust-engineer, Claude Opus 5.5, 2026-11-20) — BL-027f:
  Did: - F4: `Expr::ObjectLiteral` pre-charges the exact serialized size (new
         `limits::charge_json_text`: serde_json into `ByteCounter`, no allocation; `.to_json()` on
         arrays now uses the same helper) before `serde_json::to_string`. That charge is the
         result's cost, so the result is not charged again by `check_value` (Json has no depth).
         Arrays as values are covered (they are part of the counted tree). The literal moved to an
         out-of-line helper `eval_object_literal` (inline it measured +6% on `:nums.map(:x * 1)`).
       - F5: sk_server's `result` is a small `Serialize` type (`Wire`): arrays nested in an array
         are written as their `Debug` text through `serializer.collect_str`, i.e. formatted
         straight into serde_json's escaper, never built as a String; the response is streamed
         to the socket via `BufWriter` (no response copy in memory at all). Deviation from the
         brief: streaming instead of `to_json_exact` (that lives in http_server/utils.rs with the
         buffer-pool deps; streaming needs no shared module and holds 0 instead of 240 MB). The
         F3 sizer (response_size.rs, nested_debug=true) still rejects oversized results first.
         Wire format unchanged: 18 requests (nested arrays with control chars/quotes/unicode,
         output_json, DateTime both modes, Json, empty array, errors, bad JSON) diffed byte for
         byte against the previous release build: identical. The serialize-failure fallback line
         is gone (unreachable: serde_json::Value and Debug text cannot fail; IO errors end the
         connection as before).
       - Other JSON-text serialization inside evaluation (grep `serde_json::to_string|to_writer|
         to_value` in src/runtime, src/lib.rs, src/limits.rs):
         | Site | Before | Now |
         | evaluator.rs `Expr::ObjectLiteral` | charged after (F4) | pre-charged exact (fixed) |
         | conversion_methods.rs `.to_json()` String | pre-charged exact (027d) | unchanged |
         | conversion_methods.rs `.to_json()` Array | pre-charged exact (027d) | same, via helper |
         | conversion_methods.rs `.to_json()` Number/Bool/Currency/DateTime | tiny, constant | unchanged |
         | jsonpath.rs `json_to_value` Object (JQ match) | charged by parts (unescaped, up to 6x low; `$..*` on nested objects re-serializes every level) | `charge_json_node` charges an object at least 32 + its serialized size (fixed, new test) |
         | lib.rs `json_to_value` Object (property access `:j.a`, `.dig`, JSON(), method_calls/mod.rs) | input charged by `charge_json_parse` (text length); a re-serialized sub-object is never longer than its source text | unchanged (bounded 1x per parse) |
         | lib.rs / sk.rs / sk_client / http_server js_management | outside evaluation (inputs, CLI output, admin replies) | unchanged |
  Evidence: `cargo build --release --bin sk` + ./harness/check.sh (full, strict) green (==> OK).
         `cargo test --test eval_budget`: 31 passed, 1 ignored (T1 slow server timeout).
         `bl027e_object_literal_escaping_is_charged_before_serializing` un-ignored, passes (fails
         on the previous code: 51.5 / 26.9 MB peak); new `bl027f_jq_object_matches_are_charged_as_json_text`
         (fails on the previous code: 21.3 MB peak on a 4 MiB budget; now budget error < 9 MiB).
         No existing expectations changed.
         Release spot checks, before = a738bee, after = this branch; sk via /usr/bin/time -l,
         servers 1 worker on 127.0.0.1, peak RSS by ps sampling, 2 runs each (same results):
         | Case                                         | before                 | after                  |
         |----------------------------------------------|------------------------|------------------------|
         | F4 sk: `{"k0": :s, …×4}`, :s = 20 MB \x01     | 0.11-0.25 s, 629 MB, budget err | 0.17-0.33 s, 148 MB, budget err |
         | F4 sk_http_server, same                       | 0.11-0.15 s, 633 MB, budget err | 0.21 s, 150 MB, budget err |
         | F4 sk_server, same                            | 0.14-0.15 s, 631 MB, budget err | 0.21-0.22 s, 149 MB, budget err |
         | F4 one key (passes): sk / http / sk_server    | 209 / 233 / 351 MB, ok | 209 / 233 / 231 MB, ok |
         | F5 sk_server: `[[40 MB \x01]]` (240 MB body)  | 1.36-1.41 s, 770 MB, ok | 1.42-1.46 s, 129 MB, ok |
         | F5 sk_http_server, same (reference)           | 0.23 s, 371 MB, ok     | 0.20-0.27 s, 370 MB, ok |
         F4 is ~0.07 s slower when it fails: the counting pass makes many small writes; it then
         stops before allocating 480 MB.
         Bench `RUNS=5` lambda_scope, release, 15 interleaved rounds develop (bf7c20a) / before
         (a738bee) / after, median of per-round medians (load ~5.5-9, syspolicyd at 100%):
         | Formula                                      | develop | before  | after   | vs develop | vs before |
         |----------------------------------------------|---------|---------|---------|------------|-----------|
         | `:nums.map(:x * 1).sum()` (100k)             | 2.00 ms | 2.13 ms | 2.13 ms | +6%        | +0%       |
         | `:nums.map(:x.abs()).sum()`                  | 4.01 ms | 4.46 ms | 4.50 ms | +12%       | +1%       |
         | `:rows.map(:x.price).sum()` (10k)            | 2.62 ms | 2.77 ms | 2.76 ms | +5%        | -0%       |
         | `:rows.map(:x.price.round(1)).sum()`         | 3.03 ms | 3.23 ms | 3.21 ms | +6%        | -1%       |
         | `:rows.map(:x.price / :rows.length()).sum()` | 3.23 ms | 3.53 ms | 3.53 ms | +9%        | +0%       |
         BL-027f adds 0-1%. `:x.abs()` is +12% vs develop on this machine today, but the branch
         head before this item measured the same (+11%; BL-027d recorded +9% at load ~5): the
         overhead is BL-027d's per-op tick, not this change.
  Risks/Follow-ups: - `:x.abs()` row slightly over the 10% bench target vs develop (pre-existing,
         see above); orchestrator to decide whether a quieter re-measure is needed.
       - BL-033 (sk_server read timeout / line cap) unchanged.
       - Docs (BL-028): nothing new user-visible (object literals just fail earlier on escapes).
  ADR: none (implements ADR-0011 amendment 2)

Handoff (security-reviewer, Claude Opus 5.5, 2026-11-20) — BL-027e:
  Did: confirmation pass on release builds (sk via /usr/bin/time -l; sk_http_server and
       sk_server on 127.0.0.1, 1 worker, peak RSS by ps sampling); every run < 1 GB.
       tests/eval_budget.rs: `bl027e_reads_that_escape_into_a_result_are_charged` (ternary, IF,
       spread, nested/sequence assignment, map, reduce, index, slice, method, `.to_s()`,
       `::String`; all budget error, peak < 2x), `bl027e_borrowed_operands_do_not_copy`, and the
       ignored F4 test above.
  Results (before = BL-027c; after = this branch, release):
       | Case | before | after | rule |
       | F1 http 80 MB `[:s == :s ×4000]` | 20.4 s, ok | 6.2 s, 163 MB, ok | pass |
       | F1 sk_server same | 20.7 s, ok | 6.2 s, 162 MB, ok | pass |
       | F1 both ×16000 | ~80 s | 10.3 s, ~166 MB, timed out | pass |
       | F1 sk 20 MB ×2000 | 2.6 s, 89 MB | 0.77 s, 46 MB, ok | pass |
       | F2 CONCAT(:s ×4) 80 MB (sk / http / sk_server) | 772 MB | 160 / 162 / 161 MB, budget err | pass |
       | F2 `[:s ×4]`, `{…: :s ×4}` (plain chars) 80 MB | 488 MB | 160 MB, budget err | pass |
       | F2 MERGE(:a ×8), 1M items | — | 331 MB, budget err | pass |
       | F2 `.to_a()` 15 MB | 711 MB | 36 MB, budget err | pass |
       | F2 `.to_json()` 80 MB \x01 | 619 MB | 160 MB, budget err | pass |
       | F2 `[40 MB \x01]::String` / `.to_s()` | 249 MB | 122 / 160 MB, budget err | pass |
       | F2 `:s.to_s()` / `:s::String` ×4 80 MB | — | 236 / 160 MB, budget err | pass |
       | index `[:a[0] ×20]` / slice `[:a[0:4] ×5]` | uncharged | 122 MB, budget err | pass |
       | ternary / IF / spread / assignments / map / reduce / `.first()` | — | 160 MB, budget err | pass |
       | F3 40 MB \x01 result (http / sk_server) | 1061 MB | 315 / 314 MB, ok | pass |
       | F3 46 MB \x01 result | serialized | 53 / 52 MB, budget err | pass |
       | F3 include_variables 2×30 MB \x01 / output_json 40 MB / Json obj 30 MB (http) | — | 125 MB err / 353 MB ok / 268 MB ok | pass |
       | F3 6.5M 1-char strings (SPLIT "") | — | 314 MB, budget err | pass |
       | F3 http `[[40 MB \x01]]` | — | 353 MB, ok | pass |
       | F5 sk_server `[[40 / 44 MB \x01]]` | — | 657 / 722 MB, ok | FAIL (>2x) |
       | F4 `{k: <20 MB \x01>}` ×4 (sk / http / sk_server), ×6 (sk) | — | 599 / 602 / 600, 866 MB | FAIL (>2x) |
  Borrow paths: binary/unary ops only return Number/Boolean, so a borrowed operand never
       escapes; method/property/index/slice results go through `check_value`; every other read
       (`Expr::Variable`: ternary/IF branches, spread, args to builtins, custom and JS functions,
       `:arguments`, assignment values, lambda results) is charged before the copy.
  Sockets (58cc5b7): needed for correctness. On Linux accepted sockets were already blocking,
       so nothing changes there; on macOS it makes the pre-existing idle-client hold reachable.
       Measured, 1 worker, idle client that sends nothing: sk_http_server serves the next
       client after 29.7 s (30 s per-read timeout; a client trickling a byte every < 30 s holds
       it indefinitely -> BL-037); sk_server never serves it (no read timeout, BL-033).
       Not worse than Linux today; BL-033 stays P1.
  Deviations (027d handoff): borrowed reads tick via the operation using them (verified: F1
       times out at 10.3 s); MERGE/CONCAT prepaid then depth-checked (charge_merge counts every
       arg >= result; a custom MERGE/CONCAT is charged normally); `.to_s()` on arrays
       pre-charged; T1 workload change; F3 upper bound rejects slightly early; blocking accepted
       sockets. All acceptable: each errs towards charging more, or is a test-only change.
  Verdict: block. F4 (High) and F5 (Medium, bounded ~2.8x) exceed the ~2x rule -> BL-027f.
       F1, F2, F3 closed.
  Evidence: `cargo test --test eval_budget bl027e -- --include-ignored`: 2 pass, F4 fails as
       expected; ./harness/check.sh --fast green.
  Risks/Follow-ups: BL-027f (blocking), BL-033 (P1, unchanged), BL-037 filed.
  ADR: none

Handoff (rust-engineer, Claude Opus 5.5, 2026-11-20) — BL-027d:
  Did: - F1: every unary/binary op ticks the deadline (zero-byte charge); a copying variable read
         is charged cost(value) before the clone (that charge ticks too). Operator operands and
         method/property/index/slice targets that are plain variables are BORROWED (`Cow`), so
         `:s == :s` no longer clones `:s` at all; a borrowed read relies on the tick/charge of the
         operation using it (one tick per use, see deviations).
       - F2: copying reads charged (ADR amendment 2); index/slice results charged (L3). Pre-charged
         before allocating: CONCAT (exact length, counted with a no-alloc fmt pass), MERGE (cost of
         its args; result then only depth-checked, not charged twice), `.to_a()` on strings
         (32 + 32/char + bytes), `.to_json()` on strings (exact escaped size) and arrays
         (serde_json to a byte counter first), `.to_s()` and `::String` on arrays (exact
         formatted length, counted first).
       - F3: src/bin/http_server/response_size.rs (shared by both servers via #[path]): upper bound
         on the escaped JSON size of the result (+ returned variables), no allocation; over
         MAX_EVAL_BYTES -> "Evaluation budget exceeded (max 256 MB)" (HTTP 400 / success:false).
         sk_http_server also builds the response JSON at its exact size and writes headers and
         body separately (no extra copies).
       - Found while spot-checking (pre-existing, fixed, small): both servers poll a non-blocking
         listener and macOS/BSD accepted sockets inherit O_NONBLOCK, so sk_server dropped
         requests whose line had not fully arrived and sk_http_server truncated big responses.
         Accepted sockets are set blocking. This is very likely the cause of BL-034 (240 MB
         responses now arrive complete) -> QA to confirm and close BL-034.
       - tests/eval_budget.rs: un-ignored `bl027c_flat_uncharged_reads_meet_the_deadline` and
         `bl027c_peak_heap_stays_within_twice_the_budget` (both pass); added
         `f3_servers_reject_responses_larger_than_the_budget` (both servers, 127.0.0.1, ~1.4 s, not
         ignored) and `copying_a_large_variable_per_item_fails_hoisting_passes`
         (`:nums.map(SUM(:nums))` 5k -> budget error; `:t := SUM(:nums); :nums.map(:t)` and
         `:nums.map(:nums.sum())` pass). Server helpers factored into a `Server` struct.
  Evidence: ./harness/check.sh (full, strict) green (==> OK); `cargo test --test eval_budget`
         27 passed, 1 ignored (T1 slow server timeout; passes with --ignored in 10.3 s).
         All P/A/C rows unchanged and green (A12 included). A11 boundary is now between 2,800
         and 2,900 items (ADR model ~2,900; BL-027b had ~4,090): copying `:acc` is charged.
         Bench `RUNS=5 cargo bench --bench lambda_scope`, release, same machine (MacBook, 11
         cores, load ~5), 9 interleaved rounds develop (bf7c20a) / BL-027b head (9ce4c5c) / after,
         median of the per-round medians:
         | Formula                                      | develop | BL-027b | after   | vs develop |
         |----------------------------------------------|---------|---------|---------|------------|
         | `:nums.map(:x * 1).sum()` (100k)             | 2.02 ms | 2.12 ms | 2.13 ms | +5%        |
         | `:nums.map(:x.abs()).sum()`                  | 4.13 ms | 4.37 ms | 4.49 ms | +9%        |
         | `:rows.map(:x.price).sum()` (10k)            | 2.64 ms | 2.76 ms | 2.77 ms | +5%        |
         | `:rows.map(:x.price.round(1)).sum()`         | 3.04 ms | 3.16 ms | 3.25 ms | +7%        |
         | `:rows.map(:x.price / :rows.length()).sum()` | 3.28 ms | 3.35 ms | 3.55 ms | +8%        |
         (results identical. A first version that also ticked inside every borrowed read measured
         +10..14%; the extra tick was dropped in 28b9825.)
         Release spot checks, before = BL-027b head (9ce4c5c), after = this branch.
         `sk` via /usr/bin/time -l; servers 1 worker, peak RSS by ps sampling:
         | Case                                              | before               | after                       |
         |---------------------------------------------------|----------------------|-----------------------------|
         | F1 sk: 20 MB `:s`, `[:s == :s ×2000]`              | 2.62 s, 89 MB, ok    | 0.75 s, 49 MB, ok           |
         | F1 sk_server: 80 MB, ×4000                         | 22.4 s, 315 MB, ok   | 6.3 s, 162 MB, ok (no copy) |
         | F1 sk_http_server: 80 MB, ×4000                    | 20.4 s (027c), ok    | 6.2 s, 164 MB, ok           |
         | F1 both servers: 80 MB, ×16000                     | ~80 s (linear)       | 10.0 / 10.2 s, timed out    |
         | F2 CONCAT of 4 reads of 80 MB (sk)                 | 0.11 s, 808 MB       | 0.01 s, 168 MB              |
         | F2 `[:s ×4]` 80 MB (sk)                            | 0.04 s, 488 MB       | 0.01 s, 168 MB              |
         | F2 `.to_a()` on 15 MB (sk)                         | 0.28 s, 745 MB       | 0.00 s, 23 MB               |
         | F2 `.to_json()` on 80 MB of \x01 (sk)              | 0.11 s, 649 MB       | 0.02 s, 88 MB               |
         | F2 `[40 MB \x01]::String` (sk)                     | 0.46 s, 249 MB       | 0.26 s, 48 MB               |
         | F3 http: 40 MB \x01 result (240 MB body)           | 545 MB, body truncated (1061 MB in 027c) | 315 MB, 240 MB delivered |
         | F3 http / sk_server: 46 MB \x01 (276 MB escaped)   | serialized           | 0.1 s, 54 / 53 MB, budget error |
         | `:n.map(LENGTH(:n))`, 20k items (sk)               | 5.88 s, ok           | 0.12 s, budget error        |
         | hoisted `:t := LENGTH(:n); :n.map(:t)` (sk)        | 0.00 s, ok           | 0.00 s, ok                  |
         All budget-error rows return `Evaluation budget exceeded (max 256 MB)` in both columns
         except where marked ok.
  Deviations / readings of Amendment 2:
       - "Every variable read ticks": a copying read ticks through its cost charge; a borrowed read
         (operand/target) does not make its own tick, the operation using it does (operator tick,
         method/property/index result charge). Net: one tick per use, never zero; F1 test passes.
       - MERGE and CONCAT results are pre-charged and then only depth-checked (not charged twice).
         A custom function named MERGE/CONCAT is charged as a normal function result.
       - `.to_s()` on arrays pre-charged too (same amplification class as `::String`; not listed).
       - T1 test workload changed (expectations unchanged): `:nums.map(SUM(:nums))` now fails with
         the budget error in ~0.1 s by design, so T1 uses 1M comparisons of two borrowed 400 KB
         strings to run past 10 s.
       - F3 size is an upper bound (numbers 26 bytes, Json text + 32/node); a result just under the
         budget may be rejected slightly early; a 40 MB control-char result still passes.
  Risks/Follow-ups:
       - sk_server still has no read timeout and no line cap (BL-033, unchanged).
       - BL-034 likely fixed by the blocking-socket change; QA to confirm with its repro and close.
       - BL-036 (shared values) would let copying reads be dropped again.
       - Docs (BL-028): copying-read charge and the hoisting pattern, new A11 boundary, server
         response-size rejection.
  ADR: none (implements ADR-0011 amendment 2)

Handoff (security-reviewer, Claude Opus 5.5, 2026-11-20) — BL-027c:
  Did: - Re-attacked the budget on release builds (sk, in-process probe, sk_http_server and
         sk_server on 127.0.0.1, 1 worker each); every run < ~1.1 GB peak.
       - tests/eval_budget.rs: per-thread counting allocator + 4 BL-027c tests. Passing (pin what
         holds): lambda over uncharged reads meets a 50 ms deadline; SPLIT/SUBSTITUTE/JOIN
         pre-charges peak < 2x budget. Ignored, open (BL-027d): F1 flat reads vs deadline, F2 peak.
  Findings (peak RSS via /usr/bin/time -l or ps sampling; release):
       | # | Finding | Sev | Peak / time | Blocks | Tracking |
       | F1 | uncharged reads never tick the deadline (`[:s == :s ×4000]`) | High | 317 MB, 20.4 s HTTP / 20.7 s sk_server, success | yes | BL-027d, test F1 |
       | F2 | k uncharged reads into CONCAT/MERGE/literals; `.to_a()`/`.to_json()`/`::String` charged after | High | CONCAT k=4: 772 MB (+153 MB/arg); to_a 15 MB: 711 MB; to_json 80 MB: 619 MB | yes | BL-027d, test F2 |
       | F3 | HTTP response JSON escaping outside budget | High | 40 MB `\x01` result: 1061 MB server RSS | yes | BL-027d |
       | L1 | (027b lead 1) `:nums.map(SUM(:nums))` O(n^2) CPU | Low | 10k: 0.32 s, 8 MB; server deadline holds (T1) | no | test lambda/deadline |
       | L3 | (lead 3) index/slice results uncharged | — | same class as F1/F2 reads (slices are array-only) | via F1/F2 | BL-027d |
       | L4 | (lead 4) custom fn work on another thread escapes the budget | Low | library-only; no HTTP path | no | BL-035 (docs) |
       | L5 | (lead 5) one slow builtin runs to completion | Low | SORT/.sort/UNIQUE of 3M items: 0.11-0.12 s, 320 MB | no | BL-035 |
       | N1 | sk_server reads request lines unbounded (no size cap, no read timeout) | High (pre-existing) | 300 MB line -> 308 MB RSS, grows with input | no (not this PR's code) | BL-033 |
       | N2 | sk_http_server truncates large responses (Content-Length 20 MB, 327 KB delivered) | Low | — | no | BL-034 |
       | N3 | cost model undercounts tiny strings (33 charged vs ~48 real per 1-char string); UPPER of U+0390 is 3x, charged after | Low | SPLIT "" 10 MB: 361 MB; UPPER 64 MB: 253 MB | no | BL-035 |
       Checked and fine: cache key (`Debug` of vars, bounded by the 1 MB request), cache stores
       only <= 64 KiB entries, budget/timeout errors are not cached, timed-out HTTP worker serves
       the next request (T1), errors return 400 with the limit text and no internal paths.
  Evidence: `cargo test --test eval_budget` 23 passed, 3 ignored (T1 slow + 2 open);
         `--include-ignored bl027c` shows the two open tests failing with the numbers above.
         ./harness/check.sh --fast green (strict clippy + fmt).
  Risks/Follow-ups: BL-027d (blocking), BL-033, BL-034, BL-035 filed. F1/F2 conflict with the ADR
         amendment ("reads are not charged"); a zero-byte tick on reads keeps A12 passing, a size
         charge on reads moved into arg lists may not — language-designer to decide, may need an
         ADR-0011 amendment.
  ADR: none (ADR-0011 amendment may be needed for F2)

Handoff (rust-engineer, Claude Opus 5.5, 2026-11-20) — BL-027b:
  Did: - src/limits.rs: thread-local budget (used/max/deadline/ticks + sticky trip); `check_value`
         replaces `check_value_depth` (one walk: depth + cost, stops early); every runtime
         `eval*` entry point opens a default scope or joins the active one and returns the
         recorded limit error even when the evaluation swallowed it (SUMIF, custom fn catching
         a nested evaluate). Pub API: `MAX_EVAL_BYTES`, `#[non_exhaustive] EvalLimits`,
         `with_limits` (nested only tightens; inner charges count to the outer scope).
       - Charge sites per ADR table (minus variable read): array/object literal, function,
         method, safe-method, cast, property/safe-property, assignment, every lambda iteration
         (FILTER/MAP/FIND/REDUCE/SUMIF/AVGIF/COUNTIF and .filter/.map/.find/.reduce).
         Pre-charge: SUBSTITUTE/SUBSTITUTEM/.replace (exact length), JOIN/.join, SPLIT/.split
         (per part), JQ (per match), every Json parse (property access, object literal, DIG,
         .keys/.values/.has_key/.dig, .to_json, JQ input).
       - Servers: `with_limits(default + 10 s)` around every evaluation (sk_http_server cache
         miss path, sk_server); cache stores only entries <= 64 KiB (key + result + vars).
       - tests/eval_budget.rs: P1-P12 (exact Values), A1-A12, C1-C6, T1 (server, ignored),
         swallowed-error and nested-evaluation cases. Each < 1 s in debug; suite 3.3 s.
  Evidence: ./harness/check.sh (full, strict) green; `cargo test --test eval_budget` 21 passed,
         1 ignored; T1 run with --ignored: passes in 10.5 s (400 + "Evaluation timed out (max 10 s)",
         single worker answers the next request). Existing tests/ untouched.
         Bench `RUNS=5 cargo bench --bench lambda_scope`, release, median of 5, before = HEAD
         worktree (bf7c20a) and after alternated 3x on the same machine (MacBook 11 cores):
         | Formula                                      | before  | after   | delta |
         |----------------------------------------------|---------|---------|-------|
         | `:nums.map(:x * 1).sum()` (100k)             | 2.02 ms | 2.11 ms | +4%   |
         | `:nums.map(:x.abs()).sum()`                  | 4.15 ms | 4.40 ms | +6%   |
         | `:rows.map(:x.price).sum()` (10k)            | 2.65 ms | 2.77 ms | +5%   |
         | `:rows.map(:x.price.round(1)).sum()`         | 3.05 ms | 3.20 ms | +5%   |
         | `:rows.map(:x.price / :rows.length()).sum()` | 3.30 ms | 3.35 ms | +2%   |
         (first after-run after a rebuild is noisier; results identical before/after.)
         Release `sk` spot check (/usr/bin/time -l): A1 `[1x26].reduce([:acc, :acc], 0)` budget
         error in 0.33 s, 209 MB max RSS; A2 CONCAT doubling 0.04 s / 540 MB; A3 MERGE doubling
         0.07 s / 540 MB; A4 object doubling 0.40 s / 393 MB; A5 assignment doubling 0.05 s /
         334 MB; A7 SUBSTITUTE 0.00 s / 7 MB; A10 1e9 iterations 0.05 s / 7 MB.
  Deviations / readings of the ADR (simplest reading that keeps every P row):
       - T1 formula `:nums.map(:x.abs()).sum()` no longer times out on the server: BL-030 made it
         ~5 ms. The server test uses `:nums.map(SUM(:nums)).sum()` (100k) instead: each
         `SUM(:nums)` clones the variable (reads are not charged), ~100k clones >> 10 s.
         C6 uses 4 chained `.map(:x.abs())` passes so a fast machine still runs > 1 ms.
       - P6: 121 matches with 121 nested objects (120 wrappers around `{"s": <100 KB>}`); the
         ADR's "120 nested objects" gives 121 only if the innermost one is counted as a wrapper.
       - A11 boundary moved: without the read charge, `reduce(MERGE(:acc, [:x]), [])` fails above
         ~4,090 items (ADR model: ~2,900). 10k still fails, 2,800 still passes (both tested).
       - Error text: "max N MB" only when the limit is a whole number of MiB, else "max N bytes"
         (ADR only defines < 1 MiB). Timeout: "N s" when whole seconds, else "N ms".
       - JQ match cost: Null/Bool/Number 32, String 32 + len, Array 32 + items, Object
         32 + sum(key len + value). The ADR says "cost of each match"; an object match becomes a
         Json string, whose length we estimate this way instead of serializing it.
       - Lambda items are charged per iteration (not once per array) on purpose: each charge is
         a deadline tick, so O(n^2) lambda bodies built only from uncharged reads still time out.
       - Charges outside any scope are no-ops (direct `Evaluator::eval` / `runtime::method_calls`
         callers bypass the budget; depth still applies). Every runtime `eval*` fn opens a scope.
       - `deadline_passed()` added (crate-private, unused yet) for BL-018's QuickJS interrupt.
       - `SERVER_EVAL_TIMEOUT` is defined in both binaries (sk_server has no shared module).
  Risks/Follow-ups (for BL-027c to re-attack):
       - Uncharged reads: `:nums.map(SUM(:nums))`, `:nums.map(:nums == :x)` are O(n^2) CPU with
         no memory growth. Bounded only by the server deadline; the library/`sk` have none.
         Making reads borrow (or Rc values) would fix it; out of scope here.
       - Peak heap can reach ~2x budget (CONCAT/MERGE doubling: 540 MB RSS) because the two
         argument reads are uncharged clones and CONCAT/MERGE are charged after allocating.
         Pre-charging CONCAT/MERGE would cut that; ADR lists them as charge-after.
       - Index/slice results (`:big[0:]`) are not charged (not in the ADR table; same class as reads).
       - Inside a scope, a single builtin (SORT of a huge variable) still runs to completion
         before the next deadline check.
       - Docs (BL-028): budget, error texts, `EvalLimits`/`with_limits`, server timeout, the two
         rewrite patterns, and the A11 behaviour change.
       - Changelog: `with_limits`/`EvalLimits`/`MAX_EVAL_BYTES` added; A11 behaviour change.
  ADR: none (implements ADR-0011)

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

Review (orchestrator, 2026-11-20): BL-027 accepted. 027b budget, 027c/e re-attacks, 027d/f fixes.
  All F1-F5 closed (F4 test + JQ test pass, fail on old code; F5 770 → 129 MB with byte-identical
  wire format). eval_budget 31 passed / 1 slow ignored; strict gate green.
  Perf vs develop (orchestrator re-run, 5 interleaved rounds, load ~6): +6% to +11%
  (`:x * 1` +10%, `:x.abs()` +11%). Slightly over the 10% target; accepted for a DoS fix
  (absolute +0.2–0.45 ms per 100k items), recovery filed as BL-038.
  Changelog "Added": MAX_EVAL_BYTES, EvalLimits, with_limits; "Changed": copying-read charge,
  A11 boundary, server 10 s deadline, 64 KiB cache cap, oversized-response rejection.

### BL-028 · Document the nesting depth limit · P3 · `done` (PR #49) · docs-engineer
BL-017b added `MAX_NESTING_DEPTH = 128` (`skillet::MAX_NESTING_DEPTH`) and the error
"Expression nested too deeply (max 128)". It applies to expression nesting (parens, brackets,
calls, unary/ternary, operator and `.`/`[]` chains) and to arrays built at runtime. Add it to
`DOCUMENTATION.md` / `API_REFERENCE.md` (limits section) and the HTTP error docs.

### BL-029 · Debug-build parser stack use (~20 KB per nesting level) · P3 · `done` (PR #48) · performance-engineer → rust-engineer
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

### BL-031 · Custom functions not visible in method args and builtin lambdas · P3 · `done` (ADR-0016 Accepted, option A, PR #77; implemented in PR #82)· language-designer
Found in BL-030 (pre-existing, preserved): custom functions work inside `.map()/.filter()` lambdas but
not in plain method arguments (`:s.contains(MYFN())`) or in `FILTER()/MAP()`-style builtin lambdas.
Decide whether that is intended; likely not (one evaluator, one function namespace).

### BL-032 · `sk_concurrency` flakes when the release `sk` is stale · P2 · `done` (PR #50)· qa-engineer
`tests/sk_concurrency.rs` runs `cargo run --release` from parallel tests; if the release binary is out
of date they rebuild concurrently and some fail. Build once up front (or use `CARGO_BIN_EXE_*`).

### BL-033 · sk_server reads request lines without a size cap or read timeout · P1 · `done` (PR #51) · rust-engineer
Found in BL-027c (pre-existing). `handle_client` uses `BufReader::lines()` on the raw socket: one
connection sending a 300 MB line without `\n` grew the process to 308 MB RSS (linear in input; no
cap like sk_http_server's 1 MB, no read timeout). One client can OOM the server. Fix: bounded line
reader (e.g. 1 MB, same as HTTP) + `set_read_timeout`; regression test with an oversized line.

### BL-034 · sk_http_server truncates large responses · P3 · `done` (PR #51) · rust-engineer
Found in BL-027c. A 20 MB string result: headers say `Content-Length: 20000086`, curl receives
~327 KB and the connection closes. Probably the write path/close sequence in
`http_server/utils.rs::send_http_response`. Not a memory issue; correctness for large results.

### BL-035 · Budget hardening, low-severity leftovers from BL-027c · P3 · `done` (docs PR #60; code PR #105, ADR-0011 amendment 3: strings cost 48 + len, case-mapping check, deadline ticks)· rust-engineer → docs-engineer
- A single builtin runs to completion before the next deadline check (SORT/UNIQUE of 3M items:
  ~0.12 s release; fine today, revisit if a superlinear builtin is added).
- Custom functions that move work to another thread escape the budget (document on
  `CustomFunction`/`with_limits`).
- Cost model undercounts tiny heap strings (charged 33 B, ~48 B real per 1-char string) and
  case mapping can grow 3x (`UPPER` of U+0390) after the charge; both stay < ~2x budget today.

### BL-036 · Share values instead of copying on variable reads · P2 · `ready` (ADR-0030 Accepted, PR #104; implement after the current queue, in its 6–7 steps, with BL-011) · language-designer (ADR) → rust-engineer → performance-engineer
Option B from the BL-027c decision. Reads deep-clone `Value` (e.g. `SUM(:rows)`, `CONCAT(:s, :s)`).
Making values shared (`Rc`/`Arc` inside `Value`, or a borrowed-read evaluation path) would make reads
O(1), remove the ADR-0011 Amendment 2 read charge, and speed up every formula over large variables.
Changes the core `Value` type, so it needs an ADR and a characterization pass first.

### BL-037 · sk_http_server: no total deadline for reading a request (slowloris) · P2 · `done` (PR #51) · rust-engineer
Found in BL-027e (pre-existing on Linux; reachable on macOS since 58cc5b7 made accepted sockets
blocking). The 30 s read timeout is per `read`, so a client sending one byte every < 30 s holds a
worker until it has sent 1 MB. Measured, 1 worker: an idle client delays the next request 29.7 s.
Fix: a total deadline for reading one request (e.g. 30 s from accept), checked between reads;
regression test with a trickling client. Same fix shape as BL-033 for sk_server.

### BL-038 · Recover evaluation-budget overhead (~10% on lambda-heavy formulas) · P3 · `done` (PR #64 (rows cases still ~+5%, see BL-036))· performance-engineer → rust-engineer
BL-027 added a per-operation deadline tick and charge calls; `lambda_scope` shows +6–11% vs
develop bf7c20a. Profile and trim (e.g. batch ticks per lambda iteration instead of per op, cheaper
fast path in `check_value`). BL-036 (shared values) would also remove the copying-read charge.

---
Parallel lanes, 2026-11-20 (orchestrator). Five worktrees, one PR each: #47 (BL-008), #48 (BL-005,
BL-029), #49 (BL-006, BL-028, BL-009, BL-015), #50 (BL-004, BL-032, BL-025, BL-019 audit, BL-021,
ADR-0012), #51 (BL-033, BL-037, BL-034, BL-018 tokens). Follow-ups found by the lanes:

### BL-039 · HTTP latency floor from the 1 ms accept-loop sleep · P2 · `done` (PR #54)· performance-engineer → rust-engineer
BL-005: every request (even `/health`, cached or not) costs ~1.25 ms because `sk_http_server`'s
non-blocking accept loop sleeps 1 ms when `accept` would block. Use a blocking accept (shutdown via
a wake-up connection or flag) or a much shorter wait; re-measure with `benches/baseline.rs`.

### BL-040 · Profile CLI start-up (~2 ms above process start) · P3 · `done` (profiled; ~1 ms is CoreFoundation via chrono → BL-055 decision)· performance-engineer
BL-005: one `sk` call is ~2.9 ms vs ~0.9 ms for `/usr/bin/true`. Suspect QuickJS runtime creation
and hooks loading. Profile before changing anything.

### BL-041 · `sk_http_bench` "Range operations" case always fails · P3 · `done` (PR #57)· qa-engineer
Uses `range(1,50).sum()`; there is no RANGE function. Fix or drop the case.

### BL-042 · Remove or feature-gate `scalar-doc` · P2 · `done` (PR #81, ADR-0020 option D: removed, `/docs` is static HTML) · release-engineer (dependency decision)
Most `cargo audit` warnings (anyhow, rand, chrono-tz, the fixed crossbeam-epoch) and the edition-2024
MSRV pressure come through `scalar-doc` → `tera`, used only for the `/docs` page.

### BL-043 · Bump `lru` 0.16 → 0.18+ (RUSTSEC-2026-0253, unsound `pop()`) · P2 · `done` (PR #57)· rust-engineer
Direct dependency; `http_server/cache.rs` calls `LruCache::pop()`. `Cargo.toml` change.

### BL-044 · Release workflows use floating `stable` without `--locked` · P2 · `done` (PR #53)· release-engineer
`release-binaries.yml`, `crate-release.yml`: pin the toolchain (ADR-0012) and build `--locked`.

### BL-045 · `documentation.html` read from the working directory at runtime · P3 · `done` (PR #54)· rust-engineer
`load_html_file` in `http_server/utils.rs`; installed binaries serve the fallback page. Use `include_str!`.

### BL-046 · sk_server: deadline resets on non-work lines · P2 · `done` (PR #62)· security-reviewer → rust-engineer
Lane 1 review: blank, invalid-JSON and wrong-token lines reset the 30 s deadline, so an
unauthenticated client can keep a worker by sending `\n` every 29 s (develop: forever by sending
nothing). Don't reset on non-work lines / close after auth failure; add idle and per-connection limits.

### BL-047 · No total deadline for writing responses (slow-read) · P3 · `done` (PR #62)· rust-engineer
Both servers: `WRITE_TIMEOUT` is per write syscall. Add a `DeadlineWriter` mirroring the reader.

### BL-048 · Empty token silently disables auth · P2 · `done` (PR #62)· rust-engineer
`SKILLET_AUTH_TOKEN=""` / `--token ""` / `--admin-token ""` give `Some("")`, so tokenless requests
pass while the server prints "Token auth: enabled". Reject empty tokens at start-up.

### BL-049 · sk_http_server: real 408/413 status codes, drain before close · P3 · `done` (PR #99) · rust-engineer
All read errors are sent as HTTP 400 (the code is only in the body); on 413 it closes with unread
input, so a TCP reset can drop the error. Reuse sk_server's bounded drain.

### BL-050 · Docs/tooling leftovers from lane 5 · P3 · `done` (PR #53, #60)· docs-engineer
`.gitignore` `*.prd.*` hides future docs; ADR-0002 consequences are stale after BL-001/BL-030 (add a
status note); bench scripts still mention copy-on-write / buffer pooling; `Dockerfile.alpine-builder`
builds only aarch64 (release-engineer to decide x86_64); `Cargo.toml` has no `repository` field;
document the 404 for JS endpoints in plugin-free builds and the server request limits (1 MB, 30 s).

---
Round 3, 2026-11-20 (orchestrator). Merged #53–#66. Follow-ups:

### BL-051 · Security review follow-ups for the JS plugin path · P1 · `done` (PRs #73, #74, #78, #79; merge #78 before #74, #79 with or after #73) · maintainer decisions → rust-engineer
Findings from the threat-model review (lane I, 2026-11-20), now published in
`docs/security/THREAT_MODEL.md` (§3 top risks). Decision-free parts landed in PR #63 (upload file
names, hook paths, JS result depth, registry lock, PID file). Maintainer decisions of 2026-11-21:
- No CPU or memory limit inside QuickJS, so looping or allocating hooks pinned workers; a cyclic
  or deeply nested hook result aborted the process. Fixed: 64 MiB heap, 256 KiB stack, request
  deadline or 10 s per call, bounded conversion (ADR-0017 option B, PR #79).
- The undocumented `httpGet` global ran `curl`: local file read, SSRF, option injection and
  unbounded blocking for any hook. Removed (ADR-0017 §6 option H1, PR #79).
- Development mode plus `Access-Control-Allow-Origin: *` let any web page drive a local server,
  JS upload included (§2.3). Fixed: CORS only when configured or a token is set, JSON-only bodies
  (415), 403 for browser admin requests without an admin token, `SKILLET_ADMIN_TOKEN`
  (ADR-0019, PR #78).
- The Docker image ran unauthenticated on `0.0.0.0` and printed tokens (§2.8). Fixed: refuses to
  start without both tokens, no token logging, pinned base images, Compose on `127.0.0.1`
  (PR #74; image not built yet, see BL-062).
- Threat model and ADR-0017 published (PR #73). Still open: BL-066.

### BL-052 · `sk_http_server` ignores `SKILLET_AUTH_TOKEN` · P1 · `done` (PR #69) · rust-engineer
`sk_server` reads it; `sk_http_server` only reads `--token`. Make both behave the same.

### BL-053 · Register the documented `BETWEEN` builtin · P2 · `done` (PR #68) · rust-engineer
Implemented in `logical.rs` and documented in api.md, but missing from the builtin table.

### BL-054 · Language decisions from the Excel audit and lane M · P2 · `done` (decisions: ADR-0018 and ADR-0013 to 0016 Accepted, PR #77; implementation PRs #82, #84, #83, merge in that order; JQ PR #76) · language-designer → maintainer
IPMT sign (existing tests pin the non-Excel sign), COUNTIF with a scalar/string criterion, `TIME`
Excel normalisation, dotted names (`STDEV.P`) don't parse, `COUNT` semantics, `IF` evaluates both
branches, ROUND negative digits, CEILING/FLOOR significance, MOD sign, DATE rollover, function vs
method parity, string → DateTime cast formats. See docs/reference/excel-compat.md D-1…D-10.

### BL-055 · CLI start-up: drop CoreFoundation (~1 ms per call) · P3 · `done` (PR #81, ADR-0020 options B + D; `sk "=1+1"` 4.55 → 3.29 ms; servers still link Foundation, BL-068) · performance-engineer (ADR) → maintainer
Linked through chrono → iana-time-zone; needs a dependency/feature decision (TODAY()/TIME() local time).
Related: release profile LTO/strip (~0.1 ms), plugin-free default build (~0.3 ms).

### BL-056 · Release follow-ups · P3 · `done` (PR #75, ADR-0022: one `vX.Y.Z` tag, `cross` and musl images pinned; actions by SHA and Alpine x86_64 moved to BL-065) · release-engineer (decisions)
Two release tags (`vX.Y.Z` vs `crate-vX.Y.Z`), unpinned `cross`, actions by branch ref, Alpine builder
aarch64 only.

### BL-057 · Small leftovers · P3 · `done` (PR #99; per-IP limit was ADR-0025, PR #87; the hooks folder stays as decided)
- `/health` eviction counter only counts replaced keys (cache.rs) — rust-engineer.
- HTTP buffer pool never gets its buffer back on success (utils.rs) — rust-engineer.
- `runtime::builtin_functions::exec_builtin` is public, unused, and behaves differently — deprecate (rust-engineer).
- Hooks dir is created in the working directory when missing (`sk` leaves a `hooks/` folder) — decision.
- `DeleteJSResponse` schema vs plugin-free 404 body; servers.md pooling example truncates large responses — docs-engineer.
- No per-IP connection limit with a fixed worker pool (residual risk, threat model) — decision.
- Re-baseline benches/BASELINE.md after #54 (lane S).

---
Round 4, 2026-11-21 (orchestrator). Maintainer decisions produced PRs #73–#84. Follow-ups:

### BL-058 · Lexer parses scientific notation wrong · P1 · `done` (PR #89) · language-designer → rust-engineer
`=1e3` evaluates to 1, silently. Decide whether `1e3`/`1.5E-2` are number literals (Excel accepts
them) or an error; never a silent wrong value. Regression test either way.

### BL-059 · DateTime comparison is broken · P1 · `done` (PR #88) · language-designer → rust-engineer
`DateTime == DateTime` is always false, and `<`/`>` on two dates is an error. Excel compares dates
as numbers. Fits with the date arithmetic from PR #83.

### BL-060 · `0^-1` parses as `^` followed by unary minus · P2 · `done` (PR #93: already matched Excel, now pinned by tests) · language-designer
Check how `^` followed by a unary minus parses against Excel precedence (ADR-0018 D-10, PR #82),
and that the result (an error under D-1) is the intended one.

### BL-061 · server_limits timing tests flake under heavy load · P3 · `done` (PR #100: test servers own their port) · qa-engineer
Seen twice at load average > 20: `bl033_*`, `bl034_*`, `bl046_*` in `tests/server_limits.rs`.
Make them robust to slow machines or mark them `--ignored` like the accept-floor test.

### BL-062 · CI does not build the Docker image · P3 · `done` (PR #92; the `docker image` job is not a required check yet) · release-engineer
PR #74 was not verified with a real build. Add `docker build` to CI plus a check that the image
exits 64 without `SKILLET_AUTH_TOKEN`/`SKILLET_ADMIN_TOKEN`.

### BL-063 · `scripts/daemon_manager.sh` drives a Compose service that doesn't exist · P3 · `done` (PR #91) · release-engineer
It uses `skillet-server`; the Compose service is `skillet-http`.

### BL-064 · `sk_server.rs` has fork/setsid `unsafe` outside the allowed files · P3 · `done` (PR #99) · rust-engineer
The rust-engineer rule lists the files allowed to contain `unsafe`. Merge the `sk_server` daemon
code with `http_server` daemonize, or allow it in the rule.

### BL-065 · Pin GitHub Actions by SHA; Alpine x86_64 builder · P3 · `done` (PRs #95, #101) · release-engineer
Left over from BL-056: actions use branch or tag refs, and `Dockerfile.alpine-builder` builds
aarch64 only.

### BL-066 · Maintainer decisions (2026-11-21) · P3 · `done` (PRs #86, #87, #88, #89; ADR-0023, 0024, 0025) · maintainer
- Hooks cannot override builtins (ADR-0024, #86); the relative `hooks` default stays.
- Optional per-IP connection limit, off by default (ADR-0025, #87).
- `"a" == "A"` stays `false`; `==` stays strict; `===` considered and dropped (ADR-0023, #88).
- Excel number coercion in arithmetic and scalar math, `10%-3` = -2.9 (ADR-0023, #89).

### BL-067 · Parse-cache follow-ups · P3 · `done` (PR #99) · rust-engineer
Report parse-cache stats in `/health`. `DELETE /cache` does not clear the parse cache (PR #80).

### BL-068 · Servers still link Foundation via ctrlc → dispatch2 · P3 · `closed` (maintainer: won't fix; it costs start-up time of long-running servers only) · performance-engineer
After PR #81 `sk` no longer links CoreFoundation, but the servers do through `ctrlc`. Start-up
only, so low priority.

## Round 5 (2026-11-21)

### BL-069 · Parser ignores trailing tokens · P2 · `done` (PR #93; also made the `AND`/`OR` operators work) · language-designer → rust-engineer
`=1 2` evaluates to `1`: `parse()` stops after one expression and drops the rest. Require end of
input (error with position), check every entry point, and list any test that relied on it.

### BL-070 · Coercion follow-ups from #89 · P3 · `done` (PR #97, ADR-0027; methods stay strict) · maintainer decision → rust-engineer
- `DATE(...) + NULL` and `DATE(...) + "1"` are errors; Excel returns a date.
- Currency in the arithmetic operators errors; builtins accept it.
- Lowercase methods and `DATEADD` intervals stay strict (`[1,"2"].sum()` ignores `"2"`).

### BL-071 · Refresh excel-compat.md after round 5 · P3 · `done` (PR #94) · docs-engineer
X-1, X-6, the ABS row and "errors over silent coercion" predate ADR-0023; mark decided items done.
Threat model §2.6 should mark "hooks shadow builtins" resolved by ADR-0024.

## Round 6 (2026-11-21)

Merged #91–#105. New items:

### BL-072 · Excel text builtins · P2 · `done` (PR #96) · rust-engineer
`LEFT`/`RIGHT`/`MID`/`REPLACE` error on a negative length or a start below 1; `SORT` sorts text
like Excel; `CONCAT` and `&` keep Currency and DateTime; `DB` (B-8), `UNIQUE` (B-7) and `REVERSE`
(B-11) fixed; `SUBSTITUTEM` deprecated in favour of `SUBSTITUTE`.

### BL-073 · Excel logic: `NOT` keyword, short-circuit operators, strict `IF` · P2 · `done` (PR #102, ADR-0029) · language-designer → rust-engineer
`NOT` prefix keyword; `AND`/`OR`/`&&`/`||` short-circuit; `IF`/`IFS` conditions as in ADR-0018
D-8 (B-18); the `AND`/`OR`/`NOT`/`XOR` builtins follow Excel.

### BL-074 · Date units follow Excel · P2 · `done` (PR #103, B-15) · rust-engineer
`DATEADD`/`DATEDIFF` use VBA's unit codes (`"m"` is months, `"n"` minutes); new `DATEDIF`;
`DATEADD` clamps to the end of the month and gives `#NUM!` for huge amounts.

### BL-075 · Configurable server evaluation limits · P2 · `done` (PR #106, ADR-0011 amendment 4) · rust-engineer
Let operators set the servers' evaluation deadline and budget. Defaults (revised by the maintainer):
20 s and 256 MiB; Docker runs 4 threads.

### BL-076 · ISO 8601 date text and `SUM` with Currency · P2 · `done` (PR #108, ADR-0031; follow-ups in BL-078) · rust-engineer
Read ISO 8601 date text where a date is expected, and let `SUM` take Currency (B-13).

### BL-077 · `arguments` key collision and `/reload-hooks` leftovers · P3 · `done` (PR #111) · rust-engineer
No decision needed. From the threat model:
- 2.2: a JSON key named `arguments` overwrites the reserved `:arguments` variable that
  `sk_http_server` builds for `JQ` (`http_server/eval.rs`). Keep the reserved variable.
- 2.5: `/reload-hooks` adds and replaces functions but never removes those whose file was
  deleted from disk. Drop them on reload.

### BL-078 · Follow-ups from ADR-0031 (#108) · P3 · `done` (PR #112, ADR-0031 amendment 1; decisions as recommended; Currency parts 3 and 5 superseded by BL-097) · language-designer → rust-engineer
Recommendations, accepted by the maintainer:
1. Allow chained casts: `DATE(...)::String::DateTime` (today a parse error; needs parentheses).
2. `.to_json()` on a date stays the ISO text (the `::Json` cast still gives a number).
3. `.join()` accepts Currency and NULL like `JOIN`.
4. `LEN` and `FIND` on a date stay errors.
5. `SUMIF` and `AVERAGEIF` return Currency when a summed value is Currency, like `SUM`.

### BL-079 · Refresh `Cargo.lock` within semver ranges · P3 · `todo` · release-engineer
From BL-014: `cargo update` churns the lock heavily (syn 2.0.119 plus syn 3, r-efi 6, shlex 2,
regex 1.13, new stacker/psm). Do it in its own PR, gated on CI's `cargo audit` and MSRV 1.85 jobs.
Also decide when rustfmt moves to `style_edition = "2024"` (a fmt-only commit, touches most files).

---

Items BL-080 to BL-094 come from the audit of PRs #73-#109 (2026-11-22, five read-only role
reviews). Maintainer decisions of 2026-11-22 are recorded on each item.

### BL-080 · Criteria wildcard matching ignores the deadline · P1 · `done` (PR #116) · security-reviewer → rust-engineer
SEC-1. The `*`/`?` matcher behind `COUNTIF`/`SUMIF`/`AVERAGEIF` (`src/runtime/criteria.rs`) is
quadratic and never checks the evaluation deadline, so one request can run far past
`--eval-timeout`. Fix: a linear-time wildcard matcher and `tick_item` per row; regression test
that the deadline is honoured. No decision needed.

### BL-081 · Lambda criteria pick up an outer `:x` · P1 · `done` (PR #116) · rust-engineer
PERF-3. A lambda criterion is first evaluated once against the outer variables; when those define
`x`, a number result becomes a fixed criterion (`COUNTIF([5,5,1], :x)` with `x=5` gives 2). Also
stop cloning every item (+18-37% on `COUNTIF`). No decision needed.

### BL-082 · Compare Currency with numbers by amount · P1 · `done` (PR #115, ADR-0027 amendment 1; superseded by BL-097, #119) · rust-engineer
LANG-1. Since #108 `SUM(10::Currency, 5)` is Currency, but ordering Currency against a Number is an
error (ADR-0027), so `SUM(...) > 10` fails and `SUM(...) == 15` is silently false.
Decision (maintainer): `== != < <= > >=` compare Currency with Number by amount (amends ADR-0027).

### BL-083 · Make 0.7.0 releasable · P1 · `in-progress` (back-merge moves to the release branch; maintainer settings pending) · release-engineer (+ maintainer settings)
REL-1, REL-2, REL-5, SEC-7. `main` and `develop` conflict in 20 files; `release.yml` and `fuzz.yml`
are not on `main`, so the weekly fuzz run never fired and no release dry run is possible; any `v*`
tag publishes to crates.io.
Decision (maintainer): back-merge `main` into `develop` keeping develop's side, through a PR merged
with a merge commit (not squash), after checking every main commit's content is on develop.
Maintainer adds a `v*` tag ruleset and a protected `crates-io` environment with a required reviewer.
Release-engineer workflow part (from the back-merge lane): add `environment: crates-io` to the
`crates-io` job in `release.yml` and move `CRATES_IO_TOKEN` into that environment (deployment rule:
tags `v*`); add `pull_request: branches: [main]` so the release PR is the dry run. Until the release
PR reaches `main`, `fuzz.yml`'s schedule does not fire and the old `release-binaries.yml` on `main`
still runs on any `v*` tag. Cosmetic: rename the ruleset "protected branchews".

Status 2026-11-22: #114 and #117 were both squash-merged, so each became an ordinary one-parent commit
and `main` is still not an ancestor of `develop`. New plan: when `release/0.7.0` is cut from
`develop`, merge `main` into it with `-s ours` (the 24 main-only commits were checked in #114; check
`git log release/0.7.0..origin/main` again first). The release PR into `main` is then conflict-free
whatever merge method is used.

Settings for the maintainer (GitHub → Settings → Rules → Rulesets, and Settings → Environments):
1. New branch ruleset "main merge commits only":
   - Enforcement: Active. Bypass list: empty.
   - Target branches: include `main` only (`refs/heads/main`).
   - Rules: "Require a pull request before merging" with **Allowed merge methods: Merge** only
     (untick Squash and Rebase). Keep "Block force pushes" and "Restrict deletions".
   - The existing "protected branchews" ruleset stays as it is for `develop` and `release/*`, where
     feature PRs keep squash. When two rulesets apply to `main`, GitHub applies the stricter
     merge-method list, so `main` accepts merge commits only. Optional: rename the old ruleset
     "protected branches".
2. New tag ruleset "release tags":
   - Enforcement: Active. Bypass list: Repository admin (the maintainer) only.
   - Target tags: include `v*` (`refs/tags/v*`).
   - Rules: Restrict creations, Restrict updates, Restrict deletions, Block force pushes.
3. New environment `crates-io` (Settings → Environments → New environment):
   - Required reviewers: the maintainer. Prevent self-review: off (single maintainer).
   - Deployment branches and tags: "Selected branches and tags", add the tag rule `v*`.
   - Environment secret `CRATES_IO_TOKEN`: add it here, then delete the repository-level secret of the
     same name after the release-engineer change below is merged.
   - Optional: an environment `release` with the same tag rule for the `github-release` job
     (`contents: write`).
4. Release-engineer change, separate PR, before the release branch is cut: `environment: crates-io` on
   the `crates-io` job in `release.yml`, and `pull_request: branches: [main]` so the release PR is
   the dry run (the publish jobs stay gated on `refs/tags/v*`).
5. After 0.7.0 is tagged: the back-merge `main` → `develop` PR must use "Create a merge commit".
   It is the only PR into `develop` that must not be squashed.

### BL-084 · Docs catch up with what merged · P2 · `todo` · docs-engineer
- REL-3: migration guide misses silent changes: `[1,2]==[1,2]` false→true (also objects, Currency,
  dates), `FALSE<TRUE` false→true, `SUBSTITUTE` with an instance, `CONCAT` with Currency, `DB`,
  `SORT(…,-1)`, hooks can't use builtin names (#86). Mark #88 BREAKING.
- REL-7: CHANGELOG credits the unmerged #84; relative links break in release notes. Done in the
  round 10 sync: #84 credits moved to #83, links absolute, `**BREAKING:**` spelled one way, the
  stale 10 s line and `Token::Ampersand` fixed. Still open: order Changed newest first, group the
  BREAKING items, shorten the 0.7.0 body, edit "Planned as 0.7.0" at release time.
- SEC-4: threat model is stale (10 s timeout, 400 read errors, CI image, pinning) and contradicts
  itself.
- LANG-11, LANG-10: excel-compat lists bugs #96 fixed; X-3's stated reason is wrong.
- LANG-4: write the ADR for the date units (B-15): VBA codes in `DATEADD`/`DATEDIFF`, Excel codes in
  `DATEDIF`, as the maintainer decided.
- PERF-6: refresh `benches/BASELINE.md`; record that the +10-18% parse cost from #82 is accepted.
- REL-10: AGENTS.md, README and role-file drift.

### BL-085 · Result cache serves stale results · P2 · `in-progress` (lane cache) · rust-engineer
SEC-2. The HTTP result cache returns old values for volatile builtins (`NOW()`, `TODAY()`, `RAND`)
and drops `variables` when `include_variables` differs.
Decision (maintainer): never cache results that use a volatile builtin; add `include_variables` to
the key. Custom-function results stay cached (reload clears the cache since BL-077).

### BL-086 · HTTP JSON variables bypass the budget · P2 · `in-progress` (lane budget) · rust-engineer
SEC-3. With `--max-eval-bytes 1M`, a 0.75 MB body used 37 MB and succeeded; the banner understates
the worst case.
Decision (maintainer): charge the parsed variables to the evaluation budget; fix the banner.
Migration-guide row (some large requests now fail with the budget error).

### BL-087 · Test safety nets · P2 · `in-progress` (lane safety) · qa-engineer
- QA-1: give `evaluator_parity` expected values (it compares a function with itself; 10 results
  changed with no failing test).
- QA-2: run the 10 `#[ignore]` tests (408, read/write deadlines, ~100 s) in a scheduled CI job.
- QA-3: run `cargo test --no-default-features` in the gate.
- QA-4: finish BL-061 in `eval_budget.rs`, `js_limits.rs`, `server_pid_file.rs`; clear `SKILLET_*`
  env vars for test servers.
- QA-7, QA-8: `--fast` skips the 10 s JS test; loosen tight wall-clock bounds.

### BL-088 · Per-item costs on big arrays · P2 · `ready` · performance-engineer → rust-engineer
- PERF-1: `ROUND()` is 3.6x slower (formats and re-parses every number) and disagrees with
  `.round()` (`ROUND(1.005,2)` = 1.01, `1.005.round(2)` = 1.0). Fast path with a tie-only fallback.
  Decision (maintainer): `.round()` follows `ROUND`.
- PERF-2: `AND`/`OR`/`XOR` allocate a `Vec<bool>` per call (+18-24% in filters).
- PERF-4: date to text is 2x slower (chrono re-parses the pattern; `.to_s()`/`CONCAT` format twice).

### BL-089 · One truth rule for conditions · P2 · `in-progress` (spec done on feature/bl-089-truth-rule; implementation after #116) · language-designer (ADR-0029 amendment) → rust-engineer
LANG-2, LANG-3, QA-9. A NULL flag (e.g. from `&.`) gets different answers by form: `AND()` TRUE
(NULL skipped), the `AND`/`&&` operators an error, `IF` FALSE, `? :` an error, `NOT` TRUE but `!`
an error; `FALSE AND NULL` is false but `NULL AND FALSE` errors. `? :` rejects numbers that `IF`
accepts. `filter`/`find`/`FILTER` treat anything but TRUE as false silently, while `COUNTIF` errors
on the same lambda. `IF` ignores extra arguments.
Decision (maintainer, 2026-11-22, option A + keep): every condition uses IF's rule: TRUE/FALSE as
is, 0 false, other numbers true, NULL false, text `#VALUE!`. It applies to `IF`, `? :`, the
`AND`/`OR`/`NOT`/`!`/`&&`/`||` operators, `filter`/`find`/`FILTER` and lambda criteria. The
`AND()`/`OR()` builtins keep skipping NULL like an empty Excel cell; the docs warn that
`AND(:flag, …)` passes when the flag is missing and point to `IF` or the operators. `IF` with more
than 3 arguments is an error.
NaN as a condition is a `#NUM!` error (maintainer, 2026-11-22).

### BL-090 · Dates in aggregates, sort and criteria · P2 · `ready` · language-designer → rust-engineer
LANG-5, LANG-6. `MAX`/`MIN`/`SUM` of dates return 0 silently; no sort accepts dates; ISO date text is
read only by `::DateTime`, so `COUNTIF(r, ">" & DATE(...))` matches nothing.
Decision (maintainer): `MAX`/`MIN`/sort accept dates, `SUM` of dates is an error, criteria parse
ISO date text.

### BL-091 · One text form per value type · P2 · `ready` · language-designer → rust-engineer
LANG-7, LANG-8, REL-8, LANG-12. Currency has four text forms and `&`/`CONCAT` print float noise
(`0.30000000000000004`); `[DATE(..)]::String` prints Rust Debug output; `&` is an internal
`__CONCAT__` call that formulas can call and that shows in the public AST; `"héllo".length()`
counts bytes.
Decision (maintainer): one text form per type; `.length()` counts characters; `&` gets its own AST
node. The Currency text form is moot: BL-097 removes the Currency type.

### BL-092 · API changes that are cheap only in 0.7.0 · P2 · `ready` · rust-engineer
REL-4. Decision (maintainer): deprecate `Value::Json` (ADR-0030 step 2) and `Value::Currency`
(BL-097) in 0.7.0, not 0.7.x, and add `#[non_exhaustive]` to `Value`, `Token`, `Expr` and `BinaryOp`.
`Value::Currency` is deprecated since #119; the rest is open.

### BL-093 · Capacity for large data · P2 · `ready` · performance-engineer → rust-engineer
PERF-5, PERF-7. With the `48 + len` string cost, a `.map` over 2M short strings exceeds 256 MiB; the
1 MiB request cap limits one HTTP request to ~120k numbers, so the 20 s / 256 MiB limits never apply
to real payloads.
Decision (maintainer): make the request-size cap configurable (flag + env var, like BL-075) and
document per-item capacity in `docs/guide/limits.md`.

### BL-094 · Install, Docker and dev-mode hygiene · P3 · `todo` · release-engineer + security-reviewer
REL-6, REL-9, SEC-5, SEC-6, SEC-8. Install docs name a `linux-aarch64` archive the release doesn't
build; `install.sh` skips checksums and uses fixed /tmp paths; the Alpine `runtime-test` image
starts on all interfaces with no tokens; the compose proxy has no TLS despite its comment, a
floating Traefik tag and the Docker socket; no Host check in dev mode.

### BL-095 · Organise tests by feature area · P3 · `todo` · qa-engineer
QA-5. Test files are named by PR; the same facts are pinned in 3-4 files.

### BL-096 · Small leftovers from BL-077 · P3 · `todo` · maintainer decision → rust-engineer
`sk --json` exits 2 on the library path and 1 on the `:=` path for the same JSON errors; `sk_server`
keeps JSON keys as written, so a key like `first-name` can't be read. Decide: unify the exit code,
and whether `sk_server` sanitizes keys like the other surfaces.

### BL-097 · One numeric type: `::Currency` becomes a conversion · P1 · `done` (PR #119, ADR-0033 Accepted) · language-designer (ADR-0033) → rust-engineer
`Value::Currency(f64)` is a label on the same float as `Number`: no exact money math
(`0.1::Currency + 0.2::Currency` is `0.30000000000000004`), but every operator, builtin, method and
comparison has to handle the mix (ADR-0023, 0027, 0031, BL-078, BL-082), Currency arithmetic is
2.2x slower and it has four text forms. Excel has one number type; currency is a format.
Decision (maintainer, 2026-11-22): one numeric type, `Number` (f64, like Excel). `::Integer` and
`::Float` stay conversions. `::Currency` becomes a conversion that rounds to 4 decimals (VBA's
Currency precision) and returns a Number. `Value::Currency` is deprecated in 0.7.0 and removed in
0.8; responses say `"type": "Number"`. The Currency rules in ADR-0027 and ADR-0031 are superseded.
Exact decimal money math is not built now; a later ADR can add a real decimal type.
Supersedes the BL-082 follow-up (`CONTAINS`/`IN`/`UNIQUE` with Currency) and the Currency part of BL-091.
Implement after #112 and #115 merge (they touch the same Currency code).
