# 0029. Logical operators and the Excel logical functions

- Status: Accepted
- Date: 2026-11-21
- Deciders: Maintainer
- Related: BL-073, BL-069 and #93 (`AND`/`OR` operators), ADR-0018 D-8 (implemented here: B-18),
  ADR-0006 (null safety), ADR-0023 (no conversion in comparisons), ADR-0024 (reserved names),
  [`docs/reference/excel-compat.md`](../reference/excel-compat.md)

## Context
- #93 (BL-069) made `AND` and `OR` between two operands parse as the `&&`/`||` operators. Before,
  the parser dropped them silently (`TRUE AND FALSE` was TRUE). No ADR recorded the operators.
- The operators evaluated both sides, so `:n == 0 OR 10 / :n > 1` failed when `:n` was 0.
- `NOT TRUE` was a parse error. Only `!x` and `NOT(x)` worked.
- ADR-0018 D-8 decided that a text condition in `IF` is `#VALUE!` and that `IFS` with no true
  condition is `#N/A`. The changelog and the migration guide said so, but the code still returned
  `IF("x", 1, 2)` → `2` and `IFS(FALSE, 1)` → FALSE (B-18).
- The `AND`, `OR`, `NOT` and `XOR` builtins differed from Excel: `AND()` was TRUE, text counted as
  FALSE, and `XOR` took exactly two arguments.

On 2026-11-21 the maintainer decided the points below.

## Decision

### 1. `AND` / `OR` operators (records #93)
- `AND` and `OR`, in any case, between two operands are the same operators as `&&` and `||`.
  `AND` binds tighter than `OR`, and both bind looser than the comparisons (`docs/reference/api.md`,
  Precedence). In operand position, `AND(`/`OR(` is the builtin call:
  `AND(TRUE, FALSE) OR TRUE` is TRUE.
- Both sides must be booleans (`1 AND TRUE` is an error, as before).

### 2. Short-circuit
The operators evaluate the right side only when the left side does not decide the result:
`FALSE AND 1/0` is FALSE and `TRUE OR 1/0` is TRUE. When the left side is not a boolean, the
right side is still evaluated and the result is the same type error as before.

The builtin calls `AND(...)`, `OR(...)` and `XOR(...)` follow Excel, which evaluates every argument:
`AND(FALSE, 1/0)` is an error.

### 3. `NOT` as a prefix keyword
- `NOT x`, in any case (`NOT TRUE`, `not (:a > 1)`), negates its operand with the precedence of
  `!`. It binds tighter than `^` and every binary operator, so `NOT TRUE AND FALSE` is
  `(NOT TRUE) AND FALSE`, and `NOT :x > 1` is `(NOT :x) > 1`. Write `NOT (:x > 1)` to negate a
  comparison.
- A `NOT` name followed directly by `(`, with no space, is the `NOT(...)` builtin call. With a
  space, `NOT (x)` is the keyword. The keyword parses to the same call, `NOT(x)`, so both give the
  same result and the same errors: `NOT 0` is TRUE and `NOT "x"` is `#VALUE!`. The only
  difference is that the call is an atom: `NOT(TRUE).to_s()` is `"false"`, while
  `NOT (TRUE).to_s()` is `NOT("true")`, an error.
- `!` is unchanged and takes booleans only (`!0` is an error).

### 4. `IF` and `IFS` conditions (ADR-0018 D-8, B-18)
- A condition is TRUE/FALSE, or a number: non-zero is TRUE, as in Excel. A Currency value counts as
  a number. `NULL` is FALSE, like a blank cell (ADR-0006).
- Any other condition is `#VALUE!`: text (`IF("x", 1, 2)`, also `"TRUE"`), arrays, JSON objects and
  DateTimes. Message: `IF condition must be TRUE, FALSE or a number, got string (#VALUE!)`
  (`IFS ...` for `IFS`).
- `IFS` with no TRUE condition is `IFS: no condition is TRUE (#N/A)`.
- The lazy evaluation of D-8 is unchanged. The eager path (a call with a spread argument) has the
  same rules.

### 5. `AND`, `OR`, `NOT`, `XOR` builtins (Excel)
- **Values.** `TRUE`/`FALSE` count as themselves, and numbers (and Currency) as non-zero. An array
  argument is flattened, nested arrays included; text, `NULL` and other items in it are ignored,
  as Excel ignores text and empty cells in a range. A `NULL` argument is ignored too (an empty
  cell). Text, a DateTime or a JSON object passed directly is `#VALUE!`:
  `AND expects logical values, got string (#VALUE!)`.
- **No logical values** (`AND()`, `AND([])`, `AND(NULL)`, `OR(["a"])`) is
  `AND: no logical values (#VALUE!)`.
- **`AND`** is TRUE when every value is TRUE, **`OR`** when at least one is, and **`XOR`** when an
  odd number are TRUE. `XOR` takes any number of arguments: `XOR(TRUE, TRUE, TRUE)` is TRUE,
  `XOR(TRUE)` is TRUE.
- **`NOT`** takes exactly one argument: `NOT expects 1 argument` otherwise. TRUE or a non-zero
  number gives FALSE; FALSE, 0 or `NULL` gives TRUE. Text, an array, a DateTime or a JSON object is
  `NOT expects a logical value, got string (#VALUE!)`.
- Excel limits the argument count (255 for `AND`/`OR`, 254 for `XOR`). Skillet does not.

### Where the Excel documentation was not clear
These choices follow Microsoft's documentation where it speaks, and the maintainer's rules where
it does not. Each one is pinned by a test in `tests/logic_operators.rs`.
- **Text `"TRUE"`/`"FALSE"` passed directly.** The documentation says arguments must evaluate to
  logical values, but Excel probably converts the text `"TRUE"` typed into the formula
  (`=AND("TRUE")`). Not verified in a workbook. Skillet follows the maintainer's rule: direct text
  is `#VALUE!`.
- **A `NULL` argument to `AND`/`OR`/`XOR`.** Treated as an empty cell (ignored), so
  `AND(TRUE, NULL)` is TRUE. It was FALSE before. `NOT(NULL)` is TRUE, as Excel's `NOT` of an empty
  cell.
- **`NOT` of an array.** Excel's `NOT` of a range spills one result per cell. Skillet has no
  spill, so it is `#VALUE!`.
- **Currency and DateTime.** Excel has neither type: money is a number, and a date is a serial
  number. Skillet counts Currency as a number and makes a DateTime `#VALUE!`, because DateTime is
  a real type here (ADR-0018, `YEAR(46000)`).

## Breaking changes (ship in 0.7.0)

| Expression | Before | After |
|---|---|---|
| `IF("x", 1, 2)`, `IF([1], 1, 2)` | `2` | `Error` (`#VALUE!`) |
| `IFS(FALSE, 1)` | `false` | `Error` (`#N/A`) |
| `AND(TRUE, NULL)`, `AND(TRUE, [TRUE, "x"])` | `false` | `true` |
| `AND(TRUE, "x")`, `OR(FALSE, "TRUE")`, `NOT("x")` | `false`, `false`, `true` | `Error` (`#VALUE!`) |
| `AND()`, `OR()`, `AND([])` | `true`, `false`, `true` | `Error` (`#VALUE!`) |
| `FALSE AND 1/0`, `TRUE OR UPPER(1)` | `Error` | `false`, `true` |

Additive: `NOT x` (was a parse error), `XOR` with 1 or 3+ arguments (was an error).

## Consequences
- \+ `:n == 0 OR 10 / :n > 1` and `:list.length() > 0 AND :list[0] > 5` are safe guards.
- \+ Logical functions copied from a spreadsheet give the spreadsheet's result or an error.
- − `AND(...)` and `a AND b` differ: the function evaluates every argument and accepts numbers;
  the operator stops early and takes booleans only. The reference says so.
- − `NOT :x > 1` does not mean `NOT (:x > 1)`. This matches `!` and Excel's tight unary operators,
  but readers from SQL or Python may expect the other meaning; the reference and the migration
  guide call it out.
- − `AND(TRUE, NULL)` changes from FALSE to TRUE without an error. The migration guide lists it
  first.

## Alternatives considered
- **`NOT` with the precedence of SQL/Python (looser than comparisons).** Not chosen: the
  maintainer gave `NOT` the precedence of `!`, so the two prefix forms parse alike.
- **Short-circuit the builtins too.** Not chosen: Excel evaluates every argument of `AND`/`OR`,
  and ADR-0018 makes Excel the contract for UPPERCASE builtins.
- **`NULL` as FALSE in `AND`/`OR`/`XOR`, as in `IF`.** Not chosen by the implementation: Excel
  ignores empty cells there, and ADR-0018 treats `NULL` like a blank cell in aggregates. Open for
  the maintainer to confirm (see "Where the Excel documentation was not clear").
