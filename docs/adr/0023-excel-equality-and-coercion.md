# 0023. Equality and comparison, and Excel number coercion in arithmetic

> **Amended by ADR-0027 Amendment 1 (maintainer, 2026-11-22, BL-082):** Currency and Number
> compare by amount in `==`, `!=`, `<`, `<=`, `>`, `>=`, in either order (`15 == 15::Currency`
> is `true`, `1::Currency < 2` is `true`). This replaces the Currency parts of §1, §2 and §4.
> Every other rule here is unchanged.

- Status: Accepted
- Date: 2026-11-21
- Deciders: Maintainer
- Related: BL-066, BL-059 (DateTime comparison), BL-058 (scientific notation), ADR-0018
  (amended here), ADR-0013 (null equality, kept), ADR-0004 (JSON objects as text), PRD-001

## Context
ADR-0018 made Excel the contract for builtins and operators but kept some differences on
purpose. BL-066 and the round-4 follow-ups listed the ones still open:

- `"a" == "A"` is `false`. Excel's `=` ignores case. ADR-0018 kept the difference "until users
  ask".
- `DateTime == DateTime` was always `false`, and `<`/`>` on two DateTimes was an error (BL-059).
  The same catch-all made `[1, 2] == [1, 2]`, `{"a": 1} == {"a": 1}` and
  `1::Currency == 1::Currency` `false`, and every ordering of two booleans `false`.
- `10%-3` was `10 % (-3)` (modulo), not Excel's `10% - 3`.
- `ABS("-3")`, `"3" + 1` and `TRUE + 1` were errors. Excel converts numeric text and booleans in
  arithmetic.
- `1e3` lexed as `1` followed by a stray name, so it evaluated to `1` silently (BL-058).

On 2026-11-21 the maintainer decided all of these. The decisions are recorded together here.

## Decision

### 1. `==` / `!=` are strict and case-sensitive
- A value equals only a value of the same type with the same value. Text is case-sensitive:
  `"a" == "A"` is `false` (unchanged; ADR-0018 keeps this as a deliberate difference).
- `1 == 1.0` is `true` (every number is an `f64`, ADR-0003). `Number` and `Currency` are
  different types; two `Currency` values compare by amount.
- `NULL == NULL` is `true`; `NULL` equals nothing else (ADR-0013, unchanged).
- Arrays are equal when they have the same length and their items are equal in order, with this
  same rule. JSON objects are equal when they have the same set of keys and equal values; key
  order and number spelling (`1` vs `1.0`) do not matter.
- `!=` is always the opposite of `==`.

### 2. No comparison converts between types
Conversion of text and booleans to numbers happens only in arithmetic operators and scalar math
builtins (decision 6). Comparisons never convert: `1 == "1"`, `TRUE == 1`, `"TRUE" == TRUE` and
`1 == 1::Currency` are `false`, as before. This matches Excel, whose `=` does not convert either
(`=1="1"` and `=TRUE=1` are `FALSE`).

### 3. DateTime comparison (BL-059)
- Two DateTimes compare by timestamp under `==`, `!=`, `<`, `<=`, `>`, `>=`. Earlier is less.
- A DateTime never equals a value of another type (`DATE(2026,1,1) == 0` is `false`). Ordering
  a DateTime against a non-DateTime is an error.

### 4. Ordering
- `<`, `<=`, `>`, `>=` take two values of the same type: numbers, currency, booleans
  (`FALSE < TRUE`, as in Excel), text, or DateTimes. Any other pair is an error ("Comparison of
  incompatible types"), including a `NULL` operand (ADR-0013) and arrays.
- Text orders by Unicode code point, case-sensitive, as before (`"B" < "a"`). Text is never read
  as a number: `"10" < "9"` is `true`. This stays consistent with `==`.
- Excel orders across types (numbers < text < booleans) and ignores case. Skillet keeps an error
  for mixed types, to report a mistake instead of returning a surprising answer.

### 5. Considered and dropped: `===` / `!==`
An earlier draft of this ADR made `==` case-insensitive, like Excel, and added JS-style strict
`===`/`!==` for exact matches. When the maintainer kept `==` case-sensitive, `===` would have
given the same result as `==` for every pair of values, so it was dropped. The lexer, the AST
(`BinaryOp`, `Token`) and the grammar are unchanged; `1 === 1` stays a parse error.

### 6. Number coercion in arithmetic (lang-num lane)
- Numeric text and booleans convert to numbers in arithmetic operators and scalar math
  builtins, like Excel: `ABS("-3")` is `3`, `"3" + 1` is `4`, `TRUE + 1` is `2`.
  Non-numeric text is an error (Excel `#VALUE!`): `"abc" + 1` gives
  `Cannot convert "abc" to a number (#VALUE!)`; builtins prefix the name (`ABS: cannot convert …`).
  Numeric text is read after trimming; empty text, `"1,000"`, `"inf"`, `"NaN"` and out-of-range
  values are not numbers.
- `NULL` counts as `0` in the operators only (Excel's blank cell). Builtins keep `NULL` strict:
  a required argument errors, an optional one takes its default.
- DateTime operands are never coerced (date arithmetic is unchanged), and Currency in the
  operators is unchanged.
- Aggregates over arrays still ignore text and booleans (`SUM([1, "2", TRUE])` is `1`, audit X-3,
  unchanged). **Superseded for direct arguments by
  [ADR-0029 amendment 2](0029-logic-operators.md#amendment-2-maintainer-2026-11-22-bl-107)
  (BL-107):** a direct argument of an aggregate converts with this section's rule
  (`SUM(1, "2", TRUE)` is 4, `SUM(1, "x")` is `#VALUE!`); array items stay skipped.

### 7. `%`: percent suffix or modulo (lang-num lane)
- `%` is modulo only when an operand starts directly after it: a number, a string, a name,
  `TRUE`/`FALSE`/`NULL`, `(`, `[`, `{`, `!` or a `:var`. `10 % 3` is `1`, `:a % :b`.
- Otherwise, including before `+` and `-`, it is the percent suffix: `10%-3` is `-2.9`, `50%` is
  `0.5`. This changes `10 % -3` from `1` to `-2.9`.

### 8. Scientific notation (BL-058, lang-num lane)
- `1e3`, `2.5E-4` and `1E+2` lex as number literals.
- A number that runs straight into a letter or `_` (`1x`, `1e`, `1_000`) is an
  `Invalid number literal` error, instead of silently evaluating to the leading number.

### Unchanged
- Lowercase methods keep their semantics (ADR-0018): `.includes()`, `.contains()` and other
  methods behave as before.
- Builtin criteria (`COUNTIF`, `SUMIF`) keep their own matching rules.

## Amends ADR-0018
ADR-0018 "Deliberate differences" listed `%` as remainder only; decision 7 replaces that row.
The `"a" == "A"` → `false` row stays. ADR-0018 D-1 (Excel errors become `Error`) applies to the
new arithmetic coercion: text that is not a number is an error, not `0`.

## Breaking changes (ship in 0.7.0)

| Expression | Before | After |
|---|---|---|
| `[1, 2] == [1, 2]` | `false` | `true` |
| `{"a": 1} == {"a": 1}` | `false` | `true` |
| `1::Currency == 1::Currency` | `false` | `true` |
| `DATE(2026,1,1) == DATE(2026,1,1)`, `NOW() == NOW()` | `false` | `true` |
| `DATE(2026,1,1) < DATE(2026,1,2)` | `Error` | `true` |
| `TRUE > FALSE` | `false` | `true` |
| `1::Currency < 2::Currency` | `Error` | `true` |

The `!=` forms flip accordingly. The lang-num lane's own breaking changes (`10%-3`,
`ABS("-3")`, `"3"+1`, `TRUE+1`, `1e3`) are listed in its changelog entry.

## Consequences
- \+ Dates, arrays, objects and currency compare the way users expect.
- \+ One rule for every comparison: same type, same value, no conversion, so `1 == "1"` never
  surprises.
- − Text comparison still differs from Excel (case). Use `LOWER(:a) == LOWER(:b)` for a
  case-insensitive match.
- − Comparing two different JSON objects parses both; the parse is charged to the evaluation
  budget (ADR-0011).
