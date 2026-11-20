# 0004. Nested JSON objects carried as strings

- Status: Accepted (retroactive, v0.6.3) — candidate for supersession (BL-011)
- Date: 2026-11-20
- Related: BL-011, ADR-0003

## Context
Variables arrive as JSON (`evaluate_with_json`, HTTP `/eval`). Arrays and scalars map cleanly to
`Value`, but there was no object variant.

## Decision
JSON objects (and object literals `{...}`) are stored as `Value::Json(String)` holding serialized
JSON. Property access, `DIG`, and `JQ` re-parse the string with `serde_json` on use. The full input
document is also exposed as the variable `:arguments` for `JQ`. Top-level JSON keys are sanitized
to `[A-Za-z0-9_]` (other chars → `_`) to form variable names.

## Consequences
- + No new `Value` variant; JSON round-trips exactly.
- − Every property access pays parse cost; chained access (`:a.b.c`) parses repeatedly.
- − Key sanitization can collide (`"a-b"` and `"a_b"` both become `:a_b`) and one silently overwrites the other.

## Alternatives considered
- `Value::Object(IndexMap<String, Value>)` — the natural fix; needs a migration plan (BL-011).
