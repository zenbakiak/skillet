# 0005. Hybrid Excel + Ruby syntax with `:var` sigil

- Status: Accepted (retroactive, v0.6.3)
- Date: 2026-11-20
- Related: PRD-001
- Amended by: [ADR-0018](0018-excel-compatibility.md) (2026-11-21). From 0.7.0, unary minus
  binds tighter than `^` and `^` is left-associative, as in Excel: `-3^2 = 9`, `2^3^2 = 64`.
- Amendment 1 (**Accepted**, orchestrator, 2026-11-22, BL-106): spread `...` is also allowed as
  an element of an array literal, `[0, ...:xs, 4]`, with the same rules as in argument lists. See
  [Amendment 1](#amendment-1-accepted-2026-11-22-bl-106-spread-in-array-literals).

## Context
Target users are spreadsheet power users and backend developers. Each group knows a different idiom.

## Decision
- Optional leading `=` (Excel). Builtins are UPPERCASE calls: `SUM(...)`, `IF(c, a, b)`, `STDEV.P(...)`.
- Methods are lowercase and chainable (Ruby): `:arr.filter(:x > 1).map(:x * 2).sum()`; a `?` suffix
  marks predicates (`:n.even?`).
- Variables always use the `:name` sigil. Lambda bodies get `:x` (item) and `:acc`
  (accumulator) by default; a trailing string argument renames the parameter.
- Also supported: ternary `c ? a : b`, `^` power (right-assoc, binds tighter than unary minus:
  `-3^2 = -9`), spread `...`, index/slice `[i]`/`[a:b]`, casts `::Type`, assignments `:a := 1; :a + 1`.
- No loops, no user-defined functions in-language, no I/O (non-Turing-complete by design).

## Consequences
- + Familiar to both audiences; sigil removes ambiguity between variables and function names.
- − Two ways to do many things (`SUM(:a)` vs `:a.sum()`); docs must say which is canonical.
- Any grammar change needs an ADR plus tests in `tests/prd_*.rs`.

## Amendment 1 (Accepted, 2026-11-22, BL-106): spread in array literals

Status: **Accepted** (orchestrator, 2026-11-22). The rest of this ADR stays Accepted.

### Context
This ADR lists spread `...` as supported syntax but does not say where it may appear. Today it
is accepted in one place only: an argument of a function or method call (`parse_call_args`).
`SUM(...[1, 2, 3])` is 6, but `[0, ...[1, 2, 3], 4]` is the parse error
`Unexpected token: Ellipsis at position 5`, and the user guide documents that as a limit.
The BL-084 docs review found the gap; on 2026-11-22 the maintainer decided it is a missing
feature and that array literals must accept spread with the same rules as argument lists.

How argument-list spread works today (verified on the 0.7.0 build):

- Lexer: `...` is `Token::Ellipsis` (`src/lexer.rs`); `..` is two `Dot` tokens.
- Parser: an argument that starts with `...` is `'...' expr`; the operand is a full expression
  (`parse_expr`), so `...:a.map(:x * 2)`, `...(TRUE ? [1] : [2])` and `...:a := [1]` all spread
  the whole expression. The node is `Expr::Spread` and counts one nesting level
  (`node_depth`). `... ...x` is an error: the operand cannot itself start with `...`.
- Evaluator: for builtin and custom function calls, the operand is evaluated; a `Value::Array`
  has its items appended in order, one level only (`SUM(...[1, [2, 3]])` is 6 because `SUM`
  flattens, not because spread does). Any other value is the error `Spread expects array`
  with no position: `NULL`, a number, a boolean, a date, a string, a JSON object.
- Budget (ADR-0011): spread adds no charge of its own. Its operand is charged like any other
  value (a literal at its result site, a copying variable read at `cost(value)`, see
  `docs/guide/limits.md`), and the call result is charged as usual.
- A spread anywhere else (top level, operand of an operator, parenthesised, assignment value,
  object value, index) is a parse error. `Expr::Spread` reaching evaluation outside an argument
  list gives `Spread not allowed here` (only reachable through a hand-built AST, and through
  method arguments, see S6).

### Decision

**S1. Grammar.** An array-literal element may be a spread, exactly like a call argument:

```text
array   = "[" [ element { "," element } ] "]"
element = "..." expr | expr
```

The operand rule is the argument-list rule: one full expression, which may not itself start
with `...`. No trailing comma (unchanged: `[1, 2,]` is already an error). The AST is the
existing one: `Expr::Array(items)` where an item may be `Expr::Spread(inner)`. No new variant,
no public API change.

No currently valid expression changes how it parses: `...` was a parse error in this position.

**S2. Meaning.** Elements are evaluated left to right. A plain element appends its value. A
spread element evaluates its operand; an array appends its items in order, one level only (no
flattening); an empty array appends nothing.

**S3. Non-array operands are an error, the same error as in argument lists.** `NULL`, numbers,
booleans, dates, strings (also `""`) and JSON objects all give `Spread expects array`.
Justification:

- Consistency is the maintainer's requirement: `[...:v]` and `MERGE(...:v)` must agree.
- `NULL` as `[]` would be a silent default. ADR-0006 allows defaults only through explicit
  null-safe forms; the explicit form already exists: `...:v.to_a()` (`NULL.to_a()` is `[]`).
- A string as its characters would be a second, implicit way to split text, next to
  `.to_a()`/`SPLIT`; Excel has no such conversion. `..."ab".to_a()` is the explicit form.
- A scalar as a one-item array (`[...5]` = `[5]`) would hide type mistakes in data. Write `5`.
- An object spread into an array has no obvious meaning (keys? values? pairs?). Use
  `.keys()` / `.values()`.

Changing argument-list spread at the same time (for example `NULL` as no arguments) is out of
scope: it would change current results and need its own decision.

**S4. Errors and positions.**

- Runtime: the message is exactly `Spread expects array`, position `None`, the same as
  argument lists. Runtime errors carry no position today (the AST has no spans); adding the
  type name or a position to this message, for both sites, is left as an open question.
- An error raised while evaluating an operand is returned unchanged, and evaluation stops at
  the first error (no later element is evaluated).
- Parse errors keep the existing messages. Positions count bytes from the start of the input,
  including a leading `=` (BL-069). The lexer already gives `...` its own token, so these come
  from the existing paths:

| Input | Result |
|---|---|
| `=[...]` | `Unexpected token: RBracket at position 5` |
| `=[1, ...]` | `Unexpected token: RBracket at position 8` |
| `=[... ...[1]]` | `Unexpected token: Ellipsis at position 6` |
| `=[...[1] ...[2]]` | `Expected ',' or ']' in array at position 9` |
| `=[1, ...[2],]` | `Unexpected token: RBracket at position 12` |

Still parse errors, unchanged (spread stays limited to argument lists and array literals):

| Input | Result |
|---|---|
| `=...[1]` | `Unexpected token: Ellipsis at position 1` |
| `=(...[1])` | `Unexpected token: Ellipsis at position 2` |
| `=1 + ...[1]` | `Unexpected token: Ellipsis at position 5` |
| `=:a := ...[1]` | `Unexpected token: Ellipsis at position 7` |
| `={a: ...[1]}` | `Unexpected token: Ellipsis at position 5` |

**S5. Budget (ADR-0011) and limits: no new rule.**

- Spread adds no charge. The operand is charged as it is today wherever it is evaluated: an
  array literal at its result site, a function or method result at its site, a variable read
  as a copy (`cost(value)`, ADR-0011 amendment 2; `docs/guide/limits.md` already lists "a
  spread" among copying reads). The enclosing array literal is then charged `cost(result)` at
  its result site, as every array literal is: `32 + Σ cost(item)` over the final, spread items.
- So `[0, ...[1, 2], 3]` costs 96 (the inner literal `[1, 2]`) + 160 (the result, 4 items) =
  **256 bytes**: it passes with `max_bytes = 256` and fails with 255 with
  `Evaluation budget exceeded (max 255 bytes)`. This is the same arithmetic as
  `SUM(...[1, 2])` = 96 + 32 = 128, which passes at 128 and fails at 127 today.
- `[...:xs]` with `:xs = [1, 2, 3]` (a `HashMap` variable, not charged on entry) costs 128 for
  the copying read + 128 for the result = 256. An implementation that moves the items without
  copying must keep these charges (the charges are a contract), or come with an ADR-0011
  amendment of its own.
- Depth: the result is checked by the existing array-literal result check (`check_value`), so
  the `MAX_NESTING_DEPTH = 128` rule applies to the final array. Spread removes one level
  (`[...[[1]]]` is `[[1]]`), so `[...:a]` is never deeper than `:a`; `[[...:a]]` is one level
  deeper than `:a` and fails with `Expression nested too deeply (max 128)` when `:a` is at the
  limit.
- Parse depth: the `...` node counts one level, as in argument lists; no change to `node_depth`.
- There is no separate maximum array length; the budget bounds size. Repeated spreads
  (`[...:a, ...:a, …]`) are charged per copy, linear in input size, like `MERGE(:a, :a, …)`.

**S6. Out of scope.**

- **Object literals** (`{...:o, b: 2}`): not trivial. Objects are still `Value::Json` text
  until ADR-0030 lands `Value::Object` in 0.8, and merge rules (key order, duplicate keys) need a
  decision. `{a: ...[1]}` stays a parse error. File separately if wanted.
- **Method arguments**: `[3, 1].sort(...["desc"])` gives `Spread not allowed here` today, while
  some methods ignore their arguments (`[1, 2, 3].sum(...[1])` is 6). Not changed here; see
  open questions.
- Spread in index, slice, operands, assignment values: stay parse errors.

### Examples (acceptance table)

Variables: `:xs = [1, 2, 3]`, `:nested = [[1, 2], [3]]`, `:none = NULL`, `:s = "ab"`.

| # | Expression | Expected `Value` |
|---|---|---|
| E1 | `[0, ...[1, 2, 3], 4]` | `[0, 1, 2, 3, 4]` |
| E2 | `[0, ...:xs, 4]` | `[0, 1, 2, 3, 4]` |
| E3 | `[...:xs]` | `[1, 2, 3]` |
| E4 | `[...[]]` | `[]` |
| E5 | `[0, ...[], 1]` | `[0, 1]` |
| E6 | `[...:xs, ...:xs]` | `[1, 2, 3, 1, 2, 3]` |
| E7 | `[...[1, [2, 3]]]` | `[1, [2, 3]]` (one level, no flattening) |
| E8 | `[...:nested]` | `[[1, 2], [3]]` |
| E9 | `[...[...:xs]]` | `[1, 2, 3]` |
| E10 | `[[...:xs], ...[[4]]]` | `[[1, 2, 3], [4]]` |
| E11 | `[...:xs.map(:x * 2), 0]` | `[2, 4, 6, 0]` (operand is a full expression) |
| E12 | `[...TRUE ? [1] : [2], 3]` | `[1, 3]` |
| E13 | `[1, 'a', ...[NULL, TRUE]]` | `[1, "a", NULL, TRUE]` |
| E14 | `[...:xs].sum()` | `6` |
| E15 | `SUM([...:xs, 4])` | `10` |
| E16 | `[...:xs][0]` | `1` |
| E17 | `[1, 2].map([...[:x], 0])` | `[[1, 0], [2, 0]]` |
| E18 | `[...:none.to_a(), 1]` | `[1]` |
| E19 | `[...:s.to_a()]` | `["a", "b"]` |
| E20 | `[...:none]` | Error `Spread expects array` |
| E21 | `[...NULL]` | Error `Spread expects array` |
| E22 | `[...5]`, `[...TRUE]`, `[...DATE(2026, 1, 1)]` | Error `Spread expects array` |
| E23 | `[...:s]`, `[...""]` | Error `Spread expects array` |
| E24 | `[...{a: 1}]` | Error `Spread expects array` |
| E25 | `[1, ...:missing]` | Error `Missing variable: :missing` |
| E26 | `[...[1], ...1/0]` | Error `Division by zero (#DIV/0!)` |
| E27 | `MERGE(...[[0], :xs])` vs `[0, ...:xs]` | both `[0, 1, 2, 3]` |

Parse errors: the S4 tables. Budget and depth: S5.

### Consequences

- \+ `[0, ...:xs, 4]` works; the user-guide note "Spread works in argument lists only" goes.
- \+ One rule for spread in both places it is allowed; no new error message, no new charge.
- − Code outside the crate that walks `Expr::Array` items may now meet `Expr::Spread` there.
  `Expr` is `#[non_exhaustive]` and `Spread` already exists, so this is not an API break, but
  it is noted in the CHANGELOG.
- − `NULL` does not spread as empty; data with optional arrays needs `.to_a()`.
- Additive: only inputs that were parse errors change, so no golden result changes. New
  parity cases add golden lines.

### Docs to update with the implementation

- `docs/prd/PRD-001-expression-language.md` FR-9: "Object literals, spread (in argument lists
  and array literals; the operand must be an array, ADR-0005 amendment 1), index and slice",
  acceptance `tests/object_literals.rs`, `tests/array_spread.rs` (BL-106).
- `docs/guide/user-guide.md` "Spread Operator": replace the "argument lists only" lines with
  array-literal examples (E1, E2, E5, E18) and the `Spread expects array` rule with the
  `.to_a()` hint.
- `docs/reference/api.md` "Special Operators" and "Spread Operator": spread is allowed in
  argument lists and array literals; add `[0, ...[1, 2], 3]  # [0, 1, 2, 3]`.
- `docs/guide/limits.md`: "a spread" in the copying-reads list becomes "a spread (in an
  argument list or an array literal)".
- `docs/guide/migration-0.7.md` (or the next release's guide), additive section: "`[0, ...:xs]`
  used to be a parse error; it now builds an array (ADR-0005 amendment 1)."
- `docs/DEVELOPMENT_GUIDE.md` §5.1 lists the token as `Spread`; it is `Ellipsis`. Fix while there.

### Open questions (not blocking)

1. Should `Spread expects array` name the type (`…, got string`, like ADR-0029) for both sites?
   It is a message change for argument lists, so it needs a decision.
2. Method arguments: allow spread there too (consistent with functions), or make it a parse
   error instead of a runtime one? Today it depends on whether the method reads its arguments.
3. Object-literal spread, after `Value::Object` (ADR-0030).
