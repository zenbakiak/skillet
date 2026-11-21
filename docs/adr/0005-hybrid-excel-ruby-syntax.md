# 0005. Hybrid Excel + Ruby syntax with `:var` sigil

- Status: Accepted (retroactive, v0.6.3)
- Date: 2026-11-20
- Related: PRD-001
- Amended by: [ADR-0018](0018-excel-compatibility.md) (2026-11-21). From 0.7.0, unary minus
  binds tighter than `^` and `^` is left-associative, as in Excel: `-3^2 = 9`, `2^3^2 = 64`.

## Context
Target users are spreadsheet power users and backend developers. Each group knows a different idiom.

## Decision
- Optional leading `=` (Excel). Builtins are UPPERCASE calls: `SUM(...)`, `IF(c, a, b)`, `STDEV.P(...)`.
- Methods are lowercase and chainable (Ruby): `:arr.filter(:x > 1).map(:x * 2).sum()`; a `?` suffix
  marks predicates (`:n.even?`).
- Variables always use the `:name` sigil. Lambda bodies get `:x` (item) and `:acc`
  (accumulator) by default; a trailing string argument renames the parameter.
- Also supported: ternary `c ? a : b`, `^` power (right-assoc, binds tighter than unary minus:
  `-3^2 = -9`), spread `...`, index/slice `[i]`/`[a:b]`, casts `::Type`, assignments `:a := 1; :a + 1`.
- No loops, no user-defined functions in-language, no I/O (non-Turing-complete by design).

## Consequences
- + Familiar to both audiences; sigil removes ambiguity between variables and function names.
- − Two ways to do many things (`SUM(:a)` vs `:a.sum()`); docs must say which is canonical.
- Any grammar change needs an ADR plus tests in `tests/prd_*.rs`.
