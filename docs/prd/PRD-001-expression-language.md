# PRD-001. Skillet expression language (core)

- Status: Baseline (retroactive, v0.6.3), amended for 0.7.0 by ADR-0013…0016 and ADR-0018 (2026-11-21)
- Owner: Orchestrator
- Related: ADR-0002…0006, ADR-0013…0016, ADR-0018, `tests/prd_cases.rs`, `tests/prd_comprehensive.rs`

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
| FR-1 | Arithmetic `+ - * / % ^`, optional leading `=`. Excel precedence (ADR-0018 D-10): unary minus binds tighter than `^` (`-2^2` = 4) and `^` is left-associative (`2^3^2` = 64). `%` is the remainder (deliberate difference). Division by zero and non-finite results are an `Error` (D-1). `DateTime ± Number` counts days and `DateTime − DateTime` is a number of days (D-5) | `tests/arithmetic.rs`, `prd_basic_arithmetic` |
| FR-2 | Comparison and logical operators, ternary, `IF/IFS/AND/OR/NOT/XOR`. `IF` and `IFS` are lazy: only the chosen branch is evaluated. `IFS` with no true condition and a text condition are an `Error` (ADR-0018 D-8) | `tests/prd_cases.rs` |
| FR-3 | Variables `:name` supplied by host (map or JSON); in-expression assignment `:=` and sequences `;` | `tests/assignments.rs` |
| FR-4 | Types: number, currency, datetime, string, boolean, array, JSON object, null; casts `::Type`. `==`/`!=` with `NULL`: equal only to `NULL`; ordering with `NULL` errors (ADR-0013). String casts never roll over invalid dates (ADR-0018 D-5) | `prd_conditionals_and_casting` |
| FR-5 | Builtins: arithmetic, statistical, string, array, datetime, financial, logical, JSON (~80 functions). A builtin with an Excel name follows Excel's documented behaviour, and Excel's error cases return `Error`; the exceptions are listed in ADR-0018 ("Deliberate differences"). Dotted names such as `STDEV.P` parse (D-4). Statistical builtins reject NaN and out-of-range arguments (ADR-0015) | `tests/{strings,arrays,datetime_functions,financial_functions}.rs` |
| FR-6 | Chainable methods incl. predicates (`?`) and lambdas (`filter/map/reduce/find/...`) with `:x`/`:acc` defaults. Methods keep Ruby semantics, even where the builtin of the same name follows Excel (`.trim()`, `.int()`; ADR-0018) | `tests/arrays.rs`, `prd_chained_methods` |
| FR-7 | Null safety: `&.`, total `to_*` conversions | `tests/null.rs`, `tests/safe_navigation.rs`, `tests/conversion_methods.rs` |
| FR-8 | JSON access: property access, `DIG`, `JQ` (JSONPath) over `:arguments`. `JQ` returns a value for a singular path (`NULL` if absent) and an array for any other path (ADR-0014) | `tests/json_dig.rs`, `tests/jsonpath.rs` |
| FR-9 | Object literals, spread, index and slice | `tests/object_literals.rs` |
| FR-10 | Comments and free whitespace | `tests/comments_and_whitespace_tests.rs` |
| FR-11 | Errors carry a message and, when known, a source position | `src/error.rs` |
| FR-12 | Custom functions: the entry point decides. `evaluate_with_custom`, `evaluate_with_json_custom` and the assignment entry points see registered functions in every sub-expression; `evaluate` and `evaluate_with` see none (ADR-0016) | `tests/custom_functions.rs` |

## Non-functional requirements
| ID | Requirement |
|---|---|
| NFR-1 | Typical expression evaluates in well under 1 ms in-process (to be measured, BL-005) |
| NFR-2 | No panics on any user input (fuzzing target, PRD-003) |
| NFR-3 | Library builds without plugins (`--no-default-features`) |
| NFR-4 | Nesting depth of expressions and runtime-built arrays is at most `MAX_NESTING_DEPTH = 128`; deeper input returns `Expression nested too deeply (max 128)` (BL-017, `tests/security_limits.rs`) |
| NFR-5 | Each evaluation has a deterministic budget of `MAX_EVAL_BYTES = 256 MiB` of values created (cost model in ADR-0011); exceeding it returns `Evaluation budget exceeded (max 256 MB)`. Embedders may tighten or raise it with `with_limits` (ADR-0011, BL-027) |
| NFR-6 | Builtins whose output can exceed their input (`SUBSTITUTE(M)`, `.replace`, `JOIN`/`.join`, `SPLIT`/`.split`, `JQ`, JSON parsing) charge the budget before or while allocating, so no single call overshoots it (ADR-0011) |
| NFR-7 | `sk_http_server` and `sk_server` stop any evaluation after 10 s wall clock with `Evaluation timed out (max 10 s)` (HTTP 400) and free the worker; the library has no deadline unless set via `with_limits`. Result-cache entries are at most 64 KiB (ADR-0011) |

## Open questions
- Exact currency math (ADR-0003)? Native object values (ADR-0004 / BL-011)?
- Canonical form when both a function and a method exist (`SUM(:a)` vs `:a.sum()`). Since
  ADR-0018 the two may give different results (`TRIM` / `.trim()`), so the docs must show which one they use.
- ~~Evaluation budget: drop the variable-read charge?~~ Resolved: reads are not charged (ADR-0011 amendment, after BL-030).
  Should a server timeout map to HTTP 503/504 instead of 400 (wire-format change)? Is 256 MiB right
  for embedders, or should the default scale with input size?
