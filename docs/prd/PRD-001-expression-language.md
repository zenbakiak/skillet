# PRD-001. Skillet expression language (core)

- Status: Baseline (retroactive — describes v0.6.3 behavior)
- Owner: Orchestrator
- Related: ADR-0002…0006, `tests/prd_cases.rs`, `tests/prd_comprehensive.rs`

## Problem
Applications need user-editable business rules (pricing, taxes, eligibility, data shaping) that
are safe to run, fast, and readable by spreadsheet users — without embedding a general-purpose
scripting language.

## Users
- **Formula authors**: analysts / ops who know Excel.
- **Integrators**: developers embedding Skillet in Rust, or calling it from Ruby/Node via CLI/servers.

## Goals
- Excel-familiar formulas plus expressive method chaining over arrays and JSON.
- Deterministic, sandboxed evaluation: no loops, no I/O, no unbounded recursion from user input.
- Graceful handling of missing data.

## Non-goals
General-purpose programming; Turing completeness; database querying (use `JQ`/JSONPath);
spreadsheet cell references (`A1`).

## Functional requirements
| ID | Requirement | Acceptance |
|---|---|---|
| FR-1 | Arithmetic `+ - * / % ^` with Excel precedence, optional leading `=` | `tests/arithmetic.rs`, `prd_basic_arithmetic` |
| FR-2 | Comparison and logical operators, ternary, `IF/AND/OR/NOT/XOR` | `tests/prd_cases.rs` |
| FR-3 | Variables `:name` supplied by host (map or JSON); in-expression assignment `:=` and sequences `;` | `tests/assignments.rs` |
| FR-4 | Types: number, currency, datetime, string, boolean, array, JSON object, null; casts `::Type` | `prd_conditionals_and_casting` |
| FR-5 | Builtins: arithmetic, statistical, string, array, datetime, financial, logical, JSON (~80 functions) | `tests/{strings,arrays,datetime_functions,financial_functions}.rs` |
| FR-6 | Chainable methods incl. predicates (`?`) and lambdas (`filter/map/reduce/find/...`) with `:x`/`:acc` defaults | `tests/arrays.rs`, `prd_chained_methods` |
| FR-7 | Null safety: `&.`, total `to_*` conversions | `tests/null.rs`, `tests/safe_navigation.rs`, `tests/conversion_methods.rs` |
| FR-8 | JSON access: property access, `DIG`, `JQ` (JSONPath) over `:arguments` | `tests/json_dig.rs`, `tests/jsonpath.rs` |
| FR-9 | Object literals, spread, index and slice | `tests/object_literals.rs` |
| FR-10 | Comments and free whitespace | `tests/comments_and_whitespace_tests.rs` |
| FR-11 | Errors carry a message and, when known, a source position | `src/error.rs` |

## Non-functional requirements
| ID | Requirement |
|---|---|
| NFR-1 | Typical expression evaluates in well under 1 ms in-process (to be measured, BL-005) |
| NFR-2 | No panics on any user input (fuzzing target, PRD-003) |
| NFR-3 | Library builds without plugins (`--no-default-features`) |

## Open questions
- Exact currency math (ADR-0003)? Native object values (ADR-0004 / BL-011)?
- Canonical form when both a function and a method exist (`SUM(:a)` vs `:a.sum()`).
