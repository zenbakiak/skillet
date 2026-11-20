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
