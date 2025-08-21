![logo](/skillet-logo.png)

# Skillet — “Lightning-fast formulas, Rust-powered.”

Skillet is a tiny, embeddable expression engine (written in Rust) inspired by Excel formulas and Ruby-style chaining. It parses expressions into an AST and evaluates them with a small runtime.

This MVP supports numbers, strings, booleans, nulls, arrays, method chaining, functions (built-ins), comparisons, logical ops, ternary, array indexing/slicing, spread `...`, lambdas with named parameters, and basic type casting via `::Type`.

Skilled can be extended with JS, take a look at [Documentation](DOCUMENTATION.md)

## Build

- Requirements: Rust stable (2021 edition)
- Build and test:

```
cargo build
cargo test
```

## CLI (quick try)

A minimal CLI is included to evaluate expressions without external variables.

```
cargo run --bin sk -- "= [30,60,80,100].filter(:x > 50).map(:x * 0.9).sum()"
```

Notes:
- Wrap the expression in quotes in your shell.
- A leading `=` is optional (supported for spreadsheet-style familiarity).

## Library Usage

Add to your Cargo project (path example):

```toml
[dependencies]
skillet = { path = "../skillet" }
```

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
- No external dependencies (serde/chrono) are used yet.
- For variables beyond numbers/strings/arrays (e.g., dates, currency), see `Value` in `src/types.rs`.

## Tests

Run the test suite:

```
cargo test
```

## Author

[@zenbakiak](/zenbakiak)

## License

MIT OR Apache-2.0
