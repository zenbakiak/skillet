# Migrating from 0.6 to 0.7

Skillet 0.7.0 contains breaking changes. Before 1.0, a minor release may break behaviour
(ADR-0010). This page lists every breaking change, with the old and the new result. The
[changelog](../../CHANGELOG.md) has the full list of changes, including the ones that do not break.

Check these first. They change results without an error:

1. **`-x^2` and `a^b^c`** (operator precedence, below). Search your formulas for `^`.
2. **`COUNT`** now counts numbers only.
3. **`MOD`** with negative numbers, **`TRIM`** with inner spaces, **`ROUND`** with negative digits.
4. **`JQ`** with a wildcard path that matches one item now returns an array.
5. **`%` before `-` or `+`** is now a percent, not modulo: `10 % -3` is `-2.9` (was `1`).
6. **`1e3`** is now `1000` (it was silently `1`).
7. **`AND`/`OR` with `NULL` or an array holding text**: `AND(TRUE, NULL)` is now `true`.
8. **`DATEADD`/`DATEDIFF` units `"m"`, `"y"`, `"w"`** now follow Excel (B-15): `"m"` is months
   (was minutes), `"y"` is days (was years), `"w"` adds days (was weeks). Search for them.
9. **A date as text** is now ISO 8601 UTC (ADR-0031): `DATE(2026, 1, 1)::String`,
   `CONCAT`, `&`, `JOIN` and `.to_s()` give `"2026-01-01T00:00:00Z"` (was `"1767225600"`).
10. **`SUM`, `PRODUCT`, `AVERAGE`, `MIN`, `MAX`, `SUMIF`, `AVERAGEIF` with a Currency value**
   now return Currency (was Number), for example `SUM(10::Currency, 5)`.

## Expressions

### Operators (ADR-0018 D-10)
Skillet now uses Excel's precedence. Unary minus binds tighter than `^`, and `^` is
left-associative.

| Expression | 0.6 | 0.7 | Keep the old result with |
|---|---|---|---|
| `-2^2` | `-4` | `4` | `-(2^2)` |
| `10 % -3`, `10%-3` (BL-066) | `1` (modulo) | `-2.9` (`10% - 3`) | `MOD(10, -3)` (Excel sign) or `10 % (-3)` |
| `-:x^2` (`x` = 3) | `-9` | `9` | `-(:x^2)` |
| `2*-3^2` | `-18` | `18` | `2*-(3^2)` |
| `2^3^2` | `512` | `64` | `2^(3^2)` |
| `0-2^2`, `1-2^2`, `2^-2` | `-4`, `-3`, `0.25` | unchanged | |

### Parsing requires the end of input (BL-069, #93)
Text after a complete expression used to be dropped silently; it is now an error at the first
extra token. This also exposed `AND`/`OR`, which were never parsed as operators before.

| Expression | 0.6 | 0.7 | Fix |
|---|---|---|---|
| `=1 2` | `1` | `Error: Unexpected token after end of expression at position 3` | remove the extra text |
| `=42 as string` | `42` | `Error: … at position 4` (there is no `as`) | `42::String` |
| `TRUE AND FALSE` | `TRUE` (`AND FALSE` dropped) | `FALSE` | |
| A parse error after a leading `=` | position counted without the `=` | position in the text as written | |

### Numbers and coercion (BL-066, BL-058)
Like Excel, the arithmetic operators and the scalar math builtins now convert numeric text and
booleans to numbers, and the operators read `NULL` as `0`. Text that is not a number is a
`#VALUE!` error. Scientific notation is a number literal. Aggregates over arrays (`SUM`,
`AVERAGE`, …) still skip text. Date and currency operands follow ADR-0027 (below).

| Expression | 0.6 | 0.7 |
|---|---|---|
| `1e3`, `2.5e-4` | `1`, `2.5` (rest silently dropped) | `1000`, `0.00025` |
| `1e308 * 10` | `1` | `Error: Result is not a finite number (#NUM!)` |
| `1x`, `2ex` | `1`, `2` | `Error: Invalid number literal` |
| `"3" + 1`, `" 4 " * 2` | `Error` | `4`, `8` |
| `TRUE + 1`, `-TRUE` | `Error` | `2`, `-1` |
| `1 + NULL`, `-NULL` | `Error` | `1`, `0` |
| `"abc" + 1`, `"a" + "b"` | `Error: Arithmetic op on non-number` | `Error: Cannot convert "abc" to a number (#VALUE!)` |
| `ABS("-3")`, `ABS(TRUE)`, `FLOOR("3.7")` | `0`, `0`, `0` | `3`, `1`, `3` |
| `ABS("x")` | `0` | `Error: ABS: cannot convert "x" to a number (#VALUE!)` |
| `PMT("0.05", 10, 1000)`, `DATE("2026", 1, 1)` | `Error` | the same as with numbers |
| `SUM([1, "2", TRUE])` | `1` | unchanged |
| `ABS(NULL)`, `DATE(2026, 1, NULL)` | `Error` | unchanged |

Use `&` to join text: `"a" & "b"` is `"ab"`.

### Errors instead of numbers (ADR-0018 D-1)
Where Excel returns an error value (`#DIV/0!`, `#NUM!`, `#VALUE!`, `#N/A`), Skillet now returns an
`Error`. Aggregates still skip `NULL` items, like blank cells.

| Expression | 0.6 | 0.7 |
|---|---|---|
| `10 / 0`, `0 / 0` | `inf`, `NaN` | `Error` |
| `MOD(5, 0)` | `NaN` | `Error` |
| `SQRT(-1)`, `POWER(0, -1)` | `NaN`, `inf` | `Error` |
| `ABS("x")`, `SQRT("x")` | `0` | `Error` (`#VALUE!`) |
| `AVERAGE()`, `MEDIAN()`, `STDEV_P()`, `VAR_P()` | `0` | `Error` |
| `PERCENTILE_INC([], 0.5)`, `QUARTILE_INC([], 1)` | `0` | `Error` |
| `MODE_SNGL(1, 2, 3)` (no repeated value) | `1` | `Error` |
| `AVGIF([1, 2], :x > 5)` (no match) | `0` | `Error` |
| `ROUND(2.567, "x")`, `PMT(0.05, 10, 1000, "x")` | argument ignored | `Error` |
| `COUNTIF([1,2,3], :x)` (lambda returns a non-boolean) | `0` | `Error` |
| `SUM()`, `MAX()`, `SUM([1, NULL])` | `0`, `0`, `1` | unchanged |

To keep a default, test first: `IF(:n == 0, 0, :total / :n)` (now lazy, see below).

### Builtins that now match Excel (ADR-0018)
| Expression | 0.6 | 0.7 |
|---|---|---|
| `COUNT([1, "a", TRUE, NULL])` | `4` | `1` (numbers only) |
| `COUNT(1, 2, 3)` | `Error` | `3` |
| `COUNTIF([1,2,3,4], ">2")`, `COUNTIF([1,2,3], 2)` | `0`, `0` | `2`, `1` |
| `SUMIF(["A","b"], "a", [1,2])` | `0` | `1` (text ignores case) |
| `SUMIF(["apple","avocado"], "a*", [1,2])` | `0` | `3` (wildcards) |
| `MOD(-10, 3)`, `MOD(10, -3)` | `-1`, `1` | `2`, `-2` |
| `TRIM("  a   b  ")` | `"a   b"` | `"a b"` |
| `ROUND(1234.567, -2)`, `ROUND(15, -1)` | `1235`, `15` | `1200`, `20` |
| `ROUND(1.005, 2)` | `1.0` | `1.01` |
| `CEILING(2.1, 0.5)`, `CEILING(7, 5)` | `3`, `7` | `2.5`, `10` |
| `FLOOR(7, 5)` | `7` | `5` |
| `TIME(14, 30, 0)` | current time (0.6.3) | `0.6041666666666666` (fraction of a day) |
| `IPMT(0.1, 1, 3, 1000)`, `IPMT(0.1, 2, 3, 1000)` | `100`, `49.79…` | `-100`, `-69.79…` |
| `FIND("b", "abc")` | `Error` | `2` (text search) |
| `LEFT("hello", -1)`, `RIGHT("hello", -1)` (#96) | `""` | `Error: LEFT num_chars must not be negative (#VALUE!)` |
| `MID("hello", 0, 2)`, `REPLACE("abcdef", 0, 1, "X")` (#96) | `"he"`, `"Xbcdef"` | `Error: MID start_num must be at least 1 (#VALUE!)` |
| `SORT(["b", "a"])` (#96) | `Error` | `["a", "b"]` (text ignores case) |
| `UNIQUE(["a", "a", 1])`, `REVERSE("abc")` (#96) | `[1]`, `Error` | `["a", 1]`, `"cba"` |

The methods keep Ruby semantics and do not change: `"  a   b  ".trim()` is still `"a   b"`, and
`(-3.7).int()` is still `-3` (while `INT(-3.7)` is `-4`).

### Logical functions and operators (ADR-0018 D-8, ADR-0029)
`IF` and `IFS` evaluate only the branch they pick. Their condition must be `TRUE`/`FALSE` or a
number (non-zero is true); `NULL` is false. The `AND`/`OR`/`NOT`/`XOR` builtins follow Excel:
inside an array, text and `NULL` are skipped; text passed directly is `#VALUE!`; no logical value
at all is `#VALUE!`. The builtins evaluate every argument, while the operators `AND`, `OR`, `&&`
and `||` stop as soon as the left side decides the result.

| Expression | 0.6 | 0.7 |
|---|---|---|
| `IF(TRUE, 1, UPPER(1))` | `Error` | `1` (only the chosen branch runs) |
| `IF("x", 1, 2)`, `IF([1], 1, 2)` | `2` | `Error` (`#VALUE!`) |
| `IFS(FALSE, 1)` (no true condition) | `false` | `Error` (`#N/A`) |
| `AND(TRUE, NULL)`, `AND(TRUE, [TRUE, "x"])` | `false` | `true` (blank and text ignored) |
| `AND(TRUE, "x")`, `OR(FALSE, "TRUE")`, `NOT("x")` | `false`, `false`, `true` | `Error` (`#VALUE!`) |
| `AND()`, `OR()`, `AND([])` | `true`, `false`, `true` | `Error` (`#VALUE!`) |
| `XOR(TRUE, TRUE, TRUE)`, `XOR(TRUE)` | `Error` | `true` (odd number of TRUE) |
| `FALSE && 1/0`, `TRUE \|\| UPPER(1)` | `Error` | `false`, `true` (right side not evaluated) |
| `NOT TRUE`, `NOT :x > 1` | parse error | `false`, `NOT (:x > 1)` (`NOT` keyword) |
| `NOT (0) == FALSE` (space before `(`) | `false` (`NOT(0) == FALSE`) | `true` (`NOT ((0) == FALSE)`) |
| `AND(1, TRUE)`, `NOT(0)`, `1 AND TRUE` | `true`, `true`, `Error` | unchanged |

The `NOT` keyword binds looser than comparisons and tighter than `AND`, as in SQL:
`NOT 1 > 2 AND TRUE` is `(NOT (1 > 2)) AND TRUE`. Inside a tighter operator it needs
parentheses (`1 + (NOT x)`). `NOT(x)` with no space stays the function call, and `!` keeps its
tight precedence (`!:a == :b` is `(!:a) == :b`). To keep a 0.6 `NOT (x) == y`, remove the space:
`NOT(x) == y`.

### Dates (ADR-0018 D-5)
| Expression | 0.6 | 0.7 |
|---|---|---|
| `DATE(2026, 13, 1)` | `Error: Invalid date` | 2027-01-01 |
| `DATE(2026, 2, 30)`, `DATE(2026, 1, 0)` | `Error` | 2026-03-02, 2025-12-31 |
| `DATE(26, 1, 1)` | year 26 | 1926-01-01 (years 0–1899 get 1900 added) |
| `DATE(2026, 1, 1) + 1` | `Error` | 2026-01-02 (numbers count days) |
| `DATE(2026, 1, 2) - DATE(2026, 1, 1)` | `Error` | `1` |
| `DATE(2026, 1, 1) + NULL` (ADR-0027) | `Error` | 2026-01-01 (`NULL` is 0 days) |
| `DATE(2026, 1, 1) + "1"`, `"1" + DATE(2026, 1, 1)`, `DATE(2026, 1, 1) + TRUE` | `Error` | 2026-01-02 |
| `DATE(2026, 1, 1) + "x"` | `Error: Arithmetic op on non-number` | `Error: Cannot convert "x" to a number (#VALUE!)` |
| `"2026-02-30"::DateTime` | `Error` | `Error` (casts never roll over) |

### Dates as text (ADR-0031)
Every path that turns a DateTime into text writes ISO 8601 in UTC, with whole seconds.
`::DateTime` reads that form back, so `DATE(2026, 1, 1)::String::DateTime` is
`DATE(2026, 1, 1)` (casts chain left to right since BL-078; before, the second cast needed
parentheses). Integer text still casts as Unix seconds. To keep the old text, use
`DATEDIFF(DATE(1970, 1, 1), :d, "seconds")`, or `(:d - DATE(1970, 1, 1)) * 86400`, then turn
the number into text.

| Expression | 0.6 | 0.7 |
|---|---|---|
| `DATE(2026, 1, 1)::String`, `DATE(2026, 1, 1).to_s()` | `"1767225600"` | `"2026-01-01T00:00:00Z"` |
| `CONCAT("on ", DATE(2026, 1, 1))`, `"on " & DATE(2026, 1, 1)` | `"on 1767225600"` | `"on 2026-01-01T00:00:00Z"` |
| `JOIN([DATE(2026, 1, 1)])`, `[DATE(2026, 1, 1)].to_s()` | `"1767225600"`, `"[1767225600]"` | `"2026-01-01T00:00:00Z"`, `"[2026-01-01T00:00:00Z]"` |
| `DATE(2026, 1, 1).to_json()` | `Json("\"1767225600\"")` | `Json("\"2026-01-01T00:00:00Z\"")` |
| `[DATE(2026, 1, 1)].join()`, `TEXTJOIN(",", TRUE, DATE(2026, 1, 1))` | `Error` | `"2026-01-01T00:00:00Z"` |
| `DATE(2026, 1, 1)::String::DateTime` (BL-078) | parse error | `DATE(2026, 1, 1)` |
| `[2.5::Currency, NULL].join()` (BL-078, as `JOIN`) | `Error` | `"2.5000,"` |
| `LEN(DATE(2026, 1, 1))`, `FIND("2", DATE(2026, 1, 1))` | `Error` | unchanged; use `LEN(:d::String)` |
| `"2026-01-01T10:30:00Z"::DateTime` | `Error` | 2026-01-01 10:30 UTC |
| `DATE(2026, 1, 1)::Json`, server responses, `"1767225600"::DateTime` | Unix seconds | unchanged |

### Date units follow Excel (B-15)
`DATEADD` and `DATEDIFF` read the unit codes of VBA's `DateAdd`/`DateDiff`, case-insensitively.
The long words (`"months"`, `"minutes"`, …) are unchanged. Use them, or the new codes, to keep the
old result. The new `DATEDIF` has Excel's worksheet codes (`"y"`, `"m"`, `"d"`, `"md"`, `"ym"`,
`"yd"`).

| Expression | 0.6 | 0.7 | Keep the old result with |
|---|---|---|---|
| `DATEADD(DATE(2026, 1, 1), 1, "m")` | +1 minute | 2026-02-01 (+1 month) | `"n"` or `"minutes"` |
| `DATEDIFF(DATE(2026, 1, 1), DATE(2026, 3, 1), "m")` | `84960` (minutes) | `2` (months) | `"n"` or `"minutes"` |
| `DATEADD(DATE(2026, 1, 1), 1, "y")` | 2027-01-01 | 2026-01-02 (`"y"` is day of year) | `"yyyy"` or `"years"` |
| `DATEDIFF(DATE(2025, 1, 1), DATE(2026, 1, 1), "y")` | `1` | `365` (days) | `"yyyy"` or `"years"` |
| `DATEADD(DATE(2026, 1, 1), 1, "w")` | 2026-01-08 | 2026-01-02 (`"w"` is weekday) | `"ww"` or `"weeks"` |
| `DATEADD(DATE(2024, 1, 31), 1, "months")`, `DATEADD(DATE(2026, 3, 31), 1, "months")` | 2024-02-28, 2026-04-28 | 2024-02-29, 2026-04-30 (end of month, as VBA) | |
| `DATEADD(…, 1, "eons")` | `Error: DATEADD unit must be one of: …` | the same error, ending in `(#VALUE!)` | |
| `DATEADD(DATE(2026, 1, 1), 1e15, "m")`, `…, 1e18, "days")` | hang or panic | `Error: DATEADD result is out of range (#NUM!)` | |
| `DATEADD(…, "q")`, `"yyyy"`, `"ww"`, `"n"` | `Error` | quarters, years, weeks, minutes | |

### Currency in arithmetic (ADR-0027)
Currency now works in the arithmetic operators instead of failing. The other operand converts as
for a number (numeric text, booleans, `NULL` as 0). Results are not rounded (Currency is an
`f64`, ADR-0003); use `ROUND(x, 2)`.

| Expression | 0.6 | 0.7 |
|---|---|---|
| `1::Currency + 1`, `3 * 10::Currency`, `10::Currency / 4` | `Error` | `Currency(2)`, `Currency(30)`, `Currency(2.5)` |
| `10::Currency / 4::Currency` | `Error` | `2.5` (Number, a ratio) |
| `2::Currency ^ 2`, `7::Currency % 4` | `Error` | `4`, `3` (Number) |
| `-(5::Currency)`, `+(5::Currency)` | `Error` | `Currency(-5)`, `Currency(5)` |
| `10::Currency + "2"`, `10::Currency + NULL` | `Error` | `Currency(12)`, `Currency(10)` |
| `1::Currency / 0` | `Error` | `Error: Division by zero (#DIV/0!)` |
| `DATE(2026, 1, 1) + 1::Currency`, `1 == 1::Currency` | `Error`, `false` | unchanged |

### Currency in the aggregates (ADR-0031, B-13)
`SUM`, `PRODUCT` (`MULTIPLY`), `AVERAGE` (`AVG`), `MIN` and `MAX` return Currency when any value
they count is Currency, following `Currency + Number` (ADR-0027). `SUMIF` and `AVERAGEIF` do the
same for the values they sum or average (BL-078, ADR-0031 amendment 1); a Currency that is not
matched does not count, and `COUNTIF` is unchanged. Use `::Float` on the result to keep a Number.
The methods (`.sum()`, `.avg()`, `.min()`, `.max()`) are unchanged.

| Expression | 0.6 | 0.7 |
|---|---|---|
| `SUM(10::Currency, 5)` | `15` (Number) | `Currency(15)` |
| `PRODUCT(2::Currency, 3)`, `AVERAGE(10::Currency, 20)` | `6`, `15` (Number) | `Currency(6)`, `Currency(15)` |
| `MIN(1, 3::Currency)`, `MAX([3::Currency, 9::Currency])` | `1`, `9` (Number) | `Currency(1)`, `Currency(9)` |
| `SUMIF([1, 2::Currency, 3], ">1")`, `AVERAGEIF([1, 2::Currency, 3], ">1")` (BL-078) | `5`, `2.5` (Number) | `Currency(5)`, `Currency(2.5)` |
| `SUMIF([1, 2, 3], ">1", [10, 20::Currency, 30])` (BL-078) | `50` (Number) | `Currency(50)` |
| `SUMIF([1::Currency, 2, 3], ">1")` (the Currency is not matched), `COUNTIF([1::Currency], ">0")` | Number | unchanged |
| `SUM(1, 2)`, `[1::Currency].sum()` | Number | unchanged |

### New names (additive)
These used to fail with `Unknown function` or a parse error: `STDEV.P`, `STDEV.S`, `STDEV`,
`VAR.P`, `VAR.S`, `VAR`, `MODE.SNGL`, `MODE`, `PERCENTILE.INC`, `PERCENTILE`, `QUARTILE.INC`,
`QUARTILE`, `COUNTA`, `LEN`, `TODAY`, `PPMT`, `PV`, `NPV`, `TEXTJOIN`, `AVERAGEIF`, `DATEDIF`.

### Null equality (ADR-0013)
| Expression | 0.6 | 0.7 |
|---|---|---|
| `NULL == NULL` | `false` | `true` |
| `NULL != NULL` | `true` | `false` |
| `[NULL, 1, 2].filter(:x != NULL)` | `[NULL, 1, 2]` | `[1, 2]` |
| `NULL == 0`, `NULL < 1` | `false`, `Error` | unchanged |

### `JQ` result shape (ADR-0014)
The shape now follows the path, not the number of matches. A singular path (`.name`, `[n]`)
returns the value, or `NULL` when it is missing. Any other path returns an array.

| Expression | 0.6 | 0.7 |
|---|---|---|
| `JQ({"a":[3]}, "$.a[*]")` | `3` | `[3]` |
| `JQ({"a":[3,4]}, "$.a[0:1]")` | `3` | `[3]` |
| `JQ({"a":[{"p":1}]}, "$..p")` | `1` | `[1]` |
| `JQ({"a":5}, "$.b")` | `[]` | `NULL` |
| `JQ({"a":[3,4]}, "$.a[*]")`, `JQ({"u":{"n":"Ann"}}, "$.u.n")` | `[3, 4]`, `"Ann"` | unchanged |

If you used a wildcard path as a scalar, add `.first()`.

### NaN in statistics (ADR-0015)
| Expression | 0.6 | 0.7 |
|---|---|---|
| `PERCENTILE_INC([1,2,3], 0/0)` | `1` | `Error` |
| `QUARTILE_INC([1,2,3,4], -0.5)`, `QUARTILE_INC([1,2,3,4], 4.9)` | `1`, `4` | `Error` |
| `MEDIAN([1, 0/0, 3])` | crash (panic) | `Error` |

### Custom functions (ADR-0016)
Custom and JS functions are now visible in every sub-expression.

| Expression | 0.6 | 0.7 |
|---|---|---|
| `MAP([21000], MX_ISR(:x, "semanal").isr)` | `Unknown function` | works |
| `FILTER`, `REDUCE`, `SUMIF`, `FIND` lambdas, method arguments | `Unknown function` | works |
| `:v := MX_ISR(21000, "semanal").isr; :v` | `Unknown function` | works |
| A custom `SUM` inside a `FILTER` lambda | builtin `SUM` | the custom `SUM` |

## Evaluation limits (ADR-0011)
Each evaluation has a budget of 256 MB of created values, and the servers stop an evaluation after
20 s. On the servers both are configurable: `--max-eval-bytes` / `SKILLET_MAX_EVAL_BYTES` and
`--eval-timeout` / `SKILLET_EVAL_TIMEOUT` (ADR-0011 amendment 4). Development builds of 0.7 before
BL-075 used a fixed 10 s server deadline; the default is now 20 s. The server budget default is
unchanged at 256 MiB, only now configurable. Details: [Limits](limits.md).

| Situation | 0.6 | 0.7 |
|---|---|---|
| A formula that builds huge values (`[…].reduce([:acc, :acc], 0)`) | runs until memory is gone | `Evaluation budget exceeded (max 256 MB)` |
| Copying a large variable once per item (`:rows.map(SUM(:rows))`) | slow | may hit the budget; hoist it: `:t := SUM(:rows); :rows.map(:t)` |
| `reduce(MERGE(:acc, [:x]), [])` over more than about 2,800 items | runs | budget error |
| A server evaluation that runs longer than 20 s (or `--eval-timeout`) | runs | `Evaluation timed out (max 20 s)` (HTTP 400) |
| Expressions nested deeper than 128 | stack overflow (crash) | `Expression nested too deeply (max 128)` |
| `with_limits` budgets near a boundary with many small strings (#105) | 32 + length per string | 48 + length per string (the real heap size), so the same formula reaches a tight budget sooner |

## JavaScript plugins (ADR-0017)
| Situation | 0.6 | 0.7 |
|---|---|---|
| A hook calls `httpGet(url)` | runs `curl` | `httpGet` is not defined, so the call fails. Hooks are pure functions of their arguments |
| A hook runs longer than 10 s | runs forever | `JS function NAME timed out (max 10 s)`, or the server's `Evaluation timed out (max N s)` when `--eval-timeout` is below 10 |
| A hook uses more than 64 MiB | runs | `JS function NAME ran out of memory (max 64 MB)` |
| Deep recursion in a hook | `Exception generated by QuickJS` | `JS function NAME failed: RangeError: Maximum call stack size exceeded` |
| A hook returns an array deeper than 128, or a cyclic array | process abort | `Expression nested too deeply (max 128)` |
| Any other JS exception | `Exception generated by QuickJS` | `JS function NAME failed: <message>` |

## Rust API
| Item | 0.6 | 0.7 |
|---|---|---|
| `skillet::memory_pool`, `skillet::traits`, `skillet::concurrent_registry` | public | removed (ADR-0010) |
| `skillet::runtime::evaluation` | re-export shim | removed; use `skillet::runtime::evaluator` or the top-level `evaluate*` functions (ADR-0010) |
| `runtime::method_calls::exec_filter`/`exec_map`/`exec_find`/`exec_reduce`, `exec_string_method`, `exec_array_method` | take a variable map | take `&dyn EvaluationContext` |
| Minimum Rust version | not stated | 1.85 (ADR-0012) |
| `runtime::builtin_functions::exec_builtin` | public | deprecated (still works); it reports argument errors as `Unknown function`. Use `runtime::function_dispatch::exec_builtin_fast` or `evaluate` |

## Servers
| Change | 0.6 | 0.7 |
|---|---|---|
| `sk_http_server` request bodies | parsed as JSON whatever the `Content-Type` | `Content-Type: application/json` is required; anything else gets `415 Unsupported Media Type` |
| CORS on `sk_http_server` | every response has `Access-Control-Allow-Origin: *` | no CORS header when the server runs without a token (development mode), so web pages cannot drive a local server. Rules in [ADR-0019](../adr/0019-server-cors-and-development-mode.md) |
| Admin token from the environment | only `--admin-token` | `SKILLET_ADMIN_TOKEN` is read too, like `SKILLET_AUTH_TOKEN` |
| Docker image without tokens | starts unauthenticated on `0.0.0.0` | refuses to start; set `SKILLET_AUTH_TOKEN` and `SKILLET_ADMIN_TOKEN` (the old `AUTH_TOKEN`/`ADMIN_TOKEN` names are no longer read) ([Docker](../ops/docker.md)) |
| Empty token (`--token ""`, `--admin-token ""`, empty `SKILLET_AUTH_TOKEN`) | auth silently off | the server exits with code 1 |
| `sk_server` wrong or missing token | can retry on the same connection | `Unauthorized: invalid token`, then the connection closes |
| `/upload-js`, `/update-js`, `/delete-js` file names | any path | only `[A-Za-z0-9_-]+.js`, otherwise 400 |
| Writing one response | 10 s per socket write | 10 s in total; a slower reader gets a truncated response |
| `sk_http_server` request over 1 MB / slower than 30 s | HTTP 400 with `"error": "413 Payload Too Large"` / `"408 Request Timeout"` | HTTP 413 / 408 with `"error"` set to the limit message, e.g. `Request body too large (max 1MB)`, `Request read timed out (max 30 s)` (BL-049) |

Update clients first: send `Content-Type: application/json` on every `POST` and `PUT`, and set
tokens on every deployment.
