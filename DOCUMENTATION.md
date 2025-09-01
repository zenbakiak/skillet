# Skillet Documentation

**Skillet**: A micro expression language for arithmetic, logical operations, and Excel-like functions with custom function extensibility.

## Table of Contents

1. [Command Line Interface (CLI)](#command-line-interface-cli)
2. [Rust Integration](#rust-integration)
3. [Extending the Language](#extending-the-language)
4. [JavaScript/Node Addon](#javascriptnode-addon)
5. [API Surface](#api-surface)
6. [Excel Formula Examples](#excel-formula-examples)
7. [Built-in Functions Reference](#built-in-functions-reference)
8. [JSON Integration](#json-integration)
9. [Advanced Features](#advanced-features)

---

## Command Line Interface (CLI)

### Installation

```bash
git clone <repository-url>
cd skillet
cargo build --release
```

### Basic Usage

```bash
# Basic arithmetic
sk "2 + 3 * 4"
# Output: Number(14.0)

# With leading equals (Excel-style)
sk "= 2 + 3 * 4"
# Output: Number(14.0)

# Complex expressions
sk "(10 + 20) * 3 / 2"
# Output: Number(45.0)
```

### Using Variables

#### Key-Value Format
```bash
# Single variable
sk "=:price * :quantity" price=19.99 quantity=3
# Output: Number(59.97)

# Multiple variables
sk "=SUM(:sales, :bonus)" sales=5000 bonus=1000
# Output: Number(6000.0)

# String variables
sk "=:name.upper()" name="hello world"
# Output: String("HELLO WORLD")

# Boolean variables
sk "=IF(:active, :price, 0)" active=true price=100
# Output: Number(100.0)

# Array variables
sk "=:numbers.sum()" numbers=[1,2,3,4,5]
# Output: Number(15.0)
```

#### JSON Format
```bash
# Basic JSON variables
sk "=SUM(:sales, :bonus)" --json '{"sales": 5000, "bonus": 1000}'
# Output: Number(6000.0)

# Complex JSON with nested objects
sk "=:user.name.upper()" --json '{"user": {"name": "alice"}}'
# Output: String("ALICE")

# Arrays in JSON
sk "=:numbers.length()" --json '{"numbers": [1, 2, 3, 4, 5]}'
# Output: Number(5.0)

# Mixed data types
sk "=IF(:config.enabled, :config.price * :config.quantity, 0)" --json '{
  "config": {
    "enabled": true,
    "price": 25.50,
    "quantity": 4
  }
}'
# Output: Number(102.0)
```

### Error Handling

```bash
# Syntax errors
sk "2 + + 3"
# Output: Error: Unexpected token

# Missing variables
sk "=:missing_var + 5"
# Output: Error: Missing variable: :missing_var

# Type errors
sk "=5 + \"hello\""
# Output: Error: Cannot add number and string
```

---

## Rust Integration

### Adding Skillet to Your Project

Add to your `Cargo.toml` from crates.io:

```toml
[dependencies]
skillet = "0.0.1"
```

## JavaScript/Node Addon

See `skillet-node/README.md` for the Node.js addon built with napi-rs. It exposes `evalFormula`, `evalFormulaWith`, and async custom-function support. This addon is packaged separately for npm.

## API Surface

- `parse(&str) -> Result<Expr, Error>`
- `evaluate(&str) -> Result<Value, Error>`
- `evaluate_with(&str, &HashMap<String, Value>) -> Result<Value, Error>`
- `evaluate_with_json(&str, &str) -> Result<Value, Error>`
- `evaluate_with_custom(&str, &HashMap<String, Value>) -> Result<Value, Error>`
- `evaluate_with_json_custom(&str, &str) -> Result<Value, Error>`
- Custom functions:
  - `register_function(Box<dyn CustomFunction>) -> Result<(), Error>`
  - `unregister_function(&str) -> bool`
  - `list_custom_functions() -> Vec<String>`
- Types: `Value`, `Error`, `Expr`

### Basic Evaluation

```rust
use skillet::{evaluate, Value, Error};

fn main() -> Result<(), Error> {
    // Simple arithmetic
    let result = evaluate("2 + 3 * 4")?;
    println!("{:?}", result); // Number(14.0)

    // With functions
    let result = evaluate("SUM(1, 2, 3, 4, 5)")?;
    println!("{:?}", result); // Number(15.0)

    Ok(())
}
```

### Using Variables

```rust
use skillet::{evaluate_with, Value, Error};
use std::collections::HashMap;

fn main() -> Result<(), Error> {
    let mut vars = HashMap::new();
    vars.insert("price".to_string(), Value::Number(19.99));
    vars.insert("quantity".to_string(), Value::Number(3.0));
    vars.insert("tax_rate".to_string(), Value::Number(0.08));

    let result = evaluate_with("(:price * :quantity) * (1 + :tax_rate)", &vars)?;
    println!("{:?}", result); // Number(64.77...)

    Ok(())
}
```

### JSON Integration

```rust
use skillet::{evaluate_with_json, Value, Error};

fn main() -> Result<(), Error> {
    let json_vars = r#"{
        "user": {"name": "John", "age": 30},
        "products": [
            {"name": "laptop", "price": 999.99},
            {"name": "mouse", "price": 29.99}
        ],
        "discount": 0.1
    }"#;

    let result = evaluate_with_json(
        "=:user.name.upper()",
        json_vars
    )?;
    println!("{:?}", result); // String("JOHN")

    Ok(())
}
```

### Advanced Value Types

```rust
use skillet::{Value, evaluate_with};
use std::collections::HashMap;

fn example_values() {
    let mut vars = HashMap::new();

    // Numbers
    vars.insert("pi".to_string(), Value::Number(3.14159));

    // Strings
    vars.insert("name".to_string(), Value::String("Alice".to_string()));

    // Booleans
    vars.insert("active".to_string(), Value::Boolean(true));

    // Arrays
    vars.insert("scores".to_string(), Value::Array(vec![
        Value::Number(85.0),
        Value::Number(92.0),
        Value::Number(78.0),
    ]));

    // Null
    vars.insert("optional".to_string(), Value::Null);

    // Currency (treated as Number with special formatting)
    vars.insert("salary".to_string(), Value::Currency(75000.0));

    // DateTime (Unix timestamp)
    vars.insert("created_at".to_string(), Value::DateTime(1640995200)); // 2022-01-01
}
```

---

## Extending the Language

Skillet can be extended with custom functions in two ways:
1. **JavaScript Plugins** - Runtime extensibility without recompilation (recommended)
2. **Rust Custom Functions** - Compile-time functions for maximum performance

### JavaScript Plugins (Runtime Extension)

The easiest way to extend Skillet is by creating JavaScript functions in the `hooks` directory. These are loaded automatically when Skillet starts.

#### Quick Start

1. Create a `hooks` directory in your project
2. Add a `.js` file with your custom function:

```javascript
// hooks/double.js

// @name: DOUBLE
// @min_args: 1
// @max_args: 1
// @description: Doubles a number
// @example: DOUBLE(5) returns 10

function execute(args) {
    return args[0] * 2;
}
```

3. Use it immediately:

```bash
sk "DOUBLE(21)"
# Output: Number(42.0)
# Loaded 1 custom JavaScript function(s)
```

#### JavaScript Function Format

JavaScript functions must follow this format:

```javascript
// @name: FUNCTION_NAME        (required)
// @min_args: 1               (required - minimum arguments)
// @max_args: 2               (optional - max args, or "unlimited")
// @description: What it does  (optional - for documentation)
// @example: FUNC(5) = 10     (optional - usage example)

function execute(args) {
    // args is an array of values passed to the function
    // Return the result value
    return result;
}
```

#### JavaScript Function Examples

**Mathematical Functions:**
```javascript
// hooks/fibonacci.js
// @name: FIBONACCI
// @min_args: 1
// @max_args: 1
// @description: Calculate Fibonacci number at position n
// @example: FIBONACCI(10) returns 55

function execute(args) {
    const n = args[0];
    if (n <= 1) return n;

    let a = 0, b = 1;
    for (let i = 2; i <= n; i++) {
        let temp = a + b;
        a = b;
        b = temp;
    }
    return b;
}
```

**String Manipulation:**
```javascript
// hooks/reverse.js
// @name: REVERSE
// @min_args: 1
// @max_args: 1
// @description: Reverse a string
// @example: REVERSE("hello") returns "olleh"

function execute(args) {
    const str = args[0].toString();
    return str.split('').reverse().join('');
}
```

**Array Processing:**
```javascript
// hooks/arraysum.js
// @name: ARRAYSUM
// @min_args: 1
// @max_args: 1
// @description: Sum all numbers in an array
// @example: ARRAYSUM([1, 2, 3, 4, 5]) returns 15

function execute(args) {
    const array = args[0];
    if (!Array.isArray(array)) {
        throw new Error("ARRAYSUM expects an array as argument");
    }

    return array.reduce((sum, item) => {
        if (typeof item === 'number') {
            return sum + item;
        }
        return sum;
    }, 0);
}
```

**Variable Arguments:**
```javascript
// hooks/random.js
// @name: RANDOM
// @min_args: 0
// @max_args: 2
// @description: Generate random number
// @example: RANDOM(1, 10) returns number between 1 and 10

function execute(args) {
    if (args.length === 0) {
        return Math.random();
    } else if (args.length === 1) {
        const max = args[0];
        return Math.random() * max;
    } else {
        const min = args[0];
        const max = args[1];
        return Math.random() * (max - min) + min;
    }
}
```

**Unlimited Arguments:**
```javascript
// hooks/concat.js
// @name: CONCAT_ALL
// @min_args: 1
// @max_args: unlimited
// @description: Concatenate all arguments as strings
// @example: CONCAT_ALL("Hello", " ", "World") returns "Hello World"

function execute(args) {
    return args.map(arg => String(arg)).join('');
}
```

**Object Manipulation:**
```javascript
// hooks/object_keys.js
// @name: OBJECT_KEYS
// @min_args: 1
// @max_args: 1
// @description: Get all keys from an object as an array
// @example: OBJECT_KEYS({"name": "John", "age": 30}) returns ["name", "age"]

function execute(args) {
    const obj = args[0];

    // Handle different input types
    if (obj === null || obj === undefined) {
        return [];
    }

    // If it's already an object, get its keys
    if (typeof obj === 'object' && !Array.isArray(obj)) {
        return Object.keys(obj);
    }

    // If it's a string that looks like JSON, try to parse it
    if (typeof obj === 'string') {
        try {
            const parsed = JSON.parse(obj);
            if (typeof parsed === 'object' && !Array.isArray(parsed)) {
                return Object.keys(parsed);
            }
        } catch (e) {
            // If parsing fails, treat as regular string
            return [];
        }
    }

    // For other types, return empty array
    return [];
}
```

**Advanced Array Sorting:**
```javascript
// hooks/array_sort.js
// @name: ARRAY_SORT
// @min_args: 1
// @max_args: 2
// @description: Sort an array. Optional second argument: "asc" (default), "desc", or "numeric"
// @example: ARRAY_SORT([3, 1, 4, 1, 5]) returns [1, 1, 3, 4, 5]

function execute(args) {
    const array = args[0];
    const sortMode = args.length > 1 ? args[1] : "asc";

    if (!Array.isArray(array)) {
        throw new Error("ARRAY_SORT expects an array as first argument");
    }

    // Create a copy to avoid modifying the original
    const sortedArray = [...array];

    switch (sortMode.toLowerCase()) {
        case "desc":
            return sortedArray.sort().reverse();

        case "numeric":
            return sortedArray.sort((a, b) => {
                const numA = Number(a);
                const numB = Number(b);
                if (isNaN(numA) || isNaN(numB)) {
                    // Fall back to string comparison if not numbers
                    return String(a).localeCompare(String(b));
                }
                return numA - numB;
            });

        case "numeric_desc":
            return sortedArray.sort((a, b) => {
                const numA = Number(a);
                const numB = Number(b);
                if (isNaN(numA) || isNaN(numB)) {
                    return String(b).localeCompare(String(a));
                }
                return numB - numA;
            });

        case "asc":
        default:
            return sortedArray.sort();
    }
}
```

#### Using JavaScript Functions

Once you have JavaScript files in the `hooks` directory, they're automatically loaded:

```bash
# Basic usage
sk "DOUBLE(5)"
sk "FIBONACCI(10)"
sk "REVERSE(\"hello world\")"

# Array manipulation
sk "ARRAY_SORT([3, 1, 4, 1, 5])"                    # [1, 1, 3, 4, 5]
sk "ARRAY_SORT([10, 2, 30, 4], \"numeric\")"        # [2, 4, 10, 30]
sk "ARRAY_SORT([\"zebra\", \"apple\"], \"desc\")"   # ["zebra", "apple"]

# Object manipulation
sk "OBJECT_KEYS(:user)" --json '{"user": {"name": "John", "age": 30}}'  # ["age", "name"]

# With variables
sk "DOUBLE(:x)" x=21
sk "ARRAYSUM(:numbers)" numbers="[1,2,3,4,5]"
sk "ARRAY_SORT(:data)" data="[5,2,8,1]"

# JSON mode with complex objects
sk "OBJECT_KEYS(:config)" --json '{"config": {"host": "localhost", "port": 3000, "ssl": true}}'
sk "ARRAY_SORT(:scores, \"numeric_desc\")" --json '{"scores": [85, 92, 78, 96]}'
```

#### Configuration

By default, Skillet looks for JavaScript functions in the `hooks` directory. You can customize this:

```bash
# Use a different directory
export SKILLET_HOOKS_DIR="/path/to/my/functions"
sk "DOUBLE(5)"
```

#### Error Handling in JavaScript

JavaScript functions can throw errors that will be caught by Skillet:

```javascript
// @name: VALIDATE_POSITIVE
// @min_args: 1
// @max_args: 1

function execute(args) {
    const num = args[0];
    if (typeof num !== 'number') {
        throw new Error("Expected a number");
    }
    if (num <= 0) {
        throw new Error("Number must be positive");
    }
    return num;
}
```

#### JavaScript Value Types

The `args` array contains JavaScript representations of Skillet values:
- Numbers: `42`, `3.14`
- Strings: `"hello"`
- Booleans: `true`, `false`
- Arrays: `[1, 2, 3]`
- Null: `null`

Your function should return one of these types.

### Rust Custom Functions (Compile-time Extension)

For maximum performance or when you need access to Rust libraries, implement the `CustomFunction` trait:

```rust
use skillet::{CustomFunction, Value, Error, register_function};

// Simple custom function
struct DoubleFunction;

impl CustomFunction for DoubleFunction {
    fn name(&self) -> &str { "DOUBLE" }
    fn min_args(&self) -> usize { 1 }
    fn max_args(&self) -> Option<usize> { Some(1) }

    fn execute(&self, args: Vec<Value>) -> Result<Value, Error> {
        let num = args[0].as_number()
            .ok_or_else(|| Error::new("DOUBLE expects a number", None))?;
        Ok(Value::Number(num * 2.0))
    }

    fn description(&self) -> Option<&str> {
        Some("Doubles a number")
    }

    fn example(&self) -> Option<&str> {
        Some("DOUBLE(5) returns 10")
    }
}

// Register the function
fn main() -> Result<(), Error> {
    register_function(Box::new(DoubleFunction))?;

    // Now you can use it
    let result = skillet::evaluate_with_custom("DOUBLE(21)", &std::collections::HashMap::new())?;
    println!("{:?}", result); // Number(42.0)

    Ok(())
}
```

### Advanced Custom Functions

```rust
use skillet::{CustomFunction, Value, Error, register_function};

// Function with variable arguments
struct FormatFunction;

impl CustomFunction for FormatFunction {
    fn name(&self) -> &str { "FORMAT" }
    fn min_args(&self) -> usize { 1 }
    fn max_args(&self) -> Option<usize> { None } // Unlimited

    fn execute(&self, args: Vec<Value>) -> Result<Value, Error> {
        let template = match &args[0] {
            Value::String(s) => s,
            _ => return Err(Error::new("FORMAT expects string template as first argument", None)),
        };

        let mut result = template.clone();
        for (i, arg) in args.iter().skip(1).enumerate() {
            let placeholder = format!("{{{}}}", i);
            let value_str = match arg {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                Value::Boolean(b) => b.to_string(),
                _ => "null".to_string(),
            };
            result = result.replace(&placeholder, &value_str);
        }

        Ok(Value::String(result))
    }

    fn description(&self) -> Option<&str> {
        Some("Formats a string template with arguments")
    }

    fn example(&self) -> Option<&str> {
        Some("FORMAT(\"Hello {0}, you have {1} messages\", \"Alice\", 5)")
    }
}

// Array processing function
struct FilterEvenFunction;

impl CustomFunction for FilterEvenFunction {
    fn name(&self) -> &str { "FILTEREVEN" }
    fn min_args(&self) -> usize { 1 }
    fn max_args(&self) -> Option<usize> { Some(1) }

    fn execute(&self, args: Vec<Value>) -> Result<Value, Error> {
        let array = match &args[0] {
            Value::Array(arr) => arr,
            _ => return Err(Error::new("FILTEREVEN expects an array", None)),
        };

        let filtered: Vec<Value> = array
            .iter()
            .filter(|v| {
                if let Value::Number(n) = v {
                    (*n as i64) % 2 == 0
                } else {
                    false
                }
            })
            .cloned()
            .collect();

        Ok(Value::Array(filtered))
    }
}

// Mathematical function
struct FactorialFunction;

impl CustomFunction for FactorialFunction {
    fn name(&self) -> &str { "FACTORIAL" }
    fn min_args(&self) -> usize { 1 }
    fn max_args(&self) -> Option<usize> { Some(1) }

    fn execute(&self, args: Vec<Value>) -> Result<Value, Error> {
        let num = args[0].as_number()
            .ok_or_else(|| Error::new("FACTORIAL expects a number", None))?;

        if num < 0.0 {
            return Err(Error::new("FACTORIAL expects non-negative number", None));
        }

        let n = num as u64;
        let result = (1..=n).fold(1u64, |acc, x| acc * x);

        Ok(Value::Number(result as f64))
    }
}
```

### Function Management

```rust
use skillet::{register_function, unregister_function, list_custom_functions, has_custom_function};

fn manage_functions() -> Result<(), Error> {
    // Register multiple functions
    register_function(Box::new(DoubleFunction))?;
    register_function(Box::new(FormatFunction))?;
    register_function(Box::new(FilterEvenFunction))?;

    // List all custom functions
    let functions = list_custom_functions();
    println!("Custom functions: {:?}", functions);

    // Check if function exists
    if has_custom_function("DOUBLE") {
        println!("DOUBLE function is available");
    }

    // Override built-in functions (custom functions take priority)
    struct CustomSum;
    impl CustomFunction for CustomSum {
        fn name(&self) -> &str { "SUM" }
        fn min_args(&self) -> usize { 2 }
        fn max_args(&self) -> Option<usize> { Some(2) }

        fn execute(&self, args: Vec<Value>) -> Result<Value, Error> {
            // Custom SUM that multiplies instead of adds
            let a = args[0].as_number().ok_or_else(|| Error::new("Expected number", None))?;
            let b = args[1].as_number().ok_or_else(|| Error::new("Expected number", None))?;
            Ok(Value::Number(a * b))
        }
    }

    register_function(Box::new(CustomSum))?;

    // Now SUM(3, 4) returns 12 instead of 7
    let result = skillet::evaluate_with_custom("SUM(3, 4)", &std::collections::HashMap::new())?;
    println!("{:?}", result); // Number(12.0)

    // Clean up
    unregister_function("SUM");
    unregister_function("DOUBLE");
    unregister_function("FORMAT");
    unregister_function("FILTEREVEN");

    Ok(())
}
```

---

## Excel Formula Examples

### Arithmetic Operations

```bash
# Basic arithmetic
sk "=2 + 3 * 4"                    # 14
sk "=(10 + 20) / 3"                # 10
sk "=2 ^ 3"                        # 8 (exponentiation)
sk "=10 % 3"                       # 1 (modulo)

# With variables
sk "=:price * (1 + :tax)" price=100 tax=0.08    # 108
```

### Statistical Functions

```bash
# Basic statistics
sk "=SUM(1, 2, 3, 4, 5)"                       # 15
sk "=AVERAGE(10, 20, 30, 40, 50)"              # 30
sk "=MAX(15, 8, 23, 4, 16)"                    # 23
sk "=MIN(15, 8, 23, 4, 16)"                    # 4
sk "=COUNT(1, 2, 3, \"text\", 4)"              # 4 (counts numbers only)

# Advanced statistics
sk "=STDEV_P(2, 4, 4, 4, 5, 5, 7, 9)"         # Population standard deviation
sk "=VAR_P(2, 4, 4, 4, 5, 5, 7, 9)"           # Population variance
sk "=MEDIAN(1, 2, 3, 4, 5)"                   # 3
sk "=MODE_SNGL(1, 2, 2, 3, 4, 4, 4)"          # 4 (most frequent)
sk "=PERCENTILE_INC([1,2,3,4,5], 0.5)"        # 3 (50th percentile)
sk "=QUARTILE_INC([1,2,3,4,5,6,7,8], 1)"      # 2.75 (Q1)
```

### Logical Functions

```bash
# Basic logical
sk "=IF(5 > 3, \"Yes\", \"No\")"              # "Yes"
sk "=AND(true, false)"                         # false
sk "=OR(true, false)"                          # true
sk "=NOT(false)"                               # true
sk "=XOR(true, false)"                         # true

# Complex logical
sk "=IFS(false, \"A\", true, \"B\", true, \"C\")"  # "B" (first true condition)
sk "=IF(AND(:score >= 90, :attendance > 0.8), \"A\", IF(:score >= 80, \"B\", \"C\"))" score=85 attendance=0.9
```

### Text Functions

```bash
# String manipulation
sk "=\"Hello\".upper()"                        # "HELLO"
sk "=\"WORLD\".lower()"                        # "world"
sk "=\"  spaces  \".trim()"                    # "spaces"
sk "=\"hello\".reverse()"                      # "olleh"

# String functions
sk "=SUBSTRING(\"Hello World\", 6, 5)"         # "World"
sk "=LENGTH(\"Hello\")"                           # 5
sk "=CONCAT(\"Hello\", \" \", \"World\")"      # "Hello World"

# With variables
sk "=CONCAT(:name.upper(), ' ', :age)" name="alice" age=25  # "ALICE (25)"
```

### Date/Time Functions

```bash
# Current date/time
sk "=NOW()"                                    # Current Unix timestamp
sk "=DATE(2023, 12, 25)"                      # Unix timestamp for Dec 25, 2023
sk "=TIME(14, 30, 0)"                         # Seconds since midnight for 2:30 PM

# Date extraction
sk "=YEAR(NOW())"                              # Current year
sk "=MONTH(NOW())"                             # Current month (1-12)
sk "=DAY(NOW())"                               # Current day of month

# Date arithmetic
sk "=DATEADD(NOW(), 30, 'days')"                      # 30 days from now
sk "=DATEADD(NOW(), 2, 'months')"                     # 2 month from now
sk "=DATEADD(NOW(), 2, 'weeks')"                     # 2 weeks from now
sk "=DATEADD(NOW(), 2, 'minutes')"                     # 2 minutes from now
sk "=DATEDIFF(NOW(), DATEADD(NOW(), 7, \"days\"), \"days\")"       # Days between dates

# Practical examples
sk "=YEAR(DATE(2023, 6, 15))"                 # 2023
sk "=DATEADD( DATE(2023, 1, 1), 365, 'days')"          # One year later
```

### Array Operations

```bash
# Array creation and access
sk "=[1, 2, 3, 4, 5]"                         # Array literal
sk "=[1, 2, 3][0]"                            # 1 (first element)
sk "=[1, 2, 3, 4, 5][1:3]"                    # [2, 3] (slice)

# Array methods
sk "=[1, 2, 3, 4, 5].length()"                # 5
sk "=[5, 2, 8, 1, 9].sort()"                  # [1, 2, 5, 8, 9]
sk "=[1, 2, 2, 3, 3, 3].unique()"             # [1, 2, 3]
sk "=[1, 2, 3, 4, 5].sum()"                   # 15
sk "=[10, 5, 8, 3, 7].max()"                  # 10
sk "=[10, 5, 8, 3, 7].min()"                  # 3

# Advanced array operations
sk "=[1, 2, 3, 4, 5].filter(:x > 3)"           # [4, 5]
sk "=[1, 2, 3].map(:x * 2)"                    # [2, 4, 6]
sk "=[1, 2, 3, 4].reduce(:acc + :x, 0)"         # 10
sk "=[[1, 2], [3, 4], [5]].flatten()"         # [1, 2, 3, 4, 5]
```

### Mathematical Functions

```bash
# Basic math
sk "=ABS(-5)"                                  # 5
sk "=ROUND(3.14159, 2)"                       # 3.14
sk "=CEILING(3.2)"                            # 4
sk "=FLOOR(3.8)"                              # 3
sk "=INT(3.9)"                                # 3
sk "=MOD(10, 3)"                              # 1
sk "=POWER(2, 8)"                             # 256

# Trigonometric (if extended)
sk "=SQRT(16)"                                 # 4
sk "=PI()"                                     # 3.14159...

# With method syntax
sk "=(-5).abs()"                              # 5
sk "=(3.14159).round(2)"                      # 3.14
sk "=(3.8).floor()"                           # 3
```

### Conditional and Error Handling

```bash
# Type checking
sk "=ISNUMBER(42)"                            # true
sk "=ISTEXT(\"hello\")"                       # true
sk "=ISNUMBER(\"hello\")"                     # false

# Predicates
sk "=(5).positive?"                          # true
sk "=(-3).negative?"                         # true
sk "=(4).even?"                              # true
sk "=(5).odd?"                               # true
sk "=(\"\").blank?"                          # true
sk "=(\"hello\").present?"                   # true

# Complex conditions
sk "=IF(ISNUMBER(:value) && :value > 0, :value, 0)" value=42
sk "=IF(:data.blank?, \"No data\", :data.upper())" data=""
```

### Financial Calculations

```bash
# Loan payments with PMT function
sk "=PMT(0.05/12, 30*12, 100000)"              # Mortgage: $536.82/month
sk "=PMT(0.04/12, 5*12, 25000)"                # Car loan: $460.41/month
sk "=PMT(0.06/12, 10*12, 0, 50000)"            # Savings goal: $305.10/month

# PMT with balloon payment
sk "=PMT(0.04/12, 5*12, 50000, 10000)"         # Loan with $10k balloon

# PMT with beginning-of-period payments
sk "=PMT(0.05/12, 30*12, 100000, 0, 1)"        # Payment at start of month

# Simple interest
sk "=:principal * :rate * :time" principal=1000 rate=0.05 time=2  # 100

# Compound interest (manual)
sk "=:principal * POWER(1 + :rate, :years)" principal=1000 rate=0.05 years=10

# Tax calculations
sk "=:income * IF(:income > 50000, 0.25, 0.15)" income=60000  # Progressive tax

# Discount calculations
sk "=:price * (1 - :discount)" price=100 discount=0.20  # 80

# Bulk pricing
sk "=IF(:quantity >= 100, :unit_price * 0.9, :unit_price) * :quantity" quantity=150 unit_price=10
```

### Business Logic Examples

```bash
# Grade calculation
sk "=IFS(:score >= 90, \"A\", :score >= 80, \"B\", :score >= 70, \"C\", :score >= 60, \"D\", true, \"F\")" score=85

# Shipping cost
sk "=IF(:weight <= 1, 5, IF(:weight <= 5, 8, 12))" weight=3

# Employee bonus
sk "=IF(AND(:years >= 5, :performance > 3.5), :salary * 0.1, 0)" years=6 performance=4.2 salary=50000

# Inventory status
sk "=IF(:stock < :reorder_point, \"ORDER\", IF(:stock > :max_stock, \"EXCESS\", \"OK\"))" stock=15 reorder_point=20 max_stock=100

# Commission calculation
sk "=:sales * IF(:sales > 100000, 0.08, IF(:sales > 50000, 0.06, 0.04))" sales=75000
```

---

## Built-in Functions Reference

### Arithmetic Functions
- `SUM(...)` - Sum of all arguments
- `AVERAGE(...)`, `AVG(...)` - Average of arguments
- `MAX(...)` - Maximum value
- `MIN(...)` - Minimum value
- `COUNT(...)` - Count of numeric arguments
- `ABS(number)` - Absolute value
- `ROUND(number, digits)` - Round to specified digits
- `CEILING(number)` - Round up to nearest integer
- `FLOOR(number)` - Round down to nearest integer
- `INT(number)` - Integer part
- `MOD(dividend, divisor)` - Modulo operation
- `POWER(base, exponent)` - Exponentiation

### Statistical Functions
- `STDEV.P(...)` - Population standard deviation
- `VAR.P(...)` - Population variance
- `MEDIAN(...)` - Median value
- `MODE.SNGL(...)` - Most frequent value
- `PERCENTILE.INC(array, k)` - k-th percentile
- `QUARTILE.INC(array, quart)` - Quartile value

### Logical Functions
- `IF(condition, true_value, false_value)` - Conditional
- `IFS(condition1, value1, condition2, value2, ...)` - Multiple conditions
- `AND(...)` - Logical AND
- `OR(...)` - Logical OR
- `NOT(value)` - Logical NOT
- `XOR(...)` - Logical XOR

### Text Functions
- `LEN(text)` - Length of string
- `CONCAT(...)` - Concatenate strings
- `SUBSTRING(text, start, length)` - Extract substring
- `ISNUMBER(value)` - Check if numeric
- `ISTEXT(value)` - Check if text

### Date/Time Functions
- `NOW()` - Current timestamp
- `DATE(year, month, day)` - Create date
- `TIME(hour, minute, second)` - Create time
- `YEAR(date)` - Extract year
- `MONTH(date)` - Extract month
- `DAY(date)` - Extract day
- `DATEADD(date, days)` - Add days to date
- `DATEDIFF(date1, date2)` - Days between dates

### Array Functions
- `FILTER(array, expression)` - Filter array elements
- `MAP(array, expression)` - Transform array elements
- `REDUCE(array, expression, initial)` - Reduce array to single value
- `SUMIF(array, condition)` - Conditional sum
- `AVGIF(array, condition)` - Conditional average
- `COUNTIF(array, condition)` - Conditional count

### Financial Functions
- `PMT(rate, nper, pv, [fv], [type])` - Calculate loan payment
  - `rate`: Interest rate per period
  - `nper`: Number of payment periods
  - `pv`: Present value (loan amount)
  - `fv`: Future value (optional, default 0)
  - `type`: Payment timing (optional, 0=end of period, 1=beginning)

---

## JSON Integration

### Simple JSON Variables

```bash
# Basic types
sk "=:name" --json '{"name": "Alice"}'
sk "=:age * 2" --json '{"age": 25}'
sk "=:active" --json '{"active": true}'

# Arrays
sk "=:scores.sum()" --json '{"scores": [85, 92, 78, 90]}'
sk "=:items.length()" --json '{"items": ["apple", "banana", "cherry"]}'
```

### Nested JSON

```bash
# Object access
sk "=:user.name.upper()" --json '{"user": {"name": "john", "age": 30}}'
sk "=:config.database.port" --json '{
  "config": {
    "database": {"host": "localhost", "port": 5432},
    "cache": {"enabled": true}
  }
}'

# Array of objects
sk "=:products[0].price" --json '{
  "products": [
    {"name": "laptop", "price": 999.99},
    {"name": "mouse", "price": 29.99}
  ]
}'
```

### Complex Business Logic with JSON

```bash
# E-commerce calculation
sk "=SUM(:items.map(:price * :quantity)) * (1 + :tax_rate)" --json '{
  "items": [
    {"name": "laptop", "price": 999.99, "quantity": 1},
    {"name": "mouse", "price": 29.99, "quantity": 2}
  ],
  "tax_rate": 0.08
}'

# Employee payroll
sk "=IF(:employee.type = \"full-time\",
         :employee.salary / 12,
         :employee.hourly_rate * :hours_worked)" --json '{
  "employee": {
    "name": "Alice Johnson",
    "type": "full-time",
    "salary": 60000,
    "hourly_rate": 25
  },
  "hours_worked": 160
}'
```

---

## Advanced Features

### Method Chaining

```bash
# String method chaining
sk "=\"  hello world  \".trim().upper().reverse()"  # "DLROW OLLEH"

# Array method chaining
sk "=[5, 2, 8, 2, 1, 9, 2].unique().sort().reverse()"  # [9, 8, 5, 2, 1]

# Complex chaining
sk "=:data.filter(x > 0).map(x * 2).sum()" data=[1,-2,3,-4,5]  # 18
```

### Predicate Methods

```bash
# Numeric predicates
sk "=(42).positive()"     # true
sk "=(-5).negative()"     # true
sk "=(0).zero()"          # true
sk "=(4).even()"          # true
sk "=(5).odd()"           # true

# Value predicates
sk "=(\"\").blank()"      # true
sk "=(\"hello\").present()"  # true
sk "=([]).blank()"        # true
sk "=([1,2,3]).present()" # true
```

### Type Casting

```bash
# Explicit casting
sk "=\"42\" as number"    # 42.0
sk "=42 as string"       # "42"
sk "=1 as boolean"       # true
sk "=0 as boolean"       # false
```

### Spread Operator

```bash
# Function calls with spread
sk "=SUM(...[1, 2, 3, 4, 5])"              # 15
sk "=MAX(...:scores)" scores=[85,92,78,90]  # 92

# Array construction with spread
sk "=[0, ...[1, 2, 3], 4]"                 # [0, 1, 2, 3, 4]
```

### Error Handling Best Practices

```rust
use skillet::{evaluate_with_json, Error};

fn safe_evaluation(expression: &str, json_data: &str) -> Result<String, String> {
    match evaluate_with_json(expression, json_data) {
        Ok(value) => Ok(format!("{:?}", value)),
        Err(Error { message, position }) => {
            if let Some(pos) = position {
                Err(format!("Error at position {}: {}", pos, message))
            } else {
                Err(format!("Error: {}", message))
            }
        }
    }
}
```

This comprehensive documentation covers all aspects of using Skillet from basic CLI usage to advanced Rust integration and custom function development. The language provides a powerful foundation for expression evaluation with Excel-like functionality and extensibility for domain-specific needs.
