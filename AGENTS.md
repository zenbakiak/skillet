# AGENTS.md — Skillet agent harness

This file is the single entry point for **any** coding agent (Codex, Claude Code, Cursor,
Copilot, Aider, Gemini, …) and for humans. Tool-specific files (`CLAUDE.md`,
`.claude/agents/*`) only point back here. If you change a rule, change it here.

## 1. What Skillet is

Embeddable expression language in Rust: Excel-style formulas (`=SUM(:a, 2)`), Ruby-style
methods (`:arr.filter(:x > 2).sum()`), null-safe navigation (`:u&.name`), JSONPath (`JQ`),
JS plugins (QuickJS). Ships as a library crate (`skillet`) and five binaries
(`sk`, `sk_server`, `sk_client`, `sk_http_server`, `sk_http_bench`).

Pipeline: `lexer.rs` → `parser.rs` → `ast::Expr` → `runtime/` → `types::Value`.

## 2. Mission (current program)

See [`docs/prd/PRD-003-modernization.md`](docs/prd/PRD-003-modernization.md). In short:

1. Improve performance (measured, never assumed).
2. Improve maintainability and code structure (one evaluator, no dead modules, clippy-clean).
3. Better documentation (one source of truth per topic, generated where possible).
4. Keep decisions in ADRs (`docs/adr/`) and requirements in PRDs (`docs/prd/`).

## 3. Commands

```bash
cargo build --all-targets            # build lib + bins + tests
cargo test                           # full suite (~300 tests; sk_concurrency/sk_performance are slow)
cargo test --lib                     # fast loop
cargo test --test <file_stem>        # one integration file, e.g. --test arrays
cargo clippy --all-targets           # lint
cargo run --bin sk -- "=2 + 3 * 4"   # try an expression
./harness/check.sh                   # THE gate. Must pass before any handoff.
```

Toolchain: Rust `1.81.0` (`.tool-versions`), edition 2021. Plugins feature is default;
`--no-default-features` builds without QuickJS.

## 4. Codebase map (verified 2026-11)

| Path | Role | Notes |
|---|---|---|
| `src/lib.rs` | Public API (`parse`, `evaluate*`, registry fns) | Calls `runtime::evaluator` |
| `src/lexer.rs`, `src/parser.rs`, `src/ast.rs` | Front end | Recursive descent, `Rc<Expr>` AST |
| `src/types.rs`, `src/error.rs` | `Value` enum, `Error {message, position}` | |
| `src/runtime/evaluator.rs` | Evaluator used by **public API** | ⚠ duplicate, see BL-001 |
| `src/runtime/evaluation/` | Evaluator used by **method/lambda calls** | ⚠ duplicate, see BL-001 |
| `src/runtime/function_dispatch.rs`, `builtin_functions.rs` | `UPPERCASE()` builtins by category | |
| `src/runtime/method_calls/` | `.method()` dispatch | |
| `src/runtime/{arithmetic,string,array,datetime,financial,statistical,logical,json,jsonpath}.rs` | Builtin implementations | |
| `src/custom.rs`, `src/js_plugin.rs` | `CustomFunction` trait, JS plugins | |
| `src/bin/http_server/` | HTTP server modules | std::net + threadpool, no async |
| `src/memory_pool.rs`, `src/traits.rs`, `src/concurrent_registry.rs` | **Unused** | BL-002 |
| `src/runtime/method_calls_old.rs` | **Orphan, not compiled** | BL-002 |
| `tests/*.rs` | Integration tests, one file per feature area | `prd_*.rs` = PRD-001 acceptance |

## 5. Roles

Pick the role for your task and read its file before starting. One role per session.

| Role | File | Owns |
|---|---|---|
| Orchestrator | [`harness/roles/orchestrator.md`](harness/roles/orchestrator.md) | Backlog, task split, handoffs, ADR acceptance |
| Rust Engineer | [`harness/roles/rust-engineer.md`](harness/roles/rust-engineer.md) | `src/`, `Cargo.toml`, performance |
| QA Engineer | [`harness/roles/qa-engineer.md`](harness/roles/qa-engineer.md) | `tests/`, `benches/`, regression gate |
| Documentation Engineer | [`harness/roles/docs-engineer.md`](harness/roles/docs-engineer.md) | `*.md`, `docs/`, `openapi.yml`, rustdoc |
| Language Designer | [`harness/roles/language-designer.md`](harness/roles/language-designer.md) | Grammar/semantics, PRD-001, language ADRs |
| Performance Engineer | [`harness/roles/performance-engineer.md`](harness/roles/performance-engineer.md) | `benches/`, baselines, profiling, perf claims |
| Security Reviewer | [`harness/roles/security-reviewer.md`](harness/roles/security-reviewer.md) | Threat model, untrusted-input review, `cargo audit`/fuzz |
| Release Engineer | [`harness/roles/release-engineer.md`](harness/roles/release-engineer.md) | CI workflows, toolchain, packaging, binaries, Docker |

Ownership overlaps, resolved: QA decides *what* the gate checks, Release decides *how* CI runs it.
Performance owns `benches/`; QA owns `tests/`. Language Designer specifies, Rust Engineer implements.

No role given? Act as **Orchestrator**: read `harness/BACKLOG.md`, pick the top `ready`
item, and follow the role it names.

## 6. Shared protocol (all roles)

0. **Git setup (once per clone)**: `git config core.hooksPath .githooks`. Rules: [ADR-0009](docs/adr/0009-gitflow-branching-and-commits.md).
1. **Start**: read this file, your role file, and the backlog item. Run `./harness/check.sh` to get a baseline.
2. **Claim**: set the item to `in-progress` with your agent/model name in `harness/BACKLOG.md`.
3. **Work small**: one backlog item per branch, `feature/bl-<id>-<slug>` (or `bugfix/…`) from `develop`.
   One logical change per commit, Conventional Commits format. **No `Co-authored-by` or
   "Generated with" lines in commits or PRs.** PRs target `develop`; never push to `main`/`develop`.
4. **Gate**: `./harness/check.sh` must pass. Never weaken a test to make it pass; if behavior *should* change, say so in the handoff and update the PRD.
5. **Handoff**: append a handoff note to the item (template in `harness/roles/orchestrator.md`) and set status `review`.
6. **Decisions**: any choice that is hard to reverse (public API, syntax, `Value` layout, dependency, MSRV, wire format) needs an ADR in `docs/adr/` with status `Proposed`. Only the Orchestrator (or a human) moves it to `Accepted`.

## 7. Invariants — do not break without an accepted ADR

- Existing expressions keep evaluating to the same `Value` (tests in `tests/` are the contract).
- Public functions in `src/lib.rs` keep their signatures. Add, deprecate (`#[deprecated]`), never silently remove.
- Variables use the `:name` sigil; builtins are UPPERCASE; methods are lowercase; `?` suffix = predicate.
- Lambda defaults are `:x` (item) and `:acc` (accumulator).
- The language stays non-Turing-complete: no loops, no I/O from expressions.
- `plugins` feature stays optional; the core must build with `--no-default-features`.
- No secrets in commits (`.env`, `.claude/settings.local.json`).

## 8. Definition of done

- [ ] `./harness/check.sh` green.
- [ ] New behavior has a test; fixed bug has a regression test.
- [ ] User-visible change reflected in `DOCUMENTATION.md`/`API_REFERENCE.md` (or a docs backlog item filed).
- [ ] Performance-sensitive change has before/after numbers in the handoff.
- [ ] Backlog item updated with handoff note.
