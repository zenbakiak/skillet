# 0033. One numeric type; `::Currency` is a conversion

- Status: Accepted
- Date: 2026-11-22
- Deciders: Maintainer
- Related: BL-097, ADR-0003 (amended here), ADR-0027 and its amendment 1, ADR-0031 and its
  amendment 1 (Currency parts superseded here), ADR-0023 (Currency parts, via ADR-0027
  amendment 1), ADR-0030 (Currency rows moot), BL-091 (Currency text form moot), AGENTS.md §7

## Context
ADR-0003 made `Currency` a tagged `f64` that "is accepted wherever numbers are". Since then each
release has had to decide, operator by operator and builtin by builtin, what the tag does:
ADR-0027 (operators), its amendment 1 (comparisons), ADR-0031 (aggregates), its amendment 1
(`SUMIF`/`AVERAGEIF`, `.join()`), and an open follow-up for `CONTAINS`/`UNIQUE`. The tag buys
nothing a number does not have:

- **It is not money math.** `0.1::Currency + 0.2::Currency` is `Currency(0.30000000000000004)`,
  and `1::Currency / 3` is `Currency(0.3333333333333333)`. Nothing rounds; the tag is only a label.
- **It is slower.** Currency arithmetic leaves the `Number op Number` fast path for the cold
  `currency_arithmetic` path; the BL-097 measurement found it about 2.2x slower than the same
  arithmetic on Numbers.
- **It has four text forms.** For `2.5::Currency`: `::String` and `JOIN` give `"2.5000"`, `.to_s()`
  gives `"2.50"`, `CONCAT`/`&`/`TEXTJOIN`/`LEN` and `.to_json()`/`::Json` give `"2.5"`, and
  `.join()` was an error until BL-078 (now `"2.5000"`).
- **It is a second type everywhere.** 71 `Value::Currency` sites in `src/` (72 with one unit
  test), on develop before #112 and #115, which add more. Several still reject it: `.sort()`, the
  number methods (`(2.5::Currency).abs()` is `Error: No methods available for Currency(2.5) type`),
  `.positive?`, `DATEADD`, `SORT`'s order, date arithmetic; and `CONTAINS([1::Currency], 1)` is
  `false`.

In Excel, currency, percent and integer are cell **formats**; there is one number type. On
2026-11-22 the maintainer decided that Skillet does the same.

## Decision

### 1. One numeric type: `Number` (`f64`)
Every number the language makes is a `Value::Number`. Nothing in the language produces a
`Value::Currency` any more: not a cast, an operator, a builtin, a method or a literal. The
`Currency` rules of ADR-0027, ADR-0031 and their amendments no longer apply (§6), because there
is nothing for them to apply to: a value that was Currency is now a Number, and Numbers already
follow the number rules everywhere.

### 2. `::Integer` and `::Float` stay conversions to Number (unchanged)
Both already return a Number. Their exact behaviour today, kept as it is:

**`x::Integer`** truncates toward zero, like Excel `TRUNC`, not like Excel `INT` (which rounds
down: Skillet's `INT(-7.9)` is `-8`, Excel's too). Skillet has no `TRUNC` builtin; `.int()` also
truncates.

| Input | Rule | Examples |
|---|---|---|
| Number | toward zero (Rust `n as i64`): out of range saturates at the `i64` limits, NaN is 0 | `7.9` → `7`, `-7.9` → `-7`, `-7.5` → `-7`, `(0-0.5)::Integer` → `0` (not `-0`), `1e20` → `9.223372036854776e18` |
| Text | a prefix scan: an optional sign, digits, one `.`; it stops at the first other character; no number is `0`; then toward zero. No trimming, no saturation | `"3.7"` → `3`, `"-7.9"` → `-7`, `"+3.9x"` → `3`, `"1,5"` → `1`, `"1e3"` → `1`, `" 5"` → `0`, `"abc"` → `0`, `"99999999999999999999"` → `1e20` |
| `TRUE` / `FALSE` / `NULL` | `1` / `0` / `0` | |
| DateTime, array, JSON | `Error: Cannot cast to Integer` | |

**`x::Float`**: a Number as it is; text with Rust's strict `f64` parse (no trimming;
`"12.5"` → `12.5`, `"1e3"` → `1000`, `" 12.5 "` and `"abc"` → `Error: Cannot cast String to
Float`); `TRUE`/`FALSE`/`NULL` → `1`/`0`/`0`; DateTime, array, JSON →
`Error: Cannot cast to Float`.

### 3. `::Currency` rounds to 4 decimals and returns a Number
`x::Currency` reads its input with the same rules as today, then rounds the amount to 4
decimals (VBA `Currency` precision) and returns a `Value::Number`.

- **Input rules, unchanged**: a Number (or a host Currency, §5) as it is; text with the strict
  `f64` parse (`"12.5"`, `"+5"`, `"1e3"` work; `" 12.5 "`, `"$5"`, `"1,5"`, `"0x10"`, `"abc"` are
  `Error: Cannot cast String to Currency`); `TRUE`/`FALSE`/`NULL` → `1`/`0`/`0`; DateTime, array,
  JSON → `Error: Cannot cast to Currency`.
- **Rounding**: half away from zero on the number's 15-significant-digit decimal form, exactly
  `ROUND(x, 4)` (`round_half_away(n, 4)` in `src/runtime/arithmetic.rs`). So
  `1.00005::Currency` is `1.0001`, `-1.00005::Currency` is `-1.0001`, `0.00005::Currency` is
  `0.0001` and `(0.1 + 0.2)::Currency` is `0.3`.
- **Negative zero** becomes `0`: `(0 - 0.00004)::Currency` is `0`, not `-0` (as ADR-0027 made
  `-(0::Currency)` `0`). `ROUND(0 - 0.00004, 4)` keeps printing `-0`; only the cast normalises.
- **Non-finite text** keeps today's parse: `"inf"::Currency` and `"NaN"::Currency` parse (as they
  do for `::Float`) and stay `inf`/`NaN` after rounding. No range check is added (VBA's Currency
  stops at about ±9.2e14; Skillet does not).
- **Deliberate difference from VBA**: `CCur` rounds half to even (`CCur(0.00005)` is `0`);
  Skillet uses Excel's `ROUND` rule (`0.0001`), so `x::Currency` and `ROUND(x, 4)` never differ.

The cast rounds once. Arithmetic on the result is plain `f64` arithmetic and is not rounded
again: `0.1::Currency + 0.2::Currency` is `0.30000000000000004` (both operands are already
exact to 4 decimals, the sum is not). Round the result: `ROUND(:a + :b, 2)` or
`(:a + :b)::Currency`.

### 4. `Value::Currency` is deprecated in 0.7.0 and removed in 0.8
- The variant stays in `types::Value` with `#[deprecated(since = "0.7.0", note = "Skillet has
  one number type (ADR-0033); use Value::Number")]`, so code that builds or matches it keeps
  compiling (AGENTS.md §7), with a deprecation warning. It is removed in 0.8.0.
- `ast::TypeName::Currency` and the `::Currency` cast keyword stay (they are a conversion now).

### 5. A `Value::Currency` from Rust is a Number
A host can still pass one in: a variable in the maps of `evaluate_with*`/`evaluate_parsed_with*`,
a `CustomFunction::execute` result, or a value given to a public `runtime::` function. It is
treated as `Value::Number` of the same amount, everywhere:

1. **Normalised at the API boundary.** The evaluator's entry functions (`eval_with*` in
   `src/runtime/evaluator.rs`, which every `lib.rs` function with a variable map calls) check the
   map for a Currency, items of arrays included. If there is none (every JSON entry point, and
   every host that does not use the variant), the map is used as it is, with no copy. If there
   is one, the evaluation runs on a copy where every Currency is a Number. The result of
   `CustomFunction::execute` is normalised the same way in `execute_checked` (`src/custom.rs`).
   A result therefore never holds a Currency: `=:price` with `price = Value::Currency(2.5)`
   returns `Value::Number(2.5)`, and so does `=[:price]` inside its array. Assignment outputs
   (`evaluate_*_and_context`) hold Numbers too.
2. **Read as a Number where the runtime still matches it.** Every `match` that keeps a
   `Value::Currency` arm (an exhaustive `match` must, until 0.8) handles it exactly like
   `Value::Number`: `Value::Number(n) | Value::Currency(n)`. No arm may panic or use
   `unreachable!()`: a public `runtime::` function called directly with a Currency gets the
   Number result. This also covers `Value::as_number()`, which returns `Some(n)` for a Currency.
3. The derived `PartialEq` is not changed: `Value::Currency(1.0) == Value::Number(1.0)` is still
   `false` in Rust. Host code that compares results with `Value::Currency(..)` must compare with
   `Value::Number(..)`.

### 6. What this supersedes
| Decision | Status |
|---|---|
| ADR-0003: "`Currency` is a tagged `f64` and is accepted wherever numbers are" | superseded: there is one number type; `::Currency` is a conversion (§3) |
| ADR-0027 decision 1, Currency bullet (`DATE(2026,1,1) + 1::Currency` is an error) and decision 4 second bullet | superseded: `1::Currency` is the Number `1`, so the sum is 2026-01-02 |
| ADR-0027 decision 2 (Currency in the operators, its result-type table, "no rounding") and decision 4 first bullet (unary `+` keeps Currency) | superseded: the operands are Numbers; the Number rules and errors apply |
| ADR-0027 Implementation constraint, `currency_arithmetic` | superseded: the function goes; `date_arithmetic`, `coerced_negate`, `coerced_plus` stay |
| ADR-0027 amendment 1 (Currency compares with Number by amount) | moot: both sides are Numbers. Its results all hold, and the "unchanged" rows for `CONTAINS`, `IN`, `.includes()` and `UNIQUE` change (they now see two equal Numbers) |
| ADR-0027 Context sentence on Currency text, and Consequences on Currency | superseded |
| ADR-0023 §1, §2, §4 Currency parts (already replaced by ADR-0027 amendment 1) | moot |
| ADR-0031 decision 3 (aggregates keep Currency, B-13) | superseded: `SUM`, `PRODUCT`, `AVERAGE`, `MIN`, `MAX` return Numbers |
| ADR-0031 "Amends ADR-0027" paragraph, Currency sentence | superseded |
| ADR-0031 amendment 1 item A3 (`.join()` writes Currency with 4 decimals) | superseded: there is no Currency item; `NULL` as nothing stays |
| ADR-0031 amendment 1 item A5 (`SUMIF`/`AVERAGEIF` keep Currency) | superseded: they return Numbers |
| ADR-0031 amendment 1 item A1 example `1.5::Currency::String` → `"1.5000"` | now `"1.5"` (the cast chain rule itself stays) |
| ADR-0030 rows E13, E24, E26 and "a `Currency` stays a `Currency`" in objects | moot |
| BL-091 Currency text form; the `CONTAINS`/`UNIQUE`-with-Currency follow-up | moot |

Everything else in those ADRs (date arithmetic, ISO 8601 date text, cast chaining, `.join()` of
`NULL`, comparisons) is unchanged.

### 7. Out of scope: exact decimal money
This ADR does not add exact decimal arithmetic. A real decimal type (for example
`rust_decimal`, ADR-0003 "Alternatives") would be a new `Value` variant with its own literal or
cast, its own operator, aggregate, text and JSON rules, and a wire format; it needs its own ADR.

## Before -> after
Run with `sk` on develop (`9b36d9b`, before #112 and #115). "#112"/"#115" marks a result those
PRs change first; the "after" column is the final 0.7.0 result.

| Expression | Before | After |
|---|---|---|
| **Casts** | | |
| `1.23456789::Currency` | `Currency(1.23456789)` | `1.2346` (Number) |
| `12.5::Currency`, `"12.5"::Currency` | `Currency(12.5)` | `12.5` |
| `1.00005::Currency`, `-1.00005::Currency`, `0.00005::Currency` | `Currency(1.00005)`, `Currency(-1.00005)`, `Currency(0.00005)` | `1.0001`, `-1.0001`, `0.0001` |
| `(0.1 + 0.2)::Currency`, `(1/3)::Currency` | `Currency(0.30000000000000004)`, `Currency(0.3333333333333333)` | `0.3`, `0.3333` |
| `(0 - 0.00004)::Currency` | `Currency(-0.00004)` | `0` (not `-0`) |
| `TRUE::Currency`, `NULL::Currency`, `"1e3"::Currency` | `Currency(1)`, `Currency(0)`, `Currency(1000)` | `1`, `0`, `1000` |
| `" 12.5 "::Currency`, `"$5"::Currency`, `"abc"::Currency` | `Error: Cannot cast String to Currency` | unchanged |
| `DATE(2026,1,1)::Currency`, `[1]::Currency` | `Error: Cannot cast to Currency` | unchanged |
| `(1.23456::Currency)::Integer`, `(2.5::Currency)::Float` | `1`, `2.5` | unchanged |
| `-7.9::Integer`, `(0-7.9)::Integer`, `"abc"::Integer`, `"-7.9"::Integer` | `-7`, `-7`, `0`, `-7` | unchanged |
| **Arithmetic** | | |
| `0.1::Currency + 0.2::Currency` | `Currency(0.30000000000000004)` | `0.30000000000000004` (Number) |
| `1::Currency + 1`, `2 * 1::Currency` | `Currency(2)` | `2` |
| `1::Currency / 3` | `Currency(0.3333333333333333)` | `0.3333333333333333` |
| `10::Currency / 4::Currency`, `2::Currency ^ 2`, `7::Currency % 4` | `2.5`, `4`, `3` | unchanged |
| `-(5::Currency)`, `+(5::Currency)`, `(10::Currency)%` | `Currency(-5)`, `Currency(5)`, `Currency(0.1)` | `-5`, `5`, `0.1` |
| `DATE(2026,1,1) + 1::Currency` | `Error: Arithmetic op on non-number` | 2026-01-02 (`DateTime`) |
| `1::Currency / 0`, `1e300::Currency * 1e10` | `#DIV/0!`, `#NUM!` errors | unchanged |
| **Aggregates** | | |
| `SUM(10::Currency, 5)`, `PRODUCT(2::Currency, 3)` | `Currency(15)`, `Currency(6)` | `15`, `6` |
| `AVERAGE(10::Currency, 20)` | `Currency(15)` | `15` |
| `MIN(1, 3::Currency)`, `MAX(3::Currency, 9::Currency)` | `Currency(1)`, `Currency(9)` | `1`, `9` |
| `SUMIF([1,2::Currency,3], ">1")`, `AVERAGEIF([1,2::Currency,3], ">1")` | `5`, `2.5` (#112: `Currency(5)`, `Currency(2.5)`) | `5`, `2.5` |
| `ROUND(10::Currency, 1)`, `ROUND(1.23456::Currency, 2)` | `Currency(10)`, `Currency(1.23)` | `10`, `1.23` |
| `[1::Currency, 2].sum()`, `MEDIAN(1::Currency, 2)`, `COUNT(1::Currency)` | `3`, `1.5`, `1` | unchanged |
| **Comparisons and membership** | | |
| `1 == 1::Currency`, `SUM(10::Currency, 5) == 15` | `false` (#115: `true`) | `true` |
| `1::Currency < 2`, `SUM(10::Currency, 5) > 10` | `Error: Comparison of incompatible types` (#115: `true`) | `true` |
| `[1::Currency, 2] == [1, 2]`, `FILTER([1,2], :x > 1::Currency)` | `false`, `Error` (#115: `true`, `[2]`) | `true`, `[2]` |
| `CONTAINS([1::Currency], 1)`, `[1::Currency].includes(1)` | `false` | `true` |
| `UNIQUE([1, 1::Currency])` | `[1, Currency(1)]` | `[1]` |
| `15::Currency == "15"`, `1::Currency == TRUE` | `false` | unchanged |
| **Text** | | |
| `(2.5::Currency)::String`, `(5::Currency)::String` | `"2.5000"`, `"5.0000"` | `"2.5"`, `"5"` |
| `(1.23456::Currency)::String` | `"1.2346"` | `"1.2346"` (the amount is rounded now, not the text) |
| `JOIN([2.5::Currency])`, `JOIN([1, 2.5::Currency, NULL], ",")` | `"2.5000"`, `"1,2.5000,"` | `"2.5"`, `"1,2.5,"` |
| `[2.5::Currency].join("-")` | `Error: join method cannot convert value to string` (#112: `"2.5000"`) | `"2.5"` |
| `(2.5::Currency).to_s()`, `[2.5::Currency].to_s()` | `"2.50"`, `"[2.50]"` | `"2.5"`, `"[2.5]"` |
| `CONCAT(2.5::Currency)`, `2.5::Currency & ""`, `TEXTJOIN(",", TRUE, 2.5::Currency)` | `"2.5"` | unchanged |
| `LEN(2.5::Currency)`, `LEN(1.23456::Currency)` | `3`, `7` | `3`, `6` (the amount is `1.2346`) |
| **JSON** | | |
| `(2.5::Currency).to_json()`, `(2.5::Currency)::Json`, `{"a": 2.5::Currency}` | `Json("2.5")`, `Json("2.5")`, `Json("{\"a\":2.5}")` | unchanged |
| `sk --output-json "=2.5::Currency"`, `sk_http_server` / `sk_server` structured output | `"result": 2.5, "type": "Currency"` | `"result": 2.5, "type": "Number"` |
| `[1::Currency]` in any response | `[1.0]`, `"type": "Array"` | unchanged |
| **Methods and builtins that rejected Currency** | | |
| `[1::Currency, 2].sort()` | `Error: sort expects numeric array` | `[1, 2]` |
| `(2.5::Currency).abs()`, `(2.567::Currency).round(2)`, `(2.5::Currency).between(1, 3)` | `Error: No methods available for Currency(…) type` | `2.5`, `2.57`, `true` |
| `(2.5::Currency).positive?` | `false` | `true` |
| `DATEADD(DATE(2026,1,1), 1::Currency, "days")` | `Error: DATEADD expects number as second argument` | 2026-01-02 |
| `SORT([3,1,2], -1::Currency)` | `Error: SORT order must be …` | `[3, 2, 1]` |
| `SORT([3, 1::Currency, 2])` | `[Currency(1), 2, 3]` | `[1, 2, 3]` |
| `ISNUMBER(1::Currency)`, `IF(1::Currency, "y", "n")`, `NOT 0::Currency` | `true`, `"y"`, `true` | unchanged |
| **Host values (Rust)** | | |
| `=:p` with `p = Value::Currency(2.5)` | `Value::Currency(2.5)` | `Value::Number(2.5)` |
| `=:p * 2`, `=:p == 2.5`, `=[:p].sort()` with that `p` | `Currency(5)`, `false`, `Error` | `5`, `true`, `[2.5]` |
| A custom function returning `Value::Currency(3.0)`, then `+ 1` | `Currency(4)` | `4` |

## Consequences
- \+ One number type, as in Excel. A value converted with `::Currency` works everywhere a number
  works, with no rules left to define per function.
- \+ One text form for every number (Rust's shortest round-trip `f64` text, excel-compat X-7).
- \+ Faster: what were Currency operations run on the `Number op Number` fast path, and the
  aggregates lose their Currency branch. Expected: the ~2.2x Currency penalty disappears;
  Number-only formulas stay as fast. To be measured in the implementation (AGENTS.md §8).
- \+ About 70 `Value::Currency` arms become merged Number arms now, and all go in 0.8.
- − **Breaking**: every result that was `Currency` is a `Number` (results, `"type"` in responses,
  `match` arms in host code). `::Currency` now changes the amount (rounds to 4 decimals). Text of
  a converted value loses the fixed decimals (`"2.5000"` → `"2.5"`, `.to_s()` `"2.50"` → `"2.5"`).
  `DATE(…) + 1::Currency` is a date instead of an error. `CONTAINS`/`UNIQUE` treat `1::Currency`
  and `1` as equal.
- − Host code that built `Value::Currency` gets a deprecation warning in 0.7 and a compile error
  in 0.8.
- − Money is still binary floating point: sums of 4-decimal amounts are not exact. Round results
  (`ROUND(x, 2)`) as before; exact decimals need their own ADR (§7).
- − The 0.7.0 development builds' Currency rules (ADR-0027, ADR-0031) never ship; only this one does.

## Migration notes
- Formulas: `x::Currency` keeps working and now rounds to 4 decimals. To keep the amount exactly,
  use `x::Float`. To format money with 2 decimals, there is no format function yet; round with
  `ROUND(x, 2)`.
- Text: code that expected `"2.5000"` from `::String`/`JOIN` or `"2.50"` from `.to_s()` now gets
  `"2.5"`.
- Responses: clients that read `"type": "Currency"` should read `"Number"`; the `"result"` is the
  same JSON number.
- Rust: replace `Value::Currency(x)` with `Value::Number(x)`; until 0.8 an incoming
  `Value::Currency` is read as a Number (§5), and results never contain one.
