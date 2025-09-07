# Skillet Documentation

**Skillet**: A micro expression language for arithmetic, logical operations, and Excel-like functions with custom function extensibility.

## Table of Contents

**🚀 Getting Started:**
1. [Command Line Interface (CLI)](#command-line-interface-cli)
2. [Rust Integration](#rust-integration)
3. [API Surface](#api-surface)

**✨ New Features:**
4. [Type Conversion Methods](#type-conversion-methods) ⭐ **NEW**
5. [Safe Navigation Operator](#safe-navigation-operator) ⭐ **NEW**

**📚 Language Reference:**
6. [Excel Formula Examples](#excel-formula-examples)
7. [Built-in Functions Reference](#built-in-functions-reference) → See [API_REFERENCE.md](API_REFERENCE.md) for complete reference
8. [JSON Integration](#json-integration)

**🔧 Extensibility:**
9. [Extending the Language](#extending-the-language)
10. [JavaScript/Node Addon](#javascriptnode-addon)

**⚡ Advanced:**
11. [Advanced Features](#advanced-features)

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
sk "=LEFT(\"Hello\", 2)"                       # "He"
sk "=RIGHT(\"Hello\", 3)"                      # "llo"
sk "=MID(\"Hello\", 2, 3)"                     # "ell" (1-based start)

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

# Find and range functions
sk "=FIND([1, 2, 3, 4, 5], :x > 3)"            # 4 (first match)
sk "=[1, 2, 3, 4, 5].find(:x > 3)"             # 4 (method form)
sk "=BETWEEN(10, 20, 15)"                      # true
sk "=(17).between(10, 20)"                     # true (method form)
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
- `LEFT(text, [num_chars])` - Leftmost characters (default 1)
- `RIGHT(text, [num_chars])` - Rightmost characters (default 1)
- `MID(text, start, [num_chars])` - Substring from 1-based start
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
- `FIND(array, expression)` - Find first element matching expression (returns element or Null)
- `MAP(array, expression)` - Transform array elements
- `REDUCE(array, expression, initial)` - Reduce array to single value
- `SUMIF(array, condition)` - Conditional sum
- `AVGIF(array, condition)` - Conditional average
- `COUNTIF(array, condition)` - Conditional count
- `BETWEEN(min, max, value)` - Check if value is within range (inclusive)

### Financial Functions
- `PMT(rate, nper, pv, [fv], [type])` - Calculate loan payment
  - `rate`: Interest rate per period
  - `nper`: Number of payment periods
  - `pv`: Present value (loan amount)
  - `fv`: Future value (optional, default 0)
  - `type`: Payment timing (optional, 0=end of period, 1=beginning)

### Type Conversion Methods (Available on All Types)
- `to_s()`, `to_string()` - Convert to string
- `to_i()`, `to_int()` - Convert to integer
- `to_f()`, `to_float()` - Convert to float
- `to_a()`, `to_array()` - Convert to array
- `to_json()` - Convert to JSON object
- `to_bool()`, `to_boolean()` - Convert to boolean

**Null Conversions:**
- `null.to_s()` → `""` (empty string)
- `null.to_i()` → `0` (zero)
- `null.to_f()` → `0.0` (zero float)
- `null.to_a()` → `[]` (empty array)
- `null.to_json()` → `"{}"` (empty object)
- `null.to_bool()` → `false`

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

## Type Conversion Methods

Skillet provides Ruby-style conversion methods available on **all value types**, including null. These methods make handling mixed data types and null values much simpler and safer.

### Available Conversion Methods

All types support these conversion methods:

- `to_s()` / `to_string()` - Convert to string
- `to_i()` / `to_int()` - Convert to integer
- `to_f()` / `to_float()` - Convert to float
- `to_a()` / `to_array()` - Convert to array
- `to_json()` - Convert to JSON object
- `to_bool()` / `to_boolean()` - Convert to boolean

### Null Conversions (Safe Defaults)

Converting null values provides safe defaults:

```bash
# Null conversions always return safe defaults
sk "null.to_s()"      # ""     - empty string
sk "null.to_i()"      # 0      - zero
sk "null.to_f()"      # 0.0    - zero float
sk "null.to_a()"      # []     - empty array
sk "null.to_json()"   # "{}"   - empty JSON object
sk "null.to_bool()"   # false  - false
```

### String Conversions

```bash
# String to other types
sk "\"123\".to_i()"           # 123 - parses number
sk "\"123.45\".to_f()"        # 123.45 - parses float
sk "\"abc\".to_i()"           # 0 - invalid strings become 0
sk "\"hello\".to_a()"         # ["h", "e", "l", "l", "o"] - character array
sk "\"hello\".to_bool()"      # true - non-empty strings are true
sk "\"\".to_bool()"           # false - empty strings are false

# Identity conversion
sk "\"hello\".to_s()"         # "hello" - same string
```

### Number Conversions

```bash
# Number to other types
sk "123.to_s()"               # "123" - formatted as string
sk "123.45.to_s()"            # "123.45" - with decimals
sk "123.45.to_i()"            # 123 - truncates to integer
sk "123.45.to_f()"            # 123.45 - same float
sk "42.to_a()"                # [42] - single-element array
sk "0.to_bool()"              # false - zero is false
sk "123.to_bool()"            # true - non-zero is true
```

### Boolean Conversions

```bash
# Boolean to other types
sk "true.to_s()"              # "true" - string representation
sk "false.to_s()"             # "false"
sk "true.to_i()"              # 1 - true becomes 1
sk "false.to_i()"             # 0 - false becomes 0
sk "true.to_f()"              # 1.0 - true becomes 1.0
sk "false.to_f()"             # 0.0 - false becomes 0.0
sk "true.to_a()"              # [true] - single-element array
sk "true.to_bool()"           # true - identity
```

### Array Conversions

```bash
# Array to other types
sk "[1, 2, 3].to_s()"         # "[1, 2, 3]" - formatted string
sk "[1, 2, 3].to_i()"         # 3 - array length
sk "[].to_i()"                # 0 - empty array length
sk "[1, 2, 3].to_f()"         # 3.0 - array length as float
sk "[1, 2, 3].to_a()"         # [1, 2, 3] - identity
sk "[].to_bool()"             # false - empty arrays are false
sk "[1, 2, 3].to_bool()"      # true - non-empty arrays are true
```

### Practical Use Cases

#### 1. Null-Safe Operations

**Problem**: Accessing properties that might be null causes errors:
```bash
# This fails if FechaCierreCuenta is null
sk ":data.FechaCierreCuenta.length()"  # Error: No methods available for Null type
```

**Solution**: Use conversion methods for null-safe operations:
```bash
# This works even if FechaCierreCuenta is null
sk ":data.FechaCierreCuenta.to_s().length()"  # 0 (length of empty string)
```

#### 2. Filtering with Null Safety

```bash
# Filter accounts with empty closure dates (including nulls)
sk ":cuentas := [
  {\"FechaCierreCuenta\": null}, 
  {\"FechaCierreCuenta\": \"\"}, 
  {\"FechaCierreCuenta\": \"2023-01-01\"}
]; 
:cuentas.filter(:x.FechaCierreCuenta.to_s().length() == 0)"
# Returns: [{"FechaCierreCuenta": null}, {"FechaCierreCuenta": ""}]
```

#### 3. Data Normalization

```bash
# Convert mixed data to consistent types
sk ":mixed := [null, \"hello\", 123, true, []]; 
:mixed.map(:x.to_s())"
# Returns: ["", "hello", "123", "true", "[]"]

# Convert all to numbers (with safe defaults)
sk ":mixed := [\"123\", null, true, \"45.5\", \"abc\"]; 
:mixed.map(:x.to_i())"
# Returns: [123, 0, 1, 45, 0]
```

#### 4. Boolean Logic with Mixed Types

```bash
# Check if values are "truthy"
sk ":values := [\"\", \"hello\", 0, 42, [], [1,2]]; 
:values.map(:x.to_bool())"
# Returns: [false, true, false, true, false, true]
```

#### 5. Safe String Concatenation

```bash
# Safely concatenate potentially null values
sk ":first := null; :last := \"Smith\"; 
CONCAT(:first.to_s(), \" \", :last.to_s()).trim()"
# Returns: "Smith" (instead of failing)
```

#### 6. Dynamic Type Conversion

```bash
# Convert user input to appropriate types based on context
sk ":user_input := \"123\"; 
IF(:needs_number, :user_input.to_i(), :user_input.to_s())" needs_number=true
# Returns: 123
```

#### 7. Array Creation from Mixed Data

```bash
# Create character arrays from strings
sk "\"hello\".to_a().length()"  # 5

# Ensure single values become arrays
sk ":value := 42; :value.to_a().sum()"  # 42
```

### Ruby-Style Behavior

Skillet's conversion methods follow Ruby conventions for intuitive behavior:

1. **String to Number**: Invalid strings become 0 (like Ruby's `to_i`)
2. **Boolean to Number**: `true` → 1, `false` → 0
3. **Empty Values to Boolean**: Empty strings and arrays become `false`
4. **Null Safety**: `null` converts to safe defaults for all types
5. **String Arrays**: Strings convert to character arrays (`"hi"` → `["h", "i"]`)

### Method Name Variants

Both short and long forms are supported:

- `to_s()` or `to_string()`
- `to_i()` or `to_int()`  
- `to_f()` or `to_float()`
- `to_a()` or `to_array()`
- `to_bool()` or `to_boolean()`

### Chaining Conversions

Conversion methods can be chained with other methods:

```bash
# Chain conversions and operations
sk "null.to_s().length()"                    # 0
sk "\"123.45\".to_f().round(1).to_s()"      # "123.5"
sk "\"hello\".to_a().length().to_bool()"    # true (5 characters → true)
```

### Error Prevention

Conversion methods eliminate many common runtime errors:

```bash
# Before: These might fail
# :value.length()  # Fails if value is null
# :number + 10     # Fails if number is a string

# After: These always work
# :value.to_s().length()     # Always returns a number
# :number.to_i() + 10        # Always performs addition
```

---

## Safe Navigation Operator

Skillet provides a safe navigation operator `&.` that prevents errors when accessing properties or calling methods on null values.

### Basic Safe Navigation

The `&.` operator allows safe property access and method calls:

```bash
# Safe property access
sk ":obj := {\"name\": \"John\"}; :obj&.name"        # "John"
sk ":obj := {\"name\": \"John\"}; :obj&.missing"     # null (no error)
sk ":null_obj := null; :null_obj&.anything"          # null (no error)

# Without safe navigation (would cause errors)
sk ":null_obj := null; :null_obj.anything"           # Error!
```

### Safe Method Calls

Safe navigation also works with method calls:

```bash
# Safe method calls
sk ":str := \"hello\"; :str&.length()"               # 5
sk ":null_str := null; :null_str&.length()"          # null (no error)

# Chained safe navigation and method calls
sk ":obj := {\"name\": \"John\"}; :obj&.name&.length()"     # 4
sk ":obj := {\"name\": null}; :obj&.name&.length()"         # null (no error)
```

### Combining Safe Navigation with Conversions

Safe navigation works perfectly with conversion methods:

```bash
# Safe navigation + conversion for ultimate null safety
sk ":data := [{\"date\": null}, {\"date\": \"2023-01-01\"}]; 
:data.filter(:x&.date&.to_s().length() > 0)"
# Returns: [{"date": "2023-01-01"}]

# Traditional approach (safer)
sk ":data := [{\"date\": null}, {\"date\": \"2023-01-01\"}]; 
:data.filter(:x.date.to_s().length() > 0)"
# Also works - conversion methods handle null gracefully
```

### When to Use Safe Navigation vs Conversions

- **Use `&.`** when you want to preserve null values in the result
- **Use `.to_*()` methods** when you want to convert null to a safe default

```bash
# Preserves nulls
sk ":arr := [null, \"hello\", null]; 
:arr.map(:x&.length())"
# Returns: [null, 5, null]

# Converts nulls to defaults  
sk ":arr := [null, \"hello\", null]; 
:arr.map(:x.to_s().length())"
# Returns: [0, 5, 0]
```

---

## Advanced Features

> **Note**: This section covers advanced Skillet features including the new Object Literal syntax and updated HTTP Server API with variable tracking support.

### Method Chaining

```bash
# String method chaining
sk "=\"  hello world  \".trim().upper().reverse()"  # "DLROW OLLEH"

# Array method chaining
sk "=[5, 2, 8, 2, 1, 9, 2].unique().sort().reverse()"  # [9, 8, 5, 2, 1]

# Complex chaining
sk "=:data.filter(x > 0).map(x * 2).sum()" data=[1,-2,3,-4,5]  # 18

# Range checking method
sk "=(75).between(60, 100)"                    # true
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

### Object Literals

Skillet supports JSON-style object literal syntax for creating structured data:

```bash
# Simple object literals
sk ":obj := {name: 'John', age: 30, active: true}; :obj"
# Output: Json("{\"name\":\"John\",\"age\":30.0,\"active\":true}")

# Objects with quoted keys
sk ":obj := {\"first-name\": \"Jane\", \"last-name\": \"Doe\"}; :obj"

# Nested objects
sk ":config := {database: {host: 'localhost', port: 5432}, debug: true}; :config"

# Objects with expressions as values
sk ":data := {sum: 10 + 20, product: 5 * 6, timestamp: NOW()}; :data"

# Arrays of objects (table-like structures)
sk ":table := [{id: 1, name: 'Alice', score: 95}, {id: 2, name: 'Bob', score: 87}]; :table"

# Complex nested structures
sk ":app := {users: [{name: 'Admin', perms: ['read', 'write']}, {name: 'Guest', perms: ['read']}], settings: {theme: 'dark', lang: 'en'}}; :app"
```

#### Object Property Access

Once created, you can access object properties using dot notation:

```bash
# Simple property access
sk ":person := {name: 'Alice', age: 25}; :person.name"
# Output: String("Alice")

# Nested property access
sk ":config := {db: {host: 'localhost', port: 3306}}; :config.db.port"
# Output: Number(3306.0)

# Accessing properties with variables
sk ":user := {profile: {settings: {theme: 'dark'}}}; :user.profile.settings.theme"
# Output: String("dark")
```

#### Working with Objects in Variables

Objects integrate seamlessly with Skillet's variable assignment and expression system:

```bash
# Creating objects with variable expressions
sk ":price := 19.99; :qty := 3; :order := {item: 'Widget', price: :price, quantity: :qty, total: :price * :qty}; :order.total"
# Output: Number(59.97)

# Objects in complex expressions
sk ":data := {values: [10, 20, 30]}; SUM(:data.values)"
# Output: Number(60.0)

# Multiple objects
sk ":user := {name: 'John', id: 123}; :prefs := {theme: 'light', lang: 'en'}; :combined := {user: :user, preferences: :prefs}; :combined"
```

#### HTTP Server Integration

Object literals work perfectly with the HTTP server's new variable tracking feature:

```json
// POST /eval
{
    "expression": ":config := {api: {endpoint: 'https://api.example.com', timeout: 5000}, features: {caching: true, retries: 3}}; :config.api.timeout",
    "arguments": {},
    "include_variables": true
}

// Response
{
    "success": true,
    "result": 5000,
    "variables": {
        "config": "{\"api\":{\"endpoint\":\"https://api.example.com\",\"timeout\":5000.0},\"features\":{\"caching\":true,\"retries\":3.0}}"
    },
    "execution_time_ms": 1.2,
    "request_id": 456
}
```

#### Object Syntax Rules

- **Keys**: Can be unquoted identifiers (`name: value`) or quoted strings (`"key-name": value`)
- **Values**: Any valid Skillet expression (numbers, strings, variables, function calls, arrays, nested objects)
- **Trailing Commas**: Supported (`{a: 1, b: 2,}`)
- **Empty Objects**: Supported (`{}`)
- **Storage**: Objects are internally stored as JSON strings (`Value::Json`)
- **Access**: Use dot notation for property access (`obj.key.subkey`)

### HTTP Server API Updates

The Skillet HTTP server has been enhanced with two major updates:

#### 1. Parameter Rename: `variables` → `arguments`

The HTTP API now uses `arguments` instead of `variables` for input parameters:

```json
// NEW API (preferred)
{
    "expression": ":total := :price * :quantity",
    "arguments": {
        "price": 19.99,
        "quantity": 3
    }
}

// OLD API (deprecated)
{
    "expression": ":total := :price * :quantity", 
    "variables": {
        "price": 19.99,
        "quantity": 3
    }
}
```

#### 2. Variable Tracking with `include_variables`

Set `include_variables: true` to receive all assigned variables in the response:

```bash
# Example request
curl -X POST http://localhost:5074/eval \
  -H "Content-Type: application/json" \
  -d '{
    "expression": ":taxes := 1.16; :subtotal := :unit_price * :quantity; :total := :subtotal * :taxes;",
    "arguments": {
      "quantity": 2,
      "unit_price": 200
    },
    "include_variables": true
  }'

# Response with variables
{
  "success": true,
  "result": 464,
  "variables": {
    "taxes": 1.16,
    "subtotal": 400,
    "total": 464
  },
  "execution_time_ms": 2.5,
  "request_id": 123
}
```

#### GET Request Support

Both features work with GET requests too:

```bash
# With variable tracking
curl "http://localhost:5074/eval?expr=:sum%20:=%20:a%20+%20:b&a=10&b=20&include_variables=true"
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
