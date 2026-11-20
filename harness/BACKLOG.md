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

### BL-002 · Remove dead modules and unused dependencies · P1 · `in-progress` · rust-engineer
`memory_pool.rs`, `traits.rs`, `concurrent_registry.rs` are `pub mod` but unused;
`runtime/method_calls_old.rs` is not even compiled. Deps `string-interner`, `typed-arena`, `beef`,
`dashmap` are only used by dead code. Since the modules are `pub`, removal is a semver-visible
change → mark `#[deprecated]` in 0.7, remove in 0.8 (or ADR to remove now while pre-1.0).

Brief (orchestrator, 2026-11-20): maintainer chose to remove now (ADR-0010, Accepted).
  Goal: no dead modules, files, or dependencies left; gate green.
  Role: rust-engineer · `in-progress` · Branch: feature/bl-002-remove-dead-code
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

### BL-017 · Stack overflow on deeply nested input (DoS) · P1 · `todo` · security-reviewer → rust-engineer
`sk "((((…20000×…1"` aborts with `fatal runtime error: stack overflow` (reproduced 2026-11-20).
It's an abort, not a panic, so one HTTP request kills `sk_http_server` for everyone. Add a nesting
depth limit in `parser.rs` (and in evaluator recursion) that returns `Error`. QA adds
`regression_sec_deep_nesting`. Check `[[[[…` and `:a.b.c.…` chains too.

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
