# Product Requirements Documents

PRDs say **what** and **why**; ADRs say **how we decided**. Requirement IDs (`FR-n`, `NFR-n`)
are referenced by tests (`// PRD-001 FR-3`) and backlog items.

| # | Title | Status |
|---|---|---|
| [PRD-001](PRD-001-expression-language.md) | Skillet expression language (core) | Baseline (retroactive, v0.6.3) |
| [PRD-002](PRD-002-runtime-and-servers.md) | Embedding, CLI and servers | Baseline (retroactive, v0.6.3) |
| [PRD-003](PRD-003-modernization.md) | Modernization program 2026–27 | Active |

## Template

```markdown
# PRD-NNN. Title

- Status: Draft | Active | Baseline | Done
- Owner: <role/person>
- Related: <ADR/BL ids>

## Problem
## Users
## Goals / Non-goals
## Requirements
| ID | Requirement | Acceptance (test/command) |
## Success metrics
## Open questions
```
