# Architecture Decision Records

Format: lightweight [Nygard](https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions).
File name `NNNN-kebab-title.md`. Statuses: `Proposed` → `Accepted` | `Rejected` → later `Superseded by NNNN`.
ADRs 0001–0008 are **retroactive**: they record decisions already embodied in the code (v0.6.3).

| # | Title | Status |
|---|---|---|
| [0001](0001-record-architecture-decisions.md) | Record architecture decisions | Accepted |
| [0002](0002-tree-walking-interpreter.md) | Tree-walking interpreter over an `Rc` AST | Accepted (retroactive) |
| [0003](0003-dynamic-value-f64-numbers.md) | Single dynamic `Value` enum; all numbers are `f64` | Accepted (retroactive), amended by 0033 |
| [0004](0004-json-objects-as-strings.md) | Nested JSON objects carried as strings | Superseded by 0030 |
| [0005](0005-hybrid-excel-ruby-syntax.md) | Hybrid Excel + Ruby syntax with `:var` sigil | Accepted (retroactive), amended by 0018 |
| [0006](0006-null-safety-model.md) | Null safety via `&.` and total conversions | Accepted (retroactive) |
| [0007](0007-extensibility-custom-and-js.md) | Extensibility: `CustomFunction` trait + QuickJS plugins | Accepted (retroactive) |
| [0008](0008-servers-std-threads-unix-daemon.md) | Servers on `std::net` + thread pool; Unix-only daemon | Accepted (retroactive) |
| [0009](0009-gitflow-branching-and-commits.md) | Gitflow branching, commit format, and git hooks | Accepted |
| [0010](0010-remove-dead-public-modules-pre-1-0.md) | Remove dead public modules in 0.7.0 without deprecation | Accepted |
| [0011](0011-evaluation-budget.md) | Per-evaluation budget and server evaluation timeout | Accepted, amended (amendments 1–5 in the file; amended by 0030) |
| [0012](0012-msrv-1-85.md) | Minimum supported Rust version 1.85; CI on 1.89 | Accepted |
| [0013](0013-null-equality.md) | Null equality: `NULL == NULL` is `true` | Accepted |
| [0014](0014-jq-wildcard-results.md) | `JQ` result shape follows the path, not the match count | Accepted |
| [0015](0015-percentile-nan.md) | NaN and infinity in statistical builtins | Accepted |
| [0016](0016-custom-function-visibility.md) | Custom functions are visible in every sub-expression | Accepted |
| [0017](0017-quickjs-resource-limits.md) | Resource limits for JavaScript plugins (QuickJS) | Accepted |
| [0018](0018-excel-compatibility.md) | Excel compatibility: Excel is the contract for builtins and operators | Accepted, amended by 0023 and 0034, and a dated `.round()` note in the file (BL-088) |
| [0019](0019-server-cors-and-development-mode.md) | Server CORS and development mode | Accepted |
| [0020](0020-drop-corefoundation.md) | Drop CoreFoundation from the macOS binaries | Accepted |
| [0021](0021-parsed-expression-api-and-server-cache.md) | Evaluate a parsed expression; servers cache parsed expressions per worker thread | Accepted |
| [0022](0022-single-release-tag.md) | One release tag drives binaries, GitHub release and crates.io | Accepted |
| [0023](0023-excel-equality-and-coercion.md) | Equality and comparison, and Excel number coercion in arithmetic | Accepted, amended by 0027 |
| [0024](0024-hooks-cannot-override-builtins.md) | JS hooks cannot override builtins | Accepted |
| [0025](0025-per-ip-connection-limit.md) | Optional limit on concurrent connections per IP address | Accepted |
| [0027](0027-date-and-currency-arithmetic.md) | Date and currency arithmetic | Accepted, amended (amendment 1 in the file, BL-082) and by 0031; Currency parts superseded by 0033 |
| [0028](0028-fuzzing.md) | Fuzz the parser and evaluator with cargo-fuzz on a schedule | Accepted |
| [0029](0029-logic-operators.md) | Logical operators and the Excel logical functions | Accepted, amended (amendment 1 in the file, BL-089) |
| [0030](0030-structured-and-shared-values.md) | Structured and shared values: `Value::Object`, and `Arc`-shared strings, arrays and objects | Accepted, amended by 0034 |
| [0031](0031-date-text-and-currency-aggregates.md) | Dates as ISO 8601 text; Currency in the aggregates | Accepted, amended (amendments 1–2 in the file, BL-078, BL-090) and by 0034; Currency parts superseded by 0033 |
| [0032](0032-edition-2024.md) | Edition 2024, MSRV unchanged, std `LazyLock`, rustfmt style 2021 for now | Accepted |
| [0033](0033-one-numeric-type.md) | One numeric type; `::Currency` is a conversion | Accepted, amended by 0034 |
| [0034](0034-text-form-of-values.md) | Text form of values | Accepted |

Number 0026 is unused: no ADR was filed under it.

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
