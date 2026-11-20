# Architecture Decision Records

Format: lightweight [Nygard](https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions).
File name `NNNN-kebab-title.md`. Statuses: `Proposed` → `Accepted` | `Rejected` → later `Superseded by NNNN`.
ADRs 0001–0008 are **retroactive**: they record decisions already embodied in the code (v0.6.3).

| # | Title | Status |
|---|---|---|
| [0001](0001-record-architecture-decisions.md) | Record architecture decisions | Accepted |
| [0002](0002-tree-walking-interpreter.md) | Tree-walking interpreter over an `Rc` AST | Accepted (retroactive) |
| [0003](0003-dynamic-value-f64-numbers.md) | Single dynamic `Value` enum; all numbers are `f64` | Accepted (retroactive) |
| [0004](0004-json-objects-as-strings.md) | Nested JSON objects carried as strings | Accepted (retroactive) |
| [0005](0005-hybrid-excel-ruby-syntax.md) | Hybrid Excel + Ruby syntax with `:var` sigil | Accepted (retroactive) |
| [0006](0006-null-safety-model.md) | Null safety via `&.` and total conversions | Accepted (retroactive) |
| [0007](0007-extensibility-custom-and-js.md) | Extensibility: `CustomFunction` + QuickJS plugins | Accepted (retroactive) |
| [0008](0008-servers-std-threads-unix-daemon.md) | Servers on `std::net` + thread pool; Unix-only daemon | Accepted (retroactive) |
| [0009](0009-gitflow-branching-and-commits.md) | Gitflow branching, commit format, and git hooks | Accepted |
| [0010](0010-remove-dead-public-modules-pre-1-0.md) | Remove dead public modules in 0.7.0 without deprecation | Accepted |

## Template

```markdown
# NNNN. Title

- Status: Proposed
- Date: YYYY-MM-DD
- Deciders: <roles/people>
- Related: <ADR/PRD/BL ids>

## Context
What forces are at play? Facts, constraints, measurements.

## Decision
What we do. Active voice, one paragraph or a short list.

## Consequences
Good, bad, and debt created. Link backlog items for follow-ups.

## Alternatives considered
Option — why not (one line each).
```
