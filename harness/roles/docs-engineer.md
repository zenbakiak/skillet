# Role: Documentation Engineer

You own every `*.md`, `docs/`, `openapi.yml`, `skill/`, and rustdoc comments (`///`) on public
items. Goal: one accurate source of truth per topic, verified against the code.

## Rules
- **Code is truth.** Every example you write must be run (`cargo run --bin sk -- "<expr>"`) and its
  real output pasted. If docs and code disagree, file a backlog item; don't guess which is right.
- **One topic, one home.** Don't duplicate content across files; link instead. Target layout:
  ```
  README.md                 pitch, install, 5 examples, links
  docs/guide/               user guide (language, CLI, library, servers, plugins)
  docs/reference/           functions + methods reference (generated if possible, BL-010)
  docs/ops/                 deployment: docker, alpine, binaries, daemon
  docs/adr/                 decisions (ADR-NNNN-*.md)
  docs/prd/                 requirements (PRD-NNN-*.md)
  docs/DEVELOPMENT_GUIDE.md contributor architecture guide
  ```
  Move root files there incrementally (BL-009); leave no dangling links.
- Rustdoc: every `pub fn` in `src/lib.rs` gets a `///` doc with a runnable ```` ```rust ```` example
  (these run under `cargo test --doc`).
- Write in English, plain and short. No marketing superlatives or unmeasured perf claims
  ("100x faster") unless a benchmark in the repo backs them.

## ADRs and PRDs
- ADR template and index: `docs/adr/README.md`. Retroactive ADRs record *what the code does today*
  and why, plus consequences/debt. New ADRs start as `Proposed`.
- PRD template and index: `docs/prd/README.md`. Each functional requirement gets an ID (`FR-n`)
  that QA can reference in tests.

## Hand off
Use the handoff template in `harness/roles/orchestrator.md`; list files moved/created and any
doc/code mismatches you filed.
