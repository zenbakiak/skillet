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

#### `AVERAGE(...)`
Average of all arguments.
```bash
AVERAGE(1, 2, 3, 4, 5)       # 3.0
AVG(10, 20, 30)              # 20.0 (alias)
```

#### `MAX(...)`, `MIN(...)`
Maximum/minimum value from arguments.
```bash
MAX(1, 5, 3, 9, 2)           # 9
MIN([1, 5, 3, 9, 2])         # 1 (works with arrays too)
```

#### `COUNT(...)`
Count of numeric arguments (non-numeric values are ignored).
```bash
COUNT(1, "hello", 3, 4)      # 3
COUNT([1, 2, 3, 4, 5])       # 5
```

#### `ABS(number)`
Absolute value.
```bash
ABS(-5)                      # 5
ABS(3.14)                    # 3.14
```

#### `ROUND(number, [digits])`
Round to specified decimal places.
```bash
ROUND(3.14159)               # 3 (default: 0 digits)
ROUND(3.14159, 2)            # 3.14
ROUND(3.14159, 4)            # 3.1416
```

#### `CEILING(number)`, `FLOOR(number)`
Round up/down to nearest integer.
```bash
CEILING(3.2)                 # 4
FLOOR(3.8)                   # 3
```

#### `INT(number)`
Get integer part (truncate).
```bash
INT(3.9)                     # 3
INT(-3.9)                    # -3
```

#### `MOD(dividend, divisor)`
Modulo operation.
```bash
MOD(10, 3)                   # 1
MOD(17, 5)                   # 2
```

#### `POWER(base, exponent)`
Exponentiation.
```bash
POWER(2, 8)                  # 256
POW(3, 4)                    # 81 (alias)
```

#### `SQRT(number)`
Square root.
```bash
SQRT(16)                     # 4
SQRT(2)                      # 1.414...
```

### Statistical Functions

#### `STDEV.P(...)`, `VAR.P(...)`
Population standard deviation and variance.
```bash
STDEV_P(2, 4, 4, 4, 5, 5, 7, 9)    # Population std dev
VAR_P(2, 4, 4, 4, 5, 5, 7, 9)      # Population variance
```

#### `MEDIAN(...)`
Median value.
```bash
MEDIAN(1, 2, 3, 4, 5)        # 3
MEDIAN(1, 2, 3, 4)           # 2.5
```

#### `MODE.SNGL(...)`
Most frequently occurring value.
```bash
MODE_SNGL(1, 2, 2, 3, 4, 4, 4)     # 4
```

#### `PERCENTILE.INC(array, k)`
k-th percentile (0 ≤ k ≤ 1).
```bash
PERCENTILE_INC([1,2,3,4,5], 0.5)   # 3 (50th percentile)
PERCENTILE_INC([1,2,3,4,5], 0.25)  # 2 (25th percentile)
```

#### `QUARTILE.INC(array, quart)`
Quartile value (1=Q1, 2=Q2, 3=Q3).
```bash
QUARTILE_INC([1,2,3,4,5,6,7,8], 1) # 2.75 (Q1)
QUARTILE_INC([1,2,3,4,5,6,7,8], 2) # 4.5 (Q2/median)
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

#### `SUBSTITUTE(text, substr, replacement)`, `SUBSTITUTEM(text, substr, replacement)`
Replace all occurrences of `substr` with `replacement`. `SUBSTITUTEM` is an alias that performs the same operation (substitute multiple occurrences).
```bash
SUBSTITUTE("foo bar foo", "foo", "baz")   # "baz bar baz"
SUBSTITUTE("a-a-a", "-", "_")             # "a_a_a"
SUBSTITUTEM("a-a-a", "-", "_")            # "a_a_a"
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

#### `TIME(hour, minute, second)`
Seconds since midnight.
```bash
TIME(14, 30, 0)              # 52200 (2:30 PM in seconds)
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

#### `SUMIF(array, condition [, sum_array])`
Conditional sum. Supports both lambda expressions and Excel-style criteria.
```bash
# Lambda-style (existing)
SUMIF([1, -2, 3, -4], :x > 0)      # 4

# Excel-style criteria
SUMIF([10, 20, 30, 40], ">25")     # 70
SUMIF([10, 20, 30, 40], "=20")     # 20
SUMIF([10, 20, 30, 40], "<>20")    # 80
SUMIF([10, 20, 30, 40], ">=20")    # 90
SUMIF([10, 20, 30, 40], "<=20")    # 30

# With separate sum array
SUMIF([10, 30, 50], ">20", [1, 2, 3])  # 5 (sums 2+3)
```

#### `AVGIF(array, condition)`
Conditional average.
```bash
AVGIF([1, 3, 5, -1], :x > 0)       # 3.0
```

#### `COUNTIF(array, condition)`
Conditional count.
```bash
COUNTIF([1,2,3,4], :x % 2 == 0)    # 2
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

This reference covers all available functions, methods, and operators in Skillet. For practical examples and usage patterns, see the main [DOCUMENTATION.md](DOCUMENTATION.md) file.
