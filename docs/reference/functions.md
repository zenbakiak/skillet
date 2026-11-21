# Builtin function reference

<!-- Generated from src/runtime/builtin_table.rs. Do not edit by hand: change the table, then run `UPDATE_DOCS=1 cargo test --lib builtin_table`. -->

Every `UPPERCASE()` builtin. Names are case-insensitive in expressions (the parser upper-cases them, so `sum(1, 2)` calls `SUM`). Custom functions registered with `register_function` are checked first and can shadow a builtin.

- **Arity** is the number of arguments the function reads (`2+` = two or more). Functions check their own arguments: some return an error for a wrong count, others use a default for a missing argument and ignore extra ones.
- **Result** is what the example evaluates to, produced by the test that writes this file. Examples run with `:obj` = `{"user": {"name": "alice", "tags": ["a", "b"]}}`.
- **†** marks functions the evaluator runs itself because they take a lambda (`:x` is the item, `:acc` the accumulator) or need unevaluated arguments.
- Dotted Excel spellings (`MODE.SNGL`, `STDEV.P`, ...) are registered aliases, but the parser cannot read a dot in a function name; use the underscore form.

Methods (`:arr.sum()`), operators and the rest of the language: [api.md](api.md).

## Arithmetic

| Function | Signature | Arity | Summary | Example | Result |
|---|---|---|---|---|---|
| `SUM` | `SUM(values…)` | 0+ | Sum of numbers and currency; arrays are flattened, other values ignored. | `SUM(1, 2, [3, 4])` | `10` |
| `AVG`<br>alias `AVERAGE` | `AVG(values…)` | 0+ | Mean of numbers and currency (arrays flattened); error when there are none (Excel `#DIV/0!`). | `AVG(2, 4, 6)` | `4` |
| `MIN` | `MIN(values…)` | 0+ | Smallest number (arrays flattened); 0 when there are none. | `MIN(3, 1, 2)` | `1` |
| `MAX` | `MAX(values…)` | 0+ | Largest number (arrays flattened); 0 when there are none. | `MAX([3, 9, 2])` | `9` |
| `ROUND` | `ROUND(number, [digits])` | 1-2 | Round half away from zero to `digits` decimals (default 0); negative `digits` round left of the point. | `ROUND(1234.567, -2)` | `1200` |
| `CEILING`<br>alias `CEIL` | `CEILING(number, [significance])` | 1-2 | Round up to a multiple of `significance` (default 1), Excel rules for negatives. | `CEILING(2.1, 0.5)` | `2.5` |
| `FLOOR` | `FLOOR(number, [significance])` | 1-2 | Round down to a multiple of `significance` (default 1), Excel rules for negatives. | `FLOOR(7, 5)` | `5` |
| `ABS` | `ABS(number)` | 1 | Absolute value. | `ABS(-7)` | `7` |
| `SQRT` | `SQRT(number)` | 1 | Square root; error for a negative number. | `SQRT(16)` | `4` |
| `POW`<br>alias `POWER` | `POW(base, exponent)` | 2 | `base` raised to `exponent`; error for `0^0`, `0` to a negative power, or a non-real result. | `POW(2, 10)` | `1024` |
| `MOD` | `MOD(number, divisor)` | 2 | Remainder of `number / divisor`; the sign follows the divisor (Excel). Error for 0. | `MOD(-10, 3)` | `2` |
| `INT` | `INT(number)` | 1 | Round down to an integer: `INT(-3.9)` is -4. | `INT(5.9)` | `5` |
| `PRODUCT`<br>alias `MULTIPLY` | `PRODUCT(values…)` | 0+ | Product of numbers and currency (arrays flattened); 1 when there are none. | `PRODUCT(2, 3, 4)` | `24` |
| `SUMIF` † | `SUMIF(range, criteria, [sum_range])` | 2-3 | Sum of items meeting an Excel criteria (`">2"`, `5`, `"a*"`) or a lambda over `:x`. | `SUMIF([1, -2, 3], ">0")` | `4` |

## Logical

| Function | Signature | Arity | Summary | Example | Result |
|---|---|---|---|---|---|
| `AND` | `AND(values…)` | 0+ | TRUE when every argument is truthy (TRUE or a non-zero number). | `AND(TRUE, 1)` | `TRUE` |
| `OR` | `OR(values…)` | 0+ | TRUE when any argument is truthy. | `OR(FALSE, 1)` | `TRUE` |
| `NOT` | `NOT(value)` | 1 | Negation of the argument's truthiness. | `NOT(0)` | `TRUE` |
| `XOR` | `XOR(a, b)` | 2 | TRUE when exactly one argument is truthy. | `XOR(TRUE, FALSE)` | `TRUE` |
| `IF` | `IF(condition, then, [else])` | 2-3 | `then` when `condition` is truthy, otherwise `else` (default FALSE). Both branches are evaluated. | `IF(1 > 0, 'yes', 'no')` | `"yes"` |
| `IFS` | `IFS(condition, value, …)` | 2+ | Value of the first truthy condition; FALSE when none matches. | `IFS(FALSE, 'a', TRUE, 'b')` | `"b"` |
| `BETWEEN` | `BETWEEN(min, max, value)` | 3 | TRUE when `min <= value <= max` (both bounds inclusive). All three arguments must be numbers. | `BETWEEN(10, 20, 15)` | `TRUE` |

## String

| Function | Signature | Arity | Summary | Example | Result |
|---|---|---|---|---|---|
| `LENGTH` | `LENGTH(text_or_array)` | 1 | Characters in a string or items in an array; 0 for NULL. | `LENGTH('héllo')` | `5` |
| `CONCAT` | `CONCAT(values…)` | 0+ | Join strings, numbers, booleans and (flattened) arrays; other values add nothing. | `CONCAT('a', 1, TRUE)` | `"a1TRUE"` |
| `UPPER` | `UPPER(text)` | 1 | Upper-case. | `UPPER('abc')` | `"ABC"` |
| `LOWER` | `LOWER(text)` | 1 | Lower-case. | `LOWER('ABC')` | `"abc"` |
| `TRIM` | `TRIM(text)` | 1 | Remove leading and trailing spaces and collapse inner runs of spaces (Excel). `.trim()` keeps inner spaces. | `TRIM('  a   b  ')` | `"a b"` |
| `SUBSTRING` | `SUBSTRING(text, start, [length])` | 2-3 | Characters from 0-based `start`, to the end or `length` characters. | `SUBSTRING('hello', 1, 3)` | `"ell"` |
| `SPLIT` | `SPLIT(text, [separator])` | 1-2 | Split into an array; without a separator, split on commas and trim each part. | `SPLIT('a, b,c')` | `["a", "b", "c"]` |
| `REPLACE` | `REPLACE(text, start, count, new_text)` | 4 | Excel `REPLACE`: replace `count` characters from 1-based `start` with `new_text`. | `REPLACE('abcdef', 2, 3, 'X')` | `"aXef"` |
| `SUBSTITUTE` | `SUBSTITUTE(text, find, replacement, [instance])` | 3-4 | Replace every occurrence of `find`, or only the `instance`-th. | `SUBSTITUTE('a-b-c', '-', '+', 2)` | `"a-b+c"` |
| `SUBSTITUTEM` | `SUBSTITUTEM(text, find, replacement)` | 3 | Same as `SUBSTITUTE`. | `SUBSTITUTEM('a-b-c', '-', '')` | `"abc"` |
| `ISBLANK` | `ISBLANK(value)` | 1 | TRUE for NULL, a whitespace-only string or an empty array. | `ISBLANK('  ')` | `TRUE` |
| `ISNUMBER` | `ISNUMBER(value)` | 1 | TRUE for a number or currency. | `ISNUMBER(5)` | `TRUE` |
| `ISTEXT` | `ISTEXT(value)` | 1 | TRUE for a string. | `ISTEXT('x')` | `TRUE` |
| `INCLUDES` | `INCLUDES(text, substring)` | 2 | TRUE when `text` contains `substring`. | `INCLUDES('hello', 'ell')` | `TRUE` |
| `LEFT` | `LEFT(text, [count])` | 1-2 | First `count` characters (default 1). | `LEFT('hello', 2)` | `"he"` |
| `RIGHT` | `RIGHT(text, [count])` | 1-2 | Last `count` characters (default 1). | `RIGHT('hello', 2)` | `"lo"` |
| `MID` | `MID(text, start, [count])` | 2-3 | Characters from 1-based `start`, to the end or `count` characters. | `MID('hello', 2, 3)` | `"ell"` |

## Array

| Function | Signature | Arity | Summary | Example | Result |
|---|---|---|---|---|---|
| `ARRAY` | `ARRAY(values…)` | 0+ | Array of the arguments. | `ARRAY(1, 'a', TRUE)` | `[1, "a", TRUE]` |
| `FLATTEN` | `FLATTEN(values…)` | 0+ | All arguments flattened, recursively, into one array. | `FLATTEN([1, [2, [3]]])` | `[1, 2, 3]` |
| `FIRST` | `FIRST(array)` | 1 | First item; error on an empty array. | `FIRST([9, 8, 7])` | `9` |
| `LAST` | `LAST(array)` | 1 | Last item; error on an empty array. | `LAST([9, 8, 7])` | `7` |
| `CONTAINS` | `CONTAINS(array, value)` | 2 | TRUE when the array has an item equal to `value`. | `CONTAINS([1, 2, 3], 2)` | `TRUE` |
| `IN` | `IN(array, value)` | 2 | Same as `CONTAINS`, with an exact argument count. | `IN([1, 2, 3], 4)` | `FALSE` |
| `COUNT` | `COUNT(values…)` | 1+ | How many numbers (currency, datetime) there are; arrays flattened, other values ignored (Excel). | `COUNT([1, 'a', TRUE], 2)` | `2` |
| `COUNTA` | `COUNTA(values…)` | 1+ | How many values are not NULL; arrays flattened (Excel). | `COUNTA([1, 'a', NULL], '')` | `3` |
| `UNIQUE` | `UNIQUE(array)` | 1 | Distinct numbers in first-seen order; non-numbers are dropped. | `UNIQUE([1, 2, 2, 3])` | `[1, 2, 3]` |
| `SORT` | `SORT(array, [order])` | 1-2 | Numbers sorted ascending, or descending with order `-1` or `'DESC'`; non-numbers are an error. | `SORT([3, 1, 2], -1)` | `[3, 2, 1]` |
| `REVERSE` | `REVERSE(array)` | 1 | Items in reverse order. (For strings use the `.reverse()` method.) | `REVERSE([1, 2, 3])` | `[3, 2, 1]` |
| `JOIN` | `JOIN(array, [separator])` | 1-2 | Items joined into a string with `separator` (default `,`); nested arrays are an error. | `JOIN([1, 'a', TRUE], '-')` | `"1-a-TRUE"` |
| `MERGE` | `MERGE(values…)` | 0+ | Arrays concatenated; scalar arguments are appended as items. | `MERGE([1, 2], 3, [4])` | `[1, 2, 3, 4]` |
| `FILTER` † | `FILTER(array, expr, [param])` | 2-3 | Items for which `expr` is TRUE; the item is `:x` (or `:param`). | `FILTER([1, 2, 3, 4], :x % 2 == 0)` | `[2, 4]` |
| `FIND` † | `FIND(array, expr, [param])` | 2-3 | First item for which `expr` is TRUE, else NULL. | `FIND([1, 5, 10], :x > 3)` | `5` |
| `MAP` † | `MAP(array, expr, [param])` | 2-3 | `expr` applied to each item `:x`. | `MAP([1, 2, 3], :x * 10)` | `[10, 20, 30]` |
| `REDUCE` † | `REDUCE(array, expr, initial, [param], [acc_param])` | 3-5 | Fold: `expr` over `:acc` and `:x`, starting from `initial`. | `REDUCE([1, 2, 3], :acc + :x, 0)` | `6` |

## Date and time

| Function | Signature | Arity | Summary | Example | Result |
|---|---|---|---|---|---|
| `NOW` | `NOW()` | 0 | Current UTC time as a DateTime (seconds). | `NOW()` | (depends on the clock) |
| `DATE` | `DATE([year, month, day])` | 0-3 | Midnight UTC of the given date; today's local date with no arguments. | `DATE(2024, 1, 15)` | `2024-01-15T00:00:00Z` |
| `TIME` | `TIME([hour, minute, second])` | 0-3 | Time as a fraction of a day (Excel): parts truncated, wrapped at 24 hours; the current local time with no arguments. | `TIME(14, 30, 0)` | `0.6041666666666666` |
| `YEAR` | `YEAR(datetime)` | 1 | Year (UTC). | `YEAR(DATE(2024, 3, 9))` | `2024` |
| `MONTH` | `MONTH(datetime)` | 1 | Month 1-12 (UTC). | `MONTH(DATE(2024, 3, 9))` | `3` |
| `DAY` | `DAY(datetime)` | 1 | Day of the month (UTC). | `DAY(DATE(2024, 3, 9))` | `9` |
| `DATEADD` | `DATEADD(datetime, amount, unit)` | 3 | Add `amount` units: days, hours, minutes, seconds, weeks, months, years (or d, h, m, s, w, y). | `DATEADD(DATE(2024, 1, 31), 1, 'days')` | `2024-02-01T00:00:00Z` |
| `DATEDIFF` | `DATEDIFF(date1, date2, unit)` | 3 | `date2 - date1` in the given unit (same units as `DATEADD`). | `DATEDIFF(DATE(2024, 1, 1), DATE(2024, 3, 1), 'days')` | `60` |

## Financial

| Function | Signature | Arity | Summary | Example | Result |
|---|---|---|---|---|---|
| `PMT` | `PMT(rate, nper, pv, [fv], [type])` | 3-5 | Payment per period of a loan (Excel `PMT`). | `PMT(0.05 / 12, 360, 200000)` | `-1073.6432460242797` |
| `DB` | `DB(cost, salvage, life, period, [month])` | 4-5 | Fixed-declining-balance depreciation for a period (Excel `DB`). | `DB(10000, 1000, 5, 2)` | `2328.5017392669597` |
| `FV` | `FV(rate, nper, pmt, [pv], [type])` | 3-5 | Future value of an investment (Excel `FV`). | `FV(0.05, 10, -100)` | `1257.789253554884` |
| `IPMT` | `IPMT(rate, per, nper, pv, [fv], [type])` | 4-6 | Interest part of payment `per` (Excel `IPMT`; negative for a positive `pv`). | `IPMT(0.1 / 12, 1, 36, 8000)` | `-66.66666666666667` |

## Statistical

| Function | Signature | Arity | Summary | Example | Result |
|---|---|---|---|---|---|
| `MEDIAN` | `MEDIAN(values…)` | 0+ | Median of numbers and currency (arrays flattened); error when there are none. | `MEDIAN(1, 3, 2, 4)` | `2.5` |
| `MODE_SNGL`<br>alias `MODESNGL`, `MODE.SNGL` | `MODE_SNGL(values…)` | 0+ | Most frequent number (first seen wins a tie); error when no number repeats. | `MODE_SNGL(1, 2, 2, 3)` | `2` |
| `STDEV_P`<br>alias `STDEVP`, `STDEV.P` | `STDEV_P(values…)` | 0+ | Population standard deviation; error when there are no numbers. | `STDEV_P(2, 4, 4, 4, 5, 5, 7, 9)` | `2` |
| `VAR_P`<br>alias `VARP`, `VAR.P` | `VAR_P(values…)` | 0+ | Population variance; error when there are no numbers. | `VAR_P(1, 2, 3, 4)` | `1.25` |
| `PERCENTILE_INC`<br>alias `PERCENTILEINC`, `PERCENTILE.INC` | `PERCENTILE_INC(values…, k)` | 2+ | Inclusive percentile `k` (0-1) of the values, interpolated. | `PERCENTILE_INC([1, 2, 3, 4], 0.5)` | `2.5` |
| `QUARTILE_INC`<br>alias `QUARTILEINC`, `QUARTILE.INC` | `QUARTILE_INC(values…, quart)` | 2+ | Inclusive quartile `quart` (0-4) of the values. | `QUARTILE_INC([1, 2, 3, 4, 5], 1)` | `2` |
| `AVERAGEIF` †<br>alias `AVGIF` | `AVERAGEIF(range, criteria, [average_range])` | 2-3 | Mean of items meeting an Excel criteria or a lambda over `:x`; error when none match. | `AVERAGEIF([1, 3, 5, -1], ">0")` | `3` |
| `COUNTIF` † | `COUNTIF(range, criteria)` | 2 | Number of items meeting an Excel criteria (`">2"`, `"<>a"`, `"a*"`) or a lambda over `:x`. | `COUNTIF([1, 2, 3, 4], ">2")` | `2` |

## JSON

| Function | Signature | Arity | Summary | Example | Result |
|---|---|---|---|---|---|
| `DIG` | `DIG(json, path, [default])` | 2-3 | Value at `path` (array of keys and indexes) in a JSON object, else `default` or NULL. | `DIG(:obj, ['user', 'tags', 1])` | `"b"` |
| `JQ` † | `JQ(json, jsonpath)` | 2 | JSONPath query (`$…`) over a JSON value or array. | `JQ(:obj, '$.user.name')` | `"alice"` |
