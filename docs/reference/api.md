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
8. [Rust library: evaluating a parsed expression](#rust-library-evaluating-a-parsed-expression)

Evaluation limits (nesting depth 128, 256 MiB budget, server deadline) and their error texts are
documented in [limits](../guide/limits.md).

Server request limits: both `sk_server` and `sk_http_server` reject a request over 1 MB, give a
client 30 s in total to send one request, and give themselves 10 s in total to write one response
(a client that reads more slowly gets a truncated response). `sk_http_server` answers the first two
with HTTP 413 and HTTP 408 (400 before 0.7.0), then reads the rest of the request before closing. Both
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
- `%` - Percent suffix (`10%` is `0.1`), or modulo when an operand follows (`10 % 3` is `1`)
- `^` - Exponentiation (left-associative: `2^3^2` is `64`)
- `=` - Equals prefix (Excel-style)

**Number literals.** Integers (`42`), decimals (`2.5`, `.5`) and scientific notation (`1e3`,
`1E3`, `2.5e-4`, `1e+2`) are numbers. A number that runs into a letter is a parse error
(`Invalid number literal`): `1x`, `1e`, `2ex`. Before BL-058, `1e3` silently evaluated to `1`.

**Percent or modulo.** `%` is the percent suffix (divide by 100) unless the next token starts an
operand directly: a number, `(`, a `:variable`, a function name, a string, `[`, `{` or `!`. Then
it is modulo. Before `+`, `-`, `)`, `,`, `]`, `;`, another operator, `&`, a comparison, `?`/`:`
or the end of the expression it is a percent (BL-066).

```bash
10%            # 0.1
10%-3          # -2.9   (10% - 3)
10 % -3        # -2.9   (percent, then minus 3; before BL-066 this was modulo: 1)
10% + 1        # 1.1
50%*2          # 1
10 % 3         # 1      (modulo)
10 % (3)       # 1      (modulo)
:a % :b        # modulo
```

**Coercion (like Excel, BL-066).** The arithmetic operators (`+ - * / % ^`, unary `-` and `+`,
the percent suffix) convert their operands to numbers:

| Operand | Converts to |
|---|---|
| Text that reads as a number after trimming: `"-3"`, `"2.5"`, `"1e3"`, `" 4 "` | that number |
| `TRUE` / `FALSE` | `1` / `0` |
| `NULL` (a blank cell) | `0` |
| Other text (`"abc"`, `""`, `"1,000"`, `"inf"`) | `Error: Cannot convert "abc" to a number (#VALUE!)` |
| Arrays, JSON objects | `Error: Arithmetic op on non-number` (as before) |

```bash
"3" + 1        # 4
TRUE + 1       # 2
1 + NULL       # 1
"a" + "b"      # Error: Cannot convert "a" to a number (#VALUE!)   (use & to join text)
```

`+` never joins text; `&` does. With a DateTime operand, the other operand converts the same
way (ADR-0027); the date rules are below.

The scalar math builtins convert numeric text and booleans the same way: `ABS`, `SQRT`, `INT`,
`MOD`, `POWER`, `ROUND`, `CEILING`, `FLOOR`, the arguments of the financial functions (`PMT`,
`FV`, `PV`, `IPMT`, `PPMT`, `DB`, `NPV`'s rate) and the parts of `DATE` and `TIME`.
`ABS("-3")` is `3`, `ABS(TRUE)` is `1`, `DATE("2026", 1, 1)` works. Text that is not a number is
`#VALUE!` with the function name: `ABS("x")` is `Error: ABS: cannot convert "x" to a number
(#VALUE!)`. `NULL` in a required argument stays an error (`ABS(NULL)`); an optional argument
that is `NULL` takes its default. Aggregates over arrays (`SUM`, `AVERAGE`, `MAX`, `NPV`'s
values, …) still skip text and booleans inside arrays, as Excel skips them in a range:
`SUM([1, "2", TRUE])` is `1`.

Dates work with `+` and `-` in days, like Excel:

```bash
DATE(2026, 1, 31) + 1                  # DateTime for 2026-02-01 (Number + DateTime works too)
DATE(2026, 3, 1) - 1                   # DateTime for 2026-02-28
DATE(2026, 1, 1) + 0.5                 # DateTime for 2026-01-01 12:00 (rounded to the second)
DATE(2026, 1, 1) + TIME(12, 0, 0)      # DateTime for 2026-01-01 12:00 (TIME is a day fraction)
DATE(2026, 3, 1) - DATE(2026, 2, 1)    # 28 (days; fractional when times differ)
DATE(2026, 1, 1) + NULL                # DateTime for 2026-01-01 (NULL is 0 days)
DATE(2026, 1, 1) + "1"                 # DateTime for 2026-01-02 (numeric text and booleans convert)
DATE(2026, 1, 1) + "x"                 # Error: Cannot convert "x" to a number (#VALUE!)
```

Other operators on a DateTime (`*`, `/`, `%`, `^`, `DateTime + DateTime`, `x - DateTime`) are an
error.

Skillet has one number type (ADR-0033). `::Currency` is a conversion: it rounds to 4 decimals
and returns a Number (see [Type Casting](#type-casting)), so its result works with every operator
like any number. The operators do not round again:

```bash
10::Currency + 2               # 12
1.23456::Currency * 2          # 2.4692 (1.2346 * 2)
1::Currency / 3                # 0.3333333333333333: not rounded; use ROUND(x, 2)
0.1::Currency + 0.2::Currency  # 0.30000000000000004; (0.1 + 0.2)::Currency is 0.3
```

The arithmetic operators never produce NaN or infinity (BL-054, ADR-0018). Division or `%` by 0
and `0 ^` a negative number are `Error: Division by zero (#DIV/0!)`; `0 ^ 0` and any result that
is not a finite number (overflow, a non-real power) are a `#NUM!` error.
```bash
7 / 2                        # 3.5
1 / 0                        # Error: Division by zero (#DIV/0!)
0 / 0                        # Error: Division by zero (#DIV/0!)
10 ^ 400                     # Error: Result is not a finite number (#NUM!)
1e308 * 10                   # Error: Result is not a finite number (#NUM!)
0 ^ 0                        # Error: 0 ^ 0 is undefined (#NUM!)
```

### Text Operator
- `&` - Joins both sides as text, with `CONCAT` rules: `"a" & 1 & TRUE` is `"a1TRUE"`,
  `NULL` adds nothing. Write `& .5` with a space: `&.` is safe navigation.

### Comparison Operators
- `>` - Greater than
- `<` - Less than
- `>=` - Greater than or equal
- `<=` - Less than or equal
- `==` (or `=`) - Equal to
- `!=` - Not equal to

`==` is strict: a value equals only a value of the same type with the same value, and text is
case-sensitive (`"a" == "A"` is `FALSE`). No comparison converts between types: `1 == "1"`,
`TRUE == 1` are `FALSE` (conversion happens only in arithmetic, ADR-0023). `::Currency` gives a
Number (ADR-0033), so `1 == 1::Currency` is `TRUE`.

- Arrays are equal when they have the same length and equal items, in order
  (`[1, 2] == [1, 2]`). JSON objects are equal when they have the same keys and equal values;
  key order does not matter.
- Two DateTimes compare by timestamp (`DATE(2026,1,1) == DATE(2026,1,1)`). A DateTime never
  equals a value of another type (`DATE(2026,1,1) == 0` is `FALSE`).
- `NULL == NULL` is `TRUE` and `NULL` is unequal to every other value (`NULL == 0` is `FALSE`,
  ADR-0013).
- `<`, `<=`, `>`, `>=` need two values of the same type: numbers, booleans
  (`FALSE < TRUE`), text, or DateTimes (earlier is less). Text orders by Unicode code point,
  case-sensitive (`"B" < "a"`), as text even when it looks like a number (`"10" < "9"`). Any
  other pair, including a `NULL` operand, is an error.

### Logical Operators
- `AND`, `&&` - Logical AND
- `OR`, `||` - Logical OR
- `NOT`, `!` - Logical NOT

The operators short-circuit (ADR-0029): the right side is evaluated only when the left side does
not decide the result. Each side is a condition, read like the condition of `IF` (see
[Conditions](#conditions)), and the result is always `TRUE` or `FALSE`.

```bash
FALSE AND 1/0              # FALSE (1/0 is not evaluated)
TRUE OR 1/0                # TRUE
TRUE AND 1/0               # Error: Division by zero (#DIV/0!)
:n == 0 OR 10 / :n > 1     # safe when :n is 0
1 AND 2                    # TRUE (numbers: non-zero is TRUE)
NULL AND FALSE             # FALSE (NULL is FALSE, on either side)
TRUE AND NULL              # FALSE
NULL OR TRUE               # TRUE
"x" AND TRUE               # Error: AND condition must be TRUE, FALSE or a number, got string (#VALUE!)
```

`NOT` (any case) is a prefix keyword that binds looser than the comparisons and tighter than
`AND`, as in VB and SQL. Its operand runs to the next `AND`, `OR`, `&&` or `||`. It gives the
same result as the `NOT(...)` builtin, with the builtin's rules (`NOT 0` is `TRUE`, `NOT "x"` is
an error).

```bash
NOT :x > 1                 # NOT (:x > 1)
NOT 1 > 2 AND TRUE         # (NOT (1 > 2)) AND TRUE  ->  TRUE
NOT :a OR :b               # (NOT :a) OR :b
:a AND NOT :b == :c        # :a AND NOT (:b == :c)
NOT (0) == FALSE           # NOT ((0) == FALSE)  ->  TRUE
1 + NOT TRUE               # Error: NOT binds looser than comparisons here; write (NOT x)
TRUE == (NOT FALSE)        # TRUE
```

`NOT` followed directly by `(`, with no space, is the `NOT(...)` call. A call is an atom, so it
works anywhere: `TRUE == NOT(FALSE)`, `NOT(0) == FALSE` (FALSE). `!x` gives the same value as
`NOT(x)` (`!0` is `TRUE`, `!NULL` is `TRUE`, `!"x"` is an error), but it stays a tight unary
operator: `!TRUE == FALSE` is `(!TRUE) == FALSE`, so `!0 == FALSE` is `FALSE` while
`NOT 0 == FALSE` is `TRUE`.

#### Conditions
Every condition follows one rule (ADR-0029 Amendment 1): in `IF`, `IFS`, `? :`, the `AND`/`OR`/
`&&`/`||`/`NOT`/`!` operators, the `filter`/`find`/`FILTER`/`FIND` lambdas and the lambda
criteria of `COUNTIF`/`SUMIF`/`AVERAGEIF`.

| Value | As a condition |
|---|---|
| `TRUE`, `FALSE` | itself |
| a number or Currency | `FALSE` when 0, otherwise `TRUE` |
| NaN (only from a host variable or a custom function) | error: `<name> condition is not a number (#NUM!)` |
| `NULL` (for example a missing key read with `&.`) | `FALSE` |
| text (also `""`), a date, an array, a JSON object | error: `<name> condition must be TRUE, FALSE or a number, got string (#VALUE!)` |

The `AND()`, `OR()` and `XOR()` functions are the exception: they skip a `NULL` argument, as
Excel skips an empty cell (see [Logical Functions](#logical-functions)). A NaN is the `#NUM!`
error in them too (`AND condition is not a number (#NUM!)`).

### Unary Operators
- `+` - Positive (unary plus)
- `-` - Negative (unary minus)
- `!` - Logical NOT

### Ternary Operator
- `condition ? true_value : false_value` - Conditional expression. The condition follows the
  `IF` rule ([Conditions](#conditions)), and only the chosen value is evaluated.

```bash
1 ? "a" : "b"              # "a"
NULL ? "a" : "b"           # "b"
TRUE ? 1 : 1/0             # 1
"x" ? 1 : 2                # Error: Ternary condition must be TRUE, FALSE or a number, got string (#VALUE!)
```

### Special Operators
- `&.` - Safe navigation operator
- `::` - Type casting operator
- `...` - Spread operator

### Precedence
Tightest first, as in Excel. Operators on the same row are left-associative.

| Level | Operators |
|---|---|
| 1 | postfix `.method()`, `.prop`, `&.`, `[i]`, then `::type` casts (chained left to right) |
| 2 | unary `-`, `+`, `!` (`-2^2` is `4`) |
| 3 | percent suffix `%` |
| 4 | `^` (`2^3^2` is `64`) |
| 5 | `*`, `/`, `%` (modulo) |
| 6 | `+`, `-` |
| 7 | `&` |
| 8 | `>`, `<`, `>=`, `<=` |
| 9 | `==`, `!=` |
| 10 | prefix `NOT` keyword (starts an operand of `AND`/`OR` only) |
| 11 | `AND`, `&&` |
| 12 | `OR`, `\|\|` |
| 13 | `? :` |

`-(2^2)` gives `-4`. Before BL-054, `^` was right-associative (`2^3^2` was `512`) and
unary minus bound looser than `^` (`-2^2` was `-4`).

A minus right after `^` negates the exponent only, as in Excel (BL-060):

```bash
2^-2           # 0.25
-2^-2          # 0.25   ((-2)^(-2))
2^-1^2         # 0.25   ((2^-1)^2)
0^-1           # Error: Division by zero (#DIV/0!)
```

`AND` and `OR` (any case) between two operands are the `&&` and `||` operators; followed by `(`
in operand position they are the `AND(...)` and `OR(...)` builtins: `AND(TRUE, FALSE) OR TRUE`
is `TRUE`.

### End of input
The whole input must be one expression, or a `;` sequence of them (one trailing `;` is allowed).
Anything left after it is a parse error at the first extra token; the position counts from the
start of the input, including a leading `=`:

```bash
=1 2           # Error: Unexpected token after end of expression at position 3
1; 2 3         # Error: Unexpected token after end of expression at position 5
(1) (2)        # Error: Unexpected token after end of expression at position 4
```

Before BL-069 the rest was dropped silently: `=1 2` was `1` and `TRUE AND FALSE` was `TRUE`.
This applies to `parse`, every `evaluate*` function, `sk` and both servers. Error positions now
also count the leading whitespace and `=` (before, `=1 +` reported the position in `1 +`).

---

## Built-in Functions

### Arithmetic Functions

#### `SUM(...)`
Sum of all arguments.
```bash
SUM(1, 2, 3, 4, 5)           # 15
SUM([1, 2, 3], [4, 5])       # 15 (arrays are flattened)
SUM(...[1, 2, 3])            # 6 (spread operator)
SUM(10::Currency, 5)         # 15
```

#### `PRODUCT(...)`, `MULTIPLY(...)`
Product of all arguments.
```bash
PRODUCT(1, 2, 3, 4)          # 24
MULTIPLY(2, 3, 4)            # 24 (alias)
PRODUCT([2, 3, 4])           # 24 (works with arrays)
PRODUCT(2::Currency, 3)      # 6
```

`SUM`, `PRODUCT`, `AVERAGE`, `MIN` and `MAX` return a Number, also for values converted with
`::Currency` (one number type, ADR-0033).

UPPERCASE builtins follow Excel's documented behaviour (BL-054). Where Excel returns `#DIV/0!`,
`#NUM!`, `#VALUE!` or `#N/A`, Skillet returns an error, and the message names the Excel error.
A wrong-typed or missing argument is an error, never a silent 0. Inside arrays, text, booleans
and `NULL` are ignored by the aggregates, as Excel ignores them in a range.

Dates (ADR-0031 amendment 2): `MIN` and `MAX` of dates return a date. The aggregates that do
arithmetic (`SUM`, `PRODUCT`, `AVERAGE`, `MEDIAN`, `MODE.SNGL`, `STDEV.*`, `VAR.*`,
`PERCENTILE.INC`, `QUARTILE.INC`) return `Error: <NAME> does not accept dates (#VALUE!)` when a
DateTime is among their values. `COUNT` counts dates. Excel treats a date as a serial number;
Skillet has a DateTime type, so a date and a number are never compared or added together.

#### `AVERAGE(...)`
Average of the numbers in the arguments. No numbers is an error (Excel `#DIV/0!`).
```bash
AVERAGE(1, 2, 3, 4, 5)       # 3.0
AVG(10, 20, 30)              # 20.0 (alias)
AVERAGE([1, "x", TRUE])      # 1 (text and booleans in arrays are ignored)
AVERAGE(10::Currency, 20)    # 15
AVERAGE([])                  # Error: AVERAGE of no numbers (#DIV/0!)
```

#### `MAX(...)`, `MIN(...)`
Maximum/minimum value from arguments. As in Excel, no numbers gives 0. With only dates (and
ignored values), the latest/earliest date; dates mixed with numbers are an error.
```bash
MAX(1, 5, 3, 9, 2)           # 9
MIN([1, 5, 3, 9, 2])         # 1 (works with arrays too)
MIN(1, 3::Currency)          # 1
MAX()                        # 0
MAX(DATE(2026,1,1), DATE(2027,1,1))  # DateTime for 2027-01-01
MAX(DATE(2026,1,1), 5)       # Error: MAX cannot compare dates with numbers (#VALUE!)
SUM(DATE(2026,1,1))          # Error: SUM does not accept dates (#VALUE!)
```

#### `COUNT(...)`, `COUNTA(...)`
`COUNT` counts the numbers (and datetime values) in its arguments; arrays are
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
form, as Excel does, so `ROUND(1.005, 2)` is 1.01. `x::Currency` is `ROUND(x, 4)`.
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

#### `STDEV.P(...)`, `VAR.P(...)`, `STDEV.S(...)`, `VAR.S(...)`
Population (divide by n) and sample (divide by n - 1) standard deviation and variance, as in
Excel. The dotted names work as written; `STDEV_P`, `STDEVP`, `VAR_P`, `VARP`, `STDEV_S`,
`STDEV`, `VAR_S` and `VAR` are aliases. The sample functions need at least two numbers.
```bash
STDEV.P(2, 4, 4, 4, 5, 5, 7, 9)    # 2
VAR.P(2, 4, 4, 4, 5, 5, 7, 9)      # 4
STDEV.S(2, 4, 4, 4, 5, 5, 7, 9)    # 2.138...
VAR.S(2, 4, 4, 4, 5, 5, 7, 9)      # 4.571... (32/7)
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

#### `MODE.SNGL(...)`, `MODE(...)`
Most frequently occurring value; the first one seen wins a tie. If no value repeats, it is an
error (Excel `#N/A`). Write it `MODE_SNGL`; `MODE` is Excel's legacy name for the same function.
```bash
MODE_SNGL(1, 2, 2, 3, 4, 4, 4)     # 4
MODE(5.6, 4, 4, 3, 2, 4)           # 4
MODE_SNGL(1, 2, 3)                 # Error: MODE.SNGL: no value repeats (#N/A)
```

#### `PERCENTILE.INC(array, k)`, `PERCENTILE(array, k)`
k-th percentile (0 ≤ k ≤ 1). A `k` outside 0..1, or NaN, is an error (ADR-0015). Write it
`PERCENTILE_INC`; `PERCENTILE` is Excel's legacy name for the same function.
```bash
PERCENTILE([1, 3, 2, 4], 0.3)      # 1.9
PERCENTILE_INC([1,2,3,4,5], 0.5)   # 3 (50th percentile)
PERCENTILE_INC([1,2,3,4,5], 0.25)  # 2 (25th percentile)
PERCENTILE_INC([1,2,3], 1.5)       # Error: Percentile must be between 0 and 1 (#NUM!)
```

#### `QUARTILE.INC(array, quart)`, `QUARTILE(array, quart)`
Quartile value (0=min, 1=Q1, 2=Q2, 3=Q3, 4=max). `quart` must be in 0..4 before it is truncated,
as in Excel, so 2.7 means 2 but 4.9 is an error. Write it `QUARTILE_INC`; `QUARTILE` is Excel's
legacy name for the same function.
```bash
QUARTILE([1,2,4,7,8,9,10,12], 1)   # 3.5
QUARTILE_INC([1,2,3,4,5,6,7,8], 1) # 2.75 (Q1)
QUARTILE_INC([1,2,3,4,5,6,7,8], 2) # 4.5 (Q2/median)
QUARTILE_INC([1,2,3,4], -0.5)      # Error: Quartile must be 0-4 (#NUM!)
```

### Logical Functions

#### `IF(condition, true_value, [false_value])`
Basic conditional. Only the chosen branch is evaluated, so it can guard an error. The condition
must be `TRUE`/`FALSE` or a number (non-zero is true, as in Excel); `NULL` is false, like a blank
cell. Text or any other value is an error (Excel `#VALUE!`). Without `false_value` the result is
`FALSE`. More than 3 arguments is an error, as in Excel.
```bash
IF(5 > 3, "Yes", "No")       # "Yes"
IF(false, 10, 20)            # 20
IF(2, "a", "b")              # "a"
IF(NULL, "a", "b")           # "b"
IF(:a == 0, 0, 1 / :a)       # 0 when :a is 0
IF("x", 1, 2)                # Error: IF condition must be TRUE, FALSE or a number, got string (#VALUE!)
IF(1, 2, 3, 4)               # Error: IF expects at most 3 arguments
```

#### `IFS(condition1, value1, condition2, value2, ...)`
Value of the first true condition. Conditions follow the `IF` rules and are evaluated in order
up to the first true one, and only its value is evaluated. No true condition is an error
(Excel `#N/A`); end with `TRUE, default` to have a default.
```bash
IFS(false, "A", true, "B", true, "C")  # "B"
IFS(:score >= 90, "A", :score >= 80, "B", true, "C")
IFS(FALSE, 1)                          # Error: IFS: no condition is TRUE (#N/A)
```

#### `AND(logical1, ...)`, `OR(logical1, ...)`, `XOR(logical1, ...)`
Excel's logical functions. `AND` is TRUE when every value is TRUE, `OR` when at least one is,
and `XOR` when an odd number are. The values are booleans, and numbers (non-zero is TRUE).
Arrays are flattened; text and `NULL` inside them are ignored, as Excel ignores text and empty
cells in a range. A `NULL` argument is ignored too. Text passed directly is an error (`#VALUE!`),
and so is having no logical value at all (`AND()`, `AND([])`, `AND(NULL)`).

Unlike the `AND`/`OR` operators, these functions evaluate every argument, as Excel does:
`AND(FALSE, 1/0)` is an error, while `FALSE AND 1/0` is `FALSE`.

> **Warning: a missing flag passes `AND(...)`.** The functions skip `NULL`, and `&.` gives `NULL`
> for a missing key, so `AND(:u&.is_admin, :on)` is `TRUE` when `is_admin` is absent. Everywhere
> else `NULL` is `FALSE`. For a permission or feature check, use the operator
> (`:u&.is_admin AND :on` is `FALSE`), `IF`, or an explicit comparison
> (`AND(:u&.is_admin == TRUE, :on)`).
```bash
AND(true, true, false)       # false
AND(TRUE, 1)                 # true
OR(false, false, true)       # true
AND([TRUE, "x", NULL])       # true (text and NULL in arrays are ignored)
AND(TRUE, "x")               # Error: AND expects logical values, got string (#VALUE!)
OR()                         # Error: OR: no logical values (#VALUE!)
XOR(true, false)             # true
XOR(true, true)              # false
XOR(TRUE, TRUE, TRUE)        # true (three are TRUE)
```

#### `NOT(logical)`
Logical negation, as in Excel: `TRUE` or a non-zero number gives `FALSE`; `FALSE`, `0` or `NULL`
gives `TRUE`. Text or an array is an error (`#VALUE!`). Also written as a prefix keyword:
`NOT x` (see Logical Operators).
```bash
NOT(true)                    # false
NOT(false)                   # true
NOT(0)                       # true
NOT TRUE                     # false
NOT("x")                     # Error: NOT expects a logical value, got string (#VALUE!)
```

### Text Functions

#### `LEN(text)`, `LENGTH(text_or_array)`
String length in characters. `LEN` is Excel's: numbers and booleans count as their text and NULL
(an empty cell) is 0; an array is an error. `LENGTH` also counts array items, and is an error for
numbers.
```bash
LEN("Hello")                 # 5
LEN(12345)                   # 5
LEN(NULL)                    # 0
LENGTH("Hello World")        # 11
LENGTH([1, 2, 3])            # 3
```

#### `TEXTJOIN(delimiter, ignore_empty, text1, ...)`
Excel's `TEXTJOIN`: joins the texts (arrays are flattened) with `delimiter`. With `ignore_empty`
TRUE, empty strings and NULLs are skipped. Numbers and booleans become text, and a DateTime its
ISO 8601 text (`2026-01-01T00:00:00Z`). An array delimiter is used in turn between items. At
least one text is required.
```bash
TEXTJOIN(", ", TRUE, "a", "", ["b", "c"])     # "a, b, c"
TEXTJOIN(",", FALSE, "a", "", "b")            # "a,,b"
TEXTJOIN([",", ";"], TRUE, "a", "b", "c")     # "a,b;c"
```

#### `FIND(find_text, within_text, [start_num])`
When the first argument is text, `FIND` is Excel's: the 1-based position of the first
case-sensitive match at or after `start_num` (default 1). Not found, or a `start_num` below 1 or
past the end of `within_text`, is an error. With an array first, see
[`FIND(array, expression)`](#findarray-expression).
```bash
FIND("M", "Miriam McGovern")       # 1
FIND("m", "Miriam McGovern")       # 6
FIND("M", "Miriam McGovern", 3)    # 8
FIND("x", "abc")                   # Error: FIND: find_text not found in within_text
```

#### `CONCAT(...)`
Join every value as text, as Excel does. Numbers are written in Skillet's number text
(`1.5::Currency` is `"1.5"`), booleans as `TRUE`/`FALSE`, and a DateTime as its `::String` text,
ISO 8601 UTC (ADR-0031). Arrays are flattened. `NULL` and JSON objects add nothing. The `&`
operator uses the same rules.
```bash
CONCAT("Hello", " ", "World")          # "Hello World"
CONCAT("Value: ", 42)                  # "Value: 42"
CONCAT("Total: ", 9.5::Currency)       # "Total: 9.5"
CONCAT("On ", DATE(2026, 1, 1))        # "On 2026-01-01T00:00:00Z"
```

#### `SUBSTRING(text, start, length)`
Extract substring (0-based indexing).
```bash
SUBSTRING("Hello World", 6, 5)  # "World"
SUBSTRING("Hello", 1, 3)        # "ell"
```

#### `LEFT(text, [num_chars])`, `RIGHT(text, [num_chars])`
Extract from left/right side, as in Excel. `num_chars` defaults to 1 and is truncated; more than the
text has returns the whole text; a negative `num_chars` is an error (Excel `#VALUE!`).
```bash
LEFT("Hello", 2)             # "He"
RIGHT("Hello", 3)            # "llo"
LEFT("Hello")                # "H" (default: 1 char)
LEFT("Hello", 10)            # "Hello"
LEFT("Hello", -1)            # Error: LEFT num_chars must not be negative (#VALUE!)
```

#### `MID(text, start, [num_chars])`
Extract substring (1-based indexing, Excel-style). A `start` past the end gives `""`, and a
`num_chars` past the end stops at the end. A `start` below 1 or a negative `num_chars` is an error
(Excel `#VALUE!`). Leaving out `num_chars` (rest of the text) is a Skillet extension.
```bash
MID("Hello", 2, 3)           # "ell" (1-based start)
MID("Hello", 3)              # "llo" (rest of string)
MID("Hello", 10, 2)          # ""
MID("Hello", 0, 2)           # Error: MID start_num must be at least 1 (#VALUE!)
MID("Hello", 2, -1)          # Error: MID num_chars must not be negative (#VALUE!)
```

#### `SUBSTITUTE(text, old_text, new_text, [instance_num])`, `SUBSTITUTEM(text, substr, replacement)`
Replace every occurrence of `old_text` with `new_text`, or, with `instance_num`, only that
occurrence (1-based, as in Excel). An empty `old_text` returns the text unchanged. Matching is
case-sensitive. `SUBSTITUTEM` replaces every occurrence and takes no `instance_num`.

`SUBSTITUTEM` is **deprecated since 0.7.0** and will be removed in a later minor release: use
`SUBSTITUTE`, which gives the same result for three arguments. It still works and gives no warning.
```bash
SUBSTITUTE("foo bar foo", "foo", "baz")   # "baz bar baz"
SUBSTITUTE("a-a-a", "-", "_")             # "a_a_a"
SUBSTITUTE("a-b-c", "-", "+", 2)          # "a-b+c"
SUBSTITUTEM("a-a-a", "-", "_")            # "a_a_a" (deprecated)
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
Excel-style positional replace using 1-based `start_num` and character counts. A `start_num` past
the end appends `new_text`; a `num_chars` past the end stops at the end. A `start_num` below 1 or a
negative `num_chars` is an error (Excel `#VALUE!`).
```bash
REPLACE("abcdef", 3, 2, "XY")             # "abXYef"
REPLACE("abc", 1, 0, "X")                 # "Xabc" (insert)
REPLACE("hello", 4, 10, "X")              # "helX" (clamped)
REPLACE("abc", 10, 1, "X")                # "abcX" (appended)
REPLACE("abc", 0, 1, "X")                 # Error: REPLACE start_num must be at least 1 (#VALUE!)
```

#### `ISNUMBER(value)`, `ISTEXT(value)`
Type checking.
```bash
ISNUMBER(42)                 # true
ISTEXT("hello")              # true
ISNUMBER("hello")            # false
```

### Date/Time Functions

"Local" below means the process's time zone: the `TZ` environment variable, else the system
setting (`/etc/localtime`), else UTC. `NOW()` and the date arithmetic work in UTC.

#### `NOW()`
Current timestamp (Unix time).
```bash
NOW()                        # Current timestamp
```

#### `DATE(year, month, day)`, `DATE()`, `TODAY()`
Midnight UTC of a date, with Excel's rules: arguments are truncated, months and days outside
their range roll over, and years 0-1899 get 1900 added (`DATE(26, 1, 1)` is 1926-01-01). A year
below 0 or above 9999, or a result outside 1900-01-01..9999-12-31, is an error. `DATE()` and
`TODAY()` return today's local date at midnight.
```bash
DATE(2023, 12, 25)           # DateTime for 2023-12-25
DATE(2026, 13, 1)            # 2027-01-01
DATE(2026, 2, 30)            # 2026-03-02
DATE(2026, 1, 0)             # 2025-12-31 (day 0 = last day of the previous month)
DATE(2024, 2 + 1, 0)         # 2024-02-29 (end of month)
TODAY()                      # today, midnight
```

String casts stay strict: `"2026-02-30"::DateTime` is an error. See
[Dates as text](#dates-as-text) for the text a DateTime becomes and the text `::DateTime` reads.

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

#### Date units (B-15)
Unit codes follow Excel and ignore case. `DATEADD` and `DATEDIFF` take the codes of VBA's
`DateAdd` and `DateDiff`, the closest Excel functions; `DATEDIF` takes the codes of Excel's
worksheet `DATEDIF`. `"m"` is **months** everywhere; minutes are `"n"`.

| Unit | `DATEADD` / `DATEDIFF` (VBA) | Long word (Skillet) | `DATEDIF` (worksheet) |
|---|---|---|---|
| years | `"yyyy"` | `"years"`, `"year"` | `"y"` |
| quarters | `"q"` | | |
| months | `"m"` | `"months"`, `"month"` | `"m"` |
| days | `"d"`, `"y"` (day of year), `"w"` (weekday: adds days; `DATEDIFF` counts whole weeks) | `"days"`, `"day"` | `"d"` |
| weeks | `"ww"` (`DATEDIFF` counts Sundays) | `"weeks"`, `"week"` (whole weeks) | |
| hours, minutes, seconds | `"h"`, `"n"`, `"s"` | `"hours"`, `"minutes"`, `"seconds"` (and singular) | |
| ignoring larger units | | | `"md"`, `"ym"`, `"yd"` |

An unknown unit is an error: `#VALUE!` in `DATEADD` and `DATEDIFF` (VBA rejects it), `#NUM!` in
`DATEDIF` (as Excel).

#### `DATEADD(date, amount, unit)`
Add `amount` units to a date, like VBA `DateAdd`. `amount` is truncated. Months, quarters and
years keep the day, clamped to the end of a shorter month. A result out of range is `#NUM!`.
```bash
DATEADD(NOW(), 30, "days")              # 30 days from now
DATEADD(DATE(2024, 1, 31), 1, "m")      # 2024-02-29 (months; clamped)
DATEADD(DATE(2026, 1, 1), 90, "n")      # 2026-01-01 01:30 (minutes)
DATEADD(DATE(2026, 1, 1), 1, "q")       # 2026-04-01
DATEADD(NOW(), 1, "yyyy")               # 1 year from now
```

#### `DATEDIFF(date1, date2, unit)`
`date2 - date1` in VBA `DateDiff` units. `"yyyy"`, `"q"` and `"m"` count calendar boundaries, as
in VBA (`DATEDIFF(DATE(2025, 12, 31), DATE(2026, 1, 1), "yyyy")` is `1`), and `"ww"` counts the
Sundays after `date1` up to `date2`. The other units count whole elapsed units (VBA counts day,
hour, minute and second boundaries; for dates at midnight the result is the same).
```bash
DATEDIFF(NOW(), DATEADD(NOW(), 7, "days"), "days")          # 7
DATEDIFF(DATE(2026, 1, 31), DATE(2026, 2, 1), "m")          # 1
```

#### `DATEDIF(start_date, end_date, unit)`
Excel's `DATEDIF`: the number of **complete** units from `start_date` to `end_date`. Times are
ignored. `start_date` after `end_date` is `#NUM!`.
```bash
DATEDIF(DATE(2001, 1, 1), DATE(2003, 1, 1), "Y")     # 2
DATEDIF(DATE(2001, 6, 1), DATE(2002, 8, 15), "D")    # 440
DATEDIF(DATE(2001, 6, 1), DATE(2002, 8, 15), "YD")   # 75 (days, ignoring years)
DATEDIF(DATE(2001, 6, 1), DATE(2002, 8, 15), "YM")   # 2 (months, ignoring years)
DATEDIF(DATE(2001, 6, 1), DATE(2002, 8, 15), "MD")   # 14 (days, ignoring months and years)
DATEDIF(DATE(2026, 1, 31), DATE(2026, 2, 1), "m")    # 0 (DATEDIFF gives 1)
```
Like Excel, `"md"` is `end_date - DATE(YEAR(end_date), MONTH(end_date) - 1, DAY(start_date))`,
which can be negative: `DATEDIF(DATE(2011, 1, 31), DATE(2011, 3, 1), "md")` is `-2`.

### Array Functions

#### `FILTER(array, expression)`
Filter array elements: an item is kept when `expression` is a true condition
([Conditions](#conditions)): `TRUE` or a non-zero number. `FALSE`, 0 and `NULL` drop the item;
text, a date or an array is an error for the whole call.
```bash
FILTER([1,2,3,4,5], :x > 3)   # [4, 5]
FILTER(data, :x.active == true)
FILTER([0, 1, 2], :x)         # [1, 2]
FILTER([1, 2], "yes")         # Error: FILTER condition must be TRUE, FALSE or a number, got string (#VALUE!)
```

#### `MAP(array, expression)`
Transform array elements.
```bash
MAP([1,2,3], :x * 2)          # [2, 4, 6]
MAP(users, :x.name.upper())
```

#### `FIND(array, expression)`
Find the first element for which `expression` is a true condition (`NULL` when none); the
condition rule is the one of `FILTER`. Items after the first match are not evaluated. With text
first, `FIND` is Excel's text search
(see [Text Functions](#findfind_text-within_text-start_num)).
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
the item at the same position is summed; only numbers are summed or averaged, and a matched date
that would be summed is an error (`SUMIF does not accept dates (#VALUE!)`).
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
| `DATE(2026,1,1)`, `">" & DATE(2026,1,1)`, `">=2026-01-01T00:00:00Z"` | dates by instant, with any operator; also text items in that exact ISO form. The operand must be the ISO text `::DateTime` reads: date-only text (`">2026-01-01"`) is text and matches no date. `"<>…"` also matches every non-date |
| `:x > 2` (lambda) | items for which the expression is a true condition, as in `IF`: `TRUE` or a non-zero number; `FALSE`, 0 and `NULL` do not match; text, a date or an array is an error |

```bash
COUNTIF([1, 2, 3, 4], ">2")                       # 2
COUNTIF([1, 2, 3], 2)                             # 1
COUNTIF(["A", "b", "a"], "a")                     # 2 (case-insensitive)
COUNTIF(["apple", "Avocado", "kiwi"], "a*")       # 2
COUNTIF(5, ">2")                                  # 1 (a single value is a one-item range)
COUNTIF([1, 2, 3, 4], :x % 2 == 0)                # 2 (lambda)
COUNTIF([0, 1, 2], :x)                            # 2 (non-zero numbers match)
COUNTIF(["a"], :x)                                # Error: COUNTIF condition must be TRUE, FALSE or a number, got string (#VALUE!)
COUNTIF([DATE(2026,1,1), DATE(2027,1,1)], ">" & DATE(2026,6,1))  # 1
COUNTIF([DATE(2026,1,1), DATE(2027,1,1)], DATE(2026,1,1))        # 1

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
Range checking, both bounds inclusive. Three numbers, or three dates (compared by instant).
```bash
BETWEEN(10, 20, 15)          # true
BETWEEN(1, 10, 15)           # false
BETWEEN(DATE(2026,1,1), DATE(2027,1,1), DATE(2026,6,1))  # true
```

#### `FLATTEN(array)`
Flatten nested arrays.
```bash
FLATTEN([1,[2,[3]],4])       # [1, 2, 3, 4]
```

#### `UNIQUE(array)`
Remove duplicates, keeping values of every type in first-seen order. Two values are duplicates when
`==` says so: same type and value, text case-sensitive (ADR-0023).
```bash
UNIQUE([1,2,2,3,3,3])        # [1, 2, 3]
UNIQUE(["a", "a", 1])        # ["a", 1]
UNIQUE(["a", "A"])           # ["a", "A"]
```

#### `SORT(array, [order])`
Sort ascending, or descending with `order` `-1` (Excel) or `"DESC"` (Skillet). As in Excel, numbers
come before text and text before booleans; text ignores case, and `FALSE` comes before `TRUE`.
Dates take the place of numbers and sort by instant; an array with both dates and numbers is an
error. Equal items keep their order. `NULL`, arrays, JSON objects and NaN are an error.
```bash
SORT([3, 1, 2])                    # [1, 2, 3]
SORT([3, 1, 2], -1)                # [3, 2, 1]
SORT(["banana", "Apple", "cherry"])  # ["Apple", "banana", "cherry"]
SORT([TRUE, "b", 2, "A", 1])       # [1, 2, "A", "b", TRUE]
SORT([DATE(2027,1,1), "a", DATE(2026,1,1)])  # [2026-01-01, 2027-01-01, "a"]
SORT([DATE(2026,1,1), 5])          # Error: SORT cannot compare dates with numbers (#VALUE!)
```

#### `REVERSE(array_or_text)`
Items of an array, or characters of a string, in reverse order.
```bash
REVERSE([1, 2, 3])           # [3, 2, 1]
REVERSE("abc")               # "cba"
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

Results follow Excel's sign convention: money you pay is negative. The examples above return
-536.82, -460.41 and -305.10.

#### `PPMT(rate, per, nper, pv, [fv], [type])`
Principal part of payment `per` (1..`nper`), Excel's `PPMT`. `per` outside 1..`nper` is an error.
```bash
PPMT(0.1/12, 1, 24, 2000)      # -75.62
PPMT(0.08, 10, 10, 200000)     # -27598.05
```

#### `PV(rate, nper, pmt, [fv], [type])`
Present value of a series of equal payments, Excel's `PV`.
```bash
PV(0.08/12, 12*20, 500)        # -59777.15
PV(0.05, 10, -100, 0, 1)       # 810.78 (payments at the start of each period)
```

#### `NPV(rate, value1, ...)`
Net present value of cash flows at the end of each period, Excel's `NPV`: `Σ value_i / (1 + rate)^i`.
Arrays are flattened, and only numbers count. No numbers is an error.
```bash
NPV(0.1, -10000, 3000, 4200, 6800)                  # 1188.44
NPV(0.08, [8000, 9200, 10000, 12000, 14500]) - 40000  # 1922.06
```

#### `DB(cost, salvage, life, period, [month])`
Fixed-declining-balance depreciation for one period, Excel's `DB`. The rate is
`1 - (salvage / cost)^(1 / life)`, rounded to 3 decimals. `month` (1-12, default 12) is the number
of months in the first year; when it is less than 12 there is a last period `life + 1` for the
remaining months. A cost of 0 or less, a negative salvage, a life of 0 or less, a `month` outside
1-12, or a `period` outside 1..`life` (1..`life + 1` when `month` < 12) is an error (Excel `#NUM!`).
```bash
DB(1000000, 100000, 6, 1, 7)   # 186083.33
DB(1000000, 100000, 6, 6, 7)   # 55841.76
DB(1000000, 100000, 6, 7, 7)   # 15845.10
DB(0, 0, 5, 1)                 # Error: DB needs cost > 0, salvage >= 0 and life > 0 (#NUM!)
```

Optional arguments of `PPMT`, `PV` and `NPV` must be numbers (or NULL); other types are an error.

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
Sort an array of numbers, or an array of dates (by instant). Anything else, a mix of the two
included, is `Error: sort expects an array of numbers or of dates`.
```bash
[3,1,2].sort()               # [1, 2, 3]
[3,1,2].sort("DESC")         # [3, 2, 1]
[DATE(2027,1,1), DATE(2026,1,1)].sort()  # [2026-01-01, 2027-01-01]
```

#### `.sum()`, `.avg()`, `.min()`, `.max()`
Numeric operations. `.min()` and `.max()` also take an array of dates and return a date; an empty
array gives `NULL`. `.sum()` and `.avg()` take numbers only.
```bash
[1,2,3,4,5].sum()            # 15
[1,2,3,4,5].avg()            # 3.0
[5,1,9].min()                # 1
[5,1,9].max()                # 9
[DATE(2027,1,1), DATE(2026,1,1)].max()  # DateTime for 2027-01-01
```

#### `.join([separator])`
Join to string. Strings, numbers, booleans (`true`/`false`), DateTimes (as ISO 8601 text),
and `NULL` (as nothing, as `JOIN`) join; arrays and JSON values
are an error.
```bash
[1,2,3].join("-")            # "1-2-3"
["a","b","c"].join()         # "a,b,c" (default: comma)
[DATE(2026,1,1)].join()      # "2026-01-01T00:00:00Z"
[1, 2.5, NULL].join(",")   # "1,2.5,"
[TRUE, 1].join()             # "true,1" (JOIN gives "TRUE,1")
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
Filter elements. Same condition rule as `FILTER`: `[0, 1, 2].filter(:x)` is `[1, 2]`.
```bash
[1,2,3,4,5].filter(:x > 3)   # [4, 5]
```

#### `.map(expression)`
Transform elements.
```bash
[1,2,3].map(:x * 2)          # [2, 4, 6]
```

#### `.find(expression)`
Find first match. Same condition rule as `FIND`; an error for an item before the first match is
the result.
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

**Result shape.** The path decides the shape, never the data ([ADR-0014](../adr/0014-jq-wildcard-results.md), RFC 9535 §2.3.5.1):

| Path | Result |
|---|---|
| Singular: only `.name`, `['name']` and `[n]` steps (no `[*]`, `..`, slice, filter or union) | The matched value, or `NULL` when nothing matches |
| Any other path (`[*]`, `..`, `[a:b]`, `[?(...)]`, `[a,b]`) | An array of every match, possibly `[]`, also for a single match |

```
JQ({"u":{"n":"Ann"}}, "$.u.n")            # "Ann"
JQ({"a":5}, "$.b")                        # NULL
JQ({"a":[3]}, "$.a")                      # [3]  (the value itself is an array)
JQ({"a":[3]}, "$.a[*]")                   # [3]
JQ({"a":[]}, "$.a[*]")                    # []
JQ({"a":[{"p":1}]}, "$..p")               # [1]
JQ({"a":[3]}, "$.a[*]").sum()             # 3
JQ({"a":[3,4]}, "$.a[*]").first()         # 3  (first match of a wildcard)
```

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
value :: type_name :: type_name     # chained: (value::type_name)::type_name
```
Casts chain left to right (ADR-0031 amendment 1): `DATE(2026, 1, 1)::String::DateTime` is
`(DATE(2026, 1, 1)::String)::DateTime`. A cast applies to the whole `.method()`/`.prop`/`&.`/`[i]`
chain before it (`:u&.name::String` is `(:u&.name)::String`), and a prefix `-`, `+` or `!` applies
to the whole cast chain (`-"2"::String::Integer` is `-2`). A method, property or index after a
cast needs parentheses: `(1::String).length()` is `1`, while `1::String.length()` is an error.
An unknown type name is `Error: Unknown cast type at position N`, N being where the name starts.

### Available Types
- `Integer` - Convert to integer
- `Float` - Convert to float
- `String` - Convert to string
- `Boolean` - Convert to boolean
- `Array` - Convert to array
- `Currency` - Round to 4 decimals; the result is a Number (ADR-0033)
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

### Number conversions
Skillet has one number type, as Excel does; integer and currency are conversions, not types
(ADR-0033). `::Integer`, `::Float` and `::Currency` all return a Number.

- `::Integer` truncates toward zero, like Excel `TRUNC` (not `INT`, which rounds down:
  `INT(-7.9)` is `-8`). Text is read up to the first character that is not part of a number, and
  text with no number is `0`.
- `::Float` keeps the number; text must be a number (`"12.5"`, `"1e3"`) with no spaces around it,
  or it is an error.
- `::Currency` reads its input as `::Float` does, then rounds to 4 decimals, half away from zero,
  exactly as `ROUND(x, 4)` (so `0.00005` becomes `0.0001`, where VBA's `CCur` gives `0`). A `-0`
  result is `0`. Later arithmetic is not rounded again.
- For all three, `TRUE`/`FALSE`/`NULL` are `1`/`0`/`0`, and a DateTime, array or JSON object is an
  error.

```bash
-7.9::Integer                # -7 (the cast applies first, then the minus)
(0 - 7.9)::Integer           # -7
"+3.9x"::Integer             # 3
"abc"::Integer               # 0
"12.5"::Float                # 12.5
" 12.5 "::Float              # Error: Cannot cast String to Float
1.23456789::Currency         # 1.2346
(0.1 + 0.2)::Currency        # 0.3
"$5"::Currency               # Error: Cannot cast String to Currency
(2.5::Currency)::String      # "2.5"
```

### Dates as text
A DateTime becomes ISO 8601 text in UTC with whole seconds (ADR-0031):
`DATE(2026, 1, 1)::String` is `"2026-01-01T00:00:00Z"`. `::String`, `.to_s()`, `CONCAT`, `&`,
`JOIN`, `.join()`, `TEXTJOIN` and the string inside `.to_json()` all write this text. A year
outside 0000-9999 gets a sign (`"+10000-01-01T00:00:00Z"`).

`::DateTime` reads that exact form back, and integer text as Unix seconds:
```bash
DATE(2026, 1, 1)::String::DateTime == DATE(2026, 1, 1)     # true
"2026-01-01T10:30:00Z"::DateTime                           # DateTime for 2026-01-01 10:30 UTC
"1767225600"::DateTime                                     # DateTime for 2026-01-01
"2026-01-01"::DateTime                                     # Error: Cannot cast String to DateTime
```
Other forms (a date alone, an offset, fractional seconds, a space instead of `T`) are errors.
The `::Json` cast and the server responses still write a DateTime as its Unix seconds.

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

## Rust library: evaluating a parsed expression

`skillet::parse` returns an `Expr`. Each text entry point has a twin that takes that `Expr`, so a
formula that runs many times with different variables is parsed once:

| Text | Parsed |
|---|---|
| `evaluate(&str)` | `evaluate_parsed(&Expr)` |
| `evaluate_with(&str, &vars)` | `evaluate_parsed_with(&Expr, &vars)` |
| `evaluate_with_custom(&str, &vars)` | `evaluate_parsed_with_custom(&Expr, &vars)` |
| `evaluate_with_assignments(&str, &vars)` | `evaluate_parsed_with_assignments(&Expr, &vars)` |
| `evaluate_with_assignments_and_context(&str, &vars)` | `evaluate_parsed_with_assignments_and_context(&Expr, &vars)` |

`vars` is a `HashMap<String, Value>`. `evaluate_x(text, …)` is exactly
`evaluate_parsed_x(&parse(text)?, …)`: same result, same errors. The JSON-variable functions have
no twin; build the map with `json_to_value` instead.

```rust
use std::collections::HashMap;
use skillet::{evaluate_parsed_with, parse, Value};

let expr = parse(":price * :qty")?;
for price in [2.5, 3.0] {
    let mut vars = HashMap::new();
    vars.insert("price".to_string(), Value::Number(price));
    vars.insert("qty".to_string(), Value::Number(4.0));
    println!("{:?}", evaluate_parsed_with(&expr, &vars)?);
}
```

`parse` rejects tokens after the expression (`Unexpected token after end of expression`, see
[End of input](#end-of-input)). Limits: `parse` rejects input nested deeper than 128 (`Expression nested too deeply (max 128)`),
and every `evaluate_parsed*` call gets its own evaluation budget and the deadline of an enclosing
`with_limits`, as a text evaluation does ([limits](../guide/limits.md)). Pass trees that came from
`parse`: a hand-built `Expr` is not depth-checked. `Expr` uses `Rc`, so a parsed expression stays
on the thread that parsed it (it is not `Send`); cache one per thread, as the servers do
([servers](../guide/servers.md#parse-cache)). Decision:
[ADR-0021](../adr/0021-parsed-expression-api-and-server-cache.md).

---

This reference covers all available functions, methods, and operators in Skillet. For practical examples and usage patterns, see the main [user guide](../guide/user-guide.md) file.
