# 0006. Null safety via `&.` and total conversions

- Status: Accepted (retroactive, v0.6.3)
- Date: 2026-11-20
- Related: PRD-001, `tests/null.rs`, `tests/safe_navigation.rs`, `tests/conversion_methods.rs`

## Context
Input data from JSON is often incomplete. Erroring on every `null` made formulas brittle.

## Decision
- `Null` is a first-class value (`null` literal, JSON `null`).
- Safe navigation `&.` (`Expr::SafePropertyAccess`, `Expr::SafeMethodCall`) short-circuits to `null`.
- Conversion methods are total on every type: `to_s`, `to_i`, `to_f`, `to_a`, `to_bool`
  (e.g. `null.to_s() = ""`, `null.to_i() = 0`).
- Plain `.` on `null` is still an error — opting in to safety is explicit.

## Consequences
- + Formulas degrade gracefully with `&.` / `to_*`.
- − Silent defaults (`0`, `""`) can hide data problems; authors must choose deliberately.

## Amendment (maintainer, 2026-11-22, BL-105): `::Array` on a string
`::Array` on a string splits it into characters, exactly as `.to_a()`: `"hello"::Array` is
`["h","e","l","l","o"]` (was `["hello"]`) and `""::Array` is `[]` (was `[""]`). Characters are
Unicode scalars, as `.length()` counts them (ADR-0034 §5). One helper
(`runtime::text::string_chars`) serves both, so they cannot drift; it charges the evaluation
budget before building (ADR-0011). Every other input keeps its result: an array stays itself,
and a number, boolean, date, object or `NULL` becomes a one-item array. So `NULL::Array` is still
`[NULL]` while `NULL.to_a()` is `[]`. Ships in 0.7.0; see the migration guide.
