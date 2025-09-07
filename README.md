![logo](/skillet-logo.png)

# Skillet — “Lightning-fast formulas, Rust-powered.”

[![Crates.io](https://img.shields.io/crates/v/skillet.svg)](https://crates.io/crates/skillet)
[![Docs.rs](https://docs.rs/skillet/badge.svg)](https://docs.rs/skillet)

Skillet is a high-performance, embeddable expression engine written in Rust, inspired by Excel formulas with Ruby-style method chaining. It parses expressions into an AST and evaluates them with an optimized runtime.

**✨ New Features:**
- **Ruby-style Type Conversion Methods**: `null.to_s()`, `"123".to_i()`, `[1,2,3].to_bool()` - available on all types
- **Safe Navigation Operator**: `obj&.property&.method()` - prevents null reference errors
- **Enhanced Null Safety**: Conversion methods provide safe defaults for null values
- **Performance Optimized**: ~3ms evaluation time (100x+ improvement from original 300ms)

## Core Features

- 🚀 **Lightning Fast**: Optimized parser with string interning and memory pooling
- 🛡️ **Null Safe**: Safe navigation (`&.`) and conversion methods handle null gracefully
- 🔧 **Extensible**: JavaScript plugins for runtime extensibility without recompilation
- 📊 **Excel-like**: Familiar syntax with advanced features like array operations
- 🦀 **Rust-powered**: Memory safe with zero-cost abstractions
- 🎯 **Type Smart**: Ruby-style conversions with automatic type coercion

**Supported Types**: Numbers, strings, booleans, nulls, arrays, JSON objects, dates, currency  
**Operations**: Arithmetic, logical, comparisons, method chaining, array operations, lambdas  
**Extensions**: JavaScript plugins, Rust custom functions, HTTP/TCP server modes

📚 **[Full Documentation](DOCUMENTATION.md)** | 📖 **[API Reference](API_REFERENCE.md)**

## Build

- Requirements: Rust stable (2021 edition)
- Build and test:

```
cargo build
cargo test
```

## Quick Examples

**Traditional Excel-style formulas:**
```bash
cargo run --bin sk -- "SUM(1, 2, 3, 4, 5)"                    # 15
cargo run --bin sk -- "IF(10 > 5, \"Yes\", \"No\")"           # "Yes"  
cargo run --bin sk -- "AVERAGE([85, 92, 78, 90])"             # 86.25
```

**✨ New: Null-safe operations with conversion methods:**
```bash
cargo run --bin sk -- "null.to_s().length()"                  # 0 (no error!)
cargo run --bin sk -- "\"123\".to_i() + 10"                   # 133
cargo run --bin sk -- "[null, \"hello\"].map(:x.to_s())"      # ["", "hello"]
```

**✨ New: Safe navigation operator:**
```bash
cargo run --bin sk -- ":data := {\"name\": null}; :data&.name&.length()"  # null (no error!)
```

**Advanced array operations:**
```bash
cargo run --bin sk -- "[30,60,80,100].filter(:x > 50).map(:x * 0.9).sum()"  # 216
```

**Notes:**
- Wrap expressions in quotes in your shell
- A leading `=` is optional (supported for Excel-style familiarity)

## Library Usage

Add to your Cargo project (from crates.io):

```toml
[dependencies]
skillet = "0.2.0"
```

Or with cargo-edit:

```
cargo add skillet@0.2.0
```

## Server Modes

Skillet includes production-ready HTTP and TCP servers for high-performance expression evaluation.

### 🌐 HTTP Server (`sk_http_server`)

Run the HTTP server for REST API access:

```bash
# Basic usage
cargo run --bin sk_http_server 5074

# Production deployment
cargo run --bin sk_http_server 5074 --host 0.0.0.0 --token your_secret_token

# Background daemon
cargo run --bin sk_http_server 5074 -d --host 0.0.0.0 --token secret123 --admin-token admin456
```

**Parameters:**
- `<port>` - Port to bind (required)
- `-H, --host <addr>` - Bind address (default: 127.0.0.1)
- `-d, --daemon` - Run as background daemon
- `--token <value>` - Require token for eval requests
- `--admin-token <value>` - Require admin token for JS function management
- `--pid-file <file>` - PID file for daemon mode
- `--log-file <file>` - Log file for daemon mode

**HTTP Endpoints:**
- `GET /health` - Health check
- `GET /` - API documentation
- `POST /eval` - Evaluate expressions (JSON body)
- `GET /eval?expr=...` - Evaluate expressions (query params)
- `POST /js/functions` - Upload JavaScript functions (admin)
- `GET /js/functions` - List JavaScript functions
- `DELETE /js/functions/{name}` - Delete JavaScript function (admin)

**Example API calls:**
```bash
# Basic evaluation
curl -X POST http://localhost:5074/eval \
  -H "Content-Type: application/json" \
  -d '{"expression": "2 + 3 * 4"}'

# With variables and null safety
curl -X POST http://localhost:5074/eval \
  -H "Content-Type: application/json" \
  -d '{
    "expression": ":data.filter(:x.value.to_s().length() > 0)",
    "arguments": {
      "data": [{"value": null}, {"value": "hello"}, {"value": ""}]
    },
    "include_variables": true
  }'

# GET request with query params
curl "http://localhost:5074/eval?expr=SUM(1,2,3,4,5)"

# With authentication
curl -X POST http://localhost:5074/eval \
  -H "Authorization: Bearer your_secret_token" \
  -H "Content-Type: application/json" \
  -d '{"expression": "null.to_s().length()"}'
```

### ⚡ TCP Server (`sk_server`)

High-performance TCP server for custom protocol access:

```bash
# Basic usage
cargo run --bin sk_server 8080

# With worker threads
cargo run --bin sk_server 8080 16

# Production daemon
cargo run --bin sk_server 8080 8 -d --host 0.0.0.0 --token secret123
```

**Parameters:**
- `<port>` - Port to bind (required)
- `[num_threads]` - Worker threads (optional)
- `-H, --host <addr>` - Bind address (default: 127.0.0.1)
- `-d, --daemon` - Run as background daemon
- `--token <value>` - Require authentication token
- `--pid-file <file>` - PID file for daemon mode
- `--log-file <file>` - Log file for daemon mode

**Protocol:** JSON-based TCP protocol for maximum performance

## Library Usage

Evaluate expressions:

```rust
use skillet::{evaluate, evaluate_with, Value};
use std::collections::HashMap;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Numeric
    let v = evaluate("= 2 + 3 * 4")?; // -> Value::Number(14.0)

    // Variables
    let mut vars = HashMap::new();
    vars.insert("sales".to_string(), Value::Number(5000.0));
    let v = evaluate_with("=SUM(:sales, 1000)", &vars)?; // -> 6000.0

    // Strings and chaining
    let v = evaluate("= '  john  '.trim().upper()")?; // -> "JOHN"

    // Arrays + F/M/R
    let v = evaluate("= [30,60,80,100].filter(:x > 50).map(:x * 0.9).sum()")?; // -> 216.0

    // Named lambda parameters
    let v = evaluate("= FILTER([1,2,3,4], :n % 2 == 0, 'n')")?; // -> [2,4]

    // Type casting
    let v = evaluate("= '42'::Integer")?; // -> 42
    Ok(())
}
```

## Language Features (MVP)

- Numbers, booleans (`TRUE`/`FALSE`), strings ('...' or "..."), `NULL`
- Operators: `+ - * / % ^`, `> < >= <= == !=`, `AND/OR/NOT` (also `&&/||/!`), ternary `? :`
- Variables: `:name` (provided via `evaluate_with` map)
- Functions (subset):
  - Math: `SUM`, `AVG/AVERAGE`, `MIN`, `MAX`, `ROUND`, `CEIL`, `FLOOR`, `ABS`, `SQRT`, `POW`
  - Arrays: `ARRAY`, `FIRST`, `LAST`, `CONTAINS`, `UNIQUE`, `SORT`, `REVERSE`, `JOIN`, `FLATTEN`
  - Strings: `CONCAT`, `UPPER`, `LOWER`, `TRIM`, `LENGTH`, `SPLIT`, `REPLACE`
  - Logic: `ISBLANK`
  - Functional: `FILTER(array, expr, [param])`, `MAP(array, expr, [param])`, `REDUCE(array, expr, initial, [valParam], [accParam])`
  - Conditional aggregations: `SUMIF(array, expr)`, `AVGIF(array, expr)`, `COUNTIF(array, expr)`
- Methods (subset): chaining with `.` and predicates `?`
  - Numbers: `.abs() .round(n) .floor() .ceil()`; predicates `.positive? .negative? .zero? .even? .odd? .numeric?`
  - Arrays: `.length() .size() .first() .last() .sum() .avg() .min() .max() .sort() .unique() .reverse() .compact() .flatten()`
  - Strings: `.upper() .lower() .trim() .reverse()`
- Arrays: literals `[1, 2, 3]`; indexing `arr[i]` (negatives allowed); slicing `arr[a:b]`
- Spread: `...expr` inside arg lists
- Casting: `expr::Integer|Float|String|Boolean|Array|Currency|DateTime|Json`

## Examples

- Arithmetic precedence: `= 2 + 3 * 4` → `14`
- Ternary: `= :score >= 90 ? 'A' : 'B'`
- Named lambda param: `= [1,2,3,4].map(:v * 10, 'v')` → `[10,20,30,40]`
- Reduce with named params: `= [1,2,3].reduce(:a + :v, 0, 'v', 'a')` → `6`
- SUMIF: `= SUMIF([1,-2,3,-4], :x > 0)` → `4`
- FLATTEN: `= FLATTEN([1,[2,[3]],4])` → `[1,2,3,4]`

## Notes

- This is an MVP; error messages and type coverage are intentionally minimal.
- For variables beyond numbers/strings/arrays (e.g., dates, currency), see `Value` in `src/types.rs`.

## Install Binaries

If you want the binaries such as `sk`, `sk_server` and `sk_client` installed system-wide:

```
cargo install skillet
```

## Server Mode

Skillet includes a high-performance evaluation server that keeps the interpreter warm and eliminates per-process overhead.

- Start the server: `sk_server 8080` (binds to 127.0.0.1:8080)
- Daemonize (Unix): `sk_server 8080 -d` (writes PID to `skillet-server.pid` in CWD)
- Stop daemon: `kill $(cat skillet-server.pid)`
- Bind host/IP: `sk_server 8080 --host 0.0.0.0` (listen on all interfaces)
- Optional token auth: `sk_server 8080 --host 0.0.0.0 --token <secret>` (or set `SKILLET_AUTH_TOKEN`)

Client and benchmarks:
- One-off eval: `sk_client localhost:8080 '=2+3*4'`
- With variables: `sk_client localhost:8080 '=SUM(:a,:b)' a=10 b=5`
- JSON vars: `sk_client localhost:8080 '=:user.name' --json '{"user":{"name":"Alice"}}'`
- Benchmark: `sk_client localhost:8080 --benchmark '=2+3*4' 10000`
- With token: `sk_client localhost:8080 '=2+3*4' --token <secret>` (or set `SKILLET_SERVER_TOKEN`)

Scripts:
- Build + run multi-test benchmark: `bash scripts/benchmark_server.sh [port] [iterations] [threads]`

> take a look at the [Server Usage Guide](SERVER_USAGE_GUIDE.md) for more details about how to use it and consume in different langauages

## Built-in Functions

- Arithmetic: `SUM`, `AVG`/`AVERAGE`, `MIN`, `MAX`, `ROUND`, `CEIL`, `CEILING`, `FLOOR`, `ABS`, `SQRT`, `POW`/`POWER`, `MOD`, `INT`
- Logical: `AND`, `OR`, `NOT`, `XOR`, `IF`, `IFS`
- String: `LENGTH`, `CONCAT`, `UPPER`, `LOWER`, `TRIM`, `SUBSTRING`, `SPLIT`, `REPLACE`, `REVERSE`, `ISBLANK`, `ISNUMBER`, `ISTEXT`
- Array: `ARRAY`, `FLATTEN`, `FIRST`, `LAST`, `CONTAINS`, `IN`, `COUNT`, `UNIQUE`, `SORT`, `REVERSE`, `JOIN`
- Date/Time: `NOW`, `DATE`, `TIME`, `YEAR`, `MONTH`, `DAY`, `DATEADD`, `DATEDIFF`
- Financial: `PMT`, `DB`, `FV`, `IPMT`
- Statistical: `MEDIAN`, `MODE.SNGL` (`MODESNGL`, `MODE_SNGL`), `STDEV.P` (`STDEVP`, `STDEV_P`), `VAR.P` (`VARP`, `VAR_P`), `PERCENTILE.INC` (`PERCENTILEINC`, `PERCENTILE_INC`), `QUARTILE.INC` (`QUARTILEINC`, `QUARTILE_INC`)
- Functional: `FILTER(array, expr, [param])`, `MAP(array, expr, [param])`, `REDUCE(array, expr, initial, [valParam], [accParam])`, `SUMIF(array, expr)`, `AVGIF(array, expr)`, `COUNTIF(array, expr)`

## API Surface (Rust)

- `parse(input: &str) -> Result<Expr, Error>`: parse into AST
- `evaluate(input: &str) -> Result<Value, Error>`: evaluate without variables
- `evaluate_with(input: &str, vars: &HashMap<String, Value>) -> Result<Value, Error>`
- `evaluate_with_json(input: &str, json_vars: &str) -> Result<Value, Error>`
- `evaluate_with_custom(input: &str, vars: &HashMap<String, Value>) -> Result<Value, Error>`
- `evaluate_with_json_custom(input: &str, json_vars: &str) -> Result<Value, Error>`
- Custom functions:
  - `register_function(Box<dyn CustomFunction>) -> Result<(), Error>`
  - `unregister_function(name: &str) -> bool`
  - `list_custom_functions() -> Vec<String>`
- Types:
  - `Value` enum: `Number(f64) | Array(Vec<Value>) | Boolean(bool) | String(String) | Null | Currency(f64) | DateTime(i64) | Json(String)`
  - `Error` with `message` and optional `position`


## Tests

Run the test suite:

```
cargo test
```

## Author

[@zenbakiak](/zenbakiak)

[Github repo](https://github.com/zenbakiak/skillet)

## License

MIT OR Apache-2.0
