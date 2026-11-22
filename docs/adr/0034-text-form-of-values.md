# 0034. Text form of values

- Status: Accepted
- Date: 2026-11-22
- Deciders: Maintainer
- Related: BL-091 (audit LANG-7, LANG-8, LANG-12, REL-8, PERF-4), ADR-0018 (X-7, D-10 `&`,
  methods keep Ruby semantics; amended here), ADR-0030 (decision d; amended
  here, not blocked), ADR-0031 §1 and amendment 1 A3 (amended here), ADR-0033 (Currency is a
  Number, so its text form is moot)

## Context
A value becomes text in nine places: the `::String` cast, `.to_s()`/`.to_string()`, `CONCAT`,
the `&` operator, `JOIN`, `.join()`, `TEXTJOIN`, and the two text builtins that take non-text
(`LEN`, `FIND`'s `within_text`). Each has its own code, and they disagree. Measured with
`cargo run -q --bin sk` on `26980dd`:

| Value | `::String` | `.to_s()` | `CONCAT`, `&` | `JOIN` | `.join()` | `TEXTJOIN` |
|---|---|---|---|---|---|---|
| `0.1 + 0.2` | `0.30000000000000004` | same | same | same | same | same |
| `1e100` | `1000…000` (101 digits) | `10000000000000000159…` (exact binary expansion) | as `::String` | as `::String` | as `::String` | as `::String` |
| `-0.0` | `-0` | `-0` | `-0` | `-0` | `-0` | `-0` |
| `TRUE` | `TRUE` | `true` | `TRUE` | `TRUE` | `true` | `TRUE` |
| `["a", 1, TRUE, NULL]` | `[String("a"), Number(1.0), Boolean(true), Null]` | `[a, 1, true, ]` | `a1TRUE` (flattened) | n/a | n/a | `a,1,TRUE,` |
| `[DATE(2026,1,1)]` | `[DateTime(1767225600)]` | `[2026-01-01T00:00:00Z]` | flattened | n/a | n/a | flattened |
| `{"a":1}` | `{"a":1.0}` | `{"a":1.0}` | `""` (dropped) | `{"a":1.0}` | `Error` | `Error` |

- Numbers print Rust's shortest round-trip text, so `"Total: " & 3 * 0.1` is
  `"Total: 0.30000000000000004"` (LANG-7). Excel prints 15 significant digits (`0.3`); ADR-0018
  listed this as a deliberate difference (X-7).
- `::String` on an array prints Rust `Debug` output: internal enum names that change with the
  `Value` layout (LANG-8).
- `.to_s()` and `.join()` write booleans in Ruby's `true`/`false`; everything else writes Excel's
  `TRUE`/`FALSE`.
- `"héllo".length()` is `6` (bytes), while `LEN`/`LENGTH` give `5` (LANG-12).
- `a & b` parses to a call of the internal function `__CONCAT__`, which formulas can call
  (`__CONCAT__("a", 1)` is `"a1"`) and which embedders see in the public AST as
  `FunctionCall { name: "__CONCAT__" }` (REL-8).
- A DateTime to text formats with chrono's `StrftimeItems` pattern, parsed again on every call,
  and `.to_s()`/`CONCAT` format twice (count, then build): ~150-200 ns per date (PERF-4).

The maintainer decided (2026-11-22): one text form per value type, everywhere a value becomes
text; `.length()` counts characters; `&` gets its own AST node and `__CONCAT__` stops being
callable.

## Decision

### 1. One text form per type, in one function
`src/runtime/text.rs` (new) holds `write_text(w: &mut impl fmt::Write, v: &Value)` and the
per-type writers below. Every surface in §3 calls it; no surface formats a value itself.

| Type | Text form | Examples |
|---|---|---|
| Number | §2 (15 significant digits) | `3`, `0.3`, `-1.5`, `1E+21` |
| Boolean | `TRUE` / `FALSE` | `TRUE` |
| NULL | empty text | `` |
| DateTime | ISO 8601 UTC, ADR-0031 §1 (unchanged) | `2026-01-01T00:00:00Z` |
| String | itself | `héllo` |
| Array | its JSON text, §4 | `["a",1,true,null]` |
| Json (object) | its JSON text, as held (unchanged) | `{"a":1.0}` today, `{"a":1}` with ADR-0030 |

Error messages are not text forms and do not change.

### 2. Number
Written by `write_number(w, n)`:
1. `0` and `-0` are `0`. Excel has no negative zero.
2. An integral `n` with `|n| < 2^53` is its integer digits, exact, with no `.0` and no exponent:
   `5`, `-42`, `1000000000000000`, `1234567890123456`. These are exact in an `f64` and are often
   identifiers; ADR-0030 decision d uses the same bound for JSON.
3. Any other finite `n` is rounded to **15 significant digits**, as Excel does: the decimal digits
   of `format!("{:.14e}", n.abs())` (correctly rounded from the exact binary value, ties to even),
   with trailing zeros removed. Let `e` be the decimal exponent of that rounded value
   (`d.ddd × 10^e`).
   - `-7 <= e < 15`: plain decimal. `0.3`, `0.333333333333333`, `123456.789`, `0.0000001`.
   - otherwise: scientific, `d.ddd` then `E`, a sign, and at least two exponent digits.
     `1E+21`, `1.23456789012346E+19`, `1.5E-08`, `4.94065645841247E-324`.
   - a `-` sign in front when `n < 0`.
4. NaN and infinities (only reachable from a host variable; expressions turn them into errors,
   ADR-0018 D-1) keep Rust's text: `NaN`, `inf`, `-inf`.

The text is a valid Skillet number literal and a valid `::Float` input (`"1E+21"::Float` is
`1e21`). It does not round-trip every `f64`: `(0.1 + 0.2)::String::Float == 0.1 + 0.2` is
`FALSE`. Excel's switch points to scientific notation are not documented; Skillet's are the
ones above (excel-compat X-7).

A `JsonNumber` (ADR-0030 decision b, 0.8) keeps its source text in every text form, as ADR-0030
says. This rule is for `Number`.

### 3. Every surface
| Surface | What it writes | Unchanged rules |
|---|---|---|
| `x::String` | `write_text(x)` | |
| `.to_s()`, `.to_string()` | `write_text(x)` | |
| `CONCAT(…)`, `a & b` | `write_text` of each argument, arrays flattened item by item, `NULL` adds nothing | Excel `CONCAT` takes ranges, so an array argument is its items, not its JSON text |
| `TEXTJOIN(d, ignore_empty, …)` | `write_text` of each item, arrays flattened | `ignore_empty` skips `""` and `NULL` |
| `JOIN(array, [sep])`, `array.join([sep])` | `write_text` of each item | a nested array stays an error in both |
| `LEN(x)`, `FIND(t, x)` | `write_text(x)` for a Number, Boolean, NULL or String | a DateTime, array or object stays an error (ADR-0031 A4) |

So a JSON object in `CONCAT`, `&`, `TEXTJOIN` and `.join()` is its JSON text, as in `::String`,
`.to_s()` and `JOIN` (it was dropped by `CONCAT`/`&` and an error in the other two). This
replaces ADR-0030 E5's `CONCAT` part. `.join()` writes booleans as `TRUE`/`FALSE`, like `JOIN`
(replaces ADR-0031 A3's "booleans stay `true`/`false`"). Methods keep Ruby semantics
(ADR-0018) except for the text form of values, which is one for the language.

Not text, so not changed here: `.to_json()` and `::Json` (JSON, ADR-0030; but see §4 for
numbers), the servers' and `sk`'s output of results (wire format), values passed to JS plugins,
and `Debug`.

### 4. Array: its JSON text
An array's text is the JSON text `.to_json()` writes for it: `[` items separated by `,` `]`,
no spaces. Each item is its JSON value, not its text form, so the text reads back with `::Json`
and strings are not ambiguous (`["a, b"]` vs `["a","b"]`):
- a String is a JSON string (quoted, escaped as `serde_json` escapes);
- a Number follows ADR-0030 decision d: an integral `n` with `|n| < 2^53`, not `-0`, is its
  integer digits (`5`, not today's `5.0`); any other number is `serde_json`'s shortest
  round-trip text (`0.30000000000000004`, `1e+21`, `-0.0`, `1e-7`). JSON text is for reading
  back, so it is not rounded to 15 digits;
- a Boolean is `true`/`false`, NULL is `null` (JSON spelling, as a string gets quotes);
- a DateTime is its ISO text as a JSON string (`"2026-01-01T00:00:00Z"`, ADR-0031 §1);
- an object is its JSON text (parsed and written again, as `.to_json()` does today); a nested
  array is its JSON text.
- A non-finite number (host values only) is `Error: Invalid number for JSON`, as in `.to_json()`
  today.

`.to_json()` uses the same item rules, so its integral numbers lose the `.0` now
(`[1.5, 5].to_json()` is `Json("[1.5,5]")`, `5.to_json()` is `Json("5")`). This is the number
part of ADR-0030 E9/E20, which is accepted; `.to_json()` still returns `Json` until ADR-0030
changes the type. Numbers inside an object keep the object's own text (`{"a":1.0}` from an
object literal) until ADR-0030 step 2 (E7).

### 5. Characters, not bytes
`.length()` and `.len()` on a string count characters (Unicode scalar values, Rust
`chars().count()`), as `LEN` and `LENGTH` do: `"héllo".length()` is `5`. Checked and already
counting characters: `LEN`, `LENGTH`, `LEFT`, `RIGHT`, `MID`, `FIND`, `REPLACE`, `SUBSTRING`,
`REVERSE`, `.substring()`, `.reverse()`, `.to_a()`. Indexing and slicing (`"abc"[0]`, `[0:2]`)
are array-only (`Error: Index on non-array`) and stay so. Grapheme clusters are not counted:
an `e` followed by a combining accent has length 2 everywhere.

### 6. `&` is a binary operator
- AST: `a & b` is `Expr::Binary(a, BinaryOp::Concat, b)`. `BinaryOp` gets one new variant,
  `Concat`. No new `Expr` variant. (BL-092 adds `#[non_exhaustive]` to `BinaryOp` in the same
  release, so this is the last time adding an operator breaks exhaustive matches.)
- Grammar and precedence unchanged: level 5, between comparisons (4) and `+ -` (6),
  left-associative (D-10). `"a" & 1 + 2` is `"a3"`; `1 & 2 == "12"` is `TRUE`.
- Evaluation: both operands evaluated borrowed (like the other binary operators, no copy charge),
  left first; then `write_text` of each with `CONCAT`'s rules (§3), the result charged before it
  is built (ADR-0011 amendment 2). So `a & b` is always `CONCAT(a, b)`.
- Errors: an operand's error is the result, with its message and position unchanged
  (`1 & (1/0)` is `Error: Division by zero (#DIV/0!)`). `&` itself can only fail with the budget
  error, `Evaluation budget exceeded (max N MB)`, as `CONCAT` does. Parse errors are unchanged
  (`="a" &` is `Error: Unexpected token: Eof at position 5`).
- `__CONCAT__` is no longer a name: `__CONCAT__("a", 1)` and `__concat__("a", 1)` are
  `Error: Unknown function: __CONCAT__`, as for any name that is not a builtin or a registered
  custom function. The one-argument form's `'&' expects 2 operands` disappears with it. The name
  is removed from the reserved internal names, so it is an ordinary name for custom functions.
  `__TERNARY__`, `__CONST_TRUE__` and `__CONST_FALSE__` are not part of this decision.

### 7. Speed of date text (PERF-4)
Write the 20-byte form `YYYY-MM-DDTHH:MM:SSZ` by hand for years 0000-9999: civil date from days
since the epoch (integer arithmetic, no chrono), two-digit writes into a `[u8; 20]`, written once.
Keep the chrono path for other years and for timestamps outside chrono's range. The text is
exactly ADR-0031 §1's. Charging can use the known length (20) instead of a counting pass. The
implementation shows before/after numbers for `:d.map(:x::String)`, `.map(:x.to_s())` and
`.join(",")` over 100k DateTimes (AGENTS.md §8).

## Behaviour changes (ship in 0.7.0)
| Expression | Before | After |
|---|---|---|
| `(0.1 + 0.2)::String`, `.to_s()`, `"" & …`, `CONCAT(…)`, `JOIN([…])`, `[…].join()`, `TEXTJOIN(",", TRUE, …)` | `"0.30000000000000004"` | `"0.3"` |
| `"Total: " & 3 * 0.1` | `"Total: 0.30000000000000004"` | `"Total: 0.3"` |
| `(1/3)::String`, `SQRT(2) & ""` | `"0.3333333333333333"`, `"1.4142135623730951"` | `"0.333333333333333"`, `"1.4142135623731"` |
| `1e21::String`, `1e100.to_s()` | `"1000000000000000000000"`, `"10000000000000000159…"` | `"1E+21"`, `"1E+100"` |
| `12345678901234567890 & ""` | `"12345678901234567000"` | `"1.23456789012346E+19"` |
| `1.5e-8 & ""`, `1e-7 & ""` | `"0.000000015"`, `"0.0000001"` | `"1.5E-08"`, unchanged |
| `5::String`, `1e15 & ""`, `1234567890123456 & ""`, `123456.789 & ""` | `"5"`, … | unchanged |
| `(0 - 0.0) * -1 & ""`, `LEN((0 - 0.0) * -1)` | `"-0"`, `2` | `"0"`, `1` |
| `LEN(0.1 + 0.2)`, `LEN(1/3)` | `19`, `18` | `3`, `17` |
| `TRUE.to_s()`, `[TRUE, 1].join()` | `"true"`, `"true,1"` | `"TRUE"`, `"TRUE,1"` |
| `["a", 1, TRUE, NULL]::String` | `"[String(\"a\"), Number(1.0), Boolean(true), Null]"` | `"[\"a\",1,true,null]"` |
| `["a", 1, TRUE, NULL].to_s()` | `"[a, 1, true, ]"` | `"[\"a\",1,true,null]"` |
| `[DATE(2026,1,1)]::String`, `.to_s()` | `"[DateTime(1767225600)]"`, `"[2026-01-01T00:00:00Z]"` | `"[\"2026-01-01T00:00:00Z\"]"` |
| `[[1, 2], 3].to_s()`, `[]::String` | `"[[1, 2], 3]"`, `"[]"` | `"[[1,2],3]"`, `"[]"` |
| `[0.1 + 0.2, 1e21]::String` | `"[Number(0.30000000000000004), Number(1e21)]"` | `"[0.30000000000000004,1e+21]"` |
| `[{"a":1}]::String`, `[{"a":1}].to_s()` | `"[Json(\"{\\\"a\\\":1.0}\")]"`, `"[{\"a\":1.0}]"` | `"[{\"a\":1.0}]"` |
| `[1.5, 5].to_json()`, `5.to_json()` | `Json("[1.5,5.0]")`, `Json("5.0")` | `Json("[1.5,5]")`, `Json("5")` |
| `CONCAT("x", {"a":1})`, `"x" & {"a":1}` | `"x"` | `"x{\"a\":1.0}"` |
| `[{"a":1}].join()`, `TEXTJOIN(",", TRUE, {"a":1})` | `Error` | `"{\"a\":1.0}"` |
| `"héllo".length()`, `"héllo".len()` | `6` | `5` |
| `__CONCAT__("a", 1)`, `__CONCAT__("a")` | `"a1"`, `Error: '&' expects 2 operands` | `Error: Unknown function: __CONCAT__` |
| `parse("\"a\" & 1")` (Rust) | `FunctionCall { name: "__CONCAT__", args: [StringLit("a"), Number(1.0)] }` | `Binary(StringLit("a"), Concat, Number(1.0))` |
| `"a" & 1 & TRUE`, `CONCAT("a", [1, 2])`, `"a" & NULL`, `DATE(2026,1,1) & ""`, `JOIN([[1],2])` | `"a1TRUE"`, `"a12"`, `"a"`, `"2026-01-01T00:00:00Z"`, `Error` | unchanged |

## Amends
- **ADR-0018**: "Number to text" in the deliberate differences (X-7) is no longer a difference
  in precision; only the switch points to scientific notation and the exact integers below
  `2^53` differ from Excel (§2). "Lowercase methods keep Ruby semantics" holds except for the
  text form of values (§3).
- **ADR-0030**: decision d's "Text of a single value outside JSON is unchanged: … Rust's f64
  `Display`" is replaced by §2; E5's `CONCAT` part by §3; E15's after-value is §4's JSON text.
  Decision b (`JsonNumber` text) and the JSON number rule are kept and used by §4.
- **ADR-0031**: §1 "every path" now goes through `write_text`; amendment 1 A3's boolean
  sentence is replaced by §3.
- **ADR-0033**: the consequence "One text form for every number (Rust's shortest round-trip
  `f64` text)" now reads "(§2 of ADR-0034)".

## Consequences
- \+ One rule to learn and one function to maintain; messages built with `&` read as Excel shows
  them.
- \+ An array's text is standard JSON and reads back with `::Json`; no internal names leak.
- \+ The AST says what the user wrote; `__CONCAT__` is not public surface.
- − Breaking: text built from numbers with more than 15 significant digits, from `-0`, from very
  large or small numbers, from booleans through `.to_s()`/`.join()`, from arrays, and string
  `.length()` all change. Code that parsed `::String` back with `::Float` and compared exactly
  may differ in the last digits; keep full precision with `.to_json()` or `[x]::String`.
- − `"1E+21"::Integer` is `1` (the prefix scan of ADR-0033 §2 stops at `E`); use `::Float`.
- − Until ADR-0030 step 2, numbers in an object's text keep `1.0` while an array's are `1`.
- Out of scope, follow-ups: the binaries print a nested array in JSON output with `Debug`
  (`sk.rs:184`, `sk_server.rs:407`, `http_server/response_size.rs:108`); that is a wire format
  and needs its own decision. `impl Display for Value` (the text form for embedders) can be added
  later without breaking anything.
