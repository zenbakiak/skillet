# PRD-003. Modernization program 2026–27

- Status: Active
- Owner: Orchestrator
- Related: `harness/BACKLOG.md`, all ADRs

## Problem
Skillet works (all tests green at v0.6.3) but has accumulated structural debt: duplicate
evaluators, dead modules and dependencies, unformatted code, 95 clippy warnings, overlapping and
partly inaccurate docs, unmeasured performance claims, and CI that only guards `main`.

## Goals
1. **Performance** — measured baseline, then targeted wins; no regressions merge unnoticed.
2. **Maintainability** — one evaluator, no dead code, clippy/fmt clean, modules with one responsibility.
3. **Documentation** — single source of truth per topic; examples verified; generated reference.
4. **Decision memory** — ADRs/PRDs current; agents of any vendor can pick up work from `AGENTS.md`.

## Non-goals
Rewriting the language, changing syntax or semantics (except via accepted ADRs), 1.0 API freeze.

## Requirements
| ID | Requirement | Acceptance | Backlog |
|---|---|---|---|
| FR-1 | Exactly one evaluator implementation | `rg 'fn eval' src/runtime` shows one impl; differential tests pass | BL-001 |
| FR-2 | No unused modules or deps | `cargo udeps`/manual audit clean | BL-002 |
| FR-3 | `cargo fmt --check` and `clippy -D warnings` pass; `STRICT=1` default | `./harness/check.sh` | BL-003 |
| FR-4 | CI runs the harness gate on `main` and `develop` PRs | GitHub Actions | BL-004 |
| FR-5 | Reproducible perf baseline | `benches/BASELINE.md` | BL-005 |
| FR-6 | Docs accurate and consolidated under `docs/` | Docs review, no dead links | BL-006, BL-009 |
| FR-7 | Function reference generated from code | `docs/reference/functions.md` regenerated in CI | BL-010 |
| FR-8 | Fuzz the parser/evaluator; no panics or aborts | `cargo fuzz` target runs 10 min clean | BL-017, BL-019 |

## Success metrics
- Clippy warnings: 95 → 0. Compiler warnings: 12 → 0.
- Root-level `.md` files: 13 → ≤ 3 (`README`, `AGENTS`, `CLAUDE`) + licences.
- Lib eval latency for the BL-005 set: −30% median vs baseline without semantic changes.
- 100% of `pub` items in `lib.rs` documented with doctests.

## Phases
1. **Stabilize** (P1): BL-001…BL-006, BL-017, BL-018 — safety net, gate, truthful docs, DoS fixes.
2. **Restructure** (P2): BL-007…BL-012, BL-019…BL-021 — API cleanup, docs tree, reference, perf, fuzzing, Excel audit, changelog.
3. **Modernize** (P3): BL-013…BL-016 — Windows, toolchain/edition, error kinds.
