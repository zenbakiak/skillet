# Excel compatibility audit (BL-020)

- Status: research, decided. The maintainer decided D-1…D-10 on 2026-11-21 in
  [ADR-0018](../adr/0018-excel-compatibility.md): Excel is the contract for UPPERCASE builtins and
  operators. The Skillet column below still shows the audited v0.6.3 output; the fixes ship in 0.7.0.
- Scope: every builtin registered in `src/runtime/function_dispatch.rs`, plus the functions the
  evaluator handles itself (`FILTER`, `FIND`, `MAP`, `REDUCE`, `SUMIF`, `AVGIF`, `COUNTIF`, `JQ`).
- Skillet column: real output of the release build (`./target/release/sk "<expr>"`, v0.6.3,
  `develop` at `d3e6a5d`). Output is shown the way `sk` prints it, shortened (`3` means `Number(3.0)`).
- Excel column: Excel's documented behavior for the function (Microsoft 365). Values marked
  **verify in Excel** come from the documented rule, but nobody has checked the exact number
  in a workbook yet.
- Match values: **yes**; **edges** (same result for valid, ordinary inputs; differs only on
  empty, text, wrong-type or out-of-range inputs); **differs** (different result for ordinary
  inputs, or the signature is different); **Skillet-only** (no Excel function by that name).

QA can turn each table row into a characterization test (pins current behavior) or an
acceptance test (pins the proposed behavior once a fix is accepted).

## Summary

| | Count | Functions |
|---|---|---|
| yes | 2 | `ISNUMBER`, `ISTEXT` |
| edges | 32 | most aggregates, math, text slicing, `PMT`, `FV`, statistics |
| differs | 22 | `ROUND`, `CEILING`, `FLOOR`, `MOD`, `IF`, `XOR`, `TRIM`, `SUBSTITUTE`, `ISBLANK`, `COUNT`, `COUNTIF`, `UNIQUE`, `SORT`, `FILTER`, `MAP`, `REDUCE`, `FIND`, `DATE`, `NOW`, `TIME`, `IPMT`, `DB` |
| Skillet-only | 24 (+8 alias spellings) | `AVG`, `CEIL`, `POW`, `MULTIPLY`, `BETWEEN`, `LENGTH`, `SUBSTRING`, `SPLIT`, `SUBSTITUTEM`, `INCLUDES`, `ARRAY`, `FLATTEN`, `FIRST`, `LAST`, `CONTAINS`, `IN`, `JOIN`, `MERGE`, `REVERSE`, `AVGIF`, `DIG`, `JQ`, `DATEADD`, `DATEDIFF`; spellings `MODESNGL`, `MODE_SNGL`, `STDEV_P`, `VAR_P`, `PERCENTILEINC`, `PERCENTILE_INC`, `QUARTILEINC`, `QUARTILE_INC` |

56 functions share a name with Excel. 2 match fully, 32 match on valid inputs, and 22 give a
different result for ordinary inputs. `STDEVP` and `VARP` are Excel's legacy names, so they
count with `STDEV.P` and `VAR.P`.

## Cross-cutting differences

These patterns repeat across many rows. They are listed once here, and the tables refer to them by ID.

| ID | Pattern | Skillet | Excel | Recommendation |
|---|---|---|---|---|
| X-1 | **Errors become numbers** | Wrong-type arguments to the math functions are treated as `0` (`ABS("x")` → `0`, `SQRT("x")` → `0`, `FLOOR("3.7")` → `0`). Impossible math returns IEEE values: `SQRT(-1)` → `NaN`, `10 / 0` → `inf`, `0 / 0` → `NaN`, `MOD(5, 0)` → `NaN`, `POWER(0, -1)` → `inf` | `#VALUE!`, `#NUM!`, `#DIV/0!` | **fix** (ADR). Return `Error` (`"division by zero"`, `"SQRT of negative number"`). This follows the design rule "errors over silent coercion". `NaN` and `inf` also fail to serialize as JSON numbers on the servers |
| X-2 | **Empty input becomes 0** | `AVERAGE()`, `MEDIAN()`, `STDEV_P()`, `VAR_P()`, `MODE_SNGL()`, `PERCENTILE_INC([], 0.5)`, `QUARTILE_INC([], 1)`, `AVGIF([1,2], :x > 5)` → `0` | `#DIV/0!` (AVERAGE, STDEV.P, VAR.P, AVERAGEIF), `#NUM!` (MEDIAN, PERCENTILE.INC, QUARTILE.INC), `#N/A` (MODE.SNGL) | **fix** (ADR). Return `Error`, or `Null` under ADR-0006 null rules. "Average of nothing is 0" silently corrupts business rules |
| X-3 | **Direct text and boolean arguments** | Aggregates ignore them anywhere: `SUM(1, "2", TRUE)` → `1`, `MAX(-1, "5")` → `-1` | Values inside a range or array are ignored, which matches Skillet. Values typed directly as arguments are converted: `SUM(1,"2",TRUE)` = 4, `MAX(-1,"5")` = 5 (**verify in Excel**) | **document**. Skillet has no cell ranges, so treating every argument like a range is a reasonable and simpler rule |
| X-4 | **Dotted Excel names do not parse** | `STDEV.P(1,2)` → `Error: Unexpected identifier (expected function call) at position 5`. The same happens for `VAR.P`, `MODE.SNGL`, `PERCENTILE.INC`, `QUARTILE.INC` (and `STDEV.S`, `VAR.S` if added). Dispatch registers the dotted names, but the lexer does not allow `.` in identifiers (`src/lexer.rs:164`) | `STDEV.P(...)` | **fix**. The lexer should accept `IDENT(.IDENT)*` when it is upper case and directly followed by `(`. Lower-case `.method` chains are not affected. Until then, document `STDEV_P` or the legacy `STDEVP` |
| X-5 | **Arguments that are silently ignored** | `CEILING(n, sig)`, `FLOOR(n, sig)`, `SUBSTITUTE(..., instance_num)`, `SORT(arr, -1)`, `ROUND(n, "x")`, `PMT(..., fv="x")` accept the extra argument and do nothing with it | The argument is honored, or `#VALUE!` | **fix**: honor it, or return an error. Never ignore it |
| X-6 | **Operators** | `2 ^ 3 ^ 2` → `512` (right-associative). `-2 ^ 2` → `-4`. `%` is modulo (`-10 % 3` → `-1`). `&` is a parse error. `"a" == "A"` → `false` | `=2^3^2` = 64 (left-associative). `=-2^2` = 4 (negation binds before `^`). `%` is the percent suffix (`10%` = 0.1). `&` joins text. Text comparison ignores case | **document**. PRD-001 FR-1 says "Excel precedence", which is not true for `^` and unary minus. Changing this is a breaking change to the grammar. Recommendation: keep the math convention and correct FR-1. **Decided otherwise** (D-10): Excel precedence |
| X-7 | **Number to text** | `CONCAT(0.1 + 0.2)` → `"0.30000000000000004"` (Rust's shortest round-trip format) | Uses 15 significant digits, so `"0.3"` | **document**. Point users to `ROUND` first |
| X-8 | **No lazy evaluation** | `IF(TRUE, 1, UPPER(1))` → `Error: UPPER expects string`. `IF(FALSE, FIRST([]), 0)` → error. The ternary `c ? a : b` is lazy: `ISBLANK(NULL) ? "" : UPPER(NULL)` → `""` | `IF` evaluates only the branch it picks, so `=IF(A1=0, 0, 1/A1)` is the standard guard | **fix**. Evaluate `IF` and `IFS` lazily, like `SUMIF` and `FILTER` already are in `evaluator.rs`. Results only change where an error used to be returned, so this is low risk |

## Arithmetic

| Function | Skillet (expr → output) | Excel | Match? | Recommendation |
|---|---|---|---|---|
| `SUM` | `SUM(1,2,3)` → `6`; `SUM([1,2],3)` → `6`; `SUM()` → `0`; `SUM([1,"2",TRUE,NULL])` → `1`; `SUM(10::Currency, 5)` → `Number(15)` | `SUM(number1, ...)`. Text and logical values in arrays are ignored. Direct arguments are converted (X-3) | edges | keep and document X-3. The result loses the `Currency` type (B-13) |
| `AVERAGE` (`AVG`) | `AVERAGE(1,2,3,4)` → `2.5`; `AVERAGE()` → `0`; `AVERAGE([1,"x",TRUE])` → `1`; `AVERAGE([1,NULL,3])` → `2` | No numbers → `#DIV/0!` | edges | fix X-2 |
| `MIN` / `MAX` | `MIN(3,1,2)` → `1`; `MIN()` → `0`; `MIN(["a"])` → `0`; `MAX(-1,"5")` → `-1`. Method `[].max()` → `Null`; `["a",1].max()` → error | "If the arguments contain no numbers, MAX returns 0." Direct text is converted | edges | keep the function. The method differs from the function (see the parity table) |
| `ROUND` | `ROUND(2.5)` → `3`; `ROUND(-2.5)` → `-3`; `ROUND(2.345,2)` → `2.35`; `ROUND(1.005,2)` → `1.0`; **`ROUND(1234.567,-2)` → `1235`**; **`ROUND(15,-1)` → `15`**; `ROUND(2.567,"x")` → `3`; `ROUND()` → `0`; `ROUND("2.5")` → error; `ROUND(10::Currency,1)` → `Error: ROUND expects number` | `ROUND(number, num_digits)`. Both arguments are required. Halves round away from zero. Negative `num_digits` rounds to the left of the decimal point: `ROUND(1234.567,-2)` = 1200, `ROUND(15,-1)` = 20. `ROUND(1.005,2)` = 1.01 (**verify in Excel**: Excel corrects binary representation error, Skillet's `(n*100).round()/100` does not) | differs | **fix**: negative digits (B-4), Currency input, and an error for non-numeric digits. Consider decimal-correct rounding (round half away from zero on the shortest decimal form) so `1.005` → `1.01`. Halves away from zero already match |
| `CEILING` (`CEIL`) | `CEILING(2.1)` → `3`; **`CEILING(2.1,0.5)` → `3`**; **`CEILING(7,5)` → `7`**; `CEILING(-2.5)` → `-2`; `CEILING(-2.5,-2)` → `-2` | `CEILING(number, significance)`. Both are required. `CEILING(2.1,0.5)` = 2.5, `CEILING(7,5)` = 10, `CEILING(-2.5,-2)` = -4 | differs | **fix** B-5: honor `significance`, with default 1 so the 1-argument form keeps working. `CEIL` stays as a Skillet alias |
| `FLOOR` | `FLOOR(2.9)` → `2`; **`FLOOR(7,5)` → `7`**; `FLOOR(-2.5)` → `-3`; `FLOOR("3.7")` → `0` | `FLOOR(number, significance)`: `FLOOR(7,5)` = 5 | differs | **fix** B-5, X-1 |
| `ABS` | `ABS(-3)` → `3`; `ABS("x")` → `0`; `ABS()` → `0` | `#VALUE!` for text. 1 argument required | edges | fix X-1 |
| `SQRT` | `SQRT(16)` → `4`; `SQRT(-1)` → `NaN`; `SQRT("x")` → `0` | Negative → `#NUM!` | edges | fix X-1 |
| `POWER` (`POW`) | `POWER(2,10)` → `1024`; `POWER(0,0)` → `1`; `POWER(-8,1/3)` → `NaN`; `POWER(0,-1)` → `inf`; `POWER(2)` → `1` | `POWER(0,0)` → `#NUM!`; `POWER(-8,1/3)` → `#NUM!`; `POWER(0,-1)` → `#DIV/0!`. 2 arguments required | edges | fix X-1. Keep `0^0 = 1` only if documented as a deliberate difference (IEEE, Rust, JS and Python all give 1) |
| `MOD` | `MOD(10,3)` → `1`; **`MOD(-10,3)` → `-1`**; **`MOD(10,-3)` → `1`**; `MOD(5,0)` → `NaN`; `MOD(5.5,2)` → `1.5`; `MOD(10)` → `0` | `MOD(n, d) = n - d*INT(n/d)`, so the result has the divisor's sign: `MOD(-10,3)` = 2, `MOD(10,-3)` = -2. `MOD(5,0)` → `#DIV/0!` | differs | **fix** (ADR; changes the result for negative inputs). Use `rem_euclid`-style floored modulo for `MOD`. The `%` operator can stay a truncated remainder and be documented (X-6) |
| `INT` | `INT(3.7)` → `3`; `INT(-3.7)` → `-4`; `INT("x")` → `0`. Method `(-3.7).int()` → `-3` | Rounds down to the nearest integer: `INT(-3.7)` = -4 | edges | keep the function. **Fix the method** so it floors, or rename it `trunc` (B-14). `docs/reference/api.md` claims `INT(-3.9)` = -3, which is wrong |
| `PRODUCT` (`MULTIPLY`) | `PRODUCT(2,3,4)` → `24`; `PRODUCT()` → `1`; `PRODUCT([2,"x",3])` → `6` | At least 1 argument required. A range with no numbers gives 0 (**verify in Excel**) | edges | document. `1` is the mathematically correct empty product |

## Logical

| Function | Skillet (expr → output) | Excel | Match? | Recommendation |
|---|---|---|---|---|
| `IF` | `IF(TRUE,1,2)` → `1`; `IF(FALSE,1)` → `false`; `IF(1,"a","b")` → `"a"`; `IF("x","a","b")` → `"b"`; `IF(NULL,"a","b")` → `"b"`; `IF(TRUE,1,UPPER(1))` → error | `IF(test, [value_if_true], [value_if_false])`. A missing false branch returns FALSE (matches). Text in `test` → `#VALUE!`. Only the chosen branch is evaluated | differs | **fix** X-8 (lazy). A text condition should return an error, not `false` |
| `IFS` | `IFS(FALSE,1,TRUE,2)` → `2`; `IFS(FALSE,1)` → `false`; `IFS(TRUE)` → error | No true condition → `#N/A` | edges | fix: no match → `Error("IFS: no condition was true")`. Evaluate lazily (X-8) |
| `AND` | `AND(TRUE,1,0)` → `false`; `AND()` → `true`; `AND(TRUE,"x")` → `false`; `AND(FALSE, UPPER(1))` → error | 1 to 255 arguments. Text typed as an argument → `#VALUE!` (**verify in Excel**). Excel also evaluates all arguments | edges | fix: text argument → error; `AND()` → error |
| `OR` | `OR(FALSE,0)` → `false`; `OR()` → `false`; `OR(FALSE,"TRUE")` → `false` | Same rules as `AND` | edges | same as `AND` |
| `NOT` | `NOT(0)` → `true`; `NOT("x")` → `true`; `NOT()` → `true` | Text → `#VALUE!`. 1 argument required | edges | fix: error for text and for no argument |
| `XOR` | `XOR(TRUE,FALSE)` → `true`; `XOR(TRUE,TRUE,TRUE)` → `Error: XOR expects 2 arguments` | `XOR(logical1, [logical2], ...)`, 1 to 254 arguments. TRUE when an odd number of arguments are TRUE, so `XOR(TRUE,TRUE,TRUE)` = TRUE | differs | **fix**: accept 1..n arguments with the odd-count rule. Fully backward compatible |
| `BETWEEN` | `BETWEEN(1,10,5)` → `Error: Unknown function: BETWEEN` | No Excel function | Skillet-only | **bug** B-10: implemented in `logical.rs` and documented, but not registered in dispatch |

## Text

| Function | Skillet (expr → output) | Excel | Match? | Recommendation |
|---|---|---|---|---|
| `CONCAT` | `CONCAT("a",1,TRUE,NULL)` → `"a1TRUE"`; `CONCAT("a",[1,2])` → `"a12"`; `CONCAT(1.5,2)` → `"1.52"`; `CONCAT()` → `""` | Accepts ranges. Booleans become `TRUE`/`FALSE`. Numbers use 15 significant digits (X-7) | edges | keep. Document X-7. Currency and DateTime values are dropped silently, which should become an error or a defined format |
| `UPPER` / `LOWER` | `UPPER("abc")` → `"ABC"`; `UPPER(1)` → `Error: UPPER expects string` | `UPPER(1)` = "1" (the number is converted) | edges | keep strict (no silent coercion). Document it |
| `TRIM` | `TRIM("  a   b  ")` → `"a   b"`; `TRIM(1)` → error | Removes leading and trailing spaces **and collapses inner runs to one space**: "a b" | differs | **fix** (ADR; changes results for strings with inner space runs), or document it and add `CLEAN`/`.squish()`. Recommendation: match Excel, because the name promises it. The `.trim()` method keeps Ruby semantics |
| `LEFT` / `RIGHT` | `LEFT("hello")` → `"h"`; `LEFT("hello",2)` → `"he"`; `LEFT("hello",0)` → `""`; `LEFT("hello",-1)` → `""`; `LEFT("hello",10)` → `"hello"`; `LEFT("hello",2.9)` → `"he"`; `LEFT(12345,2)` → error; `RIGHT("héllo",4)` → `"éllo"` | `num_chars` defaults to 1 and is truncated. Negative → `#VALUE!`. Numbers are converted to text | edges | fix: negative → error. Keep the number rejection strict and document it |
| `MID` | `MID("hello",2,3)` → `"ell"`; `MID("hello",0,2)` → `"he"`; `MID("hello",10,2)` → `""`; `MID("hello",2)` → `"ello"`; `MID("hello",2,-1)` → `""` | `MID(text, start_num, num_chars)`: all 3 are required. `start_num` < 1 → `#VALUE!`. Negative `num_chars` → `#VALUE!`. A start past the end gives "" (matches) | edges | fix: errors for `start` < 1 and negative length. Keep the optional third argument as a documented extension |
| `REPLACE` | `REPLACE("abcdef",2,3,"X")` → `"aXef"`; `REPLACE("abcdef",0,1,"X")` → `"Xbcdef"`; `REPLACE("abc",10,1,"X")` → `"abcX"`; `REPLACE("abc",1,-1,"X")` → `"Xabc"` | Same signature. `start_num` < 1 or negative `num_chars` → `#VALUE!`. A start past the end appends (matches) | edges | fix: errors instead of clamping |
| `SUBSTITUTE` | `SUBSTITUTE("a-b-c","-","+")` → `"a+b+c"`; **`SUBSTITUTE("a-b-c","-","+",2)` → `"a+b+c"`**; `SUBSTITUTE("abc","","x")` → `"xaxbxcx"`; `SUBSTITUTE("Hello","hello","x")` → `"Hello"`; `SUBSTITUTE(123,"2","x")` → error | `SUBSTITUTE(text, old, new, [instance_num])`: `instance_num` replaces only that occurrence: `"a-b+c"`. Case-sensitive (matches). An empty `old` returns the text unchanged (**verify in Excel**) | differs | **fix** B-6 (`instance_num`) and B-16 (empty `old`). Fully backward compatible for 3-argument calls |
| `ISBLANK` | `ISBLANK(NULL)` → `true`; `ISBLANK("")` → `true`; `ISBLANK("  ")` → `true`; `ISBLANK([])` → `true`; `ISBLANK(0)` → `false`; `ISBLANK()` → `true` | TRUE only for an empty cell. `ISBLANK("")` = FALSE | differs | **document**. Skillet's ISBLANK is "null or whitespace or empty". That is more useful for JSON input, and ADR-0006 relies on it. Document it as deliberate |
| `ISNUMBER` | `ISNUMBER(1)` → `true`; `ISNUMBER("1")` → `false`; `ISNUMBER(TRUE)` → `false`; `ISNUMBER(1::Currency)` → `true` | Same | yes | keep |
| `ISTEXT` | `ISTEXT("a")` → `true`; `ISTEXT(1)` → `false` | Same | yes | keep |
| `LENGTH` | `LENGTH("héllo")` → `5`; `LENGTH([1,2])` → `2`; `LENGTH(NULL)` → `0`; `LENGTH(123)` → error; **`LEN("abc")` → `Unknown function: LEN`** | Excel's name is `LEN` | Skillet-only | **add `LEN` as an alias**. `docs/reference/api.md` already documents `LEN` |
| `SUBSTRING` | `SUBSTRING("hello",1,3)` → `"ell"` (0-based) | No Excel function (use `MID`, 1-based) | Skillet-only | keep. In docs, prefer `MID` for Excel users |
| `SPLIT`, `SUBSTITUTEM`, `INCLUDES` | `SPLIT("a, b,c")` → `["a","b","c"]` (default `,`, trimmed); `SPLIT("abc","")` → `["","a","b","c",""]`; `INCLUDES("hello","ell")` → `true` | Excel has `TEXTSPLIT`, and `ISNUMBER(SEARCH())` for contains | Skillet-only | keep. `SUBSTITUTEM` is an exact duplicate of `SUBSTITUTE`: deprecate it once `SUBSTITUTE` honors `instance_num` |
| `REVERSE` | `REVERSE("abc")` → `Error: REVERSE expects array`; `REVERSE([1,2,3])` → `[3,2,1]` | No Excel function | Skillet-only | **bug** B-11: the string version in `string.rs` can never be reached |

Missing Excel text functions that users will try: `LEN`, `FIND` (name taken, see below),
`SEARCH`, `EXACT`, `TEXT`, `VALUE`, `REPT`, `PROPER`, `TEXTJOIN`. All return `Unknown function`.

## Arrays, counting and conditional aggregates

| Function | Skillet (expr → output) | Excel | Match? | Recommendation |
|---|---|---|---|---|
| `COUNT` | **`COUNT([1,"a",TRUE,NULL])` → `4`**; **`COUNT(1,2,3)` → `Error: COUNT expects 1 argument: array`**; `COUNT(5)` → error; `COUNT([])` → `0` | `COUNT(value1, ...)` counts **numbers only**: `COUNT({1,"a",TRUE})` = 1, `COUNT(1,2,3)` = 3. `COUNTA` counts non-empty values | differs | **fix** (ADR, breaking). Make `COUNT` variadic and numbers-only, add `COUNTA` for "non-null values", and keep `.length()`/`.count()` for array length. `api.md` already documents the Excel behavior (`COUNT(1,"hello",3,4)` = 3), which is not what the code does |
| `COUNTIF` | `COUNTIF([1,2,3,4], :x > 2)` → `2`; **`COUNTIF([1,2,3,4], ">2")` → `0`**; **`COUNTIF([1,2,3], 2)` → `0`**; `COUNTIF([1,2,3], :x)` → `0` | `COUNTIF(range, criteria)`. Criteria are like `">2"`, `2` or `"a*"`. Case-insensitive | differs | **bug** B-3: Excel-style criteria silently give 0. Reuse `SUMIF`'s criteria parser. A lambda that returns a non-boolean should be an error |
| `SUMIF` | `SUMIF([1,2,3,4],">2")` → `7`; `SUMIF([1,2,3],">=2",[10,20,30])` → `50`; `SUMIF([1,2,3],2)` → `2`; `SUMIF([1,2,3],"<>2")` → `4`; `SUMIF(["A","b"],"a",[1,2])` → `0`; `SUMIF(["apple","avocado"],"a*",[1,2])` → `0`; `SUMIF([1,2,3],">=x")` → `0`; lambda `SUMIF([1,2,3], :x > 1)` → `5` | Same argument order. Text criteria are case-insensitive (`"a"` matches `"A"`) and support the wildcards `*`, `?` and `~` | edges | fix: case-insensitive text matching and wildcards (ADR, changes results). Unparseable numeric criteria such as `">=x"` should be an error |
| `AVGIF` | `AVGIF([1,2,3,4], :x > 2)` → `3.5`; `AVGIF([1,2], :x > 5)` → `0`; `AVERAGEIF(...)` → `Unknown function` | Excel's name is `AVERAGEIF(range, criteria, [average_range])`. No match → `#DIV/0!` | Skillet-only | add an `AVERAGEIF` alias that takes the same criteria as `SUMIF`. Fix X-2 |
| `UNIQUE` | **`UNIQUE(["a","a",1])` → `[1]`**; `UNIQUE([3,1,3,2])` → `[3,1,2]`; method `["a","a",1].unique()` → `["a",1]` | `UNIQUE(array, [by_col], [exactly_once])`. Keeps values of every type, in first-seen order | differs | **bug** B-7: the function drops every non-number. Use the method's implementation |
| `SORT` | `SORT([3,1,2])` → `[1,2,3]`; `SORT([3,1,2],"DESC")` → `[3,2,1]`; **`SORT([3,1,2],-1)` → `[1,2,3]`**; `SORT(["b","a"])` → `Error: SORT expects numeric array` | `SORT(array, [sort_index], [sort_order], [by_col])`. `sort_order` -1 means descending. Sorts text | differs | **fix**: accept `-1`/`1` as the order (B-12) and sort strings. Keep `"DESC"` as a documented extension |
| `FILTER` | `FILTER([1,2,3], :x > 1)` → `[2,3]` | `FILTER(array, include, [if_empty])`, where `include` is a boolean array. No match → `#CALC!` | differs | **document**: same intent, lambda form. Optional: an `if_empty` third argument (today the third argument renames the lambda variable) |
| `MAP` | `MAP([1,2], :x * 2)` → `[2,4]` | `MAP(array, LAMBDA(x, ...))` | differs | document (implicit `:x` replaces `LAMBDA`) |
| `REDUCE` | `REDUCE([1,2,3], :acc + :x, 0)` → `6` | **`REDUCE([initial_value], array, LAMBDA(acc, value))`**: the initial value comes first | differs | **document** prominently. Arguments in a different order under the same name catch Excel users out. Changing it would break every existing `REDUCE` |
| `FIND` | `FIND([1,2,3], :x > 1)` → `2`; `FIND("b","abc")` → `Error: FIND first arg must be array` | `FIND(find_text, within_text, [start_num])` returns a 1-based position. A **text** function | differs | **decide** (D-6): the name collides. Options: dispatch on the first argument's type (string → Excel FIND), or keep the array FIND and add `SEARCH`/`FINDTEXT` |
| `ARRAY`, `FLATTEN`, `FIRST`, `LAST`, `CONTAINS`, `IN`, `JOIN`, `MERGE` | `FIRST([])` → `Error: FIRST on empty array`; `JOIN([1,"a",TRUE],"-")` → `"1-a-TRUE"`; `JOIN([1,2])` → `"1,2"`; `MERGE([1],[2],3)` → `[1,2,3]` | `JOIN` is like Excel's `TEXTJOIN(delimiter, ignore_empty, ...)`, which puts the delimiter first | Skillet-only | keep. Consider a `TEXTJOIN` alias |
| `DIG`, `JQ` | `DIG({"a":{"b":1}}, ["a","b"])` → `1` | none | Skillet-only | keep |

## Date and time

Skillet's `DateTime` is a count of Unix seconds (UTC). Excel stores dates as serial day numbers
(1 = 1900-01-01) with the time as a fraction of a day. This difference explains most of the rows below.

| Function | Skillet (expr → output) | Excel | Match? | Recommendation |
|---|---|---|---|---|
| `DATE` | `DATE(2026,1,15)` → `DateTime(1768435200)`; **`DATE(2026,13,1)`, `DATE(2026,2,30)`, `DATE(2026,0,1)`, `DATE(2026,1,0)` → `Error: Invalid date`**; `DATE(2026.9,1.9,1.9)` → 2026-01-01; `DATE(26,1,1)` → year 26 AD; `DATE()` → today, midnight UTC | Out-of-range months and days **roll over**: `DATE(2026,13,1)` = 2027-01-01, `DATE(2026,2,30)` = 2026-03-02, `DATE(2026,1,0)` = 2025-12-31. Years 0–1899 get 1900 added (`DATE(26,1,1)` = 1926-01-01). Arguments are truncated (matches). 3 arguments are required | differs | **decide** D-5. Recommendation: support rollover (Excel users rely on `DATE(y, m+1, 0)` to get the end of a month). Keep strict years, because adding 1900 is a 1900s-era quirk; document it. `DATE()` is a Skillet extension; add `TODAY()` as the Excel name |
| `NOW` | `NOW()` → `DateTime(1795229354)` (UTC, whole seconds) | Serial number with the time as a fraction, in local time | differs | document. The representation is deliberate (ADR-0004 territory) |
| `TIME` | **`TIME(14,30,0)` → `Number(74954)`**, which is the current local seconds since midnight. The arguments are ignored | `TIME(hour, minute, second)` returns a fraction of a day: 0.6041666… | differs | **bug** B-1 (high). `api.md` documents `TIME(14,30,0)` = 52200. Fix: return `h*3600 + m*60 + s` seconds (the documented Skillet contract), with Excel-style rollover. Give the no-argument "current time" a separate name |
| `YEAR` / `MONTH` / `DAY` | `YEAR(DATE(2026,3,4))` → `2026`; `MONTH(...)` → `3`; `DAY(...)` → `4`; `YEAR(46000)` → `Error: YEAR expects datetime`; `YEAR("2026-01-01")` → error | Also accept serial numbers and date text | edges | keep strict. Strings work after the cast proposal below (`YEAR("2026-01-01"::DateTime)`). All components are UTC; document that |
| `DATEADD` | `DATEADD(DATE(2026,1,31),1,"months")` → 2026-02-28 (clamped); `DATEADD(DATE(2024,2,29),1,"years")` → 2025-02-28; `DATEADD(DATE(2026,1,1),1.9,"days")` → +1 day (truncated); **`DATEADD(..., 1, "m")` → +1 minute** | No Excel function. The closest are `EDATE` (clamps the same way) and `date + n` | Skillet-only | keep. **Document** that `"m"` means minutes (B-15), and consider rejecting bare `"m"` (in Excel's `DATEDIF`, `"m"` means months) |
| `DATEDIFF` | `DATEDIFF(DATE(2026,1,1), DATE(2026,3,1), "days")` → `59`; `DATEDIFF(DATE(2026,1,31), DATE(2026,2,1), "months")` → `1`; `DATEDIFF(DATE(2025,12,31), DATE(2026,1,1), "years")` → `1`; the unit is required | Excel's `DATEDIF(start, end, unit)` counts **complete** units: "m" → 0, "y" → 0 for those inputs. It also returns `#NUM!` when start > end, where Skillet returns a negative number | Skillet-only | **document** that months and years count calendar boundaries (like SQL `DATEDIFF`), not complete units. If Excel users ask for it, add `DATEDIF` with Excel semantics rather than changing this function. `api.md` shows `[unit]` as optional, but it is required |

Date arithmetic: `DATE(2026,1,2) - DATE(2026,1,1)` → `Error: Arithmetic op on non-number`
(Excel: 1). `DATE(2026,1,1) + 1` → error (Excel: the next day). Missing Excel functions: `TODAY`,
`WEEKDAY`, `EDATE`, `EOMONTH`, `DATEDIF`, `DATEVALUE`, `NETWORKDAYS`.

## Financial

Skillet follows Excel's cash-flow sign convention in `PMT` and `FV`: money you receive is
positive, money you pay is negative.

| Function | Skillet (expr → output) | Excel | Match? | Recommendation |
|---|---|---|---|---|
| `PMT` | `PMT(0.05/12,360,100000)` → `-536.8216230121399`; `PMT(0,10,1000)` → `-100`; `PMT(0.1,2,1000,0,1)` → `-523.8095238095234`; `PMT(0.06/12,120,0,50000)` → `-305.1025097082559`; `PMT(0.05,0,1000)` → error; `PMT(0.05,10,1000,"x")` → `-129.50457496545664` (`fv` treated as 0) | `PMT(rate, nper, pv, [fv], [type])`: -536.82, -100, -523.81, -305.10. Non-numeric `fv` → `#VALUE!` | edges | keep. Fix X-5 (non-numeric optional arguments). `api.md` shows positive results ("$536.82"), which should be shown as negative |
| `FV` | `FV(0.05,10,-100)` → `1257.789253554884`; `FV(0.05,10,-100,-1000)` → `2886.683880332326`; `FV(0.05,10,-100,0,1)` → `1320.6787162326282`; `FV(0,10,-100,-1000)` → `2000`; `FV(0.05,-1,-100)` → `Error: FV nper must be non-negative` | `FV(rate, nper, pmt, [pv], [type])`: 1257.79, 2886.68, 1320.68, 2000. A negative `nper` is computed, not rejected (**verify in Excel**) | edges | keep. Consider allowing a negative `nper` |
| `IPMT` | `IPMT(0.1,1,3,1000)` → **`100`**; `IPMT(0.1,2,3,1000)` → **`49.78851963746227`**; `IPMT(0.1,3,3,1000)` → **`4.59818731117831`**; `IPMT(0.1,1,3,1000,0,1)` → `0`; `IPMT(0.1,2,3,1000,0,1)` → **`54.35319967042025`**; `IPMT(0,1,3,1000)` → `0`; `IPMT(0.1,4,3,1000)` → error | `IPMT(rate, per, nper, pv, [fv], [type])`. The interest is **negative** for a positive `pv`, and the balance rolls forward as `balance*(1+r) + pmt`: per 1 → -100.00; per 2 → -69.79; per 3 → -36.56; type 1: per 1 → 0, per 2 → -63.44 (computed from Excel's documented formula, **verify in Excel**) | differs | **bug** B-2 (high). The sign is inverted, and from period 2 on the result is wrong because the principal is added with the wrong sign. Fix it and make it consistent with PMT's sign. Add `PPMT` so that `PMT = IPMT + PPMT` can be tested |
| `DB` | `DB(1000000,100000,6,1,7)` → `185912.95971618922`; `(...,2,7)` → `259455.9962155814`; **`(...,6,7)` → `23290.87494931791`**; **`(...,7,7)` → `0`**; `DB(10000,1000,5,1)` → `3690.4265551980675`; **`DB(10000,1000,5,0)` → `2328.50…`**; `DB(10000,1000,5,2.5)` → same as period 2; `DB(0,0,5,1)` → `NaN` | Microsoft's documented example (cost 1,000,000, salvage 100,000, life 6, month 7): periods 1–7 = 186,083.33; 259,639.42; 176,814.44; 120,410.64; 81,999.64; **55,841.76**; **15,845.10**. Excel **rounds the rate to 3 decimal places** (0.319). The partial first year is balanced by a partial period `life + 1`. Period 0 → `#NUM!` | differs | **bug** B-8. (1) Round the rate to 3 decimals. (2) Apply the remaining-months formula to period `life + 1`, not `life`, and allow `period <= life + 1` when `month < 12`. (3) Return an error when `period` < 1, and for a cost of 0 |

Missing Excel financial functions: `PV`, `NPV`, `NPER`, `RATE`, `PPMT`, `IRR`, `SLN`, `DDB`.

## Statistical

All of these hit X-4 (the dotted name does not parse) and X-2 (empty input gives 0). The numbers
themselves match Excel.

| Function | Skillet (expr → output) | Excel | Match? | Recommendation |
|---|---|---|---|---|
| `MEDIAN` | `MEDIAN(1,3,2)` → `2`; `MEDIAN([1,2,3,4])` → `2.5`; `MEDIAN()` → `0`; `MEDIAN([1,"9",TRUE])` → `1` | Same. No numbers → `#NUM!` | edges | fix X-2 |
| `MODE.SNGL` (`MODE_SNGL`, `MODESNGL`) | `MODE_SNGL(1,2,2,3)` → `2`; `MODE_SNGL(3,1,1,3)` → `3` (first value seen wins a tie); **`MODE_SNGL(1,2,3)` → `1`**; `MODE_SNGL()` → `0`; `MODE.SNGL(...)` → parse error | Same tie rule. **No repeated value → `#N/A`** | edges | fix: no repeated value → `Error`. Fix X-4. Consider the legacy alias `MODE` |
| `STDEV.P` (`STDEVP`, `STDEV_P`) | `STDEV_P(2,4,4,4,5,5,7,9)` → `2`; `STDEVP(2,4,4,4,5,5,7,9)` → `2`; `STDEV_P(5)` → `0`; `STDEV_P()` → `0` | Population standard deviation (divides by n): 2. Empty → `#DIV/0!` | edges | fix X-2 and X-4. **Add `STDEV.S`/`STDEV`** (sample, divides by n-1). It is the function analysts use most, and `STDEV(1,2)` → `Unknown function` today |
| `VAR.P` (`VARP`, `VAR_P`) | `VAR_P(2,4,4,4,5,5,7,9)` → `4`; `VARP(1,3)` → `1`; `VAR_P()` → `0` | Population variance: 4 | edges | same as above. Add `VAR.S`/`VAR` |
| `PERCENTILE.INC` (`PERCENTILE_INC`, `PERCENTILEINC`) | `PERCENTILE_INC([1,2,3,4],0.25)` → `1.75`; `(...,0.4)` → `2.2`; `(...,1.5)` → `Error: Percentile must be between 0 and 1`; `PERCENTILE_INC([],0.5)` → `0`; `PERCENTILE_INC(1,2,3,4,0.5)` → `2.5` (variadic; the last argument is k); `PERCENTILE_INC([3,1,"x",2],0.5)` → `2` | Inclusive interpolation, rank = k·(n−1): 1.75 and 2.2 (matches). k outside 0..1 → `#NUM!` (matches as an error). Empty → `#NUM!` | edges | fix X-2 and X-4. Add the legacy alias `PERCENTILE` (same as .INC) |
| `QUARTILE.INC` (`QUARTILE_INC`, `QUARTILEINC`) | `QUARTILE_INC([1..8],1)` → `2.75`; `QUARTILE_INC([1,2,3,4],1.9)` → `1.75` (truncated); `(...,0)` → `1`; `(...,4)` → `4`; `(...,5)` → `Error: Quartile must be 0-4`; `(...,-0.5)` → `1` | Same. A non-integer quart is truncated. quart < 0 or > 4 → `#NUM!`. For -0.5, check the result after truncation (**verify in Excel**) | edges | fix X-2 and X-4. Add the legacy alias `QUARTILE` |

## Function and method parity

These are prerequisites for the canonical-form proposal: today the same name gives different
results depending on how you call it.

| Pair | Function | Method | Proposed rule |
|---|---|---|---|
| `INT` / `.int()` | `INT(-3.7)` → `-4` | `(-3.7).int()` → `-3` | Both floor (Excel). Add `.trunc()` if truncation is needed |
| `ROUND` / `.round()` | `ROUND(1234.567,-2)` → `1235` | `(1234.567).round(-2)` → `1200` | Both support negative digits |
| `MAX`/`MIN` on empty | `MAX()` → `0` | `[].max()` → `Null`, `[].min()` → `Null` | One rule for both (D-2) |
| `SUM` with text | `SUM([1,"2",3])` → `4` | `[1,"2",3].sum()` → `Error: sum method expects numeric array` | One rule for both. Recommendation: the method's strictness |
| `UNIQUE` | `UNIQUE(["a","a",1])` → `[1]` | `.unique()` → `["a",1]` | The method's behavior |
| `TRIM` | inner spaces kept | `.trim()` same | If `TRIM` adopts Excel's behavior, keep `.trim()` Ruby-like and document it |
| Coverage | `MEDIAN`, `STDEV_P`, `PERCENTILE_INC`, `LEFT`, `MOD` exist | `.median()`, `.stdev()`, `.percentile()`, `.left()`, `.mod()` → `Unknown ... method` | Generate the method set from the function set, or document which forms exist |

## Bugs (separate from deliberate differences)

| ID | Severity | Repro | Actual | Expected |
|---|---|---|---|---|
| B-1 | High | `TIME(14, 30, 0)` | Current local seconds since midnight (`74954` at audit time). The arguments are ignored and the result is not deterministic | `52200` (as documented in api.md) |
| B-2 | High | `IPMT(0.1, 2, 3, 1000)` | `49.79` | `-69.79` (Excel). Wrong for every `per` ≥ 2, and the sign is inverted for `per` 1 |
| B-3 | High | `COUNTIF([1,2,3,4], ">2")`, `COUNTIF([1,2,3], 2)` | `0`, `0` | `2`, `1`. `SUMIF` accepts the same criteria |
| B-4 | Medium | `ROUND(1234.567, -2)` | `1235` (negative digits clamped to 0) | `1200` |
| B-5 | Medium | `CEILING(2.1, 0.5)`, `FLOOR(7, 5)` | `3`, `7` (significance ignored) | `2.5`, `5` |
| B-6 | Medium | `SUBSTITUTE("a-b-c", "-", "+", 2)` | `"a+b+c"` (`instance_num` ignored) | `"a-b+c"` |
| B-7 | Medium | `UNIQUE(["a", "a", 1])` | `[1]` (strings dropped) | `["a", 1]` |
| B-8 | Medium | `DB(1000000,100000,6,6,7)`; `DB(1000000,100000,6,7,7)`; `DB(10000,1000,5,0)` | `23290.87`; `0`; `2328.50` | `55841.76`; `15845.10`; error |
| B-9 | Medium | `STDEV.P(1, 2)` (any dotted name) | Parse error | `0.5`. The names are registered and documented but cannot be parsed (X-4) |
| B-10 | Low | `BETWEEN(1, 10, 5)` | `Unknown function: BETWEEN` | `true` (as documented; implemented in `logical.rs` but not dispatched) |
| B-11 | Low | `REVERSE("abc")` | `Error: REVERSE expects array` | `"cba"` (the string implementation can never be reached) |
| B-12 | Low | `SORT([3,1,2], -1)` | `[1,2,3]` | `[3,2,1]`, or an error |
| B-13 | Low | `ROUND(10::Currency, 1)`; `SUM(10::Currency, 5)` | Error; `Number(15)` | Currency accepted and kept as Currency (ADR-0003) |
| B-14 | Low | `(-3.7).int()` vs `INT(-3.7)` | `-3` vs `-4` | Same result |
| B-15 | Low | `DATEADD(DATE(2026,1,1), 1, "m")` | +1 minute | Ambiguous; Excel's `DATEDIF` uses `"m"` for months. Document it or reject it |
| B-16 | Low | `SUBSTITUTE("abc", "", "x")` | `"xaxbxcx"` | `"abc"` |
| B-17 | Low | `COUNTIF([1,2,3], :x)`, `AVGIF(...)`, `SUMIF(...)` with a lambda that returns a non-boolean | Treated as false (silently 0) | `Error` |

Doc defects found along the way, for the docs lane. These are not fixed here because lane F owns
`docs/reference/api.md`: `LEN` is documented but does not exist; the `COUNT` description and
example are wrong; `TIME(14,30,0)` = 52200 does not match the code; `INT(-3.9)` = -3 is wrong (it is -4);
`DATEDIFF`'s `[unit]` is required; `COMPACT(array)` is documented as a function but only the
`.compact()` method exists; the `PMT` examples drop the minus sign; the statistical section's
headings use dotted names that do not parse.

## Decisions for the maintainer (ranked by user impact)

1. **D-1: Errors versus silent defaults** (X-1, X-2, X-5, B-17). This decision affects the most
   rows. Proposal: one ADR that says Excel's `#DIV/0!`, `#NUM!` and `#VALUE!` cases return a
   Skillet `Error`, and that ignored or wrong-typed arguments are errors. Exception: functions
   ADR-0006 explicitly makes null-tolerant. This is a breaking change for anyone relying on
   `AVERAGE([])` being `0`, so it needs a minor-version note.
   - **Decided 2026-11-21** ([ADR-0018](../adr/0018-excel-compatibility.md)): match Excel's errors; `Null` stays tolerated under ADR-0006. Not implemented yet.
2. **D-2: Fix the high-severity bugs now** (B-1 `TIME`, B-2 `IPMT`, B-3 `COUNTIF`). They return
   wrong numbers silently, so no ADR is needed to restore documented or Excel behavior. Send each
   to the Rust Engineer with the expected values above as tests.
   - **Decided 2026-11-21** ([ADR-0018](../adr/0018-excel-compatibility.md)): fix all three; `TIME` returns a fraction of a day, as in Excel. Not implemented yet.
3. **D-3: `COUNT` semantics.** Make it variadic and numbers-only like Excel, and add `COUNTA`.
   This is breaking: `COUNT([1,"a"])` goes from 2 to 1. The current docs already promise the
   Excel behavior. Alternative: keep it as "length" and fix the docs. Recommendation: match Excel.
   - **Decided 2026-11-21** ([ADR-0018](../adr/0018-excel-compatibility.md)): match Excel (`COUNT` numbers-only and variadic), add `COUNTA`. Not implemented yet.
4. **D-4: Dotted names** (X-4/B-9). Allow `NAME.NAME(` in the lexer. Purely additive, but it is
   a grammar change, so it needs a short ADR. Check that it cannot conflict with `:var.method`
   (variables always start with `:`, and methods are lower case).
   - **Decided 2026-11-21** ([ADR-0018](../adr/0018-excel-compatibility.md)): allow `NAME.NAME(`. Not implemented yet.
5. **D-5: `DATE` rollover and date arithmetic.** Rollover is additive (it replaces errors).
   `DateTime ± Number` (in days, Excel-like) and `DateTime - DateTime` are new semantics that
   need an ADR, together with ADR-0004 and the cast proposal below.
   - **Decided 2026-11-21** ([ADR-0018](../adr/0018-excel-compatibility.md)): rollover, Excel's 0–1899 year rule (not strict years), date arithmetic in days; string casts stay strict. Not implemented yet.
6. **D-6: The `FIND` name collision.** Decide whether `FIND(text, text)` dispatches to an
   Excel-compatible text search.
   - **Decided 2026-11-21** ([ADR-0018](../adr/0018-excel-compatibility.md)): dispatch on the first argument's type. Not implemented yet.
7. **D-7: `MOD` sign, `TRIM` inner spaces, `SUMIF` case-insensitivity and wildcards.** Each one
   changes results for existing inputs. Recommendation: match Excel for the UPPER_CASE functions
   (the name is the promise) and keep Ruby semantics on the lowercase methods.
   - **Decided 2026-11-21** ([ADR-0018](../adr/0018-excel-compatibility.md)): Excel `MOD`, `TRIM` and `SUMIF` for the functions, Ruby semantics for the methods. Not implemented yet.
8. **D-8: Lazy `IF`/`IFS`** (X-8). Results only change where an error used to be returned.
   Recommendation: do it.
   - **Decided 2026-11-21** ([ADR-0018](../adr/0018-excel-compatibility.md)): lazy `IF`/`IFS`. Not implemented yet.
9. **D-9: Fill the gaps** with Excel aliases that cost little: `LEN`, `AVERAGEIF`, `STDEV.S`/`STDEV`,
   `VAR.S`/`VAR`, `TODAY`, `PPMT`, `PV`, `NPV`, `TEXTJOIN`, `PERCENTILE`, `QUARTILE`, `MODE`.
   - **Decided 2026-11-21** ([ADR-0018](../adr/0018-excel-compatibility.md)): add all of them. Not implemented yet.
10. **D-10: Operators** (X-6). Keep `^` right-associative and unary minus binding looser than
    `^` (the math convention), and correct PRD-001 FR-1, which says "Excel precedence".
    - **Decided 2026-11-21** ([ADR-0018](../adr/0018-excel-compatibility.md)): **Excel precedence**, against the recommendation above: `-2^2` = 4, `2^3^2` = 64. Not implemented yet.

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

**Prerequisite:** the function and the method must give the same result (see the parity table).
Otherwise the canonical form decides the answer, not just the spelling. Proposed invariant for
the ADR: *for every name that exists as both a function and a method, `F(x, args...)` and
`x.f(args...)` return the same `Value` or the same error*, enforced by a parity test file
(`tests/function_method_parity.rs`, for QA).

## Proposal: string to DateTime cast formats

Not a decision. Today `"2026-01-01"::DateTime` → `Error: Cannot cast String to DateTime`, and
only numeric strings work (`"1767225600"::DateTime` → `DateTime(1767225600)`).

**Accepted inputs** (a subset of ISO 8601 / RFC 3339, so the result does not depend on locale):

| Input | Result | Notes |
|---|---|---|
| `"2026-01-01"::DateTime` | `DateTime(1767225600)` | Date only → 00:00:00 UTC. The same as `DATE(2026,1,1)` |
| `"2026-01-01T10:30:00Z"::DateTime` | `DateTime(1767263400)` | RFC 3339, UTC |
| `"2026-01-01T10:30:00+02:00"::DateTime` | `DateTime(1767256200)` | The offset is applied and stored as UTC |
| `"2026-01-01T10:30:00"::DateTime` | `DateTime(1767263400)` | No offset → UTC (consistent with `DATE`). Never the server's local zone |
| `"2026-01-01 10:30:00"::DateTime` | `DateTime(1767263400)` | Space separator, for SQL and CSV exports |
| `"2026-01-01T10:30:00.987Z"::DateTime` | `DateTime(1767263400)` | Fractional seconds truncated (`DateTime` is whole seconds) |
| `"1767225600"::DateTime` | `DateTime(1767225600)` | Existing behavior, **kept** (integer string = Unix seconds) |
| `"01/02/2026"::DateTime` | `Error: Cannot cast "01/02/2026" to DateTime (expected YYYY-MM-DD or RFC 3339)` | Ambiguous between MM/DD and DD/MM, so rejected on purpose |
| `"2026-02-30"::DateTime` | `Error: ... invalid date` | No rollover in casts, even if `DATE` gets rollover (D-5) |
| `"2026-1-5"::DateTime` | `Error` | Zero-padded fields only; keeps the parser strict and predictable |

Compatibility: this only turns current errors into values. No expression that currently returns
a value changes. Follow-ups that are **not** part of this proposal: `DateTime::String`
(currently `"1767225600"`) could produce ISO 8601, but that would be a breaking change and
needs its own ADR. An Excel-named `DATEVALUE(text)` could be a thin wrapper around the cast,
and `YEAR`/`MONTH`/`DAY` could accept strings by applying the same cast.
