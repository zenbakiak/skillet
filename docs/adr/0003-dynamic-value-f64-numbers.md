# 0003. Single dynamic `Value` enum; all numbers are `f64`

- Status: Accepted (retroactive, v0.6.3)
- Date: 2026-11-20
- Related: PRD-001

## Context
Excel users expect one "number" type. The language is dynamically typed with explicit casts
(`::Integer`, `::Currency`, `::DateTime`, …).

## Decision
`types::Value` is one enum: `Number(f64)`, `Currency(f64)`, `DateTime(i64)` (Unix seconds),
`String`, `Boolean`, `Array(Vec<Value>)`, `Json(String)`, `Null`. Integers are `f64` values with
no fractional part; `::Integer` truncates. `Currency` is a tagged `f64` and is accepted wherever
numbers are.

## Consequences
- + Simple arithmetic and Excel-like behavior (`10/4 = 2.5`).
- − `Currency` inherits binary floating-point error (`0.1 + 0.2 != 0.3`); unsuitable for exact money math.
- − Integers above 2^53 lose precision (relevant for IDs coming from JSON).
- Changing either is a breaking semantic change and needs a superseding ADR (e.g. decimal currency).

## Alternatives considered
- Separate `Int(i64)` / `Float(f64)` — more correct, more coercion rules to define and document.
- `rust_decimal` for `Currency` — likely future direction; deferred.
