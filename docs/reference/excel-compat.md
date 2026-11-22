# Excel compatibility audit (BL-020)

- Status: decided and implemented. The audit (v0.6.3) listed ten decisions, D-1 to D-10. The
  maintainer decided all of them on 2026-11-21 in [ADR-0018](../adr/0018-excel-compatibility.md),
  amended by [ADR-0023](../adr/0023-excel-equality-and-coercion.md): UPPERCASE builtins and
  operators follow Excel, except for the deliberate differences listed below. They are
  implemented in PRs #82, #83, #84, #88 and #89 and ship in 0.7.0. Each row names its PR.
  The logical functions and operators follow [ADR-0029](../adr/0029-logic-operators.md), which
  also implements the last two items of D-8 (B-18). The old v0.6.3 results are in the
  [0.7 migration guide](../guide/migration-0.7.md).
- Scope: every builtin in `src/runtime/builtin_table.rs`, including the functions the evaluator
  handles itself (`IF`, `IFS`, `FILTER`, `FIND`, `MAP`, `REDUCE`, `SUMIF`, `AVERAGEIF`,
  `COUNTIF`, `JQ`).
- Skillet column: real output of `cargo run -q --bin sk -- '=…'` on `develop` at `824df14`
  (2026-11-21). Output is shortened (`3` means `Number(3.0)`).
- Excel column: Excel's documented behavior for the function (Microsoft 365). Values marked
  **verify in Excel** come from the documented rule, but nobody has checked the exact number
  in a workbook yet.
- Match values: **yes**; **edges** (same result for valid, ordinary inputs; differs only on
  empty, text, wrong-type or out-of-range inputs); **differs** (different result for ordinary
  inputs, or the signature is different); **Skillet-only** (no Excel function by that name).
  The deliberate cross-cutting differences X-3, X-7 and X-9 are explained once below and are not
  counted again in each row.
- Status values: **implemented (#n)**: decided and in the code; **deliberate**: differs from
  Excel on purpose, with the reason; **open**: still differs, and under ADR-0018 a difference that
  is not deliberate is a bug.

## Summary

| | Count | Functions |
|---|---|---|
| yes | 47 | `SUM`, `AVERAGE`, `MIN`, `MAX`, `ROUND`, `CEILING`, `FLOOR`, `ABS`, `SQRT`, `POWER`, `MOD`, `INT`, `UPPER`, `LOWER`, `TRIM`, `SUBSTITUTE`, `ISNUMBER`, `ISTEXT`, `LEN`, `TEXTJOIN`, `COUNT`, `COUNTA`, `COUNTIF`, `SUMIF`, `AVERAGEIF`, `FIND`, `DATE`, `TIME`, `PMT`, `IPMT`, `PPMT`, `PV`, `NPV`, `MEDIAN`, `MODE.SNGL`, `STDEV.P`, `STDEV.S`, `VAR.P`, `VAR.S`, `PERCENTILE.INC`, `QUARTILE.INC`, `IF`, `IFS`, `AND`, `OR`, `NOT`, `XOR` |
| edges | 11 | `PRODUCT`, `CONCAT`, `LEFT`, `RIGHT`, `MID`, `REPLACE`, `YEAR`, `MONTH`, `DAY`, `DATEDIF`, `FV` |
| differs | 9 | `ISBLANK`, `UNIQUE`, `SORT`, `FILTER`, `MAP`, `REDUCE`, `TODAY`, `NOW`, `DB` |
| Skillet-only | 23 (+8 alias spellings) | `AVG`, `CEIL`, `POW`, `MULTIPLY`, `BETWEEN`, `LENGTH`, `SUBSTRING`, `SPLIT`, `SUBSTITUTEM`, `INCLUDES`, `ARRAY`, `FLATTEN`, `FIRST`, `LAST`, `CONTAINS`, `IN`, `JOIN`, `MERGE`, `REVERSE`, `DIG`, `JQ`, `DATEADD`, `DATEDIFF`; spellings `MODESNGL`, `MODE_SNGL`, `STDEV_P`, `VAR_P`, `PERCENTILEINC`, `PERCENTILE_INC`, `QUARTILEINC`, `QUARTILE_INC` |

67 functions share a name with Excel (56 in the v0.6.3 audit, plus the 10 added under D-9 and
D-3, and `DATEDIF` from B-15). 47 match, 11 match on valid inputs, and 9 give a different result for ordinary inputs;
6 of those 9 are deliberate. Legacy and alias names count with their function: `STDEVP`,
`STDEV` and `VARP`, `VAR` with `STDEV.P`/`STDEV.S` and `VAR.P`/`VAR.S`; `MODE`, `PERCENTILE`,
`QUARTILE` with the `.SNGL`/`.INC` names; `AVGIF` with `AVERAGEIF`. In v0.6.3 the counts were
2 yes, 32 edges and 22 differs.

## Cross-cutting differences

These patterns repeat across many rows. They are listed once here, and the tables refer to them by ID.

| ID | Pattern | Skillet | Excel | Status |
|---|---|---|---|---|
| X-1 | **Error cases and numeric text** | Impossible math is an error: `10 / 0` and `0 / 0` → `Error: Division by zero (#DIV/0!)`, `SQRT(-1)` → `Error: SQRT of a negative number (#NUM!)`, `MOD(5, 0)` → `Error: MOD divisor is 0 (#DIV/0!)`, `POWER(0, -1)` → `#DIV/0!` error, `1e308*10` → `Error: Result is not a finite number (#NUM!)`. Numeric text and booleans convert in the operators and the scalar math builtins: `ABS("-3")` → `3`, `"3" + 1` → `4`, `TRUE + 1` → `2`, `FLOOR("3.7")` → `3`. Other text is an error: `ABS("x")` → `Error: ABS: cannot convert "x" to a number (#VALUE!)`, `"abc" + 1` → `Error: Cannot convert "abc" to a number (#VALUE!)`. `NULL` is `0` in the operators (`1 + NULL` → `1`) | `#DIV/0!`, `#NUM!`, `#VALUE!`; numeric text and logical values convert in arithmetic; a blank cell is 0 | yes. **Implemented**: errors D-1 (ADR-0018, #84); coercion ADR-0023 §6 (#89). Open: `DB(0,0,5,1)` still returns `NaN` (B-8) |
| X-2 | **Empty input** | `AVERAGE()` → `Error: AVERAGE of no numbers (#DIV/0!)`; `MEDIAN()`, `PERCENTILE_INC([], 0.5)`, `QUARTILE_INC([], 1)` → `#NUM!` errors; `STDEV_P()`, `VAR_P()`, `AVERAGEIF([1,2], ">5")` → `#DIV/0!` errors; `MODE_SNGL()` → `Error: MODE.SNGL: no value repeats (#N/A)`. `SUM()` → `0`, `MAX()` → `0`, `COUNT([])` → `0` | Same errors; `SUM`, `MAX` and `COUNT` of nothing are 0 | yes. **Implemented** D-1 (#84) |
| X-3 | **Direct text and boolean arguments to aggregates** | Aggregates skip them anywhere: `SUM(1, "2", TRUE)` → `1`, `MAX(-1, "5")` → `-1`, `AVERAGE(1, "2")` → `1`, `COUNT(1, "2")` → `1` | Values inside a range or array are skipped (matches). Values typed directly as arguments convert: `SUM(1,"2",TRUE)` = 4, `MAX(-1,"5")` = 5 (**verify in Excel**) | **deliberate** (ADR-0018 "Deliberate differences", kept by ADR-0023 §6). Skillet has no cell ranges, so every argument is treated like a range value. Scalar math builtins do convert (X-1), so `ABS("2")` → `2` but `SUM("2")` → `0` |
| X-4 | **Dotted Excel names** | `STDEV.P(1, 2)` → `0.5`. `VAR.P`, `VAR.S`, `STDEV.S`, `MODE.SNGL`, `PERCENTILE.INC`, `QUARTILE.INC` parse too; the underscore and legacy spellings stay | `STDEV.P(...)` | yes. **Implemented** D-4 (#82) |
| X-5 | **Arguments that were silently ignored** | Honored: `CEILING(2.1, 0.5)` → `2.5`, `SUBSTITUTE("a-b-c","-","+",2)` → `"a-b+c"`, `SORT([3,1,2], -1)` → `[3,2,1]`. Errors: `ROUND(2.567, "x")` → `Error: ROUND num_digits: cannot convert "x" to a number (#VALUE!)`, `PMT(0.05,10,1000,"x")` → `Error: PMT fv: cannot convert "x" to a number (#VALUE!)` | Honored, or `#VALUE!` | yes. **Implemented** D-1 (#84) |
| X-6 | **Operators** | `2 ^ 3 ^ 2` → `64`; `-2 ^ 2` → `4`; `2*-3^2` → `18`; `0-2^2` → `-4`. `50%` → `0.5`; `10%-3` and `10 % -3` → `-2.9` (percent before `+`/`-`). `%` followed directly by an operand is modulo: `10 % 3` → `1`, `-10 % 3` → `-1`. `"a" & 1` → `"a1"`. `"a" == "A"` → `false`. `1 == "1"` and `TRUE == 1` → `false`. `"10" < "9"` → `true`; `1 < "2"` → `Error: Comparison of incompatible types` | `=2^3^2` = 64, `=-2^2` = 4. `%` is only the percent suffix. `&` joins text. `=` ignores case for text, and does not convert (`=1="1"` is FALSE). Mixed types order numbers < text < logical | **Implemented**: Excel precedence D-10 and `&` (#82); `%` suffix (#82) and `%` before `+`/`-` (ADR-0023 §7, #89). **Deliberate**: `==` stays case-sensitive (ADR-0023 §1; use `LOWER(:a) == LOWER(:b)`), text orders by code point and mixed-type ordering is an error so a mistake is reported (ADR-0023 §4), and `%` with an operand after it stays a truncated remainder because changing it would break every existing `%` (ADR-0018). `MOD` is Excel's |
| X-7 | **Number to text** | `CONCAT(0.1 + 0.2)` and `0.1 + 0.2 & ""` → `"0.30000000000000004"` (Rust's shortest round-trip format) | 15 significant digits, so `"0.3"` | **deliberate** (ADR-0018). Use `ROUND` first |
| X-8 | **Lazy `IF`** | `IF(TRUE, 1, UPPER(1))` → `1`; `IF(FALSE, FIRST([]), 0)` → `0`; `IFS` stops at the first true condition | Only the chosen branch is evaluated | yes. **Implemented** D-8 (#82); the text-condition and no-match errors (B-18) in ADR-0029 |
| X-9 | **Strict argument types where Excel converts** | Builtins refuse `NULL`: `ABS(NULL)` → `Error: ABS expects number, got null`, `ROUND(NULL)`, `SQRT(NULL)` likewise. Text builtins refuse numbers: `UPPER(1)` → `Error: UPPER expects string`, `LEFT(12345, 2)`, `TRIM(1)`, `SUBSTITUTE(123, "2", "x")` → errors. `LEN`, `CONCAT`, `TEXTJOIN` and `&` do convert numbers and booleans to text | A blank cell is 0 or `""`; numbers convert to text | **deliberate**. ADR-0023 §6 narrowed "errors over silent coercion" (ADR-0018) to this: text and booleans convert to numbers in arithmetic and scalar math, `NULL` is 0 in the operators only, and other conversions stay errors so a type mistake is reported instead of guessed |

## Arithmetic

| Function | Skillet (expr → output) | Excel | Match? | Status |
|---|---|---|---|---|
| `SUM` | `SUM(1,2,3)` → `6`; `SUM([1,2],3)` → `6`; `SUM()` → `0`; `SUM([1,"2",TRUE,NULL])` → `1`; `SUM(10::Currency, 5)` → `Currency(15)` | `SUM(number1, ...)`. Text and logical values in arrays are ignored | yes | X-3 deliberate. B-13 fixed (ADR-0031): Currency when any value is Currency; the same for `PRODUCT`, `AVERAGE`, `MIN`, `MAX` |
| `AVERAGE` (`AVG`) | `AVERAGE(1,2,3,4)` → `2.5`; `AVERAGE()` → `Error: AVERAGE of no numbers (#DIV/0!)`; `AVERAGE([1,"x",TRUE])` → `1`; `AVERAGE([1,NULL,3])` → `2` | No numbers → `#DIV/0!` | yes | implemented (#84) |
| `MIN` / `MAX` | `MIN(3,1,2)` → `1`; `MIN()` → `0`; `MIN(["a"])` → `0`; `MAX(-1,"5")` → `-1`. Method `[].max()` → `Null`; `["a",1].max()` → error | "If the arguments contain no numbers, MAX returns 0" | yes | X-3 deliberate. The method keeps its own rule (ADR-0018: methods are not bound to the builtin) |
| `ROUND` | `ROUND(2.5)` → `3`; `ROUND(-2.5)` → `-3`; `ROUND(2.345,2)` → `2.35`; `ROUND(1.005,2)` → `1.01`; `ROUND(1234.567,-2)` → `1200`; `ROUND(15,-1)` → `20`; `ROUND(2.567,"x")` → error (X-5); `ROUND()` → `Error: ROUND expects (number, [num_digits])`; `ROUND("2.5")` → `3`; `ROUND(10::Currency,1)` → `Currency(10)` | `ROUND(number, num_digits)`. Halves away from zero; negative digits round left of the point; `ROUND(1.005,2)` = 1.01 | yes | implemented (#84, "Also decided"; `"2.5"` converts since #89). Leaving out `num_digits` (default 0) is a Skillet extension |
| `CEILING` (`CEIL`) | `CEILING(2.1)` → `3`; `CEILING(2.1,0.5)` → `2.5`; `CEILING(7,5)` → `10`; `CEILING(-2.5)` → `-2`; `CEILING(-2.5,-2)` → `-4`; `CEILING(2.5,-1)` → `Error: CEILING: a positive number needs a positive significance (#NUM!)` | `CEILING(2.1,0.5)` = 2.5, `CEILING(7,5)` = 10, `CEILING(-2.5,-2)` = -4 | yes | implemented (#84). `CEIL` is an alias; significance defaults to 1 |
| `FLOOR` | `FLOOR(2.9)` → `2`; `FLOOR(7,5)` → `5`; `FLOOR(-2.5)` → `-3`; `FLOOR("3.7")` → `3` | `FLOOR(7,5)` = 5 | yes | implemented (#84; text since #89) |
| `ABS` | `ABS(-3)` → `3`; `ABS("-3")` → `3`; `ABS(" 4 ")` → `4`; `ABS(TRUE)` → `1`; `ABS("x")` → `Error: ABS: cannot convert "x" to a number (#VALUE!)`; `ABS()` → `Error: ABS expects (number)` | Numeric text converts; other text → `#VALUE!`. 1 argument required | yes | implemented (#84 errors, ADR-0023 §6 coercion #89). `ABS(NULL)` is an error (X-9, deliberate) |
| `SQRT` | `SQRT(16)` → `4`; `SQRT(-1)` → `Error: SQRT of a negative number (#NUM!)`; `SQRT("x")` → `#VALUE!` error | Negative → `#NUM!` | yes | implemented (#84, #89) |
| `POWER` (`POW`) | `POWER(2,10)` → `1024`; `POWER(0,0)` → `Error: POWER(0, 0) is undefined (#NUM!)`; `POWER(-8,1/3)` → `Error: POWER result is not a real number or is out of range (#NUM!)`; `POWER(0,-1)` → `Error: POWER: 0 to a negative power (#DIV/0!)`; `POWER(2)` → `Error: POWER expects (number, power)` | `POWER(0,0)` → `#NUM!`; `POWER(-8,1/3)` → `#NUM!`; `POWER(0,-1)` → `#DIV/0!`. 2 arguments required | yes | implemented (#84). The operator matches: `0^0` → `#NUM!` error, `0^-1` → `Division by zero (#DIV/0!)` (BL-060 checks how `^-` parses) |
| `MOD` | `MOD(10,3)` → `1`; `MOD(-10,3)` → `2`; `MOD(10,-3)` → `-2`; `MOD(5,0)` → `Error: MOD divisor is 0 (#DIV/0!)`; `MOD(5.5,2)` → `1.5`; `MOD(10)` → `Error: MOD expects (number, divisor)` | `MOD(n, d) = n - d*INT(n/d)`: the divisor's sign | yes | implemented D-7 (#84). The `%` operator stays a truncated remainder (X-6, deliberate) |
| `INT` | `INT(3.7)` → `3`; `INT(-3.7)` → `-4`; `INT("x")` → `#VALUE!` error. Method `(-3.7).int()` → `-3` | Rounds down: `INT(-3.7)` = -4 | yes | implemented (#84). `.int()` truncates on purpose, like Ruby's `to_i` (ADR-0018) |
| `PRODUCT` (`MULTIPLY`) | `PRODUCT(2,3,4)` → `24`; `PRODUCT()` → `1`; `PRODUCT([2,"x",3])` → `6` | At least 1 argument required. A range with no numbers gives 0 (**verify in Excel**) | edges | open, low: `1` is the mathematically correct empty product; confirm Excel's value before changing it |

## Logical

| Function | Skillet (expr → output) | Excel | Match? | Status |
|---|---|---|---|---|
| `IF` | `IF(TRUE,1,2)` → `1`; `IF(FALSE,1)` → `false`; `IF(1,"a","b")` → `"a"`; `IF(NULL,"a","b")` → `"b"`; `IF(TRUE,1,UPPER(1))` → `1`; `IF("x","a","b")` → `Error: IF condition must be TRUE, FALSE or a number, got string (#VALUE!)` | A missing false branch returns FALSE. Text in `test` → `#VALUE!`. Only the chosen branch is evaluated | yes | implemented: lazy D-8 (#82); text condition B-18 (ADR-0029) |
| `IFS` | `IFS(FALSE,1,TRUE,2)` → `2`; `IFS(FALSE,1)` → `Error: IFS: no condition is TRUE (#N/A)`; `IFS(TRUE)` → `Error: IFS expects pairs of condition,value arguments` | No true condition → `#N/A` | yes | implemented: lazy D-8 (#82); `#N/A` B-18 (ADR-0029) |
| `AND` | `AND(TRUE,1,0)` → `false`; `AND([TRUE,"x",NULL])` → `true`; `AND(TRUE,NULL)` → `true`; `AND(TRUE,"x")` → `#VALUE!` error; `AND()`, `AND([])` → `Error: AND: no logical values (#VALUE!)`; `AND(FALSE, UPPER(1))` → error | 1 to 255 arguments, every one evaluated. Text and empty cells in a range are ignored; no logical values → `#VALUE!`. Text typed as an argument → `#VALUE!` (**verify in Excel**: `"TRUE"` typed directly may convert) | yes | implemented (ADR-0029). A `NULL` argument counts as an empty cell |
| `OR` | `OR(FALSE,0)` → `false`; `OR([FALSE,"TRUE"])` → `false`; `OR(FALSE,"TRUE")` → `#VALUE!` error; `OR()` → `#VALUE!` error | Same rules as `AND` | yes | implemented (ADR-0029) |
| `NOT` | `NOT(0)` → `true`; `NOT(NULL)` → `true`; `NOT("x")` → `#VALUE!` error; `NOT()` → `Error: NOT expects 1 argument`. Keyword form: `NOT TRUE` → `false` | Text → `#VALUE!`. 1 argument required | yes | implemented (ADR-0029) |
| `XOR` | `XOR(TRUE,FALSE)` → `true`; `XOR(TRUE,TRUE,TRUE)` → `true`; `XOR(TRUE)` → `true`; `XOR()` → `#VALUE!` error | 1 to 254 arguments; TRUE when an odd number are TRUE | yes | implemented (ADR-0029) |
| `BETWEEN` | `BETWEEN(1,10,5)` → `true` (`min, max, value`); `BETWEEN(5,1,10)` → `false` | No Excel function | Skillet-only | fixed (B-10, #68) |

## Text

| Function | Skillet (expr → output) | Excel | Match? | Status |
|---|---|---|---|---|
| `CONCAT` | `CONCAT("a",1,TRUE,NULL)` → `"a1TRUE"`; `CONCAT("a",[1,2])` → `"a12"`; `CONCAT(1.5,2)` → `"1.52"`; `CONCAT()` → `""`; `CONCAT("a", 1::Currency, DATE(2026,1,1))` → `"a12026-01-01T00:00:00Z"` | Accepts ranges. Booleans become `TRUE`/`FALSE`. Numbers use 15 significant digits (X-7). A date is its serial number | edges | **deliberate** (ADR-0031): Currency is its number, a DateTime its ISO 8601 UTC text (the `::String` text), not a serial number. X-7 deliberate |
| `UPPER` / `LOWER` | `UPPER("abc")` → `"ABC"`; `UPPER(1)` → `Error: UPPER expects string` | `UPPER(1)` = "1" | yes | X-9 deliberate |
| `TRIM` | `TRIM("  a   b  ")` → `"a b"`; `TRIM(1)` → `Error: TRIM expects string` | Removes leading and trailing spaces and collapses inner runs to one | yes | implemented D-7 (#84). `.trim()` keeps Ruby `strip`: `"  a   b  ".trim()` → `"a   b"`. `TRIM(1)` is X-9 |
| `LEFT` / `RIGHT` | `LEFT("hello")` → `"h"`; `LEFT("hello",-1)` → `""`; `LEFT("hello",10)` → `"hello"`; `LEFT("hello",2.9)` → `"he"`; `LEFT(12345,2)` → `Error: LEFT expects string as first argument`; `RIGHT("héllo",4)` → `"éllo"` | `num_chars` defaults to 1 and is truncated. Negative → `#VALUE!` | edges | open: negative length should be an error. `LEFT(12345,2)` is X-9 |
| `MID` | `MID("hello",2,3)` → `"ell"`; `MID("hello",0,2)` → `"he"`; `MID("hello",10,2)` → `""`; `MID("hello",2)` → `"ello"`; `MID("hello",2,-1)` → `""` | All 3 arguments required. `start_num` < 1 → `#VALUE!`. Negative `num_chars` → `#VALUE!`. A start past the end gives "" (matches) | edges | open: errors for `start` < 1 and negative length. The optional third argument stays an extension |
| `REPLACE` | `REPLACE("abcdef",2,3,"X")` → `"aXef"`; `REPLACE("abcdef",0,1,"X")` → `"Xbcdef"`; `REPLACE("abc",10,1,"X")` → `"abcX"`; `REPLACE("abc",1,-1,"X")` → `"Xabc"` | `start_num` < 1 or negative `num_chars` → `#VALUE!`. A start past the end appends (matches) | edges | open: errors instead of clamping |
| `SUBSTITUTE` | `SUBSTITUTE("a-b-c","-","+")` → `"a+b+c"`; `SUBSTITUTE("a-b-c","-","+",2)` → `"a-b+c"`; `SUBSTITUTE("abc","","x")` → `"abc"`; `SUBSTITUTE("Hello","hello","x")` → `"Hello"`; `SUBSTITUTE(123,"2","x")` → error | `instance_num` replaces only that occurrence. Case-sensitive. An empty `old` returns the text unchanged (**verify in Excel**) | yes | implemented B-6, B-16 (#84). `SUBSTITUTE(123, …)` is X-9 |
| `ISBLANK` | `ISBLANK(NULL)` → `true`; `ISBLANK("")` → `true`; `ISBLANK("  ")` → `true`; `ISBLANK([])` → `true`; `ISBLANK(0)` → `false`; `ISBLANK()` → `true` | TRUE only for an empty cell. `ISBLANK("")` = FALSE | differs | **deliberate** (ADR-0018): "null or whitespace or empty" suits JSON input, and ADR-0006 relies on it |
| `ISNUMBER` | `ISNUMBER(1)` → `true`; `ISNUMBER("1")` → `false`; `ISNUMBER(TRUE)` → `false`; `ISNUMBER(1::Currency)` → `true` | Same | yes | unchanged |
| `ISTEXT` | `ISTEXT("a")` → `true`; `ISTEXT(1)` → `false` | Same | yes | unchanged |
| `LEN` | `LEN("héllo")` → `5`; `LEN(123)` → `3`; `LEN(TRUE)` → `4`; `LEN(NULL)` → `0`; `LEN([1,2])` → `Error: LEN expects text` | Number of characters; numbers and logical values count as their text | yes | implemented D-9 (#83) |
| `TEXTJOIN` | `TEXTJOIN("-", TRUE, "a", "", "b")` → `"a-b"`; `TEXTJOIN("-", FALSE, "a", "", "b")` → `"a--b"`; `TEXTJOIN(",", TRUE, [1,NULL,"a"])` → `"1,a"` | `TEXTJOIN(delimiter, ignore_empty, text1, ...)` | yes | implemented D-9 (#83). `JOIN` is unchanged |
| `LENGTH` | `LENGTH("héllo")` → `5`; `LENGTH([1,2])` → `2`; `LENGTH(NULL)` → `0`; `LENGTH(123)` → `Error: LENGTH expects array or string` | Excel's name is `LEN` (above) | Skillet-only | keep |
| `SUBSTRING` | `SUBSTRING("hello",1,3)` → `"ell"` (0-based) | No Excel function (use `MID`, 1-based) | Skillet-only | keep. In docs, prefer `MID` for Excel users |
| `SPLIT`, `SUBSTITUTEM`, `INCLUDES` | `SPLIT("a, b,c")` → `["a","b","c"]` (default `,`, trimmed); `INCLUDES("hello","ell")` → `true`; `SUBSTITUTEM("a-b-c","-","+")` → `"a+b+c"` | Excel has `TEXTSPLIT`, and `ISNUMBER(SEARCH())` for contains | Skillet-only | keep. `SUBSTITUTEM` takes 3 arguments and duplicates `SUBSTITUTE`, which now honors `instance_num`: a deprecation candidate (no backlog item yet) |
| `REVERSE` | `REVERSE("abc")` → `Error: REVERSE expects array`; `REVERSE([1,2,3])` → `[3,2,1]` | No Excel function | Skillet-only | open bug B-11: the string version in `string.rs` can never be reached |

Excel text functions that still return `Unknown function` (checked): `SEARCH`, `EXACT`, `TEXT`,
`VALUE`, `REPT`, `PROPER`.

## Arrays, counting and conditional aggregates

| Function | Skillet (expr → output) | Excel | Match? | Status |
|---|---|---|---|---|
| `COUNT` | `COUNT([1,"a",TRUE,NULL])` → `1`; `COUNT(1,2,3)` → `3`; `COUNT(5)` → `1`; `COUNT([])` → `0` | `COUNT(value1, ...)` counts numbers only | yes | implemented D-3 (#84). `.length()`/`.count()` keep returning the array length. X-3 deliberate |
| `COUNTA` | `COUNTA([1,"a",TRUE,NULL])` → `3`; `COUNTA([1,"",NULL])` → `2` | Counts non-empty values (`""` counts) | yes | implemented D-3 (#84) |
| `COUNTIF` | `COUNTIF([1,2,3,4], ">2")` → `2`; `COUNTIF([1,2,3], 2)` → `1`; `COUNTIF(["a","A","b"], "a")` → `2`; `COUNTIF(["apple","Avocado","pear"], "a*")` → `2`; `COUNTIF([1,2,3], :x)` → `Error: COUNTIF criteria expression must return TRUE or FALSE, got number` | Criteria like `">2"`, `2`, `"a*"`; case-insensitive | yes | implemented D-2, B-3, B-17 (#84). The lambda form (`:x > 2`) is a Skillet extension |
| `SUMIF` | `SUMIF([1,2,3,4],">2")` → `7`; `SUMIF([1,2,3],">=2",[10,20,30])` → `50`; `SUMIF([1,2,3],2)` → `2`; `SUMIF([1,2,3],"<>2")` → `4`; `SUMIF(["A","b"],"a",[1,2])` → `1`; `SUMIF(["apple","avocado"],"a*",[1,2])` → `3`; `SUMIF([1,2,3],">=x")` → `0`; `SUMIF([1,2,3], :x > 1)` → `5`; `SUMIF([1,2,3],">1",[10,20::Currency,30])` → `Currency(50)` | Text criteria are case-insensitive with `*`, `?`, `~`. `">=x"` compares text cells only, so numbers never match (0) | yes | implemented D-7 (#84). Currency when a summed value is Currency, as `SUM` (B-13; ADR-0031 amendment 1, BL-078) |
| `AVERAGEIF` (`AVGIF`) | `AVERAGEIF([1,2,3,4], ">2")` → `3.5`; `AVGIF([1,2,3,4], :x > 2)` → `3.5`; `AVERAGEIF([1,2], ">5")` → `Error: AVERAGEIF: no matching numbers (#DIV/0!)`; `AVERAGEIF([1,2::Currency,3], ">1")` → `Currency(2.5)` | `AVERAGEIF(range, criteria, [average_range])`. No match → `#DIV/0!` | yes | implemented D-9 (#84). `AVGIF` is an alias. Currency when an averaged value is Currency (B-13; ADR-0031 amendment 1, BL-078) |
| `UNIQUE` | `UNIQUE(["a","a",1])` → `[1]`; `UNIQUE([3,1,3,2])` → `[3,1,2]`; method `["a","a",1].unique()` → `["a",1]` | Keeps values of every type, in first-seen order | differs | open bug B-7: the function drops every non-number |
| `SORT` | `SORT([3,1,2])` → `[1,2,3]`; `SORT([3,1,2],"DESC")` → `[3,2,1]`; `SORT([3,1,2],-1)` → `[3,2,1]`; `SORT(["b","a"])` → `Error: SORT expects numeric array` | `SORT(array, [sort_index], [sort_order], [by_col])`. Sorts text | differs | `-1`/`1` implemented (B-12, #84). Open: sorting text. `"DESC"` stays an extension |
| `FILTER` | `FILTER([1,2,3], :x > 1)` → `[2,3]` | `FILTER(array, include, [if_empty])`, `include` is a boolean array | differs | **deliberate** (ADR-0018): Skillet lambda syntax (ADR-0005). Same intent |
| `MAP` | `MAP([1,2], :x * 2)` → `[2,4]` | `MAP(array, LAMBDA(x, ...))` | differs | **deliberate** (ADR-0018): implicit `:x` replaces `LAMBDA` |
| `REDUCE` | `REDUCE([1,2,3], :acc + :x, 0)` → `6` | `REDUCE([initial_value], array, LAMBDA(acc, value))`: the initial value comes first | differs | **deliberate** (ADR-0018). Document it prominently: same name, different argument order. Changing it would break every `REDUCE` |
| `FIND` | `FIND("b","abc")` → `2`; `FIND("B","abc")` → `Error: FIND: find_text not found in within_text`; `FIND("b","abcb",3)` → `4`; `FIND("b","abc",9)` → `Error: FIND start_num must be between 1 and the length of within_text`; `FIND([1,2,3], :x > 1)` → `2`; `FIND(1, 2)` → `Error: FIND first arg must be array` | `FIND(find_text, within_text, [start_num])`: 1-based, case-sensitive | yes | implemented D-6 (#83): dispatch on the first argument's type. The array form is a Skillet extension |
| `ARRAY`, `FLATTEN`, `FIRST`, `LAST`, `CONTAINS`, `IN`, `JOIN`, `MERGE` | `FIRST([])` → `Error: FIRST on empty array`; `JOIN([1,"a",TRUE],"-")` → `"1-a-TRUE"`; `JOIN([1,2])` → `"1,2"`; `MERGE([1],[2],3)` → `[1,2,3]` | `JOIN` is like `TEXTJOIN`, which puts the delimiter first | Skillet-only | keep. `TEXTJOIN` now exists (above). The `.join()` method writes Currency and `NULL` as `JOIN` does (BL-078); its booleans stay `true`/`false` |
| `DIG`, `JQ` | `DIG({"a":{"b":1}}, ["a","b"])` → `1` | none | Skillet-only | keep |

## Date and time

Skillet's `DateTime` is a count of Unix seconds (UTC). Excel stores dates as serial day numbers
(1 = 1900-01-01) with the time as a fraction of a day. Since D-5, a `Number` next to a
`DateTime` counts days, so most Excel date formulas work, but the two types stay distinct.

| Function | Skillet (expr → output) | Excel | Match? | Status |
|---|---|---|---|---|
| `DATE` | `DATE(2026,1,15)` → `DateTime(1768435200)`; `DATE(2026,13,1)` → 2027-01-01; `DATE(2026,2,30)` → 2026-03-02; `DATE(2026,0,1)` → 2025-12-01; `DATE(2026,1,0)` → 2025-12-31; `DATE(26,1,1)` → 1926-01-01; `DATE(0,1,1)` → 1900-01-01; `DATE(10000,1,1)` → `Error: DATE year must be between 0 and 9999`; `DATE(2026.9,1.9,1.9)` → 2026-01-01; `DATE()` → today, midnight UTC | Months and days roll over; years 0–1899 get 1900 added; arguments are truncated | yes | implemented D-5 (#83). `DATE()` is a Skillet extension |
| `TODAY` / `NOW` | `TODAY()` → today at 00:00:00 UTC (`TODAY() == DATE()` → `true`); `NOW()` → `DateTime(…)` (UTC, whole seconds) | Serial number, local time | differs | `TODAY` implemented D-9 (#83). **Deliberate** (ADR-0018): UTC gives the same result on every server |
| `TIME` | `TIME(14,30,0)` → `0.6041666666666666`; `TIME(12,0,0)` → `0.5`; `TIME(0,90,0)` → `0.0625`; `TIME(25,0,0)` → `0.041666666666666664`; `TIME(-1,0,0)` → `Error: TIME hour must not be negative (#NUM!)`; `TIME(1)` → error | A fraction of a day, with rollover | yes | implemented D-2 (B-1: #61, fraction of a day: #84). `TIME()` returns the current local time in the same unit (extension) |
| `YEAR` / `MONTH` / `DAY` | `YEAR(DATE(2026,3,4))` → `2026`; `MONTH(...)` → `3`; `YEAR(46000)` → `Error: YEAR expects datetime`; `YEAR("2026-01-01")` → same error | Also accept serial numbers and date text | edges | **deliberate** (ADR-0018): `DateTime` is a real type, not a serial number. All components are UTC. Strings would work after the cast proposal below |
| `DATEADD` | `DATEADD(DATE(2026,1,31),1,"m")` → 2026-02-28 (clamped to the month end); `DATEADD(DATE(2024,1,31),1,"months")` → 2024-02-29; `DATEADD(DATE(2024,2,29),1,"yyyy")` → 2025-02-28; `DATEADD(DATE(2026,1,1),1.9,"days")` → +1 day (truncated); `DATEADD(DATE(2026,1,1),1,"n")` → +1 minute; `DATEADD(DATE(2026,1,1),1,"eons")` → `Error: … (#VALUE!)` | No worksheet function; VBA `DateAdd` takes the same codes (`yyyy`, `q`, `m`, `y`, `d`, `w`, `ww`, `h`, `n`, `s`) and clamps the same way | Skillet-only | implemented B-15: VBA `DateAdd` codes, any case, plus Skillet's long words (`"months"`, `"minutes"`, …) |
| `DATEDIFF` | `DATEDIFF(DATE(2026,1,1), DATE(2026,3,1), "days")` → `59`; `DATEDIFF(DATE(2026,1,31), DATE(2026,2,1), "m")` → `1`; `DATEDIFF(DATE(2025,12,31), DATE(2026,1,1), "yyyy")` → `1`; `DATEDIFF(DATE(2026,1,3), DATE(2026,1,4), "ww")` → `1` (one Sunday); without a unit → `Error: DATEDIFF expects date1, date2, unit` | VBA `DateDiff` takes the same codes and counts boundaries for every unit; Skillet counts boundaries for `yyyy`, `q`, `m`, `ww` and whole elapsed units for `d`, `y`, `w`, `h`, `n`, `s` (the same for dates at midnight) | Skillet-only | implemented B-15: VBA `DateDiff` codes, any case, plus the long words. Unknown unit → `#VALUE!` |
| `DATEDIF` | `DATEDIF(DATE(2001,1,1), DATE(2003,1,1), "Y")` → `2`; `DATEDIF(DATE(2001,6,1), DATE(2002,8,15), "D")` → `440`; `… "YD")` → `75`; `… "YM")` → `2`; `… "MD")` → `14`; `DATEDIF(DATE(2011,1,31), DATE(2011,3,1), "md")` → `-2`; start after end or unit `"q"` → `Error: … (#NUM!)` | Complete units; `#NUM!` for start > end or an unknown unit; accepts serial numbers and date text | edges | implemented B-15: Excel's codes and `"md"`/`"ym"`/`"yd"` rules. Takes `DateTime` values only; times are ignored |

Date arithmetic (D-5, #83; comparison BL-059, #88): `DATE(2026,1,2) - DATE(2026,1,1)` → `1`;
`DATE(2026,1,1) + 1` and `1 + DATE(2026,1,1)` → 2026-01-02; `DATE(2026,1,1) - 1` → 2025-12-31;
`DATE(2026,1,1) + 0.5` and `DATE(2026,1,1) + TIME(12,0,0)` → 2026-01-01 12:00 UTC;
`DATE(2026,1,1) + DATE(2026,1,1)` → `Error: Arithmetic op on non-number`;
`DATE(2026,1,1) == DATE(2026,1,1)` → `true`; `DATE(2026,1,1) < DATE(2026,1,2)` → `true`;
`DATE(2026,1,1) == 0` → `false`. Open (BL-070): `DATE(2026,1,1) + "1"` and `DATE(...) + NULL` are
errors, where Excel returns a date. Excel date functions that still return `Unknown function`
(checked): `WEEKDAY`, `EDATE`, `EOMONTH`, `DATEVALUE`, `NETWORKDAYS`.

## Financial

Skillet follows Excel's cash-flow sign convention: money you receive is positive, money you pay is
negative.

| Function | Skillet (expr → output) | Excel | Match? | Status |
|---|---|---|---|---|
| `PMT` | `PMT(0.05/12,360,100000)` → `-536.8216230121399`; `PMT(0,10,1000)` → `-100`; `PMT(0.1,2,1000,0,1)` → `-523.8095238095234`; `PMT(0.06/12,120,0,50000)` → `-305.1025097082559`; `PMT(0.05,0,1000)` → `Error: PMT nper must be positive`; `PMT(0.05,10,1000,"x")` → `#VALUE!` error; `PMT(0,10,1000,"0")` → `-100` | -536.82, -100, -523.81, -305.10. Non-numeric `fv` → `#VALUE!` | yes | implemented X-5 (#84; numeric text #89) |
| `FV` | `FV(0.05,10,-100)` → `1257.789253554884`; `FV(0.05,10,-100,-1000)` → `2886.683880332326`; `FV(0.05,10,-100,0,1)` → `1320.6787162326282`; `FV(0,10,-100,-1000)` → `2000`; `FV(0.05,-1,-100)` → `Error: FV nper must be non-negative` | 1257.79, 2886.68, 1320.68, 2000. A negative `nper` is computed, not rejected (**verify in Excel**) | edges | open, low: confirm Excel's negative-`nper` result first |
| `IPMT` | `IPMT(0.1,1,3,1000)` → `-100`; `IPMT(0.1,2,3,1000)` → `-69.78851963746224`; `IPMT(0.1,3,3,1000)` → `-36.55589123867071`; `IPMT(0.1,1,3,1000,0,1)` → `0`; `IPMT(0.1,2,3,1000,0,1)` → `-63.44410876132931`; `IPMT(0.1,4,3,1000)` → `Error: IPMT period must be between 1 and nper` | -100.00; -69.79; -36.56; type 1: 0, -63.44 | yes | implemented D-2, B-2 (#84) |
| `PPMT` | `PPMT(0.1,1,3,1000)` → `-302.1148036253773`; `PPMT(0.1,1,3,1000) + IPMT(0.1,1,3,1000) - PMT(0.1,3,1000)` → `0` | `PMT = IPMT + PPMT` | yes | implemented D-9 (#83) |
| `PV` | `PV(0.05,10,-100)` → `772.1734929184817` | 772.17 | yes | implemented D-9 (#83) |
| `NPV` | `NPV(0.1,100,100)` → `173.55371900826447`; `NPV(0.1,[100,100])` → same | 173.55 | yes | implemented D-9 (#83) |
| `DB` | `DB(1000000,100000,6,1,7)` → `185912.95971618922`; `DB(1000000,100000,6,6,7)` → `23290.87494931791`; `DB(1000000,100000,6,7,7)` → `0`; `DB(10000,1000,5,0)` → `2328.5017392669597`; `DB(0,0,5,1)` → `NaN` | Microsoft's example (1,000,000, 100,000, life 6, month 7): periods 1–7 = 186,083.33; 259,639.42; 176,814.44; 120,410.64; 81,999.64; 55,841.76; 15,845.10. The rate is rounded to 3 decimals (0.319). Period 0 → `#NUM!` | differs | open bug B-8: round the rate to 3 decimals, apply the remaining-months formula to period `life + 1`, error for period < 1 and for a cost of 0. `NaN` also breaks D-1 |

Excel financial functions that still return `Unknown function` (checked): `NPER`, `RATE`, `IRR`,
`SLN`, `DDB`.

## Statistical

All of these parse with the dotted name (X-4) and return an error on empty input (X-2).

| Function | Skillet (expr → output) | Excel | Match? | Status |
|---|---|---|---|---|
| `MEDIAN` | `MEDIAN(1,3,2)` → `2`; `MEDIAN([1,2,3,4])` → `2.5`; `MEDIAN()` → `Error: MEDIAN of no numbers (#NUM!)`; `MEDIAN([1,"9",TRUE])` → `1` | Same. No numbers → `#NUM!` | yes | implemented (#84). X-3 deliberate |
| `MODE.SNGL` (`MODE_SNGL`, `MODESNGL`, `MODE`) | `MODE.SNGL(1,2,2)` → `2`; `MODE_SNGL(3,1,1,3)` → `3` (first value seen wins a tie); `MODE_SNGL(1,2,3)` → `Error: MODE.SNGL: no value repeats (#N/A)`; `MODE(1,2,2)` → `2` | Same tie rule. No repeated value → `#N/A` | yes | implemented (#84; `MODE` #83) |
| `STDEV.P` (`STDEVP`, `STDEV_P`) | `STDEV.P(1,2)` → `0.5`; `STDEV_P(2,4,4,4,5,5,7,9)` → `2`; `STDEVP(2,4,4,4,5,5,7,9)` → `2`; `STDEV_P(5)` → `0`; `STDEV_P()` → `#DIV/0!` error | Population standard deviation (divides by n) | yes | implemented (#82, #84) |
| `STDEV.S` (`STDEV`, `STDEV_S`) | `STDEV.S(2,4,4,4,5,5,7,9)` → `2.138089935299395`; `STDEV(…)` → same; `STDEV.S(1)` → `Error: STDEV.S needs at least two numbers` | Sample standard deviation (divides by n − 1) | yes | implemented D-9 (#82) |
| `VAR.P` (`VARP`, `VAR_P`) | `VAR.P(2,4,4,4,5,5,7,9)` → `4`; `VARP(1,3)` → `1`; `VAR_P()` → `#DIV/0!` error | Population variance | yes | implemented (#82, #84) |
| `VAR.S` (`VAR`, `VAR_S`) | `VAR.S(2,4,4,4,5,5,7,9)` → `4.571428571428571`; `VAR(1)` → `Error: VAR needs at least two numbers` | Sample variance | yes | implemented D-9 (#82) |
| `PERCENTILE.INC` (`PERCENTILE_INC`, `PERCENTILEINC`, `PERCENTILE`) | `PERCENTILE.INC([1,2,3,4],0.25)` → `1.75`; `PERCENTILE_INC([1,2,3,4],0.4)` → `2.2`; `(…,1.5)` → `Error: Percentile must be between 0 and 1 (#NUM!)`; `PERCENTILE_INC([],0.5)` → `#NUM!` error; `PERCENTILE_INC(1,2,3,4,0.5)` → `2.5` (variadic; the last argument is k); `PERCENTILE_INC([3,1,"x",2],0.5)` → `2`; `PERCENTILE([1,2,3,4],0.25)` → `1.75` | Rank = k·(n−1); k outside 0..1 → `#NUM!`; empty → `#NUM!` | yes | implemented (#82, #84; `PERCENTILE` #83) |
| `QUARTILE.INC` (`QUARTILE_INC`, `QUARTILEINC`, `QUARTILE`) | `QUARTILE.INC([1..8],1)` → `2.75`; `QUARTILE_INC([1,2,3,4],1.9)` → `1.75` (truncated); `(…,0)` → `1`; `(…,4)` → `4`; `(…,5)` and `(…,-0.5)` → `Error: Quartile must be 0-4 (#NUM!)` | A non-integer quart is truncated; quart < 0 or > 4 → `#NUM!`. For -0.5, check the result after truncation (**verify in Excel**) | yes | implemented (#82, #84; `QUARTILE` #83) |

## Function and method parity

ADR-0018 decided that lowercase methods keep Ruby semantics and do not have to give the same
result as the builtin of the same name. The pairs that differ:

| Pair | Function | Method | Status |
|---|---|---|---|
| `INT` / `.int()` | `INT(-3.7)` → `-4` | `(-3.7).int()` → `-3` | **deliberate** (ADR-0018): `.int()` truncates like Ruby's `to_i` |
| `ROUND` / `.round()` | `ROUND(1234.567,-2)` → `1200` | `(1234.567).round(-2)` → `1200` | same result since #84 |
| `MAX`/`MIN` on empty | `MAX([])` → `0` | `[].max()` → `Null`, `[].min()` → `Null` | **deliberate** (ADR-0018 methods rule) |
| `SUM` with text | `SUM([1,"2",3])` → `4` | `[1,"2",3].sum()` → `Error: sum method expects numeric array` | **deliberate** (ADR-0018 methods rule; BL-070 lists methods staying strict) |
| `UNIQUE` | `UNIQUE(["a","a",1])` → `[1]` | `["a","a",1].unique()` → `["a",1]` | open: the function is wrong (B-7) |
| `TRIM` | `TRIM("  a   b  ")` → `"a b"` | `"  a   b  ".trim()` → `"a   b"` | **deliberate** (ADR-0018 D-7) |
| Coverage | `MEDIAN`, `STDEV_P`, `PERCENTILE_INC`, `LEFT`, `MOD` exist | `[1,2].median()` → `Unknown array method: median`; `(5).mod(2)` → `Unknown number method: mod` | open: document which forms exist |

## Bugs (separate from deliberate differences)

| ID | Severity | Repro | Now | Status |
|---|---|---|---|---|
| B-1 | High | `TIME(14, 30, 0)` | `0.6041666666666666` | fixed (#61; Excel unit #84) |
| B-2 | High | `IPMT(0.1, 2, 3, 1000)` | `-69.78851963746224` | fixed (#84) |
| B-3 | High | `COUNTIF([1,2,3,4], ">2")`, `COUNTIF([1,2,3], 2)` | `2`, `1` | fixed (#84) |
| B-4 | Medium | `ROUND(1234.567, -2)` | `1200` | fixed (#84) |
| B-5 | Medium | `CEILING(2.1, 0.5)`, `FLOOR(7, 5)` | `2.5`, `5` | fixed (#84) |
| B-6 | Medium | `SUBSTITUTE("a-b-c", "-", "+", 2)` | `"a-b+c"` | fixed (#84) |
| B-7 | Medium | `UNIQUE(["a", "a", 1])` | `[1]` (strings dropped) | **open**; expected `["a", 1]` |
| B-8 | Medium | `DB(1000000,100000,6,6,7)`; `DB(1000000,100000,6,7,7)`; `DB(10000,1000,5,0)`; `DB(0,0,5,1)` | `23290.87`; `0`; `2328.50`; `NaN` | **open**; expected `55841.76`; `15845.10`; error; error |
| B-9 | Medium | `STDEV.P(1, 2)` | `0.5` | fixed (#82) |
| B-10 | Low | `BETWEEN(1, 10, 5)` | `true` | fixed (#68) |
| B-11 | Low | `REVERSE("abc")` | `Error: REVERSE expects array` | **open**; expected `"cba"` |
| B-12 | Low | `SORT([3,1,2], -1)` | `[3,2,1]` | fixed (#84) |
| B-13 | Low | `ROUND(10::Currency, 1)`; `SUM(10::Currency, 5)` | `Currency(10)`; `Currency(15)` | fixed: `ROUND` (#84); `SUM`, `PRODUCT`, `AVERAGE`, `MIN`, `MAX` keep Currency (ADR-0031, 2026-11-21); `SUMIF`, `AVERAGEIF` too (ADR-0031 amendment 1, BL-078) |
| B-14 | Low | `(-3.7).int()` vs `INT(-3.7)` | `-3` vs `-4` | closed: deliberate (ADR-0018) |
| B-15 | Low | `DATEADD(DATE(2026,1,1), 1, "m")` | +1 minute | fixed (2026-11-21): units follow Excel, `"m"` is months, `"n"` minutes; `DATEDIF` added |
| B-16 | Low | `SUBSTITUTE("abc", "", "x")` | `"abc"` | fixed (#84) |
| B-17 | Low | `COUNTIF([1,2,3], :x)` (lambda returns a non-boolean) | `Error: COUNTIF criteria expression must return TRUE or FALSE, got number` | fixed (#84) |
| B-18 | Medium | `IF("x", 1, 2)`; `IFS(FALSE, 1)` | `#VALUE!` error; `#N/A` error | fixed (ADR-0029, BL-073); were `2` and `false` |

Doc defects in `docs/reference/api.md` that are still there: `COMPACT(array)` is documented as a
function but only the `.compact()` method exists (`COMPACT([1,NULL])` → `Unknown function`); the
`PMT` examples show positive amounts ("$536.82") where the result is negative. The other defects
from the audit (`LEN`, `COUNT`, `TIME`, `INT(-3.9)`, dotted headings) are fixed.

## Decisions for the maintainer (ranked by user impact)

All ten are decided in [ADR-0018](../adr/0018-excel-compatibility.md) (2026-11-21).

1. **D-1: Errors versus silent defaults** (X-1, X-2, X-5, B-17). Excel's error cases return
   `Error`; `Null` stays tolerated where ADR-0006 and blank cells agree. **Implemented** in #84.
   ADR-0023 §6 (#89) added Excel's conversion of numeric text and booleans in arithmetic; other
   text is still an error. Left over: `DB(0,0,5,1)` → `NaN` (B-8).
2. **D-2: Fix `TIME`, `IPMT` and `COUNTIF`** (B-1, B-2, B-3). `TIME` returns a fraction of a day.
   **Implemented** in #84 (`TIME` arguments first fixed in #61).
3. **D-3: `COUNT` semantics.** Numbers only and variadic, plus `COUNTA`. **Implemented** in #84.
4. **D-4: Dotted names** (X-4, B-9). `NAME.NAME(` parses. **Implemented** in #82.
5. **D-5: `DATE` rollover and date arithmetic.** Rollover, Excel's 0–1899 year rule, date
   arithmetic in days; string casts stay strict. **Implemented** in #83; DateTime comparison
   in #88 (BL-059).
6. **D-6: The `FIND` name collision.** Dispatch on the first argument's type. **Implemented** in #83.
7. **D-7: `MOD` sign, `TRIM` inner spaces, `SUMIF` case and wildcards.** Excel for the
   functions, Ruby for the methods. **Implemented** in #84.
8. **D-8: Lazy `IF`/`IFS`** (X-8). **Implemented** in #82; the errors for a text condition and
   for `IFS` with no true condition (B-18) with ADR-0029 (BL-073).
9. **D-9: Excel names** `LEN`, `AVERAGEIF`, `STDEV.S`/`STDEV`, `VAR.S`/`VAR`, `TODAY`, `PPMT`,
   `PV`, `NPV`, `TEXTJOIN`, `PERCENTILE`, `QUARTILE`, `MODE`. **Implemented**: `STDEV.S` and
   `VAR.S` in #82; `LEN`, `TEXTJOIN`, `TODAY`, `PPMT`, `PV`, `NPV` and the legacy aliases in #83;
   `AVERAGEIF` (and `COUNTA`, D-3) in #84.
10. **D-10: Operators** (X-6). Excel precedence (`-2^2` = 4, `2^3^2` = 64), the `%` suffix and
    `&` text join. **Implemented** in #82. ADR-0023 (#89) made `%` before `+`/`-` the percent
    suffix (`10%-3` = -2.9) and kept `==` case-sensitive (#88).

Also decided in ADR-0018 and implemented in #84: `ROUND` negative digits and decimal-correct
halves, `INT` errors, `CEILING`/`FLOOR` significance.

## Proposal: canonical form (`SUM(:a)` vs `:a.sum()`)

Not a decision. For the Orchestrator or a human to accept as an ADR.

**Rule.** Docs and examples use the **function form** for a single operation that has an Excel
name, and the **method form** for pipelines and for operations that only exist as methods.

| Situation | Canonical | Not in docs |
|---|---|---|
| One aggregate or math call over a value | `SUM(:prices)`, `ROUND(:total, 2)`, `AVERAGE(:scores)` | `:prices.sum()` |
| Two or more chained transformations | `:items.filter(:x.active).map(:x.price).sum()` | `SUM(MAP(FILTER(:items, :x.active), :x.price))` |
| Something that only exists as a method | `:name.trim().upper()` | n/a |
| Null-safe access | `:user&.address&.city` | `DIG(...)` for fixed paths; `DIG` for dynamic key arrays |

Why: the primary users (PRD-001) are analysts who know Excel, so a single `SUM(:a)` reads like a
spreadsheet. Pipelines read left to right only as method chains. Nested function calls are the
main reason Excel formulas are hard to read. The rule is easy to apply: is it a single call? Use
the function. Is it chained? Use methods.

**Note since ADR-0018:** this proposal assumed that a function and the method of the same name
give the same result. ADR-0018 decided the opposite (methods keep Ruby semantics; see the parity
table), so the canonical form can change the answer, not only the spelling. An ADR for this
proposal must say that docs show the function form wherever Excel semantics are meant.

## Proposal: string to DateTime cast formats

Not a decision and not implemented, except one row: ADR-0031 made a DateTime's text ISO 8601 UTC
(`DATE(2026,1,1)::String` → `"2026-01-01T00:00:00Z"`) and made the cast read that exact form
(`"2026-01-01T10:30:00Z"::DateTime` below). Today `"2026-01-01"::DateTime` →
`Error: Cannot cast String to DateTime`, and otherwise only numeric strings work
(`"1767225600"::DateTime` → `DateTime(1767225600)`). ADR-0018 D-5 decided that casts stay strict
(no rollover, no format guessing) and left the accepted formats to this proposal.

**Accepted inputs** (a subset of ISO 8601 / RFC 3339, so the result does not depend on locale):

| Input | Result | Notes |
|---|---|---|
| `"2026-01-01"::DateTime` | `DateTime(1767225600)` | Date only → 00:00:00 UTC. The same as `DATE(2026,1,1)` |
| `"2026-01-01T10:30:00Z"::DateTime` | `DateTime(1767263400)` | RFC 3339, UTC. **Implemented** (ADR-0031) |
| `"2026-01-01T10:30:00+02:00"::DateTime` | `DateTime(1767256200)` | The offset is applied and stored as UTC |
| `"2026-01-01T10:30:00"::DateTime` | `DateTime(1767263400)` | No offset → UTC (consistent with `DATE`). Never the server's local zone |
| `"2026-01-01 10:30:00"::DateTime` | `DateTime(1767263400)` | Space separator, for SQL and CSV exports |
| `"2026-01-01T10:30:00.987Z"::DateTime` | `DateTime(1767263400)` | Fractional seconds truncated (`DateTime` is whole seconds) |
| `"1767225600"::DateTime` | `DateTime(1767225600)` | Existing behavior, **kept** (integer string = Unix seconds) |
| `"01/02/2026"::DateTime` | `Error: Cannot cast "01/02/2026" to DateTime (expected YYYY-MM-DD or RFC 3339)` | Ambiguous between MM/DD and DD/MM, so rejected on purpose |
| `"2026-02-30"::DateTime` | `Error: ... invalid date` | No rollover in casts, although `DATE` rolls over (D-5) |
| `"2026-1-5"::DateTime` | `Error` | Zero-padded fields only; keeps the parser strict and predictable |

Compatibility: this only turns current errors into values. No expression that currently returns
a value changes. `DateTime::String` producing ISO 8601 was a follow-up here; ADR-0031 decided it
(a breaking change in 0.7). An Excel-named `DATEVALUE(text)` could be a thin wrapper around the cast,
and `YEAR`/`MONTH`/`DAY` could accept strings by applying the same cast.
