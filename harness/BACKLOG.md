# Backlog

Single source of truth for work. Statuses: `todo` → `ready` (has brief) → `in-progress (<agent>)`
→ `review` → `done`. Priority P1 (now) · P2 (next) · P3 (later). Only the Orchestrator reorders.

Seeded 2026-11-20 from a codebase audit: 153 tracked files, ~16k LOC, all tests green,
12 compiler warnings, 95 clippy warnings, `cargo fmt --check` reports ~970 diffs.

---

### BL-001 · Unify the two evaluators · P1 · `todo`
Two parallel implementations exist: `src/runtime/evaluator.rs` (used by `lib.rs` public API) and
`src/runtime/evaluation/` (used by `method_calls/*` when evaluating lambdas). Top-level and
nested evaluation can silently diverge.
- **BL-001a (qa-engineer)**: differential tests — same expressions through `runtime::evaluator::*`
  and `runtime::evaluation::*`; document every divergence found.
- **BL-001b (rust-engineer)**: keep one (likely `evaluation/`, it is already modular), make the
  other a re-export, delete the duplicate. Blocked by BL-001a.

### BL-002 · Remove dead modules and unused dependencies · P1 · `todo` · rust-engineer
`memory_pool.rs`, `traits.rs`, `concurrent_registry.rs` are `pub mod` but unused;
`runtime/method_calls_old.rs` is not even compiled. Deps `string-interner`, `typed-arena`, `beef`,
`dashmap` are only used by dead code. Since the modules are `pub`, removal is a semver-visible
change → mark `#[deprecated]` in 0.7, remove in 0.8 (or ADR to remove now while pre-1.0).

### BL-003 · Format + clippy to zero, then make the gate strict · P1 · `todo` · rust-engineer
One commit `cargo fmt` (no other changes), then fix clippy (mostly `args.get(0)` → `first()`,
empty `println!("")`). Finish by setting `STRICT=1` default in `harness/check.sh`.

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
