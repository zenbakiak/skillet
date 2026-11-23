# 0002. Tree-walking interpreter over an `Rc` AST

- Status: Accepted (retroactive, v0.6.3)
- Date: 2026-11-20
- Related: BL-001, BL-012, PRD-003

## Context
Expressions are short (typically < 200 chars) and evaluated once per input. Simplicity of
implementation and fast iteration on features mattered more than peak throughput.

## Decision
A hand-written lexer (`src/lexer.rs`) and recursive-descent parser (`src/parser.rs`) build an
`ast::Expr` tree whose children are `Rc<Expr>`. The runtime evaluates the tree directly by
recursive match on `Expr`. There is no bytecode, constant folding, or AST caching in the library.

## Consequences
- + Easy to extend: new syntax = new `Expr` variant + parser rule + evaluator arm.
- − `Rc` makes `Expr` `!Send`/`!Sync`: a parsed expression cannot be shared across server threads,
  so servers re-parse per request (BL-012).
- − Lambdas re-evaluate their body per element, cloning variable scopes.
- − Debt: the evaluator currently exists twice (`runtime/evaluator.rs` and `runtime/evaluation/`), BL-001.

## Status note (2026-11-20)
The decision is unchanged. Two of the consequences above are resolved:
- Duplicate evaluator: resolved by BL-001. `runtime/evaluation/` was deleted; `runtime/evaluator.rs`
  is the only evaluator (the leftover re-export module went in BL-002, ADR-0010).
- Scope cloning in lambdas: resolved by BL-030. Lambdas, method calls and `FILTER`/`MAP`/`REDUCE`-style
  builtins bind `:x`/`:acc` in a small local `Scope` over the borrowed parent variables, so no call
  copies the variable map. The lambda body is still re-evaluated per element (tree walking).

The `Rc`/`!Send` consequence still holds (BL-012).

## Alternatives considered
- Bytecode VM — faster for repeated evaluation, but much more code; revisit after BL-005 numbers.
- Parser generator (pest/lalrpop) — new dependency and worse error messages for little gain.
