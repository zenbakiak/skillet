# 0029. Logical operators and the Excel logical functions

- Status: Accepted
- Date: 2026-11-21
- Deciders: Maintainer
- Related: BL-073, BL-069 and #93 (`AND`/`OR` operators), ADR-0018 D-8 (implemented here: B-18),
  ADR-0006 (null safety), ADR-0023 (no conversion in comparisons), ADR-0024 (reserved names),
  [`docs/reference/excel-compat.md`](../reference/excel-compat.md)

> **Amendment 1 (maintainer, 2026-11-22, BL-089):** every condition uses the `IF` rule (§4):
> `? :`, the `AND`/`OR`/`&&`/`||` operators, `!`, the `filter`/`find`/`FILTER`/`FIND` lambdas and
> lambda criteria in `COUNTIF`/`SUMIF`/`AVERAGEIF`. The `AND()`/`OR()`/`XOR()` builtins keep
> ignoring `NULL`. A NaN condition is a `#NUM!` error. `IF` with more than 3 arguments is an error. This replaces "Both sides must be
> booleans" in §1, "When the left side is not a boolean, the right side is still evaluated" in §2,
> and "`!` ... takes booleans only" in §3. Full text and the before/after table:
> [Amendment 1](#amendment-1-maintainer-2026-11-22-bl-089) below.

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
- `NOT x`, in any case (`NOT TRUE`, `not (:a > 1)`), negates its operand. As in VB and SQL it
  binds looser than the comparisons and tighter than `AND`. From tightest to loosest:
  `!` and the other unary operators, `^`, arithmetic, `&`, comparisons, `NOT`, `AND`/`&&`,
  `OR`/`||`, `? :`.
  - `NOT :x > 1` is `NOT (:x > 1)`.
  - `NOT 1 > 2 AND TRUE` is `(NOT (1 > 2)) AND TRUE`.
  - `NOT :a OR :b` is `(NOT :a) OR :b`, and `:a AND NOT :b == :c` is `:a AND NOT (:b == :c)`.
  - `NOT NOT x` nests.
- `NOT` starts an operand of `AND`, `OR`, `? :` or the whole expression. Inside a tighter operator
  it is a parse error, `NOT binds looser than comparisons here; write (NOT x)`: `1 + NOT TRUE`,
  `-NOT TRUE`, `TRUE == NOT FALSE`. Write `TRUE == (NOT FALSE)`.
- A `NOT` name followed directly by `(`, with no space, is the `NOT(...)` builtin call. It is an
  atom, so it is allowed anywhere: `TRUE == NOT(FALSE)`, `NOT(TRUE).to_s()`. With a space,
  `NOT (x)` is the keyword, whose operand runs to the next `AND`/`OR`: `NOT (0) == FALSE` is
  `NOT ((0) == FALSE)`. The keyword parses to the same call, `NOT(operand)`, so the builtin's rules
  apply: `NOT 0` is TRUE and `NOT "x"` is `#VALUE!`.
- `!` is unchanged: a tight unary that takes booleans only. `!TRUE == FALSE` is
  `(!TRUE) == FALSE`, and `!0` is an error.

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
it does not. The maintainer accepted all four on 2026-11-21. Each one is pinned by a test in
`tests/logic_operators.rs`.
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
| `NOT (0) == FALSE` (space before `(`) | `false` (`NOT(0) == FALSE`) | `true` (`NOT (0 == FALSE)`) |

Additive: `NOT x` (was a parse error), `XOR` with 1 or 3+ arguments (was an error).

## Consequences
- \+ `:n == 0 OR 10 / :n > 1` and `:list.length() > 0 AND :list[0] > 5` are safe guards.
- \+ Logical functions copied from a spreadsheet give the spreadsheet's result or an error.
- − `AND(...)` and `a AND b` differ: the function evaluates every argument and accepts numbers;
  the operator stops early and takes booleans only. The reference says so.
- \+ `NOT :x > 1` reads as it does in VB, SQL and Python: `NOT (:x > 1)`.
- − `NOT` and `!` have different precedence: `NOT :a == :b` is `NOT (:a == :b)`, while
  `!:a == :b` is `(!:a) == :b`. The reference shows both.
- − `NOT (x) ...` with a space before `(` changed meaning when a comparison follows (table above).
- − `AND(TRUE, NULL)` changes from FALSE to TRUE without an error. The migration guide lists it
  first.

## Alternatives considered
- **`NOT` with the precedence of `!`.** An earlier draft of this decision. The maintainer changed
  it on 2026-11-21: with it, `NOT :x > 1` was `(NOT :x) > 1`, an error for a number, which is
  not what readers from VB or SQL expect.
- **`NOT` allowed inside tighter operators (`1 == NOT x`).** Not chosen: the operand would have to
  stop at an unclear point. A parse error that asks for parentheses is clearer.
- **Short-circuit the builtins too.** Not chosen: Excel evaluates every argument of `AND`/`OR`,
  and ADR-0018 makes Excel the contract for UPPERCASE builtins.
- **`NULL` as FALSE in `AND`/`OR`/`XOR`, as in `IF`.** Not chosen: Excel ignores empty cells
  there, and ADR-0018 treats `NULL` like a blank cell in aggregates. The maintainer confirmed
  `AND(TRUE, NULL)` = TRUE on 2026-11-21.

## Amendment 1 (maintainer, 2026-11-22, BL-089)

### Context
The audit of 2026-11-22 (LANG-2, LANG-3, QA-9) found four truth rules. A missing flag, which `&.`
turns into `NULL` (ADR-0006), got five answers: `AND(:u&.is_admin, :on)` was TRUE,
`IF(:u&.is_admin, :on, FALSE)` was FALSE, and `:u&.is_admin AND :on`, `:u&.is_admin ? :on : FALSE`
and `!:u&.is_admin` were errors. The operators depended on the order of their operands:
`FALSE AND NULL` was FALSE, `NULL AND FALSE` an error. `1 ? 1 : 2` was an error although
`IF(1, 1, 2)` is 1. `filter`, `find`, `FILTER` and `FIND` kept an item only when the lambda gave
exactly TRUE, so `[0, 1, 2].filter(:x)` was `[]` and `[1, 2].filter("yes")` dropped every item
without an error, while `COUNTIF([1, 2, 3], :x)` was an error. `IF(1, 2, 3, 4)` ignored the
fourth argument, which Excel rejects.

On 2026-11-22 the maintainer decided "A + keep": one rule for every condition (option A of
LANG-3), and keep the `AND()`/`OR()` builtins as they are.

### Decision

#### A1. The condition rule
A **condition** is the value that decides a branch, an operator or whether an item matches. It is
read with the rule of §4 (`logical::condition`), unchanged:

| Value | As a condition |
|---|---|
| `TRUE`, `FALSE` | itself |
| a Number or a Currency | `FALSE` when it is 0, `TRUE` otherwise (`2.5`, `-1`, `2::Currency`) |
| NaN (a Number or Currency that is not a number) | error, Excel `#NUM!` |
| `NULL` | `FALSE` (a blank cell, ADR-0006) |
| text (also `""` and `"TRUE"`), a DateTime, an array (also `[]`), a JSON object | error, Excel `#VALUE!` |

The error message is `<NAME> condition must be TRUE, FALSE or a number, got <type> (#VALUE!)`,
with `<type>` from `type_name` (`string`, `datetime`, `array`, `json`). Currency counts as a number
and a DateTime is an error, as §4 and "Where the Excel documentation was not clear" already say for
`IF`. Currency is listed separately only until BL-097 makes it a plain Number; the rule for it
does not change then.

**NaN** (maintainer, 2026-11-22): `<NAME> condition is not a number (#NUM!)`, worded like
`ROUND num_digits is not a number (#NUM!)`. This is a change to `logical::condition` itself, so it
also changes `IF`, `IFS` and `NOT` (they gave TRUE, TRUE and FALSE for NaN, because NaN is not 0).
Expressions cannot make NaN (`0/0` is `#DIV/0!`, overflow is `#NUM!`); it reaches a condition only
from a host variable or a custom function (`Value::Number(f64::NAN)`). For `NOT` and `!` the
message is `NOT condition is not a number (#NUM!)`.

#### A2. Where the rule applies
| Form | `<NAME>` in the error | Notes |
|---|---|---|
| `IF`, `IFS` | `IF`, `IFS` | unchanged (§4) |
| `NOT x`, `NOT(x)` | message of §5 (`NOT expects a logical value, got …`) | unchanged (§5 already uses the rule) |
| `c ? a : b` | `Ternary` | still lazy: only the chosen branch is evaluated |
| `a AND b`, `a && b` | `AND` | still short-circuit (A3); the result is always a boolean |
| `a OR b`, `a \|\| b` | `OR` | still short-circuit (A3); the result is always a boolean |
| `!x` | message of `NOT` | `!x` gives the same value or error as `NOT(x)`; only its precedence differs (A5) |
| `.filter(expr)`, `FILTER(array, expr)` | `filter`, `FILTER` | an item is kept when `expr` is a true condition |
| `.find(expr)`, `FIND(array, expr)` | `find`, `FIND` | the first item whose `expr` is a true condition |
| lambda criteria of `COUNTIF`, `SUMIF`, `AVERAGEIF` (`AVGIF`) | `COUNTIF`, `SUMIF`, `AVERAGEIF` | only when the criteria is a lambda; an Excel criterion (`">2"`, `2`, `TRUE`) keeps its matching rules |

Not affected: `map` and `reduce` (no condition), the `?` predicate methods (`positive?`, `nil?`:
they return booleans and never read one), `.to_bool()` and `::Boolean` (conversions), `BETWEEN`,
JSONPath filters inside `JQ` (`?(@.price > 100)`, the JSONPath engine's rules), and the
`AND()`/`OR()`/`XOR()` builtins (A4). Skillet has no `all?`/`any?`/`none?`/`reject`/`count(expr)`
method; one added later must use this rule.

#### A3. The operators: short-circuit without order dependence
`a AND b`: read `a` as a condition; if it is FALSE the result is `FALSE` and `b` is not evaluated;
otherwise the result is `b` read as a condition. `a OR b`: if `a` is TRUE the result is `TRUE`
and `b` is not evaluated; otherwise the result is `b` read as a condition. An error in `a`
(evaluating it, or reading it as a condition) is the result, and `b` is not evaluated.

So for any two valid conditions (booleans, numbers, Currency, `NULL`), `a AND b` and `b AND a`
have the same value: `NULL AND FALSE`, `FALSE AND NULL` and `TRUE AND NULL` are `FALSE`;
`NULL OR TRUE` is `TRUE`. Only an invalid side that is never reached stays unseen, as with `IF`:
`FALSE AND "a"` is `FALSE`, `"a" AND FALSE` is an error. This replaces §2's "the right side is
still evaluated": `"a" AND 1/0` is now the `AND` condition error, not `Division by zero`.

The result is `TRUE` or `FALSE`, never an operand: `1 AND 2` is `TRUE`, `0 OR NULL` is `FALSE`.

#### A4. The `AND()`, `OR()` and `XOR()` builtins do not change
They keep §5: a `NULL` argument is ignored like an empty cell, so `AND(TRUE, NULL)` is `TRUE`, and
an array argument is flattened (`AND([1])` is `TRUE`). This is the one place where a condition
that is `NULL` does not count as `FALSE`. The reference warns that `AND(:flag, …)` passes when
`:flag` is missing, and points to `IF`, the operators or an explicit comparison:
`:u&.is_admin AND :on` is `FALSE`, `AND(:u&.is_admin == TRUE, :on)` is `FALSE`.

NaN (maintainer, 2026-11-22): the builtins read numbers through their own path
(`logical_values`, `src/runtime/logical.rs:37` for array items and `:47` for arguments,
`*n != 0.0`), and that path now checks `is_nan()` first. A NaN Number or Currency, passed directly
or inside an array, is `<NAME> condition is not a number (#NUM!)` with `<NAME>` = `AND`, `OR` or
`XOR`: `AND(:n)`, `OR(:n, FALSE)`, `XOR([:n])` are errors. Everything else in A4 stays: `NULL` is
skipped like an empty cell (`AND(TRUE, NULL)` is `TRUE`), and text inside an array is skipped.

#### A5. Precedence and positions do not change
- The grammar does not change. `!` stays a tight unary and `NOT` stays the loose keyword (§3), so
  they now differ only in precedence: `!0 == FALSE` is `(!0) == FALSE`, that is
  `TRUE == FALSE`, `FALSE`; `NOT 0 == FALSE` is `NOT (0 == FALSE)`, `TRUE` (ADR-0023: `0 == FALSE`
  is `FALSE`). `!:x > 1` with `:x` = 5 is `FALSE > 1`, still an error (comparison of incompatible
  types).
- The new errors are runtime errors and have no position (`position: None`), like every runtime
  error today (BL-016). Parse errors and their positions do not change.

#### A6. Errors inside a lambda predicate
An error while evaluating the lambda for an item, or a lambda value that is not a valid condition,
is the result of the whole call. It is not dropped, it does not count as FALSE, and no partial
array is returned. Items are evaluated in order, so `find`/`FIND` stop at the first true
condition and never see a later item: `[2, "a"].find(:x)` is `2`, `["a", 2].find(:x)` is the
`find` error. `filter`, `FILTER` and the `*IF` lambdas look at every item, so one bad item is an
error.

#### A7. `IF` arity
`IF` takes 2 or 3 arguments. With more, it is `IF expects at most 3 arguments`, raised before any
argument is evaluated (`IF(0, 2, 3, 1/0)` is the arity error, not `3` and not `#DIV/0!`). The
eager path checks the count after spreading: `IF(...[TRUE, 1, 2, 3])` is the same error. The
message for fewer than 2 arguments stays `IF expects at least 2 arguments`.

The other logical builtins were checked: `NOT` takes exactly 1 and `BETWEEN` exactly 3 (both
already enforced); `AND`, `OR`, `XOR` and `IFS` take any number (§5; `IFS` with an odd count is
already `IFS expects pairs of condition,value arguments`). `IFS()` with no argument is today
`IFS: no condition is TRUE (#N/A)`; it becomes the pairs error, so the builtin table's minimum of
2 holds. `SWITCH`, `IFERROR`, `IFNA` and `CHOOSE` do not exist (`Unknown function`); when added,
they take Excel's arity.

### Before and after
Run on `develop` at 9b36d9b with `cargo run -q --bin sk`. Each row puts the value of the column
in place of `c`. Legend: `T`/`F` is the boolean result, the `"T"`/`"F"` branch, an item kept or
dropped (`filter`: `[c]` or `[]`; `find`: `c` or `NULL`), or a match (`COUNTIF` 1 or 0, `SUMIF`
with `[10]` 10 or 0, `AVERAGEIF` with `[10]` 10 or `Error: AVERAGEIF: no matching numbers
(#DIV/0!)`). `E` is the old type error: `Ternary condition must be boolean`, `Logical op on
non-boolean`, `Unary '!' on non-boolean` or `<NAME> criteria expression must return TRUE or
FALSE, got <type>`. `#V` is the condition error of A1 (or `NOT`'s and `AND`'s `#VALUE!`).
**Bold** cells change. The Currency columns hold until BL-097 makes Currency a plain Number; after
it they are the `2` and `0` cases.

| Form | `TRUE` | `FALSE` | `NULL` | `0` | `1` | `2.5` | `"a"` | `""` | `2::Currency` (until BL-097) | `0::Currency` (until BL-097) | `DATE(2026,1,1)` | `[1]` |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| `IF(c, "T", "F")`, `IFS(c, "T", TRUE, "F")` | T | F | F | F | T | T | #V | #V | T | F | #V | #V |
| `NOT c`, `NOT(c)` | F | T | T | T | F | F | #V | #V | F | T | #V | #V |
| `c ? "T" : "F"` | T | F | **E → F** | **E → F** | **E → T** | **E → T** | **E → #V** | **E → #V** | **E → T** | **E → F** | **E → #V** | **E → #V** |
| `c AND TRUE`, `TRUE AND c` (`&&`) | T | F | **E → F** | **E → F** | **E → T** | **E → T** | **E → #V** | **E → #V** | **E → T** | **E → F** | **E → #V** | **E → #V** |
| `c AND FALSE` | F | F | **E → F** | **E → F** | **E → F** | **E → F** | **E → #V** | **E → #V** | **E → F** | **E → F** | **E → #V** | **E → #V** |
| `FALSE AND c` | F | F | F | F | F | F | F | F | F | F | F | F |
| `c OR FALSE`, `FALSE OR c` (`\|\|`) | T | F | **E → F** | **E → F** | **E → T** | **E → T** | **E → #V** | **E → #V** | **E → T** | **E → F** | **E → #V** | **E → #V** |
| `c OR TRUE` | T | T | **E → T** | **E → T** | **E → T** | **E → T** | **E → #V** | **E → #V** | **E → T** | **E → T** | **E → #V** | **E → #V** |
| `TRUE OR c` | T | T | T | T | T | T | T | T | T | T | T | T |
| `!(c)` | F | T | **E → T** | **E → T** | **E → F** | **E → F** | **E → #V** | **E → #V** | **E → F** | **E → T** | **E → #V** | **E → #V** |
| `[c].filter(:x)`, `FILTER([c], :x)` | T | F | F | F | **F → T** | **F → T** | **F → #V** | **F → #V** | **F → T** | F | **F → #V** | **F → #V** |
| `[c].find(:x)`, `FIND([c], :x)` | T | F | F | F | **F → T** | **F → T** | **F → #V** | **F → #V** | **F → T** | F | **F → #V** | **F → #V** |
| `COUNTIF([c], :x)`, `SUMIF([c], :x, [10])`, `AVERAGEIF([c], :x, [10])` | T | F | F | **E → F** | **E → T** | **E → T** | **E → #V** | **E → #V** | **E → T** | **E → F** | **E → #V** | **E → #V** |
| `AND(c, TRUE)` (builtin, A4) | T | F | T | F | T | T | #V | #V | T | F | #V | T |
| `OR(c, FALSE)` (builtin, A4) | T | F | F | F | T | T | #V | #V | T | F | #V | T |
| `XOR(c)` (builtin, A4) | T | F | #V | F | T | T | #V | #V | T | F | #V | T |

After the change every row above the builtins reads its column the same way, the A1 rule:
`T F F F T T #V #V T F #V #V` (negated for `NOT` and `!`, and fixed by the left side for
`FALSE AND c` and `TRUE OR c`).

NaN column. `sk` cannot make NaN, so this was run through the library (`evaluate_with`) with
`:n` = `Value::Number(f64::NAN)`; `Value::Currency(f64::NAN)` gives the same results. `#N` is
`<NAME> condition is not a number (#NUM!)`.

| Form | `:n` = NaN |
|---|---|
| `IF(:n, "T", "F")`, `IFS(:n, "T", TRUE, "F")` | **T → #N** |
| `NOT :n`, `NOT(:n)` | **F → #N** |
| `:n ? "T" : "F"`, `:n AND TRUE`, `TRUE AND :n`, `:n AND FALSE`, `:n OR FALSE`, `!(:n)` | **E → #N** |
| `FALSE AND :n`, `TRUE OR :n` | F, T (right side not evaluated) |
| `[:n].filter(:x)`, `FILTER([:n], :x)`, `[:n].find(:x)`, `FIND([:n], :x)` | **F → #N** |
| `COUNTIF([:n], :x)`, `SUMIF([:n], :x, [10])`, `AVERAGEIF([:n], :x, [10])` | **E → #N** |
| `AND(:n, TRUE)`, `OR(:n, FALSE)`, `XOR(:n)`, `AND([:n])` (builtins, A4) | **T → #N** (`AND`/`OR`/`XOR condition is not a number (#NUM!)`) |

Other changed results:

| Expression | Before | After |
|---|---|---|
| `:u&.is_admin AND :on` with `{"u": {"name": "ann"}, "on": true}` | `Error: Logical op on non-boolean` | `false` |
| `NULL AND FALSE`, `TRUE AND NULL` | `Error` | `false` |
| `NULL OR TRUE` | `Error` | `true` |
| `1 AND 2`, `0 \|\| NULL` | `Error` | `true`, `false` |
| `"a" AND 1/0` | `Error: Division by zero (#DIV/0!)` | `Error: AND condition must be TRUE, FALSE or a number, got string (#VALUE!)` |
| `1 ? 2 : 3`, `NULL ? 1 : 2` | `Error` | `2`, `2` |
| `!0 == FALSE`, `!NULL` | `Error` | `false`, `true` |
| `[0, 1, 2].filter(:x)`, `FILTER([0, 1, 2], :x)` | `[]` | `[1, 2]` |
| `[1, 2].filter("yes")` | `[]` | `Error: filter condition must be TRUE, FALSE or a number, got string (#VALUE!)` |
| `[2, "a"].find(:x)`, `["a", 2].find(:x)` | `NULL`, `NULL` | `2`, `Error` (A6) |
| `COUNTIF([1, 2, 3], :x)`, `SUMIF([0, 1, 2], :x, [10, 20, 30])` | `Error` | `3`, `50` |
| `IF(1, 2, 3, 4)`, `IF(0, 2, 3, 1/0)` | `2`, `3` | `Error: IF expects at most 3 arguments` |
| `IFS()` | `Error: IFS: no condition is TRUE (#N/A)` | `Error: IFS expects pairs of condition,value arguments` |

Unchanged, pinned by tests: `FALSE AND 1/0` (`false`), `TRUE OR 1/0` (`true`), `TRUE AND 1/0`
(`#DIV/0!`), `[1, 0].filter(10 / :x > 1)` (`#DIV/0!`, A6), `[NULL, TRUE].filter(:x)` (`[TRUE]`),
`AND(TRUE, NULL)` (`true`), `AND(:u&.is_admin, :on)` with the JSON above (`true`, A4),
`NOT TRUE AND NULL` (`false`), `TRUE ? 1 : 1/0` (`1`).

### Consequences
- \+ One truth rule outside the three Excel builtins: a missing flag is FALSE wherever it is
  tested, and `a AND b` no longer depends on the order of valid operands.
- \+ `filter`/`find` no longer drop items silently when the lambda returns text or a date: that is
  now an error, as in `IF` and `COUNTIF`.
- − Results change without an error: `[0, 1, 2].filter(:x)` keeps the non-zero items (was `[]`),
  and `find` can return an item it skipped before. The migration guide lists them.
- − `AND(:flag, …)` and `:flag AND …` still differ when `:flag` is `NULL`. The reference warns.
- − `!` now accepts numbers and `NULL`, so a typo like `!:count` where a boolean was meant is no
  longer reported. `NOT` already accepted them.

### Alternatives considered
- **Lambda predicates error on anything but a boolean (LANG-3 option B, like the old `COUNTIF`).**
  Not chosen: it keeps two rules, and `IF(:x, …)` and `filter(:x)` would still disagree.
- **`NULL` as FALSE in the `AND()`/`OR()` builtins too.** Not chosen again ("keep"): Excel ignores
  empty cells there. The warning in the reference covers the `&.` case.
- **Keep §2's "the right side is still evaluated" for a non-boolean left side.** Not needed: with
  one rule, an invalid left side is an error by itself.
