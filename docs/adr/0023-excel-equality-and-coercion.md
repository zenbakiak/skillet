# 0023. Excel equality, strict `===`, and Excel number coercion

- Status: Accepted
- Date: 2026-11-21
- Deciders: Maintainer
- Related: BL-066, BL-059 (DateTime comparison), BL-058 (scientific notation), ADR-0018
  (amended here), ADR-0013 (null equality, kept), ADR-0004 (JSON objects as text), PRD-001

## Context
ADR-0018 made Excel the contract for builtins and operators but kept some differences on
purpose. BL-066 listed the ones still open:

- `"a" == "A"` was `false`. Excel's `=` ignores case. ADR-0018 kept the difference "until users
  ask".
- `DateTime == DateTime` was always `false`, and `<`/`>` on two DateTimes was an error (BL-059).
  Excel compares dates as numbers.
- `10%-3` was `10 % (-3)` (modulo), not Excel's `10% - 3`.
- `ABS("-3")`, `"3" + 1` and `TRUE + 1` were errors. Excel converts numeric text and booleans in
  arithmetic.
- `1e3` lexed as `1` followed by a stray name, so it evaluated to `1` silently (BL-058).

On 2026-11-21 the maintainer decided all five. The decisions are recorded together here.

## Decision

### 1. No comparison converts between types
Conversion of text and booleans to numbers happens only in arithmetic operators and scalar math
builtins (decision 5). Comparisons never convert: `1 == "1"`, `TRUE == 1`, `"TRUE" == TRUE` and
`1 == 1::Currency` are `false` under both `==` and `===`, as before. This matches Excel, whose `=`
does not convert either (`=1="1"` and `=TRUE=1` are `FALSE`).

### 2. `==` / `!=`: text ignores case, like Excel
- Two texts are equal when they are equal ignoring case: `"a" == "A"` is `true`. Characters are
  lowercased with Unicode rules (`"ÉCOLE" == "école"`), one character at a time (no full case
  folding: `"ß" == "SS"` is `false`).
- Every other pair is equal only when both values have the same type and the same value:
  numbers, currency, booleans, DateTimes (by timestamp).
- `NULL == NULL` is `true`; `NULL` equals nothing else (ADR-0013, unchanged).
- Arrays are equal when they have the same length and their items are equal in order, with this
  same rule (`["a"] == ["A"]`). JSON objects are equal when they have the same set of keys and
  equal values; keys match exactly, values use this rule. Key order does not matter.
- `!=` is always the opposite of `==`.

### 3. `===` / `!==`: strict equality, like JS
- Same type and exactly the same value, case-sensitive text, no conversion: `"a" === "A"` is
  `false`, `1 === "1"` is `false`, `NULL === NULL` is `true`, `1 === 1.0` is `true` (every
  number is an `f64`, ADR-0003). `Number` and `Currency` are different types.
- Arrays and JSON objects compare item by item with the strict rule.
- `!==` is always the opposite of `===`.
- Both have the precedence of `==` (`1 + 1 === 2` is `true`). The lexer reads `===` and `!==`
  as single tokens; `==` and `=` are unchanged; `====` is a parse error.

### 4. DateTime comparison (BL-059)
- Two DateTimes compare by timestamp under `==`, `===`, `<`, `<=`, `>`, `>=`. Earlier is less.
- A DateTime never equals a value of another type, under either operator
  (`DATE(2026,1,1) == 0` is `false`). Ordering a DateTime against a non-DateTime is an error.

### 5. Ordering
- `<`, `<=`, `>`, `>=` take two values of the same type: numbers, currency, booleans
  (`FALSE < TRUE`, as in Excel), text, or DateTimes. Any other pair is an error ("Comparison of
  incompatible types"), including a `NULL` operand (ADR-0013) and arrays.
- **Text orders case-insensitively**, like Excel: `"apple" < "Banana"`, `"a" < "A"` is `false`
  and `"a" <= "A"` is `true`. This keeps ordering consistent with `==`: two texts that are `==`
  are neither `<` nor `>` each other. Text is never read as a number: `"10" < "9"` is `true`.
  Ties in case are not broken (no secondary case-sensitive order).
- Excel orders across types (numbers < text < booleans). Skillet keeps an error for mixed types,
  to report a mistake instead of returning a surprising answer.

### 6. Number coercion in arithmetic (lang-num lane)
- Numeric text and booleans convert to numbers in arithmetic operators and scalar math
  builtins, like Excel: `ABS("-3")` is `3`, `"3" + 1` is `4`, `TRUE + 1` is `2`.
  Non-numeric text is an error (Excel `#VALUE!`): `"abc" + 1` is an error.
- Aggregates over arrays still ignore text and booleans (`SUM([1, "2", TRUE])` is `1`, audit X-3,
  unchanged).

### 7. `%`: percent suffix or modulo (lang-num lane)
- `%` is the percent suffix when it is followed by `+`, `-`, `)`, `,`, another operator, or the
  end of the expression: `10%-3` is `-2.9`, `50%` is `0.5`.
- `%` is modulo only when an operand follows directly: `10 % 3` is `1`, `:a % :b`.

### 8. Scientific notation (BL-058, lang-num lane)
- `1e3`, `2.5E-4` and `1E+2` lex as number literals.

### Unchanged
- Lowercase methods keep their semantics (ADR-0018): `.includes()`, `.contains()` and other
  methods stay case-sensitive and type-exact.
- Builtin criteria (`COUNTIF`, `SUMIF`) keep their own matching rules.

## Amends ADR-0018
ADR-0018 "Deliberate differences" listed `"a" == "A"` → `false` and `%` as remainder only. Both
rows are replaced by decisions 2 and 7. ADR-0018 D-1 (Excel errors become `Error`) applies to
the new coercion: text that is not a number is an error, not `0`.

## Breaking changes (ship in 0.7.0)

| Expression | Before | After |
|---|---|---|
| `"a" == "A"` | `false` | `true` |
| `"a" != "A"` | `true` | `false` |
| `["a"] == ["A"]`, `[1, 2] == [1, 2]` | `false` | `true` |
| `{"a": 1} == {"a": 1}` | `false` | `true` |
| `1::Currency == 1::Currency` | `false` | `true` |
| `DATE(2026,1,1) == DATE(2026,1,1)` | `false` | `true` |
| `DATE(2026,1,1) < DATE(2026,1,2)` | `Error` | `true` |
| `"a" < "B"` | `false` | `true` |
| `TRUE > FALSE` | `false` | `true` |
| `1::Currency < 2::Currency` | `Error` | `true` |
| `1 === 1` (and `!==`) | parse error | `true` |
| `FILTER(:names, :x == "ann")` | exact match | matches `"Ann"`, `"ANN"` |

The lang-num lane's own breaking changes (`10%-3`, `ABS("-3")`, `"3"+1`, `TRUE+1`, `1e3`) are
listed in its changelog entry.

`skillet::ast::BinaryOp` gains `StrictEq` and `StrictNe`, and `skillet::lexer::Token` gains
`StrictEq` and `StrictNe`. Code outside the crate that matches these enums exhaustively must add
the new arms.

## Consequences
- \+ Comparisons copied from Excel give Excel's answer for text and dates.
- \+ `===` gives an exact match when case matters (IDs, codes).
- \+ One rule for every comparison: no type conversion, so `1 == "1"` never surprises.
- − Filters and conditions that relied on case-sensitive `==` now match more. Use `===`.
- − Case-insensitive comparison of non-ASCII text lowercases character by character, which
  costs more than a byte compare. ASCII text and equal strings take a fast path.
