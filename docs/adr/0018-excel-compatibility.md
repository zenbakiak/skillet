# 0018. Excel compatibility: Excel is the contract for builtins and operators

- Status: Accepted; amended by ADR-0023 (`%` percent vs modulo, number coercion in arithmetic)
- Date: 2026-11-21
- Deciders: Maintainer
- Related: BL-054, BL-020 (audit: [`docs/reference/excel-compat.md`](../reference/excel-compat.md)),
  ADR-0005 (amended here: `^` and unary minus), ADR-0006 (null safety), ADR-0010 (breaking
  changes before 1.0), ADR-0013 (null equality), ADR-0015 (NaN), PRD-001

## Context
The Excel audit (BL-020) compared every builtin with Excel. 22 of 56 functions that share an Excel
name give a different result for ordinary inputs. Some are plain bugs (`IPMT`, `COUNTIF`). Others
are choices nobody made on purpose (`MOD` sign, `TRIM`, `COUNT`, `^` associativity). The audit
listed ten decisions, D-1 to D-10. On 2026-11-21 the maintainer decided: "make it Excel compliant".

The primary users are analysts who know Excel (PRD-001). A builtin that has an Excel name promises
Excel's result. When it gives a different number, the user gets no error, only a wrong answer.

## Decision

### The rule
- **UPPERCASE builtins and operators follow Excel's documented behaviour.** Same arguments, same
  result, and an Excel error value (`#DIV/0!`, `#NUM!`, `#VALUE!`, `#N/A`) becomes a Skillet `Error`.
- **Lowercase methods keep Ruby semantics** (`.trim()`, `.int()`). A method does not
  have to give the same result as the builtin of the same name. Each difference is documented.
  Exception (BL-088, maintainer decision 2026-11-22): `.round([digits])` is `ROUND`.
- Skillet-only builtins (`AVG`, `JOIN`, `DATEADD`, `DIG`, `JQ`, …) keep their own documented
  behaviour. A Skillet extension to an Excel builtin (an optional argument Excel does not have,
  a lambda criterion) is allowed if it does not change any result Excel defines.
- A difference from Excel that is not in "Deliberate differences" below is a bug. Fix it with a
  test; no new ADR is needed.

### D-1: Excel's error cases return `Error`
Where Excel returns an error value, Skillet returns `Error`. This covers:
- Division by zero: `10 / 0`, `0 / 0`, `MOD(5, 0)` → `Error` (today `inf`, `NaN`, `NaN`).
- Impossible math: `SQRT(-1)`, `POWER(0, -1)`, `POWER(-8, 1/3)`, and any operator or builtin
  whose result is not a finite number (Excel `#NUM!`) → `Error`.
- Wrong-type arguments: `ABS("x")`, `SQRT("x")`, `FLOOR("3.7")` → `Error` (today `0`).
- Empty input where Excel errors: `AVERAGE()`, `MEDIAN()`, `STDEV.P()`, `VAR.P()`,
  `PERCENTILE.INC([], 0.5)`, `QUARTILE.INC([], 1)`, `AVERAGEIF` with no match → `Error`.
  `MODE.SNGL` with no repeated value → `Error` (`#N/A`).
- Arguments that are silently ignored today (audit X-5): honour them or return `Error`.
  `ROUND(2.5, "x")` → `Error`; `PMT(0.05, 10, 1000, "x")` → `Error`.
- A lambda criterion (`COUNTIF`, `SUMIF`, `AVGIF`) that returns a non-boolean → `Error` (B-17).

**Exception (ADR-0006):** `Null` stays tolerated where ADR-0006 and Excel's blank cells agree.
Aggregates skip `Null` items the way Excel skips blank cells (`SUM([1, NULL])` → `1`,
`AVERAGE([1, NULL, 3])` → `2`). `to_*` conversions, `&.` and `ISBLANK` stay total. Where Excel
returns a value for empty input, Skillet does too: `SUM()` → `0`, `MAX()` → `0`, `COUNT([])` → `0`.

Error messages name the function and the cause. The implementation sets the exact texts and tests
pin them.

ADR-0015 (accepted, option B) still applies to NaN that arrives from a host value or a custom
function. With D-1, expressions cannot produce NaN or infinity themselves.

### D-2: Fix `TIME`, `IPMT` and `COUNTIF`
- **`TIME(h, m, s)`** returns a fraction of a day, as Excel does: `TIME(14, 30, 0)` →
  `0.6041666666666666`, `TIME(12, 0, 0)` → `0.5`. Values roll over like Excel: `TIME(0, 90, 0)`
  → `0.0625`, and hours of 24 or more wrap (`TIME(25, 0, 0)` → `TIME(1, 0, 0)`). A negative total
  → `Error`. This unit fits D-5: `DATE(2026, 1, 1) + TIME(12, 0, 0)` is noon. `TIME()` with no
  arguments is a Skillet extension; it returns the current local time in the same unit.
- **`IPMT(rate, per, nper, pv, [fv], [type])`** uses Excel's sign and balance rule. Interest on a
  positive `pv` is negative: `IPMT(0.1, 1, 3, 1000)` → `-100`, `IPMT(0.1, 2, 3, 1000)` →
  `-69.79…`, `IPMT(0.1, 3, 3, 1000)` → `-36.56…`; with `type` 1, period 1 → `0` and period 2 →
  `-63.44…`. `PMT = IPMT + PPMT` holds for every period (D-9 adds `PPMT`).
- **`COUNTIF(range, criteria)`** accepts Excel criteria, with the same parser as `SUMIF`:
  `COUNTIF([1,2,3,4], ">2")` → `2`, `COUNTIF([1,2,3], 2)` → `1`, `COUNTIF(["a","A","b"], "a")`
  → `2`, `COUNTIF(["apple","Avocado","pear"], "a*")` → `2`. Text criteria ignore case and support
  the wildcards `*`, `?` and `~`. The lambda form (`COUNTIF(:a, :x > 2)`) stays as an extension.

### D-3: `COUNT` counts numbers; add `COUNTA`
- `COUNT(value1, ...)` is variadic and counts numbers only (`Number`, `Currency`, `DateTime`).
  Array arguments are flattened. `COUNT([1, "a", TRUE, NULL])` → `1`, `COUNT(1, 2, 3)` → `3`.
- `COUNTA(value1, ...)` counts every value that is not `Null`, including `""`:
  `COUNTA([1, "a", TRUE, NULL])` → `3`.
- `.length()` and `.count()` keep returning the array length.

### D-4: Dotted names
The lexer accepts an UPPERCASE name with dotted parts, `NAME(.NAME)*`, when a `(` follows directly.
`STDEV.P(1, 2)` → `0.5`. The dotted names are `STDEV.P`, `STDEV.S`, `VAR.P`, `VAR.S`,
`MODE.SNGL`, `PERCENTILE.INC` and `QUARTILE.INC`. The underscore spellings (`STDEV_P`, …) and the
legacy names (`STDEVP`, `VARP`) stay as aliases. No valid expression changes: an UPPERCASE name
without `(` is not valid today, variables start with `:`, and methods are lowercase.

### D-5: `DATE` rollover, Excel years, date arithmetic in days
- **Rollover.** Months and days out of range roll over: `DATE(2026, 13, 1)` → 2027-01-01,
  `DATE(2026, 2, 30)` → 2026-03-02, `DATE(2026, 1, 0)` → 2025-12-31, `DATE(2026, 0, 1)` →
  2025-12-01. So `DATE(y, m + 1, 0)` is the last day of month `m`.
- **Years.** Excel's rule: a year from 0 to 1899 gets 1900 added (`DATE(26, 1, 1)` → 1926-01-01,
  `DATE(0, 1, 1)` → 1900-01-01). 1900 to 9999 are used as given. Below 0 or above 9999 → `Error`.
- **Arithmetic in days.** A `Number` next to a `DateTime` counts days, fractions included:

  | Expression | Result |
  |---|---|
  | `DATE(2026, 1, 1) + 1` | `DATE(2026, 1, 2)` |
  | `1 + DATE(2026, 1, 1)` | `DATE(2026, 1, 2)` |
  | `DATE(2026, 1, 1) - 1` | `DATE(2025, 12, 31)` |
  | `DATE(2026, 1, 1) + 0.5` | 2026-01-01 12:00:00 UTC |
  | `DATE(2026, 1, 2) - DATE(2026, 1, 1)` | `1` |
  | `DATE(2026, 1, 1) - DATE(2026, 1, 2)` | `-1` |
  | `DATE(2026, 1, 1) + DATE(2026, 1, 1)` | `Error` |

  `DateTime` stays whole Unix seconds (UTC). A fractional day is rounded to the nearest second, so
  `DATE(2026, 1, 1) + TIME(0, 0, 1)` is exactly one second later.
- **String casts stay strict.** `::DateTime` never rolls over and never guesses a format.
  `"2026-02-30"::DateTime` → `Error`. Which string formats the cast accepts is a separate
  proposal (excel-compat.md, "string to DateTime cast formats").

### D-6: `FIND` dispatches on the first argument's type
- First argument is a string: Excel `FIND(find_text, within_text, [start_num])`. It is
  case-sensitive and returns a 1-based position: `FIND("b", "abc")` → `2`. Not found, or
  `start_num` out of range → `Error` (`#VALUE!`).
- First argument is an array: the current array `FIND(array, lambda)`. `FIND([1,2,3], :x > 1)` → `2`.
- Any other type → `Error`.

### D-7: Excel `MOD`, `TRIM` and `SUMIF`; Ruby methods
- **`MOD(n, d)`** = `n - d * INT(n / d)`, so the result has the sign of the divisor:
  `MOD(-10, 3)` → `2`, `MOD(10, -3)` → `-2`, `MOD(5.5, 2)` → `1.5`. The `%` operator stays a
  truncated remainder (`-10 % 3` → `-1`), see "Deliberate differences".
- **`TRIM(text)`** removes leading and trailing spaces and collapses inner runs of spaces to one:
  `TRIM("  a   b  ")` → `"a b"`. The method `.trim()` keeps Ruby's `strip`: `"a   b"`.
- **`SUMIF(range, criteria, [sum_range])`** matches text case-insensitively and supports `*`, `?`
  and `~`: `SUMIF(["A","b"], "a", [1,2])` → `1`, `SUMIF(["apple","avocado"], "a*", [1,2])` → `3`.
  A text criterion like `">=x"` compares text cells only, so numbers never match it (Excel: `0`).

### D-8: Lazy `IF` and `IFS`
`IF` evaluates only the branch it picks. `IFS` evaluates conditions in order and only the value of
the first true one. `IF(TRUE, 1, UPPER(1))` → `1`; `IF(:n == 0, 0, 1 / :n)` is a safe guard.
`IFS` with no true condition → `Error` (`#N/A`). A text condition (`IF("x", 1, 2)`) → `Error`
(`#VALUE!`). `Null` as a condition stays false, like a blank cell.

### D-9: Add Excel names
| Name | Behaviour |
|---|---|
| `LEN(text)` | Number of characters. Same as `LENGTH` for strings |
| `TODAY()` | Today at 00:00:00 UTC, the same as `DATE()` |
| `PPMT(rate, per, nper, pv, [fv], [type])` | Principal part of a payment. `PMT = IPMT + PPMT`. `PPMT(0.1, 1, 3, 1000)` → `-302.11…` |
| `PV(rate, nper, pmt, [fv], [type])` | Present value. `PV(0.05, 10, -100)` → `772.17…` |
| `NPV(rate, value1, ...)` | Net present value; arrays are flattened. `NPV(0.1, 100, 100)` → `173.55…` |
| `TEXTJOIN(delimiter, ignore_empty, text1, ...)` | Excel argument order (delimiter first). `JOIN` stays as is |
| `PERCENTILE`, `QUARTILE`, `MODE` | Legacy aliases of `PERCENTILE.INC`, `QUARTILE.INC`, `MODE.SNGL` |
| `AVERAGEIF(range, criteria, [average_range])` | Criteria as in `SUMIF`. No match → `Error` |
| `STDEV.S`, `STDEV` | Sample standard deviation (divides by n − 1). `STDEV.S(2,4,4,4,5,5,7,9)` → `2.138…`. Fewer than 2 numbers → `Error` |
| `STDEV.P` | Population standard deviation (dotted name, D-4) |
| `VAR.S`, `VAR` | Sample variance. `VAR.S(2,4,4,4,5,5,7,9)` → `4.571…`. Fewer than 2 numbers → `Error` |
| `VAR.P` | Population variance (dotted name, D-4) |

### D-10: Excel operator precedence
This overrides the audit's recommendation (keep the math convention) and amends ADR-0005.
- **Unary minus binds tighter than `^`**: `=-2^2` → `4`, `=-3^2` → `9`, `=2*-3^2` → `18`.
- **`^` is left-associative**: `=2^3^2` → `64` (`(2^3)^2`).
- Binary minus is unchanged: `=0-2^2` → `-4`, `=1-2^2` → `-3`. `=2^-2` → `0.25` (unchanged).
- Write `-(2^2)` for the old meaning of `-2^2`, and `2^(3^2)` for the old `2^3^2`.
- Postfix operations (method calls, property access, index) still bind tighter than unary minus.

### Also decided
- **`ROUND(number, num_digits)`**: negative digits round left of the decimal point:
  `ROUND(1234.567, -2)` → `1200`, `ROUND(15, -1)` → `20`. Halves round away from zero, on the
  decimal value: `ROUND(1.005, 2)` → `1.01`. A non-numeric `num_digits` → `Error`. A `Currency`
  input stays `Currency`. `num_digits` may be left out (default 0), as a Skillet extension.
- **`INT(number)`** rounds down: `INT(-3.7)` → `-4` (unchanged). `INT("x")` → `Error` (D-1). The
  method `.int()` keeps truncation, like Ruby's `to_i`: `(-3.7).int()` → `-3`.
- **`CEILING(number, [significance])` and `FLOOR(number, [significance])`** honour significance:
  `CEILING(2.1, 0.5)` → `2.5`, `CEILING(7, 5)` → `10`, `CEILING(-2.5, -2)` → `-4`,
  `FLOOR(7, 5)` → `5`, `FLOOR(2.9)` → `2`. A positive number with a negative significance →
  `Error` (`#NUM!`). Significance defaults to 1, so the one-argument form keeps working. `CEIL`
  stays an alias of `CEILING`.

### Deliberate differences (kept)
These differ from Excel on purpose. Docs must say so.

| Area | Skillet | Excel | Why |
|---|---|---|---|
| `NULL == 0`, `NULL == ""` | `false` | blank equals `0` and `""` | Skillet has a real `Null` (ADR-0013) |
| `%` operator | Remainder: `-10 % 3` → `-1` | Percent suffix: `10%` = 0.1 | Changing it would break every `%` |
| `&` operator | Not an operator (`&.` is safe navigation) | Joins text | Use `CONCAT` |
| `"a" == "A"` | `false` (case-sensitive) | `TRUE` | Revisit in a separate ADR if users ask |
| Direct text arguments | `SUM(1, "2", TRUE)` → `1` | 4 | No cell ranges: every argument is treated like a range value (audit X-3) |
| `ISBLANK("")` | `true` | FALSE | ADR-0006 relies on "null or blank" |
| `UPPER(1)`, `LEFT(12345, 2)` | `Error` | Number converted to text | Errors over silent coercion |
| `YEAR(46000)` | `Error` | Serial numbers accepted | `DateTime` is a real type, not a serial number |
| `NOW()`, `TODAY()` | UTC, whole seconds | Local time, serial number | Same result on every server |
| `REDUCE`, `MAP`, `FILTER` | Lambda with implicit `:x` / `:acc` | `LAMBDA(...)`; `REDUCE(initial, array, …)` | Skillet lambda syntax (ADR-0005) |
| Number to text | Shortest round-trip (`"0.30000000000000004"`) | 15 significant digits | Use `ROUND` first (audit X-7) |

## Breaking changes (ship in 0.7.0)
Before 1.0 a minor release may break behaviour (ADR-0010 precedent). These ship in **0.7.0** and
are listed in the changelog and in [`docs/guide/migration-0.7.md`](../guide/migration-0.7.md).

| Expression | 0.6.x | 0.7.0 |
|---|---|---|
| `-2^2` | `-4` | `4` |
| `2^3^2` | `512` | `64` |
| `10 / 0`, `0 / 0` | `inf`, `NaN` | `Error` |
| `SQRT(-1)`, `ABS("x")` | `NaN`, `0` | `Error` |
| `AVERAGE()`, `MEDIAN()` | `0` | `Error` |
| `MODE_SNGL(1, 2, 3)` | `1` | `Error` |
| `TIME(14, 30, 0)` | current time (0.6.3) | `0.6041666666666666` |
| `IPMT(0.1, 2, 3, 1000)` | `49.79…` | `-69.79…` |
| `COUNTIF([1,2,3,4], ">2")` | `0` | `2` |
| `COUNT([1, "a", TRUE, NULL])` | `4` | `1` |
| `DATE(2026, 13, 1)` | `Error` | 2027-01-01 |
| `DATE(26, 1, 1)` | year 26 | 1926-01-01 |
| `DATE(2026,1,2) - DATE(2026,1,1)` | `Error` | `1` |
| `FIND("b", "abc")` | `Error` | `2` |
| `MOD(-10, 3)` | `-1` | `2` |
| `TRIM("  a   b  ")` | `"a   b"` | `"a b"` |
| `SUMIF(["A","b"], "a", [1,2])` | `0` | `1` |
| `IF(TRUE, 1, UPPER(1))` | `Error` | `1` |
| `IF("x", 1, 2)`, `IFS(FALSE, 1)` | `2`, `false` | `Error` |
| `ROUND(1234.567, -2)` | `1235` | `1200` |
| `CEILING(2.1, 0.5)`, `FLOOR(7, 5)` | `3`, `7` | `2.5`, `5` |

Additive only (errors become values): dotted names (D-4), the new names (D-9), `COUNTA`.

## Consequences
- \+ One rule answers "what should this return?" for every Excel name: look it up in Excel.
- \+ Formulas copied from a spreadsheet give the spreadsheet's answer, or an error. Never a
  silently different number.
- − Many existing results change in 0.7.0 (table above). Tests that pin the old values are
  updated in the implementation PRs, and each PR lists them.
- − Function and method of the same name can now differ (`TRIM` / `.trim()`, `INT` / `.int()`).
  The reference must show both.
- − `^` and unary minus now differ from most programming languages. A formula with `-x^2` changes
  meaning silently; the migration guide calls it out first.
- Implementation is split across lanes (lang-core: precedence, dotted names, lazy `IF`; lang-fix:
  existing builtins; lang-new: new names, `FIND`, `DATE`, date arithmetic). Each lane adds tests
  for its rows of this ADR.

## Alternatives considered
- **Keep the math convention for `^`** (the audit's D-10 recommendation) and correct PRD-001
  instead. Rejected by the maintainer: PRD-001 already promised Excel precedence, and the users
  are Excel users.
- **Case by case** (one ADR per function). Rejected: the same question keeps coming back, and one
  rule is easier to apply and to review.
- **Excel semantics for methods too.** Rejected: methods are the Ruby half of the language
  (ADR-0005), and Ruby users expect `strip` and `to_i`.
