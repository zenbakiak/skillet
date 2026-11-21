# 0013. Null equality: `NULL == NULL` is `true`

- Status: Proposed
- Date: 2026-11-20
- Deciders: Language Designer (proposal), Maintainer
- Related: BL-023, ADR-0006, PRD-001 FR-4/FR-7, `tests/evaluator_parity.rs`

## Context
`src/runtime/evaluator.rs` compares only `Number/Number`, `String/String` and `Boolean/Boolean`.
Every other pair falls into a catch-all: `==` is `false`, `!=` is `true`, ordering errors. So
`Null` is never equal to itself. Measured on the release build (`./target/release/sk`):

```
=NULL == NULL                   -> Boolean(false)
=NULL != NULL                   -> Boolean(true)
=NULL == 0 / =NULL == "" / =NULL == FALSE -> Boolean(false)
=NULL < 1  / =NULL >= NULL      -> Error: Comparison of incompatible types
=[NULL, 1, 2].filter(:x == NULL) -> Array([])
=[NULL, 1, 2].filter(:x != NULL) -> Array([Null, Number(1.0), Number(2.0)])
=:u.nil?()   (u=null)           -> Boolean(true)
```

The obvious way to test for a missing value (`:x == NULL`) silently returns the wrong answer, and
`:x != NULL` keeps the nulls it was written to drop. `.nil?()` / `ISBLANK()` work.
The same catch-all also makes `1::Currency == 1::Currency`, `[1,2] == [1,2]` and
`NOW() == NOW()` evaluate to `false` (out of scope here; see Consequences).

Precedent: **Ruby** `nil == nil` is `true`, `nil == 0` is `false`, `nil < 1` raises.
**SQL** `NULL = NULL` is `NULL` (unknown), so you write `IS NULL`. **Excel** has no null; a blank cell
equals both `0` and `""` (`=A1=0` is TRUE for an empty A1). **JS** `null == null` is `true`.

## Options

| Expression | Current | A: Ruby (identity) | B: Excel blank | C: SQL unknown |
|---|---|---|---|---|
| `NULL == NULL` | `false` | **`true`** | `true` | `NULL` |
| `NULL != NULL` | `true` | **`false`** | `false` | `NULL` |
| `NULL == 0` | `false` | `false` | `true` | `NULL` |
| `NULL == ""` | `false` | `false` | `true` | `NULL` |
| `NULL == FALSE` | `false` | `false` | `true` | `NULL` |
| `NULL != 0` | `true` | `true` | `false` | `NULL` |
| `NULL < 1` | Error | Error | `true` (as 0) | `NULL` |
| `NULL >= NULL` | Error | Error | `true` | `NULL` |
| `[NULL,1].filter(:x != NULL)` | `[NULL, 1]` | `[1]` | `[1]` | `[]` |

- **A. Ruby/identity.** `Null` equals only `Null`. Ordering with `Null` keeps erroring.
- **B. Excel blank.** `Null` is equal to `0`, `""` and `FALSE`, and orders as `0`/`""`. Hides
  missing data (ADR-0006 asks for explicit defaults via `to_*`). `:qty == 0` would be true for a
  missing qty.
- **C. SQL three-valued.** Comparisons with `Null` yield `Null`. Correct for SQL, but it spreads
  `Null` into `IF`/`AND`/`filter`, and `== NULL` becomes useless. Formula authors don't expect this.

## Decision (recommended)
Adopt **A**. `Null == Null` is `true`, `Null != Null` is `false`. `Null` compared with any non-null
value is unequal. `<`, `<=`, `>`, `>=` with a `Null` operand stay an error, because ordering a
missing value is a data bug. Error over silent coercion is the design rule here.
`.nil?()` stays the documented idiom; `== NULL` becomes a correct alternative.

PRD-001 FR-4 gains a row: "`==`/`!=` with `NULL`: equal only to `NULL`; ordering with `NULL` errors."
QA acceptance cases are the rows of column A above.

## Compatibility
- Changed results: `NULL == NULL`, `NULL != NULL`, and any `x == NULL` / `x != NULL` where `x` is
  null at runtime. This is a breaking change, but the old result was almost certainly a bug for
  anyone relying on it. Note it in the 0.7.0 changelog.
- Tests: no test asserts the old value. `tests/evaluator_parity.rs` (`NULL == NULL`,
  `:nil == NULL`, and the D5 comment at line ~798) compares the two layers against each other,
  so it keeps passing. Update the D5 comment. `tests/null.rs` uses `.nil?`, which is unaffected.
- Docs: `docs/` and `*.md` don't show `== NULL`. Nothing to rewrite.

## Consequences
- + `filter(:x != NULL)` does what it says. Matches Ruby and JS, which our users know.
- − Not Excel-compatible for blanks. This is deliberate: Skillet has a real `Null`.
- Follow-up (separate BL): the same catch-all makes equal `Currency`, `DateTime`, `Array` and
  `Json` values unequal. Same-type structural equality is the natural next step. It needs its own
  small ADR because it changes results too.

## Alternatives considered
- Keep current behaviour and only document `.nil?()`. Leaves a trap in the most obvious spelling.
- B (Excel blank) or C (SQL unknown): see Options.
