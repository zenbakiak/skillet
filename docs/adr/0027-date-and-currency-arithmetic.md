# 0027. Date and currency arithmetic

- Status: Accepted
- Date: 2026-11-21
- Deciders: Maintainer
- Related: BL-070, ADR-0023 (amended here), ADR-0018 D-5 (date arithmetic), ADR-0003
  (`Currency` is a tagged `f64`), ADR-0004

> **Currency parts superseded by [ADR-0033](0033-one-numeric-type.md) (2026-11-22).** Skillet has
> one number type; `::Currency` rounds to 4 decimals and returns a Number. Superseded: the
> Currency bullet of decision 1, decision 2, the `currency_arithmetic` part of the
> implementation constraint, decision 4, and the Currency rows and consequences. Amendment 1
> (Currency compares by amount) is moot. Date arithmetic (decision 1) and decision 3 stand.

## Context
ADR-0023 decision 6 made the arithmetic operators convert numeric text, booleans and `NULL`
like Excel, but left two operand types out:

- A DateTime operand was never coerced. `DATE(2026,1,1) + NULL` and `DATE(2026,1,1) + "1"` were
  `Error: Arithmetic op on non-number`; Excel returns a date.
- Currency in the operators was unchanged, which meant it was always an error:
  `1::Currency + 1` and `-(1::Currency)` failed, although builtins (`ROUND`, `SUM`, `ABS`, …)
  accept Currency and ADR-0003 says Currency "is accepted wherever numbers are".

BL-070 also said that lowercase methods "ignore" text (`[1,"2"].sum()` ignores `"2"`). That is
wrong: `[1, "2"].sum()` is `Error: sum method expects numeric array`. The methods stay strict.

Currency is `Value::Currency(f64)` (ADR-0003). In an expression only the `::Currency` cast and
`ROUND` of a Currency create it (hosts can also pass it as a variable); nothing rounds it to cents. Text output (`::String`, `JOIN`) prints it with 4 decimals, and JSON
output writes the plain `f64`.

## Decision

### 1. Date arithmetic converts the other operand (like Excel)
- In `DateTime + x`, `x + DateTime` and `DateTime - x`, `x` converts with the ADR-0023 rule for
  operator operands: a number as it is, `NULL` as `0`, `TRUE`/`FALSE` as `1`/`0`, numeric text
  (trimmed) as its number. Other text is `Error: Cannot convert "abc" to a number (#VALUE!)`.
  Arrays and JSON objects stay `Arithmetic op on non-number`.
- `x` counts days, as before: `DATE(2026,1,1) + NULL` is that date, `DATE(2026,1,1) + "1"` is
  2026-01-02, `DATE(2026,1,1) + "0.5"` is noon.
- `DateTime - DateTime` is unchanged (days, a Number). Every other DateTime operation stays an
  error: `x - DateTime`, `DateTime + DateTime`, `*`, `/`, `%`, `^` with a DateTime.
- Currency is not a number of days: `DATE(2026,1,1) + 1::Currency` stays
  `Arithmetic op on non-number` (confirmed by the maintainer on 2026-11-21). A DateTime operand is checked first, so this holds for either
  order.
- Out-of-range shifts stay `Date arithmetic out of range` (`DATE(2026,1,1) + "1e300"`).

### 2. Currency works in the arithmetic operators
With a Currency operand (and no DateTime), the other operand converts as a Number operand does
(ADR-0023): a number, a Currency, `NULL` as `0`, booleans, numeric text. Other text is
`#VALUE!`; arrays and JSON objects stay `Arithmetic op on non-number`.

| Operation | Result type |
|---|---|
| `Currency op Number`, `Number op Currency`, `Currency op Currency` for `+`, `-`, `*` | Currency |
| `Currency / Number`, `Number / Currency` | Currency |
| `Currency / Currency` | Number (a ratio) |
| `^` or `%` (modulo) with any Currency operand | Number |
| unary `-` on Currency | Currency (`0 - x`, so `-(0::Currency)` is `0`, not `-0`) |
| unary `+` on Currency | Currency, unchanged (the identity) |
| percent suffix on Currency (`(10::Currency)%`) | Currency (it is `/ 100`) |

"Number" here also means converted text, booleans and `NULL`: `10::Currency + "2"` is
`Currency(12)`, `NULL - 10::Currency` is `Currency(-10)`.

- **No rounding.** The result is the plain `f64` result of the operation, tagged. `1::Currency / 3`
  is `Currency(0.333…)` and `0.1::Currency + 0.2` is `Currency(0.30000000000000004)`, the same
  binary floating-point behaviour ADR-0003 documents. Round explicitly with `ROUND(x, 2)`, which
  keeps Currency. Rounding inside the operators would make `(a / 3) * 3 != a` in a new way and
  would need a decimal type (ADR-0003, deferred).
- **Errors are the number errors.** Currency uses the same checked arithmetic as numbers:
  division or `%` by 0 and `0 ^` a negative number are `Division by zero (#DIV/0!)`, `0 ^ 0` is
  `#NUM!`, and a non-finite result is `Result is not a finite number (#NUM!)`.
- Comparisons do not change (ADR-0023 decision 2): `1 == 1::Currency` stays `false`, and
  ordering a Currency against a Number stays an error.

### 3. Lowercase methods are unchanged
`[1, "2"].sum()` stays an error, and `DATEADD` intervals stay strict. The BL-070 text that said
the methods "ignore" text is corrected here: they reject it.

### Implementation constraint
`Number op Number` keeps its current code path. Date and currency arithmetic run only after
that path fails, in out-of-line `#[cold]` functions (`date_arithmetic`, `currency_arithmetic`,
`coerced_negate`, `coerced_plus` in `src/runtime/evaluator.rs`).

### 4. Follow-up decisions (maintainer, 2026-11-21)
- Unary `+` on Currency is the identity and keeps Currency: `+(1::Currency)` is `Currency(1)`.
- A Currency operand in date arithmetic stays an error, in either order (decision 1).

## Amends ADR-0023
Decision 6 said "DateTime operands are never coerced (date arithmetic is unchanged), and Currency
in the operators is unchanged". Both parts are replaced by decisions 1 and 2 above. The rest of
decision 6 (what converts, `NULL` strict in builtins, aggregates skip text) is unchanged.

## Behaviour changes (ship in 0.7.0)

| Expression | Before | After |
|---|---|---|
| `DATE(2026,1,1) + NULL` | `Error` | 2026-01-01 |
| `DATE(2026,1,1) + "1"`, `"1" + DATE(2026,1,1)`, `DATE(2026,1,1) + TRUE` | `Error` | 2026-01-02 |
| `DATE(2026,1,1) + "x"` | `Arithmetic op on non-number` | `Cannot convert "x" to a number (#VALUE!)` |
| `1::Currency + 1`, `2 * 1::Currency` | `Error` | `Currency(2)` |
| `10::Currency / 4::Currency` | `Error` | `2.5` (Number) |
| `2::Currency ^ 2`, `7::Currency % 4` | `Error` | `4`, `3` (Number) |
| `-(5::Currency)`, `+(5::Currency)` | `Error` | `Currency(-5)`, `Currency(5)` |

## Consequences
- \+ Dates from data with blank (`NULL`) or text day counts work like a spreadsheet.
- \+ Money values work in formulas (`:price * :qty`) without a `::Float` cast, and keep their type.
- − Currency results are unrounded `f64`; exact money math still needs `ROUND` or a future
  decimal type (ADR-0003).
- − `Currency * Currency` is Currency, although money times money has no unit. This follows
  the maintainer's rule; use `::Float` first when a plain number is wanted.
