# Skillet Language - Development Guide for LLMs and Contributors

**Version:** 0.6.0
**Last Updated:** March 2026
**Purpose:** Comprehensive guide for understanding Skillet's architecture, conventions, and development patterns

---

## Table of Contents

1. [Project Overview](#1-project-overview)
2. [Design Philosophy & Objectives](#2-design-philosophy--objectives)
3. [Language Conventions](#3-language-conventions)
4. [Architecture Overview](#4-architecture-overview)
5. [Core Components Deep Dive](#5-core-components-deep-dive)
6. [Development Workflow](#6-development-workflow)
7. [Performance Optimization Patterns](#7-performance-optimization-patterns)
8. [Testing Strategy](#8-testing-strategy)
9. [Extension Mechanisms](#9-extension-mechanisms)
10. [Common Development Tasks](#10-common-development-tasks)
11. [Troubleshooting & Known Issues](#11-troubleshooting--known-issues)
12. [Future Development Guidelines](#12-future-development-guidelines)

---

## 1. Project Overview

### What is Skillet?

Skillet is a **high-performance, embeddable expression language** written in Rust that combines:
- **Excel-like formula syntax** for familiarity
- **Ruby-style method chaining** for expressiveness
- **Functional programming** with lambdas and higher-order functions
- **Rust-powered performance** with memory safety guarantees

### Primary Use Cases

1. **Business Logic Engine**: Embed complex calculations in applications
2. **Data Transformation**: Process JSON/arrays with functional operations
3. **Template System**: Dynamic content generation with expressions
4. **Rule Engine**: Evaluate conditional logic for decision-making
5. **Microservice**: Standalone HTTP/TCP servers for expression evaluation

### Key Differentiators

- **Sub-3ms evaluation time** (100x+ faster than original design)
- **Null-safe by design** with `&.` operator and conversion methods
- **Runtime extensible** via JavaScript plugins (no recompilation needed)
- **Production-ready servers** with authentication, caching, and daemon mode
- **JSONPath integration** for complex data queries

---

## 2. Design Philosophy & Objectives

### Core Principles

#### 1. **Familiarity First**
- Excel-like function names (`SUM`, `AVERAGE`, `IF`)
- Optional `=` prefix for Excel compatibility
- Predictable operator precedence

#### 2. **Safety Without Ceremony**
- Null-safe operations by default
- Conversion methods provide safe defaults (`null.to_s()` → `""`)
- Safe navigation operator (`&.`) prevents null reference errors

#### 3. **Performance Matters**
- Zero-cost abstractions where possible
- String interning for repeated identifiers
- Memory pooling for AST nodes
- HashMap reuse in loops (no per-iteration clones)

#### 4. **Extensibility by Design**
- JavaScript plugins for runtime customization
- Rust custom functions for compile-time extensions
- Server modes (HTTP/TCP) for language-agnostic integration

#### 5. **Developer Ergonomics**
- Method chaining for readability (`.filter().map().sum()`)
- Lambda parameters with default names (`:x`, `:acc`)
- Type conversion methods on all types (`.to_i()`, `.to_s()`)

### Non-Goals

- ❌ **Not a general-purpose programming language** (no loops, no file I/O)
- ❌ **Not Turing-complete** (intentionally limited scope)
- ❌ **Not a database query language** (use JSONPath for data queries)

---

## 3. Language Conventions

### Variable Naming

```bash
# Variables ALWAYS use colon prefix
:variable_name        # Reference
:count := 10          # Assignment

# Lambda parameters (default names)
:x                    # Element in filter/map
:acc                  # Accumulator in reduce
:v                    # Custom named parameter

# Special built-in variable
:arguments            # Auto-populated from JSON input (--json flag)
```

### Type System

```rust
pub enum Value {
    Number(f64),      // All numbers are f64
    Array(Vec<Value>), // Homogeneous or heterogeneous
    Boolean(bool),     // true/false
    String(String),    // UTF-8 strings
    Null,             // Explicit null type
    Currency(f64),    // Special formatting, same as Number
    DateTime(i64),    // Unix timestamp
    Json(String),     // JSON object as string
}
```

**Type Coercion Rules:**
1. Numbers coerce to strings in concatenation
2. Empty strings/arrays are falsy in boolean context
3. `null` converts to safe defaults in conversion methods

### Operator Precedence (Highest to Lowest)

```
1. Method calls             .method()
2. Unary operators          !, -, +
3. Exponentiation           ^
4. Multiplication/Division  *, /, %
5. Addition/Subtraction     +, -
6. Comparisons              >, <, >=, <=, ==, !=
7. Logical AND              AND, &&
8. Logical OR               OR, ||
9. Ternary                  ? :
10. Assignment              :=
```

### Syntax Patterns

#### Method Chaining
```bash
"  hello  ".trim().upper().reverse()  # "OLLEH"
[1,2,3,4,5].filter(:x > 2).map(:x * 10).sum()  # 120
```

#### Safe Navigation
```bash
:user&.profile&.avatar&.url  # Returns null if any part is null
```

#### Lambda Expressions
```bash
# Default parameter name :x
[1,2,3].filter(:x > 1)  # [2, 3]

# Custom parameter name
[1,2,3].map(:num * 2, 'num')  # [2, 4, 6]

# Reduce with two parameters
[1,2,3].reduce(:total + :val, 0, 'val', 'total')  # 6
```

#### Type Casting
```bash
"42" :: Integer    # Explicit cast
42 :: String       # "42"
```

---

## 4. Architecture Overview

### High-Level Pipeline

```
Input String
    ↓
┌─────────────────┐
│ Lexer           │  Tokenization with string interning
├─────────────────┤
│ Token Stream    │
└─────────────────┘
    ↓
┌─────────────────┐
│ Parser          │  Builds AST with memory pooling
├─────────────────┤
│ AST (Expr)      │
└─────────────────┘
    ↓
┌─────────────────┐
│ Evaluator       │  Interprets AST with variable context
├─────────────────┤
│ Value           │
└─────────────────┘
```

### Module Structure

```
skillet/
├── src/
│   ├── lib.rs                    # Public API surface
│   ├── types.rs                  # Value enum definition
│   ├── error.rs                  # Error type with position tracking
│   ├── ast.rs                    # Abstract syntax tree nodes
│   ├── lexer.rs                  # Tokenizer with string interning
│   ├── parser.rs                 # Recursive descent parser
│   ├── memory_pool.rs            # Arena allocator for AST nodes
│   ├── custom.rs                 # CustomFunction trait & registry
│   ├── js_plugin.rs              # JavaScript plugin loader (rquickjs)
│   ├── concurrent_registry.rs    # Thread-safe function registry
│   │
│   ├── runtime/
│   │   ├── evaluator.rs          # Main evaluation engine
│   │   ├── evaluation/
│   │   │   ├── core.rs           # Core expression evaluation
│   │   │   ├── higher_order.rs   # FILTER/MAP/REDUCE/FIND/SUMIF/AVGIF/COUNTIF
│   │   │   └── assignments.rs    # Variable assignment logic
│   │   │
│   │   ├── method_calls/
│   │   │   ├── mod.rs            # Method dispatch hub
│   │   │   ├── array_methods.rs  # .length(), .sum(), .sort(), etc.
│   │   │   ├── string_methods.rs # .upper(), .trim(), .reverse()
│   │   │   ├── lambda_methods.rs # .filter(), .map(), .reduce()
│   │   │   ├── predicates.rs     # .even?, .blank?, .positive?
│   │   │   └── conversion_methods.rs # .to_s(), .to_i(), .to_a()
│   │   │
│   │   ├── arithmetic.rs         # SUM, PRODUCT, AVERAGE, etc.
│   │   ├── logical.rs            # IF, AND, OR, NOT, XOR
│   │   ├── string.rs             # CONCAT, SUBSTRING, LEFT, RIGHT
│   │   ├── array.rs              # FLATTEN, UNIQUE, SORT, JOIN
│   │   ├── datetime.rs           # NOW, DATE, DATEADD, DATEDIFF
│   │   ├── financial.rs          # PMT, FV, IPMT
│   │   ├── statistical.rs        # MEDIAN, STDEV, PERCENTILE
│   │   ├── json.rs               # DIG, object manipulation
│   │   ├── jsonpath.rs           # JQ function (JSONPath queries)
│   │   └── builtin_functions.rs  # Function registry & dispatch
│   │
│   └── bin/
│       ├── sk.rs                 # CLI binary
│       ├── sk_server.rs          # TCP server
│       ├── sk_client.rs          # TCP client
│       ├── sk_http_server.rs     # HTTP REST API server
│       └── http_server/          # HTTP server modules
│           ├── auth.rs           # Token authentication
│           ├── cache.rs          # LRU result caching
│           ├── eval.rs           # Evaluation endpoint
│           ├── js_management.rs  # JS function upload/delete
│           └── stats.rs          # Metrics & health checks
```

---

## 5. Core Components Deep Dive

### 5.1 Lexer (`src/lexer.rs`)

**Purpose:** Tokenize input into a stream of tokens with string interning.

**Key Features:**
- **String Interning**: Reuse identical identifier strings to reduce allocations
- **Position Tracking**: Track line/column for error reporting
- **Operator Recognition**: Multi-character operators (`>=`, `<=`, `!=`, `::`, `&.`)

**Token Types:**
```rust
pub enum Token {
    Number(f64),
    Identifier(String),
    String(String),
    Plus, Minus, Star, Slash, Percent, Caret,
    LeftParen, RightParen, LeftBracket, RightBracket,
    Greater, Less, GreaterEqual, LessEqual, Equal, NotEqual,
    And, Or, Not, AndAnd, OrOr,
    Comma, Colon, ColonEqual, Dot, Question, Spread,
    SafeNav,       // &.
    TypeCast,      // ::
    // ...
}
```

### 5.2 Parser (`src/parser.rs`)

**Purpose:** Build an Abstract Syntax Tree (AST) from tokens using recursive descent.

**Grammar Overview:**
```
expression    → assignment
assignment    → COLON IDENT COLONEQUAL expression | ternary
ternary       → logical_or ( QUESTION expression COLON expression )?
logical_or    → logical_and ( (OR | OROR) logical_and )*
logical_and   → equality ( (AND | ANDAND) equality )*
equality      → comparison ( (EQUAL | NOTEQUAL) comparison )*
comparison    → term ( (GREATER | LESS | GE | LE) term )*
term          → factor ( (PLUS | MINUS) factor )*
factor        → unary ( (STAR | SLASH | PERCENT) unary )*
unary         → (NOT | MINUS | PLUS) unary | exponent
exponent      → postfix ( CARET unary )?
postfix       → primary ( method_call | array_access | property_access )*
primary       → NUMBER | STRING | IDENT | LPAREN expression RPAREN | array | object
```

**Memory Pooling:**
- Uses `typed_arena::Arena` for AST node allocations
- Reduces allocation overhead for large expressions

### 5.3 Evaluator (`src/runtime/evaluator.rs`)

**Purpose:** Interpret the AST and produce a `Value`.

**Key Structures:**
```rust
pub struct Evaluator<'a> {
    context: VariableContext<'a>,
}

pub struct VariableContext<'a> {
    variables: Cow<'a, HashMap<String, Value>>,
}

impl<'a> VariableContext<'a> {
    // Borrow existing HashMap (for read-only access)
    pub fn with_borrowed(vars: &'a HashMap<String, Value>) -> Self;

    // Take ownership of HashMap (for modifications)
    pub fn with_owned(vars: HashMap<String, Value>) -> Self;

    // Recover owned HashMap after evaluation
    pub fn into_variables(self) -> HashMap<String, Value>;
}
```

**Evaluation Pattern:**
```rust
// For functions that don't modify variables
pub fn eval_with_vars(expr: &Expr, vars: &HashMap<String, Value>) -> Result<Value, Error> {
    let evaluator = Evaluator::new(VariableContext::with_borrowed(vars));
    evaluator.eval(expr)
}

// For higher-order functions (reuse HashMap across iterations)
pub fn eval_filter(array: &[Value], predicate: &Expr, vars: &HashMap<String, Value>) -> Result<Value, Error> {
    let mut working_vars = vars.clone();  // Clone once
    let mut result = Vec::with_capacity(array.len());

    for item in array {
        working_vars.insert("x".to_string(), item.clone());
        if evaluator.eval_with_vars(predicate, &working_vars)?.as_bool() == Some(true) {
            result.push(item.clone());
        }
        // Reuse working_vars for next iteration (no clone!)
    }
    Ok(Value::Array(result))
}
```

### 5.4 Method Dispatch (`src/runtime/method_calls/mod.rs`)

**Purpose:** Route method calls to appropriate implementations based on receiver type.

**Dispatch Pattern:**
```rust
pub fn call_method(receiver: &Value, method_name: &str, args: Vec<Value>) -> Result<Value, Error> {
    let method_lower = method_name.to_lowercase();  // Compute once!

    match receiver {
        Value::Array(arr) => {
            // Try lambda methods first (.filter, .map, .reduce)
            if let Some(result) = lambda_methods::try_lambda_method(arr, &method_lower, args)? {
                return Ok(result);
            }
            // Fall back to array methods (.length, .sum, .sort)
            array_methods::call_array_method(arr, &method_lower, args)
        }
        Value::String(s) => string_methods::call_string_method(s, &method_lower, args),
        Value::Number(n) => number_methods::call_number_method(*n, &method_lower, args),
        // ...
    }
}
```

### 5.5 Higher-Order Functions (`src/runtime/evaluation/higher_order.rs`)

**Purpose:** Implement FILTER, MAP, REDUCE, FIND, SUMIF, AVGIF, COUNTIF as standalone functions.

**Performance Pattern:**
```rust
// ✅ GOOD: Reuse HashMap across iterations
pub fn eval_filter(array: &[Value], predicate: &Expr, param_name: &str) -> Result<Vec<Value>, Error> {
    let mut vars = HashMap::with_capacity(1);  // Pre-allocate
    let mut result = Vec::with_capacity(array.len());  // Hint capacity

    for item in array {
        vars.insert(param_name.to_string(), item.clone());
        let matches = eval_with_vars(predicate, &vars)?.as_bool().unwrap_or(false);
        if matches {
            result.push(item.clone());
        }
        // vars is reused here, no clone!
    }

    Ok(result)
}

// ❌ BAD: Clone HashMap every iteration (slow!)
pub fn eval_filter_slow(array: &[Value], predicate: &Expr, param_name: &str) -> Result<Vec<Value>, Error> {
    let mut result = Vec::new();

    for item in array {
        let mut vars = HashMap::new();  // ❌ Allocation per iteration!
        vars.insert(param_name.to_string(), item.clone());
        // ...
    }

    Ok(result)
}
```

---

## 6. Development Workflow

### 6.1 Building

```bash
# Development build
cargo build

# Release build (optimized)
cargo build --release

# Build specific binary
cargo build --bin sk --release
cargo build --bin sk_http_server --release
```

### 6.2 Testing

```bash
# Run all tests
cargo test

# Run specific test module
cargo test test_arithmetic
cargo test test_higher_order

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_filter_basic -- --nocapture

# Skip flaky tests (sk_concurrency has known file lock issues)
cargo test --lib  # Skip integration tests
```

### 6.3 Benchmarking

```bash
# Run benchmarks
cargo bench

# Server benchmark (requires server running)
bash scripts/benchmark_server.sh 8080 10000 8
```

### 6.4 Local Testing Workflow

```bash
# Test expression via CLI
cargo run --bin sk -- "2 + 3 * 4"
cargo run --bin sk -- ":x := 10; :y := 20; :x + :y" x=5 y=10
cargo run --bin sk -- "SUM([1,2,3].filter(:x > 1))" --json '{"data": [1,2,3]}'

# Start HTTP server for testing
cargo run --bin sk_http_server 5074

# Test HTTP endpoint
curl -X POST http://localhost:5074/eval \
  -H "Content-Type: application/json" \
  -d '{"expression": "2 + 3 * 4"}'
```

---

## 7. Performance Optimization Patterns

### 7.1 Memory Optimizations Applied (Feb 2026)

1. **HashMap Reuse in Loops**
   - Clone once, reuse across iterations in FILTER/MAP/REDUCE
   - Avoids per-iteration allocations

2. **Array Method Optimizations**
   - Use `&[Value]` references instead of cloning entire arrays
   - `Vec::with_capacity` for known-size outputs

3. **String Operations**
   - Use `char_indices()` + byte slicing instead of `chars().collect::<Vec<char>>()`
   - Avoids intermediate Vec allocations

4. **Method Dispatch**
   - Compute `to_lowercase()` once per method call
   - Cache result for repeated comparisons

5. **Type Checks**
   - Match on `&Value` references instead of cloning for ISBLANK/ISNUMBER/ISTEXT

### 7.2 Performance Guidelines

#### ✅ DO:
- Use `&[Value]` slices for read-only array access
- Pre-allocate `Vec` with `Vec::with_capacity(n)` when size is known
- Reuse `HashMap` across loop iterations
- Use string interning for repeated identifiers
- Match on references (`match &value`) to avoid clones

#### ❌ DON'T:
- Clone entire arrays just to call `.iter()`
- Allocate new `HashMap` in every loop iteration
- Convert strings to `Vec<char>` unless absolutely necessary
- Clone values unnecessarily in type checks
- Use `.clone()` in hot paths without profiling

### 7.3 Benchmarking Results (Phase 2)

| Operation | Before | After | Improvement |
|-----------|--------|-------|-------------|
| FILTER (100 items) | 15.2ms | 1.8ms | **8.4x** |
| MAP (100 items) | 12.1ms | 1.5ms | **8.0x** |
| String ops | 850µs | 120µs | **7.0x** |
| Array reverse | 450µs | 80µs | **5.6x** |

**Overall:** ~3ms average evaluation time (100x+ improvement from original 300ms baseline)

---

## 8. Testing Strategy

### 8.1 Test Organization

```
tests/
├── arithmetic_tests.rs       # +, -, *, /, %, ^
├── array_tests.rs            # Array operations & methods
├── higher_order_tests.rs     # FILTER, MAP, REDUCE, FIND
├── logical_tests.rs          # IF, AND, OR, NOT, IFS
├── string_tests.rs           # String functions & methods
├── conversion_tests.rs       # Type conversion methods
├── json_tests.rs             # JSON integration & DIG
├── jsonpath_tests.rs         # JQ function
├── safe_navigation_tests.rs  # &. operator
└── integration_tests.rs      # End-to-end scenarios
```

### 8.2 Test Patterns

```rust
#[test]
fn test_filter_basic() {
    let result = evaluate("[1,2,3,4,5].filter(:x > 3)").unwrap();
    assert_eq!(result, Value::Array(vec![
        Value::Number(4.0),
        Value::Number(5.0)
    ]));
}

#[test]
fn test_null_safety() {
    // Ensure null.to_s() returns empty string
    let result = evaluate("null.to_s().length()").unwrap();
    assert_eq!(result, Value::Number(0.0));
}

#[test]
fn test_with_variables() {
    let mut vars = HashMap::new();
    vars.insert("price".to_string(), Value::Number(19.99));
    vars.insert("qty".to_string(), Value::Number(3.0));

    let result = evaluate_with(":price * :qty", &vars).unwrap();
    assert_eq!(result, Value::Number(59.97));
}
```

### 8.3 Known Test Issues

- **`sk_concurrency` tests are flaky**: Cargo file lock contention on some systems
  - **Workaround**: Run `cargo test --lib` to skip integration tests
  - **Root cause**: Pre-existing issue, not introduced by recent changes

---

## 9. Extension Mechanisms

### 9.1 JavaScript Plugins (Runtime Extension)

**Location:** `hooks/` directory (customizable via `SKILLET_HOOKS_DIR`)

**Format:**
```javascript
// hooks/my_function.js

// @name: MY_FUNCTION
// @min_args: 1
// @max_args: 2
// @description: What the function does
// @example: MY_FUNCTION(5) returns 10

function execute(args) {
    // args[0], args[1], ...
    return result;
}
```

**Loading:**
```rust
#[cfg(feature = "plugins")]
use skillet::JSPluginLoader;

let loader = JSPluginLoader::new("hooks");
loader.load_all()?;  // Auto-loads all .js files
```

**Use Cases:**
- Business-specific calculations without recompiling
- Rapid prototyping of new functions
- Domain-specific extensions (date formatting, custom aggregations)

### 9.2 Rust Custom Functions (Compile-Time Extension)

**Trait Definition:**
```rust
pub trait CustomFunction: Send + Sync {
    fn name(&self) -> &str;
    fn min_args(&self) -> usize;
    fn max_args(&self) -> Option<usize>;  // None = unlimited
    fn execute(&self, args: Vec<Value>) -> Result<Value, Error>;
    fn description(&self) -> Option<&str> { None }
    fn example(&self) -> Option<&str> { None }
}
```

**Implementation Example:**
```rust
struct DoubleFunction;

impl CustomFunction for DoubleFunction {
    fn name(&self) -> &str { "DOUBLE" }
    fn min_args(&self) -> usize { 1 }
    fn max_args(&self) -> Option<usize> { Some(1) }

    fn execute(&self, args: Vec<Value>) -> Result<Value, Error> {
        match &args[0] {
            Value::Number(n) => Ok(Value::Number(n * 2.0)),
            _ => Err(Error::new("DOUBLE expects a number", None)),
        }
    }
}

// Register globally
register_function(Box::new(DoubleFunction))?;
```

**Use Cases:**
- Performance-critical functions
- Functions requiring Rust libraries (crypto, regex, etc.)
- Core functionality that ships with Skillet

---

## 10. Common Development Tasks

### 10.1 Adding a New Built-in Function

**Example:** Add a `CUBE(n)` function that returns n³

1. **Define function in `src/runtime/arithmetic.rs`:**
```rust
pub fn cube(args: Vec<Value>) -> Result<Value, Error> {
    if args.len() != 1 {
        return Err(Error::new("CUBE expects 1 argument", None));
    }

    match &args[0] {
        Value::Number(n) => Ok(Value::Number(n.powi(3))),
        _ => Err(Error::new("CUBE expects a number", None)),
    }
}
```

2. **Register in `src/runtime/builtin_functions.rs`:**
```rust
pub fn get_builtin_function(name: &str) -> Option<fn(Vec<Value>) -> Result<Value, Error>> {
    match name {
        "CUBE" => Some(arithmetic::cube),
        // ... existing functions
        _ => None,
    }
}
```

3. **Add test in `tests/arithmetic_tests.rs`:**
```rust
#[test]
fn test_cube() {
    assert_eq!(evaluate("CUBE(3)").unwrap(), Value::Number(27.0));
    assert_eq!(evaluate("CUBE(-2)").unwrap(), Value::Number(-8.0));
}
```

4. **Document in `API_REFERENCE.md`:**
```markdown
#### `CUBE(number)`
Cube a number (n³).
\`\`\`bash
CUBE(3)    # 27
CUBE(-2)   # -8
\`\`\`
```

### 10.2 Adding a New Method

**Example:** Add `.square()` method to numbers

1. **Implement in `src/runtime/method_calls/mod.rs`:**
```rust
fn call_number_method(n: f64, method: &str, args: Vec<Value>) -> Result<Value, Error> {
    match method {
        "square" => {
            if !args.is_empty() {
                return Err(Error::new("square() takes no arguments", None));
            }
            Ok(Value::Number(n * n))
        }
        // ... existing methods
        _ => Err(Error::new(&format!("Unknown number method: {}", method), None)),
    }
}
```

2. **Add test:**
```rust
#[test]
fn test_square_method() {
    assert_eq!(evaluate("(5).square()").unwrap(), Value::Number(25.0));
    assert_eq!(evaluate("(-3).square()").unwrap(), Value::Number(9.0));
}
```

### 10.3 Adding JSONPath Support for New Use Case

**Example:** Support aggregation in JQ function

Already implemented! JQ returns arrays that can be used with SUM/AVG/MIN/MAX:

```bash
sk 'SUM(JQ(:arguments, "$.items[*].price"))' --json '{"items":[{"price":10},{"price":20}]}'
```

To extend with new JSONPath features, modify `src/runtime/jsonpath.rs`.

### 10.4 Performance Profiling

```bash
# Install cargo-flamegraph
cargo install flamegraph

# Profile sk binary
cargo flamegraph --bin sk -- "complex_expression_here"

# Profile tests
cargo flamegraph --test higher_order_tests -- test_filter_large

# View flamegraph.svg in browser
```

**Look for:**
- Red blocks = hot paths
- Wide blocks = time-consuming functions
- Clone operations in loops
- Unnecessary allocations

---

## 11. Troubleshooting & Known Issues

### 11.1 Compilation Issues

**Issue:** `rquickjs` fails to compile
- **Cause:** Missing system dependencies for QuickJS
- **Fix:** Disable plugins feature: `cargo build --no-default-features`

**Issue:** Linker errors on macOS
- **Cause:** Outdated Xcode command-line tools
- **Fix:** `xcode-select --install`

### 11.2 Runtime Issues

**Issue:** "Missing variable: :x" in lambda
- **Cause:** Lambda parameter name mismatch
- **Fix:** Use correct parameter name or specify custom name:
  ```bash
  [1,2,3].filter(:item > 2, 'item')  # Custom name
  ```

**Issue:** Null reference errors
- **Cause:** Accessing methods on null without safe navigation
- **Fix:** Use `&.` operator or conversion methods:
  ```bash
  :user&.name&.length()       # Safe navigation
  :user.name.to_s().length()  # Conversion method
  ```

**Issue:** Type mismatch errors
- **Cause:** Unexpected type in operation
- **Fix:** Use conversion methods or type checks:
  ```bash
  :value.to_i() + 10          # Force to integer
  IF(ISNUMBER(:value), :value, 0)  # Type check
  ```

### 11.3 Performance Issues

**Issue:** Slow evaluation for large arrays
- **Diagnosis:** Profile with `cargo flamegraph`
- **Common causes:**
  - Excessive cloning in loops
  - Deep recursion in nested arrays
  - Inefficient string operations
- **Fixes:** See [Performance Optimization Patterns](#7-performance-optimization-patterns)

### 11.4 Test Failures

**Issue:** `sk_concurrency` tests fail intermittently
- **Cause:** Cargo file lock contention (known issue)
- **Workaround:** `cargo test --lib` to skip

**Issue:** Tests fail after adding new function
- **Checklist:**
  - ✅ Function registered in `builtin_functions.rs`?
  - ✅ Test name unique (no conflicts)?
  - ✅ Expected vs actual values match exactly?

---

## 12. Future Development Guidelines

### 12.1 Planned Features (Roadmap)

1. **WebAssembly Support** (High Priority)
   - Compile Skillet to WASM for browser/Cloudflare Workers
   - Target: `wasm32-unknown-unknown`

2. **Enhanced JSONPath** (Medium Priority)
   - Support for recursive descent (`$..property`)
   - Script expressions in filters

3. **Streaming Evaluation** (Low Priority)
   - Process large datasets without loading into memory
   - Iterator-based approach

4. **Query Optimization** (Low Priority)
   - AST-level optimizations (constant folding, dead code elimination)
   - Compilation to bytecode

### 12.2 Architecture Evolution

**Current State:** Interpreter (tree-walking evaluator)

**Future Options:**
1. **Bytecode Compiler** (Medium term)
   - Compile AST → bytecode once, execute multiple times
   - Better performance for repeated evaluations
   - Example: Lua, Python

2. **JIT Compilation** (Long term)
   - Compile hot paths to native code
   - Requires significant complexity
   - Example: JavaScript V8, LuaJIT

**Recommended Next Step:** Bytecode compiler for ~2x performance gain with moderate complexity.

### 12.3 Code Quality Guidelines

When making changes:

1. **Maintain Backward Compatibility**
   - Existing expressions should continue to work
   - Deprecate features gradually (don't remove suddenly)

2. **Performance First**
   - Profile before optimizing
   - Measure impact with benchmarks
   - Document performance characteristics

3. **Test Coverage**
   - Add tests for new features
   - Include edge cases (null, empty arrays, type mismatches)
   - Test error messages

4. **Documentation**
   - Update `DOCUMENTATION.md` for user-facing features
   - Update `API_REFERENCE.md` for function/method additions
   - Update this `DEVELOPMENT_GUIDE.md` for architecture changes

5. **Error Messages**
   - Clear and actionable
   - Include position information when possible
   - Suggest fixes where applicable

### 12.4 Extension Guidelines

**Adding Features:**
- ✅ Does it align with Skillet's design philosophy?
- ✅ Is it useful for the primary use cases?
- ✅ Can it be implemented efficiently?
- ❌ Avoid language bloat (not everything needs to be built-in)

**Deprecating Features:**
- Announce in release notes
- Provide migration path
- Keep for at least 2 major versions

---

## 13. Quick Reference Card

### Essential Commands

```bash
# Build & test
cargo build --release
cargo test --lib

# Run expression
cargo run --bin sk -- "expression" var1=value1

# Start HTTP server
cargo run --bin sk_http_server 5074

# Benchmark
cargo bench
bash scripts/benchmark_server.sh 8080 10000 8
```

### Key Files for Common Tasks

| Task | Files to Modify |
|------|-----------------|
| Add function | `runtime/arithmetic.rs`, `builtin_functions.rs`, `tests/*_tests.rs` |
| Add method | `method_calls/mod.rs`, `method_calls/*_methods.rs`, tests |
| Add operator | `lexer.rs`, `parser.rs`, `evaluation/core.rs`, tests |
| Fix performance | Profile → identify hot path → optimize (see §7) |
| Add JS plugin | Create `hooks/my_function.js` |

### Critical Performance Rules

1. **Reuse HashMap in loops** (don't clone per iteration)
2. **Use `&[Value]` slices** (avoid array clones)
3. **Pre-allocate `Vec`** when size is known
4. **Match on references** (`&Value`) to avoid clones
5. **Profile before optimizing** (use `cargo flamegraph`)

---

## Conclusion

This guide provides a comprehensive foundation for understanding and extending Skillet. Whether you're:
- **An LLM assistant** helping a developer
- **A contributor** adding new features
- **A maintainer** fixing bugs

You now have the context to work effectively with this codebase.

**Key Takeaways:**
- Skillet prioritizes **performance, safety, and familiarity**
- The architecture is **modular and extensible**
- **Null safety** is achieved through `&.` and conversion methods
- **Performance optimizations** focus on reducing allocations
- **Testing is comprehensive** with clear patterns to follow

Happy coding! 🚀

---

**Document Maintenance:**
- Update this guide when making architectural changes
- Keep code examples in sync with latest implementation
- Add new patterns to the troubleshooting section as discovered

**Last updated:** March 2026 by Claude Code (Sonnet 4.5)
