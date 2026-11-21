# Skillet API Reference

Complete reference for all Skillet functions, methods, and operators.

## Table of Contents

1. [Operators](#operators)
2. [Built-in Functions](#built-in-functions)
3. [Method Calls](#method-calls)
4. [Type Conversion Methods](#type-conversion-methods)
5. [Safe Navigation Operator](#safe-navigation-operator)
6. [Type Casting](#type-casting)
7. [Variable Syntax](#variable-syntax)

Evaluation limits (nesting depth 128, 256 MiB budget, server deadline) and their error texts are
documented in [limits](../guide/limits.md).

Server request limits: both `sk_server` and `sk_http_server` reject a request over 1 MB, give a
client 30 s in total to send one request, and give themselves 10 s in total to write one response
(a client that reads more slowly gets a truncated response). `sk_http_server` answers the first two
with HTTP 400 and `"error": "413 Payload Too Large"` or `"error": "408 Request Timeout"`. Both
servers refuse to start with an empty token. `/upload-js`, `/update-js` and `/delete-js` accept
only plain file names matching `[A-Za-z0-9_-]+.js` (HTTP 400 otherwise). In a build without the
`plugins` feature, the JavaScript endpoints (`/upload-js`, `/update-js`, `/delete-js`, `/list-js`,
`/reload-hooks`) answer HTTP 404 `"JavaScript plugins are not enabled in this build"` after the
admin-token check. Details: [limits](../guide/limits.md#server-request-limits-1-mb-30-s-10-s) and
[servers](../guide/servers.md#connections-and-authentication). The HTTP API itself is specified in
[`openapi.yml`](../../openapi.yml).

---

## Operators

### Arithmetic Operators
- `+` - Addition
- `-` - Subtraction  
- `*` - Multiplication
- `/` - Division
- `%` - Modulo
- `^` - Exponentiation
- `=` - Equals prefix (Excel-style)

The arithmetic operators never produce NaN or infinity (BL-054, ADR-0018). Division or `%` by 0
and `0 ^` a negative number are `Error: Division by zero (#DIV/0!)`; `0 ^ 0` and any result that
is not a finite number (overflow, a non-real power) are a `#NUM!` error.
```bash
7 / 2                        # 3.5
1 / 0                        # Error: Division by zero (#DIV/0!)
0 / 0                        # Error: Division by zero (#DIV/0!)
10 ^ 400                     # Error: Result is not a finite number (#NUM!)
0 ^ 0                        # Error: 0 ^ 0 is undefined (#NUM!)
```

### Comparison Operators
- `>` - Greater than
- `<` - Less than
- `>=` - Greater than or equal
- `<=` - Less than or equal
- `==` - Equal to
- `!=` - Not equal to

### Logical Operators
- `AND`, `&&` - Logical AND
- `OR`, `||` - Logical OR
- `NOT`, `!` - Logical NOT

### Unary Operators
- `+` - Positive (unary plus)
- `-` - Negative (unary minus)
- `!` - Logical NOT

### Ternary Operator
- `condition ? true_value : false_value` - Conditional expression

### Special Operators
- `&.` - Safe navigation operator
- `::` - Type casting operator
- `...` - Spread operator

---

## Built-in Functions

### Arithmetic Functions

#### `SUM(...)`
Sum of all arguments.
```bash
SUM(1, 2, 3, 4, 5)           # 15
SUM([1, 2, 3], [4, 5])       # 15 (arrays are flattened)
SUM(...[1, 2, 3])            # 6 (spread operator)
```

#### `PRODUCT(...)`, `MULTIPLY(...)`
Product of all arguments.
```bash
PRODUCT(1, 2, 3, 4)          # 24
MULTIPLY(2, 3, 4)            # 24 (alias)
PRODUCT([2, 3, 4])           # 24 (works with arrays)
```

UPPERCASE builtins follow Excel's documented behaviour (BL-054). Where Excel returns `#DIV/0!`,
`#NUM!`, `#VALUE!` or `#N/A`, Skillet returns an error, and the message names the Excel error.
A wrong-typed or missing argument is an error, never a silent 0. Inside arrays, text, booleans
and `NULL` are ignored by the aggregates, as Excel ignores them in a range.

#### `AVERAGE(...)`
Average of the numbers in the arguments. No numbers is an error (Excel `#DIV/0!`).
```bash
AVERAGE(1, 2, 3, 4, 5)       # 3.0
AVG(10, 20, 30)              # 20.0 (alias)
AVERAGE([1, "x", TRUE])      # 1 (text and booleans in arrays are ignored)
AVERAGE([])                  # Error: AVERAGE of no numbers (#DIV/0!)
```

#### `MAX(...)`, `MIN(...)`
Maximum/minimum value from arguments. As in Excel, no numbers gives 0.
```bash
MAX(1, 5, 3, 9, 2)           # 9
MIN([1, 5, 3, 9, 2])         # 1 (works with arrays too)
MAX()                        # 0
```

#### `COUNT(...)`, `COUNTA(...)`
`COUNT` counts the numbers (and currency and datetime values) in its arguments; arrays are
flattened and other values are ignored. `COUNTA` counts every value that is not `NULL`, including
empty text. Both need at least one argument. For the length of an array use `.length()`.
```bash
COUNT(1, "hello", 3, 4)      # 3
COUNT([1, "a", TRUE, NULL])  # 1
COUNT([1, 2, 3, 4, 5])       # 5
COUNTA([1, "a", TRUE, NULL]) # 3
COUNTA("", NULL)             # 1
```

#### `ABS(number)`
Absolute value.
```bash
ABS(-5)                      # 5
ABS(3.14)                    # 3.14
ABS("x")                     # Error: ABS expects number, got string
```

#### `ROUND(number, [digits])`
Round half away from zero to `digits` decimal places (default 0; `digits` is truncated). Negative
`digits` round to the left of the decimal point. Rounding works on the number's 15-digit decimal
form, as Excel does, so `ROUND(1.005, 2)` is 1.01. A currency number stays currency.
```bash
ROUND(3.14159)               # 3 (default: 0 digits)
ROUND(3.14159, 2)            # 3.14
ROUND(2.15, 1)               # 2.2
ROUND(-2.5)                  # -3
ROUND(1234.567, -2)          # 1200
ROUND(15, -1)                # 20
ROUND(2.5, "x")              # Error: ROUND num_digits must be a number, got string
```

#### `CEILING(number, [significance])`, `FLOOR(number, [significance])`
Round up (`CEILING`, alias `CEIL`) or down (`FLOOR`) to a multiple of `significance` (default 1).
Excel's rules apply: a positive number with a negative significance is an error (`#NUM!`); a
negative number with a negative significance rounds away from zero for `CEILING` and toward zero
for `FLOOR`. `CEILING(n, 0)` is 0 and `FLOOR(n, 0)` is an error (`#DIV/0!`).
```bash
CEILING(3.2)                 # 4
CEILING(2.1, 0.5)            # 2.5
CEILING(7, 5)                # 10
CEILING(-2.5, 2)             # -2
CEILING(-2.5, -2)            # -4
FLOOR(3.8)                   # 3
FLOOR(7, 5)                  # 5
FLOOR(-2.5, 2)               # -4
FLOOR(-2.5, -2)              # -2
```

#### `INT(number)`
Round down to the nearest integer (Excel). For truncation toward zero use the `.int()` method.
```bash
INT(3.9)                     # 3
INT(-3.9)                    # -4
```

#### `MOD(number, divisor)`
Remainder `number - divisor * INT(number / divisor)`, so the result has the sign of the divisor,
as in Excel. A divisor of 0 is an error (`#DIV/0!`). The `%` operator is different: it keeps the
sign of the left operand (`-10 % 3` is -1).
```bash
MOD(10, 3)                   # 1
MOD(17, 5)                   # 2
MOD(-10, 3)                  # 2
MOD(10, -3)                  # -2
MOD(5, 0)                    # Error: MOD divisor is 0 (#DIV/0!)
```

#### `POWER(base, exponent)`
Exponentiation. As in Excel, `POWER(0, 0)` and results that are not real numbers are errors
(`#NUM!`), and 0 to a negative power is `#DIV/0!`. (The `^` operator is unchanged.)
```bash
POWER(2, 8)                  # 256
POW(3, 4)                    # 81 (alias)
POWER(-8, 1/3)               # Error: POWER result is not a real number or is out of range (#NUM!)
```

#### `SQRT(number)`
Square root. A negative number is an error (`#NUM!`).
```bash
SQRT(16)                     # 4
SQRT(2)                      # 1.414...
SQRT(-1)                     # Error: SQRT of a negative number (#NUM!)
```

### Statistical Functions

With no numbers in the data these return Excel's error: `MEDIAN`, `PERCENTILE_INC` and
`QUARTILE_INC` `#NUM!`, `STDEV_P` and `VAR_P` `#DIV/0!`, `MODE_SNGL` `#N/A`.

#### `STDEV.P(...)`, `VAR.P(...)`
Population standard deviation and variance. Write them `STDEV_P` and `VAR_P` (or `STDEVP`, `VARP`).
```bash
STDEV_P(2, 4, 4, 4, 5, 5, 7, 9)    # 2
VAR_P(2, 4, 4, 4, 5, 5, 7, 9)      # 4
STDEV_P()                          # Error: STDEV.P of no numbers (#DIV/0!)
```

#### `MEDIAN(...)`
Median value.
```bash
MEDIAN(1, 2, 3, 4, 5)        # 3
MEDIAN(1, 2, 3, 4)           # 2.5
MEDIAN()                     # Error: MEDIAN of no numbers (#NUM!)
MEDIAN(:data)                # Error: Cannot sort NaN values, when :data holds a NaN
```

`MEDIAN`, `PERCENTILE_INC`, `QUARTILE_INC`, `SORT` and `.sort()` sort their data first. If the data
contains NaN, they return the error `Cannot sort NaN values`. Expressions cannot make NaN (`0/0` is
an error), but a host program can pass one in a variable.

#### `MODE.SNGL(...)`
Most frequently occurring value; the first one seen wins a tie. If no value repeats, it is an
error (Excel `#N/A`). Write it `MODE_SNGL`.
```bash
MODE_SNGL(1, 2, 2, 3, 4, 4, 4)     # 4
MODE_SNGL(1, 2, 3)                 # Error: MODE.SNGL: no value repeats (#N/A)
```

#### `PERCENTILE.INC(array, k)`
k-th percentile (0 ≤ k ≤ 1). A `k` outside 0..1, or NaN, is an error (ADR-0015). Write it `PERCENTILE_INC`.
```bash
PERCENTILE_INC([1,2,3,4,5], 0.5)   # 3 (50th percentile)
PERCENTILE_INC([1,2,3,4,5], 0.25)  # 2 (25th percentile)
PERCENTILE_INC([1,2,3], 1.5)       # Error: Percentile must be between 0 and 1 (#NUM!)
```

#### `QUARTILE.INC(array, quart)`
Quartile value (0=min, 1=Q1, 2=Q2, 3=Q3, 4=max). `quart` must be in 0..4 before it is truncated,
as in Excel, so 2.7 means 2 but 4.9 is an error. Write it `QUARTILE_INC`.
```bash
QUARTILE_INC([1,2,3,4,5,6,7,8], 1) # 2.75 (Q1)
QUARTILE_INC([1,2,3,4,5,6,7,8], 2) # 4.5 (Q2/median)
QUARTILE_INC([1,2,3,4], -0.5)      # Error: Quartile must be 0-4 (#NUM!)
```

### Logical Functions

#### `IF(condition, true_value, false_value)`
Basic conditional.
```bash
IF(5 > 3, "Yes", "No")       # "Yes"
IF(false, 10, 20)            # 20
```

#### `IFS(condition1, value1, condition2, value2, ...)`
Multiple conditions (returns first matching).
```bash
IFS(false, "A", true, "B", true, "C")  # "B"
IFS(score >= 90, "A", score >= 80, "B", true, "C")
```

#### `AND(...)`, `OR(...)`
Logical operations on multiple values.
```bash
AND(true, true, false)       # false
OR(false, false, true)       # true
```

#### `NOT(value)`
Logical negation.
```bash
NOT(true)                    # false
NOT(false)                   # true
```

#### `XOR(...)`
Exclusive OR.
```bash
XOR(true, false)             # true
XOR(true, true)              # false
```

### Text Functions

#### `LEN(text)`, `LENGTH(text)`
String length.
```bash
LEN("Hello")                 # 5
LENGTH("Hello World")        # 11
```

#### `CONCAT(...)`
Concatenate strings.
```bash
CONCAT("Hello", " ", "World") # "Hello World"
CONCAT("Value: ", 42)         # "Value: 42"
```

#### `SUBSTRING(text, start, length)`
Extract substring (0-based indexing).
```bash
SUBSTRING("Hello World", 6, 5)  # "World"
SUBSTRING("Hello", 1, 3)        # "ell"
```

#### `LEFT(text, [num_chars])`, `RIGHT(text, [num_chars])`
Extract from left/right side.
```bash
LEFT("Hello", 2)             # "He"
RIGHT("Hello", 3)            # "llo"
LEFT("Hello")                # "H" (default: 1 char)
```

#### `MID(text, start, [num_chars])`
Extract substring (1-based indexing, Excel-style).
```bash
MID("Hello", 2, 3)           # "ell" (1-based start)
MID("Hello", 3)              # "llo" (rest of string)
```

#### `SUBSTITUTE(text, old_text, new_text, [instance_num])`, `SUBSTITUTEM(text, substr, replacement)`
Replace every occurrence of `old_text` with `new_text`, or, with `instance_num`, only that
occurrence (1-based, as in Excel). An empty `old_text` returns the text unchanged. Matching is
case-sensitive. `SUBSTITUTEM` replaces every occurrence and takes no `instance_num`.
```bash
SUBSTITUTE("foo bar foo", "foo", "baz")   # "baz bar baz"
SUBSTITUTE("a-a-a", "-", "_")             # "a_a_a"
SUBSTITUTE("a-b-c", "-", "+", 2)          # "a-b+c"
SUBSTITUTEM("a-a-a", "-", "_")            # "a_a_a"
```

#### `TRIM(text)`
Excel `TRIM`: removes leading and trailing spaces and collapses each inner run of spaces to one.
Only the space character counts; tabs and newlines are kept. The `.trim()` method is different: it
strips all whitespace at both ends and keeps inner spaces.
```bash
TRIM("  a   b  ")            # "a b"
"  a   b  ".trim()           # "a   b"
```

#### `REPLACE(old_text, start_num, num_chars, new_text)`
Excel-style positional replace using 1-based `start_num` and character counts.
```bash
REPLACE("abcdef", 3, 2, "XY")             # "abXYef"
REPLACE("abc", 1, 0, "X")                 # "Xabc" (insert)
REPLACE("hello", 4, 10, "X")              # "helX" (clamped)
```

#### `ISNUMBER(value)`, `ISTEXT(value)`
Type checking.
```bash
ISNUMBER(42)                 # true
ISTEXT("hello")              # true
ISNUMBER("hello")            # false
```

### Date/Time Functions

#### `NOW()`
Current timestamp (Unix time).
```bash
NOW()                        # Current timestamp
```

#### `DATE(year, month, day)`
Create date timestamp.
```bash
DATE(2023, 12, 25)           # Unix timestamp for Dec 25, 2023
```

#### `TIME()`, `TIME(hour, minute, second)`
The time as a fraction of a day, as in Excel (ADR-0018): `TIME(12, 0, 0)` is 0.5. Each part is
truncated, a part above 32767 is an error, a negative minute or second is allowed while the total
is not negative, and the total wraps at 24 hours. With no arguments it returns the current local
time in the same unit. One or two arguments are an error. Multiply by 86400 for seconds.
```bash
TIME(14, 30, 0)              # 0.6041666666666666 (2:30 PM)
TIME(12, 0, 0)               # 0.5
TIME(25, 0, 0)               # 0.041666666666666664 (wraps to 1:00)
TIME(0, 750, 0)              # 0.5208333333333334 (750 minutes = 12:30)
TIME(0, -1, 0)               # Error: TIME is before midnight (#NUM!)
TIME(14, 30, 0) * 86400      # 52200 (seconds since midnight)
TIME()                       # current local time, e.g. 0.9402 (22:33:56)
TIME(14, 30)                 # Error: TIME expects either no arguments or three arguments (hour, minute, second)
```

#### `YEAR(date)`, `MONTH(date)`, `DAY(date)`
Extract date components.
```bash
YEAR(NOW())                  # Current year
MONTH(DATE(2023, 6, 15))     # 6
DAY(DATE(2023, 6, 15))       # 15
```

#### `DATEADD(date, amount, unit)`
Add time to date.
```bash
DATEADD(NOW(), 30, "days")     # 30 days from now
DATEADD(NOW(), 2, "months")    # 2 months from now
DATEADD(NOW(), 1, "years")     # 1 year from now
```

#### `DATEDIFF(date1, date2, [unit])`
Difference between dates.
```bash
DATEDIFF(NOW(), DATEADD(NOW(), 7, "days"), "days")  # 7
```

### Array Functions

#### `FILTER(array, expression)`
Filter array elements.
```bash
FILTER([1,2,3,4,5], :x > 3)   # [4, 5]
FILTER(data, :x.active == true)
```

#### `MAP(array, expression)`
Transform array elements.
```bash
MAP([1,2,3], :x * 2)          # [2, 4, 6]
MAP(users, :x.name.upper())
```

#### `FIND(array, expression)`
Find first matching element.
```bash
FIND([1,2,3,4,5], :x > 3)     # 4
FIND(users, :x.id == 123)
```

#### `REDUCE(array, expression, initial)`
Reduce array to single value.
```bash
REDUCE([1,2,3,4], :acc + :x, 0)    # 10
REDUCE(orders, :acc + :x.total, 0)
```

#### `COUNTIF(range, criteria)`, `SUMIF(range, criteria, [sum_range])`, `AVERAGEIF(range, criteria, [average_range])`
Count, sum or average the items that meet `criteria`. `AVGIF` is an alias of `AVERAGEIF`. The
range is an array, or a single value (treated as a one-item range). With a sum or average range,
the item at the same position is summed; only numbers and currency are summed or averaged.
`AVERAGEIF` with no matching number is an error (`#DIV/0!`); `SUMIF` and `COUNTIF` give 0.

`criteria` is either an Excel criterion (a string or a number) or a lambda over `:x`:

| Criterion | Matches |
|---|---|
| `5`, `"5"`, `"=5"` | the number 5 (and text that reads as 5) |
| `">2"`, `">=2"`, `"<2"`, `"<=2"`, `"<>2"` | numbers by comparison; `"<>2"` also matches every non-number |
| `"a"`, `"=a"` | text equal to `a`, ignoring case |
| `"a*"`, `"a?c"`, `"~*"` | wildcards: `*` any run, `?` one character, `~` escapes `*`, `?` or `~` |
| `"<>a"` | everything except text equal to `a` |
| `">b"` | text after `b` (ignoring case) |
| `"TRUE"`, `"FALSE"` | booleans |
| `""`, `"="` | blanks (`NULL` or `""`); `"<>"` matches everything else |
| `:x > 2` (lambda) | items for which the expression is TRUE. It must return a boolean (`NULL` counts as FALSE); any other result is an error |

```bash
COUNTIF([1, 2, 3, 4], ">2")                       # 2
COUNTIF([1, 2, 3], 2)                             # 1
COUNTIF(["A", "b", "a"], "a")                     # 2 (case-insensitive)
COUNTIF(["apple", "Avocado", "kiwi"], "a*")       # 2
COUNTIF(5, ">2")                                  # 1 (a single value is a one-item range)
COUNTIF([1, 2, 3, 4], :x % 2 == 0)                # 2 (lambda)
COUNTIF([1, 2, 3], :x)                            # Error: COUNTIF criteria expression must return TRUE or FALSE, got number

SUMIF([1, -2, 3, -4], :x > 0)                     # 4
SUMIF([10, 20, 30, 40], ">25")                    # 70
SUMIF([10, 20, 30, 40], "<>20")                   # 80
SUMIF([10, 30, 50], ">20", [1, 2, 3])             # 5 (sums 2+3)
SUMIF(["apple", "avocado", "kiwi"], "a*", [1, 2, 4])  # 3

AVERAGEIF([1, 2, 3, 4], ">2")                     # 3.5
AVGIF([1, 3, 5, -1], :x > 0)                      # 3.0
AVERAGEIF([1, 2], ">5")                           # Error: AVERAGEIF: no matching numbers (#DIV/0!)
```

#### `BETWEEN(min, max, value)`
Range checking.
```bash
BETWEEN(10, 20, 15)          # true
BETWEEN(1, 10, 15)           # false
```

#### `FLATTEN(array)`
Flatten nested arrays.
```bash
FLATTEN([1,[2,[3]],4])       # [1, 2, 3, 4]
```

#### `UNIQUE(array)`
Remove duplicates.
```bash
UNIQUE([1,2,2,3,3,3])        # [1, 2, 3]
```

#### `COMPACT(array)`
Remove null values.
```bash
COMPACT([1, null, 2, null])  # [1, 2]
```

### Financial Functions

#### `PMT(rate, nper, pv, [fv], [type])`
Calculate loan payment.

**Parameters:**
- `rate`: Interest rate per period
- `nper`: Number of payment periods
- `pv`: Present value (loan amount)
- `fv`: Future value (optional, default 0)
- `type`: Payment timing (optional, 0=end, 1=beginning)

```bash
# 30-year mortgage at 5% annual rate
PMT(0.05/12, 30*12, 100000)    # Monthly payment: $536.82

# Car loan: 4% for 5 years
PMT(0.04/12, 5*12, 25000)      # Monthly payment: $460.41

# Savings goal: 6% return, save for 10 years to reach $50k
PMT(0.06/12, 10*12, 0, 50000)  # Need to save: $305.10/month
```

---

## Method Calls

Methods are called on values using dot notation: `value.method(args)`

### String Methods

#### `.upper()`, `.lower()`
Change case.
```bash
"hello".upper()              # "HELLO"
"WORLD".lower()              # "world"
```

#### `.trim()`
Remove leading/trailing whitespace.
```bash
"  spaces  ".trim()          # "spaces"
```

#### `.reverse()`
Reverse string.
```bash
"hello".reverse()            # "olleh"
```

#### `.includes(substring)`
Check if string contains substring.
```bash
"hello world".includes("world")  # true
"hello".includes("xyz")          # false
```

#### `.length()`
String length.
```bash
"hello".length()             # 5
```

### Array Methods

#### `.length()`, `.count()`
Array length.
```bash
[1,2,3,4,5].length()         # 5
[].count()                   # 0
```

#### `.first()`, `.last()`
First/last element.
```bash
[1,2,3,4,5].first()          # 1
[1,2,3,4,5].last()           # 5
```

#### `.reverse()`
Reverse array.
```bash
[1,2,3].reverse()            # [3, 2, 1]
```

#### `.unique()`
Remove duplicates.
```bash
[1,2,2,3].unique()           # [1, 2, 3]
```

#### `.sort([order])`
Sort array.
```bash
[3,1,2].sort()               # [1, 2, 3]
[3,1,2].sort("DESC")         # [3, 2, 1]
```

#### `.sum()`, `.avg()`, `.min()`, `.max()`
Numeric operations.
```bash
[1,2,3,4,5].sum()            # 15
[1,2,3,4,5].avg()            # 3.0
[5,1,9].min()                # 1
[5,1,9].max()                # 9
```

#### `.join([separator])`
Join to string.
```bash
[1,2,3].join("-")            # "1-2-3"
["a","b","c"].join()         # "a,b,c" (default: comma)
```

#### `.contains(value)`, `.includes(value)`
Check if array contains value.
```bash
[1,2,3].contains(2)          # true
[1,2,3].includes(4)          # false
```

#### `.flatten()`
Flatten nested arrays.
```bash
[1,[2,[3]],4].flatten()      # [1, 2, 3, 4]
```

#### `.compact()`
Remove null values.
```bash
[1, null, 2, null].compact() # [1, 2]
```

#### Higher-order methods:

#### `.filter(expression)`
Filter elements.
```bash
[1,2,3,4,5].filter(:x > 3)   # [4, 5]
```

#### `.map(expression)`
Transform elements.
```bash
[1,2,3].map(:x * 2)          # [2, 4, 6]
```

#### `.find(expression)`
Find first match.
```bash
[1,2,3,4,5].find(:x > 3)     # 4
```

#### `.reduce(expression, initial)`
Reduce to single value.
```bash
[1,2,3,4].reduce(:acc + :x, 0)  # 10
```

### Number Methods

#### `.abs()`
Absolute value.
```bash
(-5).abs()                   # 5
```

#### `.ceil()`, `.floor()`
Round up/down.
```bash
(3.2).ceil()                 # 4
(3.8).floor()                # 3
```

#### `.round([digits])`
Round to decimal places.
```bash
(3.14159).round()            # 3
(3.14159).round(2)           # 3.14
```

#### `.sqrt()`
Square root.
```bash
(16).sqrt()                  # 4
```

#### `.int()`
Integer part.
```bash
(3.9).int()                  # 3
```

#### Trigonometric methods:
- `.sin()`, `.cos()`, `.tan()`

### Predicate Methods

Methods ending with `?` return boolean values:

#### Numeric predicates:
```bash
(42).positive?               # true
(-5).negative?               # true
(0).zero?                    # true
(4).even?                    # true
(5).odd?                     # true
```

#### Value predicates:
```bash
("").blank?                  # true
("hello").present?           # true
([]).blank?                  # true
([1,2,3]).present?           # true
(null).nil?                  # true
```

#### Range predicate:
```bash
(15).between(10, 20)         # true
```

### JSON Object Methods

#### `.keys()`
Get object keys.
```bash
{"name": "John", "age": 30}.keys()  # ["name", "age"]
```

#### `.values()`
Get object values.
```bash
{"name": "John", "age": 30}.values()  # ["John", 30]
```

#### `.has_key(key)`, `.has(key)`
Check if key exists.
```bash
{"name": "John"}.has_key("name")     # true
{"name": "John"}.has("age")          # false
```

#### `.dig(path_array, [default])`
Safely navigate nested JSON using an array of keys and/or indexes. Returns `default` or `null` if not found.
```bash
{"user": {"posts": [{"title": "First"}]}}.dig(['user','posts',0,'title'])  # "First"
{"a": {"b": 1}}.dig(['a','x'], 'fallback')                                  # "fallback"
```

---

## JSON Functions

#### `DIG(json_obj, path_array, [default])`
Navigate nested JSON by path. Accepts keys (strings) and array indexes (numbers). If the path is missing, returns `default` if provided, otherwise `null`.
```bash
:obj := {"user": {"name": "Jane", "posts": [{"title": "First"}, {"title": "Second"}]}};
DIG(:obj, ['user','posts',1,'title'])      # "Second"
DIG(:obj, ['user','missing'], 'N/A')       # "N/A"
```

#### `JQ(json_data, jsonpath_expression)`
Execute JSONPath queries on JSON data. Supports complex path expressions, filtering, and array operations. Returns the matching data that can be used with aggregation functions.
```bash
# Basic property access
JQ(:arguments, "$.user.name")              # Extract user name
JQ(:arguments, "$.accounts[*].amount")     # All account amounts

# Array filtering
JQ(:arguments, "$.products[?(@.price > 100)]")                    # Products over $100
JQ(:arguments, "$.items[?(@.category == 'electronics')].price")   # Electronic item prices

# Nested queries
JQ(:arguments, "$.departments[*].employees[*].salary")            # All employee salaries

# Use with aggregation functions
SUM(JQ(:arguments, "$.sales[*].amount"))   # Total sales
AVG(JQ(:arguments, "$.scores[*].value"))   # Average score
MIN(JQ(:arguments, "$.prices[*].cost"))    # Minimum price
```

**JSONPath Syntax:**
- `$.property` - Access property
- `$.array[n]` - Access array element by index
- `$.array[*]` - Access all array elements
- `$.array[?(@.field == 'value')]` - Filter array elements
- `$.nested.property` - Access nested properties

---

## Type Conversion Methods

Available on **all value types** including null.

### Method Names
- `to_s()` or `to_string()` - Convert to string
- `to_i()` or `to_int()` - Convert to integer
- `to_f()` or `to_float()` - Convert to float
- `to_a()` or `to_array()` - Convert to array
- `to_json()` - Convert to JSON
- `to_bool()` or `to_boolean()` - Convert to boolean

### Null Conversions
```bash
null.to_s()                  # ""
null.to_i()                  # 0
null.to_f()                  # 0.0
null.to_a()                  # []
null.to_json()               # "{}"
null.to_bool()               # false
```

### String Conversions
```bash
"123".to_i()                 # 123
"123.45".to_f()              # 123.45
"abc".to_i()                 # 0 (invalid → 0)
"hello".to_a()               # ["h", "e", "l", "l", "o"]
"hello".to_bool()            # true
"".to_bool()                 # false
```

### Number Conversions
```bash
123.to_s()                   # "123"
123.45.to_i()                # 123 (truncated)
42.to_a()                    # [42]
0.to_bool()                  # false
123.to_bool()                # true
```

### Boolean Conversions
```bash
true.to_s()                  # "true"
true.to_i()                  # 1
false.to_i()                 # 0
true.to_a()                  # [true]
```

### Array Conversions
```bash
[1,2,3].to_s()               # "[1, 2, 3]"
[1,2,3].to_i()               # 3 (length)
[].to_i()                    # 0 (empty length)
[].to_bool()                 # false
[1,2].to_bool()              # true
```

---

## Safe Navigation Operator

The `&.` operator prevents errors when accessing properties or calling methods on null values.

### Safe Property Access
```bash
obj&.property                # Returns null if obj is null
null&.anything               # Returns null (no error)
```

### Safe Method Calls
```bash
str&.length()                # Returns null if str is null
null&.method()               # Returns null (no error)
```

### Chaining
```bash
user&.profile&.name&.length()  # Stops at first null, returns null
```

---

## Type Casting

Explicit type conversion using the `::` operator.

### Casting Syntax
```bash
value :: type_name
```

### Available Types
- `Integer` - Convert to integer
- `Float` - Convert to float
- `String` - Convert to string
- `Boolean` - Convert to boolean
- `Array` - Convert to array
- `Currency` - Convert to currency
- `DateTime` - Convert to datetime
- `Json` - Convert to JSON

### Examples
```bash
"42" :: Integer              # 42
42 :: String                 # "42"
1 :: Boolean                 # true
0 :: Boolean                 # false
"hello" :: Array             # ["h", "e", "l", "l", "o"]
```

---

## Variable Syntax

### Variable Reference
```bash
:variable_name               # Reference variable
```

### Variable Assignment
```bash
:var := expression           # Assign expression result to variable
```

### Lambda Parameters
```bash
:parameter_name              # In lambda expressions
# Example: array.filter(:x > 5)
# :x is the lambda parameter
```

### Lambda Parameters with Custom Names
```bash
# Default parameter names
[1,2,3].filter(:x > 2)       # :x is the element
[1,2,3].reduce(:acc + :x, 0) # :acc is accumulator, :x is element

# Custom parameter names
[1,2,3].filter(:item > 2, "item")           # Custom element name
[1,2,3].reduce(:total + :val, 0, "val", "total")  # Custom names
```

### Spread Operator
```bash
...array                     # Spread array elements
# Example: SUM(...[1,2,3])  # Same as SUM(1,2,3)
```

### Object Literals
```bash
{key: value, key2: value2}   # Create JSON object
# Example: {name: "John", age: 30}
```

---

This reference covers all available functions, methods, and operators in Skillet. For practical examples and usage patterns, see the main [user guide](../guide/user-guide.md) file.
