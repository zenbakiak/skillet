# 0030. Structured and shared values: `Value::Object`, and `Arc`-shared strings, arrays and objects

> **Amended by [ADR-0034](0034-text-form-of-values.md) (2026-11-22):** the text of a single
> Number (decision d, last paragraph) is ADR-0034 §2; `CONCAT` of an object is its JSON text (E5);
> an array's text is its JSON text (E15). Decisions b and d's JSON number rule stand.

- Status: Accepted (2026-11-21, with decisions a–d)
- Date: 2026-11-21
- Deciders: Maintainer (accepts), Language Designer and Performance Engineer (authors), Rust Engineer
- Related: BL-011, BL-036, BL-038, ADR-0002 (`Rc` AST), ADR-0003 (`Value`, `f64`), ADR-0004 (JSON
  objects as text), ADR-0007 (custom functions, JS plugins), ADR-0008 (server threads), ADR-0011
  (evaluation budget, amended here), ADR-0021 (parse cache), ADR-0023 (equality)
- Supersedes: ADR-0004. Amends: ADR-0011 (read charge, cost model)

## Decisions (maintainer review of PR #104, 2026-11-21)
- **(a)** `.to_json()` returns a `String` holding JSON text: accepted (E9, E10).
- **(b)** Large integers are **not** rounded. A JSON number whose f64 does not print back its
  source text (an integer with |n| > 2^53, or any number such as `2.50`, `1.0`, `1e21`) keeps that
  text while it passes through unchanged, and prints it (`sk --json`, server output, `:arguments`,
  `.to_s()`, `JOIN`). Arithmetic still uses the f64 (ADR-0003). Design: §2 "Exact JSON numbers",
  the `JsonNumber` variant in §1, plan steps 5 and 6. E16 is updated; E27–E31 added.
- **(c)** `Value::Json` is deprecated in 0.7.x: accepted (§3, plan step 2).
- **(d)** Integral numbers print without `.0`, but **only** a `Number` that is exactly integral
  (and below 2^53, not `-0.0`). Non-integral numbers print exactly as today, and `Currency` keeps
  today's formatting (§2 "Printing numbers", E19–E26).

## Context

### Today: the layout
`types::Value` (`src/types.rs`) is a public enum, not `#[non_exhaustive]`, 32 bytes, `Send + Sync`,
with owned payloads: `Number(f64)`, `Currency(f64)`, `DateTime(i64)`, `Boolean`, `Null`,
`String(String)`, `Array(Vec<Value>)` and `Json(String)`. `Json` holds JSON **text**. It is used for
objects (JSON variables, object literals, `JQ` matches, JS plugin results) but also for any text
that `.to_json()` or `::Json` produce: `5.to_json()` is `Json("5.0")`, `"abc"::Json` is
`Json("abc")` (not valid JSON; it fails only when read). Cloning a `Value` deep-copies it.

### Today: JSON text is parsed on every use
Library code parses `Json` text with `serde_json::from_str` at **10 sites**, each with a budget
pre-charge (`limits::charge_json_parse`, 11 calls): property access (`evaluator.rs:455`), object
literals that embed a `Json` (`evaluator.rs:858`), `DIG` (`json.rs:31`), `JQ` input
(`jsonpath.rs:84`), `==` on two objects (`comparison.rs:93`), `.keys/.values/.has_key/.dig`
(`method_calls/mod.rs:197/210/235/271`) and `.to_json()` on arrays holding objects
(`conversion_methods.rs:217`). It builds `Json` text at **6 sites**: `json_to_value` (`lib.rs:263`),
its copy in `jsonpath.rs:114`, object literals (`evaluator.rs:433`), `.to_json()`
(`conversion_methods.rs`), `::Json` (`type_casting.rs`, hand-built, unvalidated) and JS plugin
results (`js_plugin.rs:198`). The binaries parse it once more to print results (5 sites:
`sk.rs:196/202`, `sk_server.rs:388`, `http_server/eval.rs:345/370`), and `sk_http_server`
re-serializes the whole request arguments into `:arguments` on every request (`eval.rs:219`).

One `:x.price` on a row parses the whole row, clones the `price` node and converts it; if the
property is itself an object (`:user.profile.age`), it is serialized back to text and parsed again.

Two consistency bugs follow from the text form: number spelling depends on where the object came
from (`{"a":1}::String` is `{"a":1.0}`, the same object from a JSON variable prints `{"a":1}`, so
`{"a":1}.to_s() == :o.to_s()` is `false`), and `Currency`/`DateTime` tags are lost inside object
literals (`{"p": 5::Currency}.p` is `Number(5.0)`).

### Today: values are copied on read
A plain variable used as a function argument, array or object element, `IF` branch, or assignment
right-hand side is deep-cloned and, since ADR-0011 amendment 2, charged `cost(value)`
(`evaluator.rs:236-242`). Borrowed reads (operator operands, method and property targets) are free.
Other copies: each lambda iteration clones its item (`lambda_methods.rs:49/75/98/130`,
`evaluator.rs:663/704/824`), index and slice results, `REVERSE`/`UNIQUE`/`FIRST`-style builtins, and
the assignment entry points clone the **whole variable map** (`evaluator.rs:915`) before the first
step. So `SUM(:nums)` copies `:nums` while `:nums.sum()` does not, and
`:nums.map(SUM(:nums)).length()` over 10k numbers fails with the budget error while
`:nums.map(:nums.sum()).length()` passes (77 ms for the whole `sk` run).

### Measurements
Apple M3 Pro, macOS 27.0.1, rustc 1.89.0, release, commit `f4f72da`, load average 6–9 (other
agents running). Medians of 15 samples, three runs each; the range is across the runs. "Replica" rows
repeat outside the evaluator exactly what it does per item today; "structured" rows do the same work
on a prototype `Arc` object/array/string layout. Prototype: throwaway crate depending on this commit
(not committed); the existing benches confirm the totals (`baseline` `json_lambda eval` 3.68 µs,
`lambda_scope` `:rows.map(:x.price).sum()` 2.84 ms on the same run).

| What | Today | Structured, shared |
|---|---|---|
| `json_lambda` (baseline), eval only | 2.48–2.63 µs | ~0.6 µs (projected) |
| … of which the `Json` work: 4 item clones + 8 × (scan, parse, get, convert) | 1.95–2.07 µs | 0.10 µs |
| `json_vars` (baseline), eval only | 1.89–2.01 µs | not prototyped (today: 4 parses and 2 re-serializations of `:user`/`profile`) |
| `:rows.map(:x.price).sum()`, 10k rows (lambda_scope) | 2.81–2.94 ms | ~0.35 ms (projected) |
| … replica of the `Json` work (10k × clone, scan, parse, get, convert) | 2.80–3.25 ms | 0.135–0.15 ms |
| … of which `serde_json::from_str` alone | 2.24–2.48 ms | — |
| … of which the item clone (`String`) | 205–214 µs | Arc clone: ~3 ns/item |
| `:rows.map(:x.price.round(1)).sum()` | 3.27–3.40 ms | |
| `:rows.map(:x.price / :rows.length()).sum()` | 3.53–3.78 ms | |
| `:rows.filter(:x.cat == "a").map(:x.price * :x.qty).sum()` | 5.40–5.72 ms | |
| `:rows.map({"id": :x.id, "total": :x.price * :x.qty}).length()` | 11.2–11.8 ms | |
| `SUM(:nums)`, 100k numbers (copying read) vs `:nums.sum()` (borrowed) | 263–295 µs vs 68–75 µs | ~70 µs |
| `COUNT(:rows)` (copying read) vs `:rows.length()` | 225–253 µs vs 56–60 ns | O(items), no copy |
| `:a := 1; :a` with `:rows` in the variables (map clone at entry) | 222 µs | O(number of variables) |
| deep clone: `:nums` / `:rows` / 1 MiB string | 151–163 / 211–225 / 13–15 µs | 3 ns each |
| `Arc` vs `Rc` clone + drop, uncontended | 3–4 ns vs 2–3 ns | |
| Ingest `:rows` from JSON text: today (`from_str` + `json_to_value`) | 3.33–3.61 ms | direct `Deserialize`: 3.06–3.43 ms; via `serde_json::Value`: 4.56–4.87 ms |
| Option A replica (text kept, content-hashed parse cache) | | 0.70–0.85 ms |
| Option C replica (unshared `BTreeMap` object, deep clone per item) | | 1.27–1.39 ms |
| Decision (b): classify the 30k numbers of `:rows` (1,003 keep their text, all prices like `12.80`) | | 0.28–0.29 ms (9.2–9.6 ns each; `str::parse::<f64>` of the same: 0.18 ms) |
| Decision (b): sum / clone 10k prices, plain vs 10% `JsonNumber` | | 6.7–6.9 vs 6.6–7.0 µs / 5.6–6.7 vs 6.1–6.4 µs (noise) |

Projections: the replica of today's `Json` work is about all of the `:rows` evaluation time, so
the projected time is the structured work plus the evaluator's own per-item cost, measured at
~21 ns/item on number arrays (`:nums.map(:x * 1).sum()` 2.10 ms / 100k).

Real heap bytes versus the ADR-0011 cost model (counting allocator, 10k objects):

| Object layout | `{"id","cat","price","qty"}` rows | `{"a": n}` |
|---|---|---|
| `Arc<BTreeMap<…>>` | 5.57 MB, 3.2× the model | 5.21 MB, **8.0×** |
| sorted `Arc<[(key, Value)]>` | 2.37 MB, 1.36× | 0.81 MB, 1.25× |

A `BTreeMap` allocates a whole 11-slot node even for one key, so it would break ADR-0011's
"live heap ≤ ~2× budget" bound; a sorted slice stays within the cost model once keys are charged
like strings (Decision §4). The prototype `Value` with `Arc` payloads is 24 bytes (today 32).

## Options

Each option is judged on cost (work and diff), risk, public API and the budget (ADR-0011).

**A. Keep `Json(String)`, add a lazily parsed cache.** Either a field next to the text (a
`OnceLock` of the parsed tree, which changes the variant's payload: as breaking as B) or a
thread-local cache keyed by the text. Measured: 0.70–0.85 ms for the `:rows` work (3–4× better
than today, 5–6× worse than B): the item clone and an O(length) hash per access stay, and every hit
still converts a `serde_json` node into a `Value`. Risk: a cache needs bounds, eviction and a budget
charge of its own (an uncharged cache is a memory hole), and keying by hash needs a full text compare
on hit. It fixes neither the number spelling nor the lost `Currency` tags, nor BL-036. Rejected.

**B. Copy-on-write by borrowing (`Cow`) through the evaluator, no type change.** Builtins take
borrowed arguments (`&[Cow<Value>]`) like operator operands already do, and lambda scopes bind items
by reference. Removes the read copy for builtins (`SUM(:nums)` → ~70 µs) with no public API change.
But custom function arguments (`Vec<Value>`), assignments, the assignment entry map clone, results
returned to callers and the server cache still copy, so the read charge must stay for those; the
`:rows` cases do not change at all (they are dominated by parsing). Risk: lifetimes through the ~90
builtins and the methods. Kept as a fallback only if the maintainer rejects any `Value` break; it does not
address BL-011.

**C. An object variant, unshared (`Object(BTreeMap<String, Value>)` or `IndexMap`).** Removes
parsing, but each lambda iteration deep-clones its row: 1.27–1.39 ms per 10k rows against 0.21 ms
for today's text clone, copying reads get more expensive than today, and the `BTreeMap` uses 3–8×
the bytes the budget charges. `IndexMap` adds a dependency and changes `.keys()` order (today sorted,
because `serde_json` is built without `preserve_order`). Rejected.

**D. An object variant behind a shared pointer: `Rc` or `Arc`.** Parsing disappears and an item
clone is a reference-count bump. `Rc` is not possible: `Value` is `Send + Sync` today, the
`sk_http_server` result cache stores `Value`s in a process-wide `Mutex` (`http_server/cache.rs:91`),
`CustomFunction` is `Send + Sync`, and embedders move results between threads. `Rc` would break all
three. ADR-0002's `Rc` AST is a different case: an `Expr` stays on the thread that parsed it
(ADR-0021's per-thread cache), while values cross threads. `Arc` cost the same as `Rc` in the
measurement (3–4 ns versus 2–3 ns per clone and drop, uncontended); contention needs two threads
cloning the same value, which only the result cache does, once per cached request.

**E. `Arc` sharing for strings and arrays too.** Every read becomes O(1), so the ADR-0011
amendment 2 read charge can go (Decision §4), the assignment entry map clone becomes
O(variables), and `SUM(:nums)` and `:nums.sum()` cost the same. Copy-on-write comes from
`Arc::make_mut` inside builtins that build a new value from a uniquely owned one; values stay
immutable to expressions, so sharing is never observable. Cost: the same public break as D, on two
more variants (`Value::String(x)` and `Value::Array(x)` constructors need `.into()`). Without E,
D leaves strings and arrays copied on read and the read charge in place, and a second break later.

## Decision

Adopt **D with `Arc` plus E**, in one breaking release (0.8.0), prepared by additive steps.

### 1. Layout

```rust
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]          // Debug is hand-written for Object (§2)
pub enum Value {
    Number(f64),
    Currency(f64),
    DateTime(i64),
    Boolean(bool),
    Null,
    String(Arc<str>),
    Array(Array),     // Arc<{ cost: u64, depth: u16, items: Vec<Value> }>; Deref<Target = [Value]>
    Object(Object),   // Arc<{ cost: u64, depth: u16, entries: Box<[(Box<str>, Value)]> }>
    JsonNumber(JsonNumber), // Arc<{ value: f64, text: str }> (one allocation); decision (b)
}
```

- `JsonNumber` is a number read from JSON text whose f64 does not print back its source text
  (§2). It exists only so the text can be printed again; every operation reads `value`.
  `Value::as_number()` returns `Some(f64)` for `Number` and `JsonNumber`, and the hand-written
  `PartialEq` treats `JsonNumber(v, _)` and `Number(v)` as equal, like the language's `==`.

- `Object` entries are sorted by key with duplicates removed (last wins), so `.keys()` order and
  duplicate handling stay as today; lookup is a binary search. Objects are immutable once built.
- `Array` and `Object` cache their logical cost (§4) and nesting depth, computed once from their
  direct children when they are built, so charging or depth-checking an existing value is O(1).
- `Value` becomes 24 bytes. The cost model keeps 32 per value (it becomes an upper bound).
- `Expr` (ADR-0002) is unchanged and keeps `Rc`.

### 2. Language semantics (PRD-001 rows to add)
- The language-level type name stays **Json**: `::Json`, `type_name` `"json"` in error messages,
  and `"type": "Json"` on the servers' wire. Only the Rust variant is called `Object`.
- `::Json` parses a `String` as JSON (objects become `Object`, arrays `Array`, …) and fails at the
  cast with `Invalid JSON: …` on bad text; every other value passes through unchanged.
- `.to_json()` returns a `String` holding JSON text (it is text).
- Serializing an object or array to text follows "Printing numbers" below; keys are sorted.
- Objects keep any `Value` (a `Currency` stays a `Currency`).
- Equality (ADR-0023): two objects are equal when they have the same keys and `==` holds for each
  value; `Arc::ptr_eq` is a fast path. Unchanged results, since today's `json_eq` already ignores
  number spelling and key order.
- Depth (BL-017): an object counts as one nesting level, like an array.
- `CONCAT` and `::Boolean` on objects keep today's results (empty text, `true`).
- `Debug` prints an object as `Object(<JSON text>)`, e.g. `Object({"a":1})`. The CLI prints
  `Debug`, so `sk` output for objects changes from `Json("{\"a\":1}")`.

#### Printing numbers (decision d)
JSON text of an object or array (`.to_s()`, `::String`, `.to_json()`, `JOIN` of objects, server
and `sk --json` output, `:arguments`) prints each number as follows. This is today's code path
(`serde_json` printing an f64, `json!(n)`) with one change, the first bullet.
- `Number(n)` with n finite, `n.fract() == 0`, |n| < 2^53 and not `-0.0`: integer digits, `5`
  (today `5.0`). This is the only change.
- Every other `Number`: exactly what `serde_json` prints today: `1.5`, `0.1`, `1e-7`, `1e+21`,
  `-0.0`. Non-finite numbers cannot reach an object (division by zero is an error).
- `JsonNumber`: its source text, verbatim (`9007199254740993`, `2.50`, `1e21`).
- `Currency(c)`: exactly as today, `serde_json`'s f64 text: `5::Currency` prints `5.0`,
  `5.25::Currency` prints `5.25`. The integral rule does not apply and no fixed decimals are
  added. `DateTime` prints its integer seconds, as today.

Text of a single value outside JSON is unchanged: `::String`, `.to_s()`, `JOIN` and `CONCAT` on a
`Number` use Rust's f64 `Display` (`5`, `-0`, `1000000000000000000000`); on a `Currency`, `::String`
and `JOIN` use 4 decimals (`5.0000`) and `.to_s()` uses `Display` (`5.25`); a `JsonNumber` prints
its source text in all of them (decision b).

#### Exact JSON numbers (decision b)
- **When one is made.** Only when a number is read from JSON *text*: the `evaluate_with_json*`
  entry points, the servers' `arguments`, `sk --json`, `::Json` on a `String`, and
  `Value::from_json_text`. It is a `JsonNumber` when its source text differs from what the
  printing rule above prints for its f64: `9007199254740993`, `100000000000000000000000`, `2.50`,
  `1.0`, `1e21`, `0.0000001`, `-0`. It stays a `Number` for `1`, `1.5`, `0.1`, `1e-7`, `-0.0`.
  `json_to_value(serde_json::Value)` and `Deserialize` see an already-parsed number: they keep
  integers beyond 2^53 exactly (`serde_json` holds them as `u64`/`i64`, which is why today's
  pass-through is exact) but a decimal arrives as an f64, so `2.50` becomes `Number(2.5)`. The
  rustdoc says to pass JSON text to keep the spelling.
- **What it is in the language.** A number (`type_name` `"number"`, wire type `"Number"`). Every
  operator, comparison, builtin and method uses its f64 (ADR-0003): `:big == 9007199254740992` is
  `TRUE` and `SORT`/`UNIQUE` compare f64s, as today. Any computed result is a plain `Number`
  (`:big + 0` is `9007199254740992`). A value that is only moved keeps its text: variable reads,
  property, index, slice, `DIG` and `JQ` results, lambda items, array and object literal elements,
  `IF`/`IFS` branches, assignments, and the elements of `MERGE`, `FLATTEN`, `REVERSE`, `SORT`,
  `UNIQUE`, `FIRST`, `LAST`. Casts (`::Number`, `::Integer`, `::Currency`) compute, so they return
  plain values.
- **Where the text comes from.** `serde_json` (built without `arbitrary_precision`) reports a
  decimal only as an f64. `arbitrary_precision` cannot be enabled: Cargo unifies features, so it
  would change `serde_json::Number` in every embedder's build. So the text entry points use a
  Skillet JSON reader (`runtime/json_reader.rs`): RFC 8259, the same 128 depth limit as
  `serde_json`, building `Value`s directly (including the shared `:arguments`). It classifies each
  number while scanning: a plain decimal with no exponent, no leading zero, no trailing fractional
  zero, at most 15 significant digits and not below 1e-4 always prints back unchanged, so it is a
  `Number` with no further work; anything else (3.3% of the numbers in the `:rows` data) prints its
  f64 once and compares. `sk_http_server` reads `arguments` as a `serde_json` `RawValue` (the
  additive `raw_value` feature) and hands that text to the reader.
- **Debug** prints `Number(<text>)`, e.g. `Number(9007199254740993)`, so the CLI shows the exact
  value.

Example table (acceptance for QA; "today" measured on `f4f72da`):

| # | Expression (vars) | Today | After |
|---|---|---|---|
| E1 | `{"b":2,"a":1}.keys()` | `["a","b"]` | same |
| E2 | `:user.profile.age` (baseline DOC) | `Number(34.0)` | same |
| E3 | `("{\"a\":1}"::Json).a`, `"{\"a\":1}"::Json == {"a":1}` | `1`, `true` | same |
| E4 | `{"a":1} == {"a":1.0}`, `{"a":1,"a":2}.a` | `true`, `2` | same |
| E5 | `CONCAT({"a":1})`, `{"a":1}::Boolean` | `""`, `true` | same |
| E6 | `{"a":{"b":1}}.a` | `Json("{\"b\":1.0}")` | `Object({"b":1})` |
| E7 | `{"a":1}::String` | `"{\"a\":1.0}"` | `"{\"a\":1}"` |
| E8 | `{"a":1}.to_s() == :o.to_s()` (`o = {"a":1}` from JSON) | `false` | `true` |
| E9 | `5.to_json()`, `[1,{"b":2}].to_json()` | `Json("5.0")`, `Json("[1.0,{\"b\":2.0}]")` | `String("5")`, `String("[1,{\"b\":2}]")` |
| E10 | `NULL.to_json()` | `Json("{}")` | `String("{}")` (documented behaviour kept) |
| E11 | `"abc"::Json` | `Json("abc")` (fails later) | Error `Invalid JSON: …` |
| E12 | `"[1,2]"::Json`, `5::Json` | `Json("[1,2]")`, `Json("5")` | `Array([1,2])`, `Number(5.0)` |
| E13 | `{"p": 5::Currency}.p` | `Number(5.0)` | `Currency(5.0)` |
| E14 | `JOIN([{"a":1}], ",")` | `"{\"a\":1.0}"` | `"{\"a\":1}"` |
| E15 | `[{"a":1}]::String` | `"[Json(\"{\\\"a\\\":1.0}\")]"` | `"[Object({\"a\":1})]"` |
| E16 | `:o` printed by a server or `sk --json`, and `:o.to_s()` (`o = {"id": 9007199254740993}`) | `9007199254740993` | `9007199254740993` (decision b; kept exact) |
| E17 | `:nums.map(SUM(:nums)).length()` (10k numbers) | budget error | `Number(10000.0)` (relaxation, ADR-0011 §Consequences) |
| E18 | `:o` inside 100 nested array literals (`o` = a JSON object nested 40 deep) | value | `Expression nested too deeply (max 128)` |
| E19 | `{"a": 1.5}::String`, `{"a": 0.1}::String` | `{"a":1.5}`, `{"a":0.1}` | same |
| E20 | `{"a": 5}::String`, `[1.5, 5].to_json()` | `{"a":5.0}`, `Json("[1.5,5.0]")` | `{"a":5}`, `String("[1.5,5]")` |
| E21 | `{"a": 1e21}::String` | `{"a":1e+21}` | same (not below 2^53) |
| E22 | `{"a": -0.0}::String` | `{"a":-0.0}` | same (`-0.0` is excluded from the integral rule) |
| E23 | `{"a": 1e-7}::String` | `{"a":1e-7}` | same |
| E24 | `{"a": 5::Currency}::String`, `[5::Currency].to_json()` | `{"a":5.0}`, `Json("[5.0]")` | `{"a":5.0}`, `String("[5.0]")` |
| E25 | `{"a": 5.25::Currency}::String` | `{"a":5.25}` | same |
| E26 | `(5::Currency)::String`, `JOIN([5::Currency, 5.25::Currency], ",")`, `(5.25::Currency).to_s()` | `5.0000`, `5.0000,5.2500`, `5.25` | same |
| E27 | `:o.to_s()` (`o = {"c": 2.50, "i": 1.0, "e": 1e21}` from JSON text) | `{"c":2.5,"e":1e+21,"i":1.0}` | `{"c":2.50,"e":1e21,"i":1.0}` (decision b) |
| E28 | `:o.c * 2`, `:o.c` (same `o`) | `5`, `Number(2.5)` | `5`, `Number(2.50)` (prints its text, computes with 2.5) |
| E29 | `:big`, `:big.to_s()`, `JOIN([:big], ",")` (`big = 9007199254740993` from JSON text) | `Number(9007199254740992.0)`, `"9007199254740992"`, `"9007199254740992"` | `Number(9007199254740993)`, `"9007199254740993"`, `"9007199254740993"` |
| E30 | `:big == 9007199254740992`, `:big + 0` (same `big`) | `TRUE`, `9007199254740992` | same (f64 arithmetic, ADR-0003) |
| E31 | `json_to_value(json!({"c": 2.50}))` printed | `{"c":2.5}` | same (already-parsed input has no source text) |

### 3. Public API and migration

What breaks for embedders in 0.8.0:

| Use | 0.7 | 0.8.0 |
|---|---|---|
| Exhaustive `match` on `Value` | compiles | needs a `_` arm (`#[non_exhaustive]`, so later variants such as decimal `Currency` are not breaking) |
| Build an object | `Value::Json(text)` | `Value::from_json_text(&text)?` or `Value::Object(Object::from_iter(pairs))` |
| Read an object | `Value::Json(s)` + `serde_json::from_str(s)` | `Value::Object(o)` + `o.get("k")`, `o.iter()`, `o.to_json_text()` |
| Build a string | `Value::String(s)` (`s: String`) | `Value::String(s.into())` or `Value::from(s)` (`Value::String("x".into())` keeps compiling) |
| Build an array | `Value::Array(vec)` | `Value::Array(vec.into())` or `Value::from(vec)` |
| Read a string or array | `&String`, `&Vec<Value>` | `&Arc<str>`, `&Array`; both deref to `&str`, `&[Value]`, so reads compile; mutation needs `into_vec()` / `Array::make_mut` |
| `json_to_value(json)` | objects → `Json(text)` | objects → `Object`; signature unchanged |
| `evaluate_with_json*` | `:arguments` is `Json(text)` | `:arguments` is an `Object` whose children are shared with the top-level variables (one copy of the document instead of two); signatures unchanged |
| `CustomFunction::execute(Vec<Value>)` | objects arrive as text | objects arrive as `Object`; cloning arguments is O(1); signature unchanged; returning `Value::Json` must become `from_json_text` |
| Read a number | `Value::Number(n)` or `as_number()` | `as_number()` (covers `JsonNumber`); a bare `Value::Number(n)` pattern misses exact numbers that came from JSON text; `JsonNumber::text()` gives the source |
| Threads | `Value: Send + Sync` | unchanged |
| serde | `Value` implements neither | `Serialize` (0.7.x, additive; prints numbers by §2) and `Deserialize` (0.8.0, direct visitor, no intermediate `serde_json::Value`: measured faster than today's ingest; keeps integers beyond 2^53 exact, decimal spelling needs the text entry points) |
| JS plugins (ADR-0007) | objects passed as JSON text strings | unchanged: still JSON text, so existing hooks (`hook_examples/object_keys.js` parses text) keep working; JS objects returned become `Object` |
| Server wire | objects as JSON with `"type":"Json"` | unchanged, except E9/E10 (`.to_json()` is now `"type":"String"`), E7/E14/E20 integral spelling, E27/E29 source spelling kept, and `Debug` text inside nested arrays (E15) |

Migration path:
- **0.7.x (additive, no break):** `Value::from_json_text`, `Value::to_json_text`, `as_str`,
  `as_array`, `From<String | &str | Vec<Value> | f64 | bool> for Value`, `impl Serialize for Value`;
  `#[deprecated]` on `Value::Json` (note: "becomes `Value::Object` in 0.8.0; build with
  `Value::from_json_text`"), with `#[allow(deprecated)]` at the internal sites; rustdoc on `Value`
  recommends a `_` arm; `docs/guide/migration-0.8.md` started.
- **0.8.0 (one breaking release):** the layout in §1, every row above, the CHANGELOG **BREAKING**
  lines and the finished migration guide. No later release changes `Value` again for BL-011/036.

### 4. Budget accounting (amends ADR-0011)

Cost of a value (normative; the `Object` row is new, keys are charged like strings so the model
stays an upper bound on the measured heap):

```
cost(Number | Currency | DateTime | Boolean | Null) = 32
cost(String(s))       = 32 + len(s)
cost(Array(items))    = 32 + Σ cost(item)
cost(Object(entries)) = 32 + Σ (32 + len(key) + cost(value))
cost(JsonNumber(n))   = 48 + len(text)       // 32 + the 16-byte shared header
```

Costs are **logical**: a shared child is counted every time it appears, exactly as if it had been
copied. What is charged, and when:

| Site | Today | After |
|---|---|---|
| Copying variable read (function argument, array/object element, `IF` branch, …) | `cost(value)`, walked | **nothing**; a deadline tick (amendment 2's read charge is dropped) |
| Borrowed read (operand, method/property target) | tick | tick |
| Result of a literal, function, method, property access, index, slice, cast | `cost(result)`, walked | `cost(result)`, same number; O(1) for existing parts, O(k) for a new container with k direct children |
| Lambda item binding, assignment | `cost(item)` / `cost(value)` | same numbers, O(1) |
| Parsing JSON text (10 sites) | `len + 32 × (1 + count of , : [ {)` per access | gone; only `::Json` on a `String` parses, pre-charged `len + 64 × (1 + count of , : [ {)` (an upper bound for the new model) |
| Object literal | exact serialized size | `cost(object)` |
| `JQ` | parse charge on the input, `charge_json_node` per match | `cost(input)` before converting to a `serde_json` tree; matches charged with the new model |
| `.to_json()`, `::String`, `.to_s()`, `JOIN` on objects/arrays; HTTP response size check | exact text size, counted first | same rule (escaping can grow text 6×) |
| Embedder variables, ingest by `evaluate_with_json*` | not charged | not charged |
| Exact numbers (`JsonNumber`) | — | charged like any value where created or returned (`48 + len(text)`); a computation on one returns a 32-byte `Number` |

Why ADR-0011's guarantees still hold:
- **Deterministic.** Charges depend only on the logical values, never on what happens to be shared,
  so an expression with the same variables always passes or always fails.
- **Memory.** Sharing only lowers real memory below the logical cost; with the sorted-slice object
  layout the model is an upper bound for arrays and objects (measured above), and strings and exact
  numbers are within a few bytes, as strings are today (ADR-0011 finding N3). So live heap is still
  at most the budget plus the value that crossed it.
- **No exponential blow-up through sharing.** `[1×26].reduce([:acc, :acc], 0)` now builds 26 real
  nodes, but its logical cost doubles each step and is charged at each result site, so A1–A6 fail
  at the same steps as today. Anything that later walks such a value (serialize, flatten, compare)
  is bounded by a cost that was already charged.
- **Deadline.** Every read and operation still ticks; nothing that ticked today stops ticking.
- **What relaxes.** Formulas that copy a large variable per item (E17, ADR-0011 amendment 2's
  `:rows.map(SUM(:rows))`) pass again, as `:rows.map(:rows.sum())` already does. Their CPU is
  bounded by the server deadline, as ADR-0011 finding L1 already accepts for the method form. The
  P, A, T and C rows of ADR-0011 keep their outcomes; boundaries that involve objects move (QA
  re-pins them with the new formula).

### 5. Implementation plan
Ordered; each step is one PR to `develop`, keeps `./harness/check.sh` green and leaves the crate
releasable (steps 4 and 5 are both 0.8.0 material; if a release has to happen between them, step 4
alone is a coherent 0.8.0). "Must not regress" means more than 10% on the median (performance role
rule); same machine, before/after.

1. **Characterization and benches (QA, Performance; no `src/` change).** `tests/structured_values.rs`
   pins E1–E5 and today's results of E6–E18 (each changed row commented with its E number, updated
   in step 5). `benches/lambda_scope.rs` gains the rows of the Measurements table that it lacks
   (`filter…map`, object literal map, `SUM(:nums)`, `COUNT(:rows)`, assignment with `:rows`, ingest
   of `:rows`); `benches/BASELINE.md` records them.
2. **Additive API (0.7.x).** The helpers, `Serialize` and the `Json` deprecation from §3. Tests: doc
   tests per helper, `Serialize` round-trips for every variant. Bench: all `baseline` lib rows must
   not regress (no hot-path change).
3. **One JSON module (internal refactor, no behaviour change).** Route the 10 parse sites and 6 build
   sites through `runtime/object.rs` (`get`, `keys`, `values`, `has_key`, `dig`, `eq`, `to_text`,
   `to_serde`), so step 5 swaps one module. Tests: the suite unchanged. Bench: `json_vars`,
   `json_lambda`, `jsonpath_jq` and the `:rows` rows must not regress.
4. **Shared strings and arrays (BL-036; break 1).** `String(Arc<str>)`, `Array(Array)` with cached
   cost and depth; `check_value`/`charge_value` read the cache; drop the copying-read charge; the
   assignment entry clones the map of `Arc`s. Every expected `Value` in `tests/` stays the same
   (only construction syntax changes). New tests: E17; ADR-0011 C1–C6 and A1–A12 unchanged; an
   array literal of k reads of a big string is still charged k × its cost (finding F2). Bench: must
   improve `SUM(:nums)`, `COUNT(:rows)`, assignment with `:rows`; must not regress any `baseline`
   lib or HTTP row, `:nums.map(:x * 1).sum()` or `:nums.map(:x.abs()).sum()` (number arrays: no
   reference counts on the hot path).
5. **Object variant (BL-011; break 2).** `Object`, removal of `Json`, direct `Deserialize`,
   `json_to_value`, ingest with a shared `:arguments`, §2 semantics, server output, JS boundary,
   `#[non_exhaustive]`, and `JsonNumber` for integers beyond 2^53 (from `serde_json`'s `u64`/`i64`,
   so today's exact pass-through (E16) never regresses at any commit). Tests: E6–E26 and E29 at
   their new values, `limits.rs` unit tests for the
   object cost and the `::Json` pre-charge, and the existing tests that build or match `Value::Json`
   updated (`tests/{object_literals,conversion_methods,safe_navigation,evaluator_parity,jsonpath,
   eval_budget,dotted_function_names,evaluate_parsed,comments_and_whitespace_tests,
   http_server_jsonpath}.rs`, the `json_to_value` doc test, `benches/*` variable builders). Bench:
   must improve `json_vars`, `json_lambda` and every `:rows` row; ingest of `:rows` and
   `jsonpath_jq` must not regress.
6. **Exact JSON numbers from text (decision b; 0.8.0).** The Skillet JSON reader for the text
   entry points, number classification, `RawValue` in `sk_http_server`, `JsonNumber` for every
   number whose text does not print back. All internal number reads go through `as_number()`.
   Tests: E27, E28, E30, E31; the reader against a conformance list (valid and invalid RFC 8259 inputs,
   depth 128/129, escapes, invalid UTF-8 surrogates, numbers at the edges of f64); a property test
   that the reader and `serde_json` produce equal values (by f64) on random documents; a fuzz
   target for the reader (same infrastructure as the existing fuzz targets); and one CI test run
   with a `cfg` (`skillet_exact_numbers_everywhere`, set through `RUSTFLAGS`, not a Cargo feature)
   that makes the reader produce `JsonNumber` for every number, so any site that matches only
   `Value::Number` fails the existing suite. Bench: ingest of `:rows` must not regress more than
   10% against step 5 (estimate: classification is 0.28 ms for the 30k numbers of `:rows`, about
   9% of the 3.1–3.4 ms ingest if it were added on top of `serde_json`; the reader replaces that
   parse, so the net is expected at or below today); every `:rows` eval row must not regress
   (measured: summing or cloning 10k prices, 10% of them exact, is within noise, 6.6–6.9 µs both).
7. **Cleanup and docs.** Remove `charge_json_parse`/`count_node_bytes` and the `Json` estimate in
   `http_server/response_size.rs`; update `docs/guide/limits.md` (cost model, read charge),
   `docs/guide/user-guide.md` (object storage, `.to_json`, `::Json`), the migration guide, PRD-001;
   add a status note to ADR-0011; mark ADR-0004 `Superseded by 0030`.

## Consequences
- \+ Property access, `DIG`, `.keys()` and `JQ` inputs stop parsing text: `json_lambda` eval
  ~2.5 → ~0.6 µs and the 10k-row cases ~2.8 → ~0.35 ms (projected from the replica above; step 5
  measures them). BL-038's remaining `:rows` overhead goes with it.
- \+ Every read is O(1); `SUM(:nums)` and `:nums.sum()` cost the same; the read charge and its
  documentation (hoisting advice for copying reads) go away.
- \+ One object model: number spelling, key order and `Currency` tags no longer depend on where an
  object came from (E7, E8, E13). Servers stop re-serializing `:arguments` per request.
- \+ `Value` shrinks from 32 to 24 bytes.
- − One breaking release for embedders (§3), plus the language changes E6–E18 (mostly text
  spelling of objects, `.to_json()` returning a `String`, `::Json` validating at the cast).
- \+ JSON numbers passed through unchanged keep their exact text (decision b), now also outside
  objects (`:big`, `:o.id`, E29) and for decimal spelling (E27), which today is lost.
- − A second number variant: embedders must read numbers with `as_number()`, and two integers that
  differ only beyond 2^53 still compare equal (`==` is on f64, ADR-0003; an exact integer type
  would be a separate ADR).
- − Skillet owns a JSON reader for the text entry points (conformance, property and fuzz tests).
- − Atomic reference counts on clones of strings, arrays and objects (3–4 ns each, measured); number
  arrays are unaffected.
- − Objects nested inside arrays now count towards `MAX_NESTING_DEPTH` (E18).
- The key sanitization collision of ADR-0004 (`"a-b"` and `"a_b"` → `:a_b`) is unchanged and stays
  open.

## Alternatives considered
Options A, B and C above, and `Rc` in D. For decision (b): changing `Number(f64)` to
`Number(Num)` with an optional text (compiler-checked, but every `Value::Number(2.0)` in embedder
code and 262 + 241 sites in `src/` and `tests/` change); a side table of texts in arrays and
objects (cannot cover a scalar variable such as `:big` or a property result such as `:o.id`);
`serde_json`'s `arbitrary_precision` (Cargo feature unification changes `serde_json` for the
embedder's whole build); a decimal type for every number (ADR-0003 territory). Also: Also: keeping both `Json(String)`
(deprecated, accepted as input) and `Object` in 0.8.0. Rejected: embedders that match `Value::Json` on results with a
wildcard arm would silently stop matching, and every internal site would handle both forms for a
release; removing it makes every affected line a compile error instead.
