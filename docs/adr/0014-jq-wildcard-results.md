# 0014. `JQ` result shape follows the path, not the match count

- Status: Accepted (2026-11-21, option B)
- Date: 2026-11-20
- Deciders: Language Designer (proposal), Maintainer
- Related: BL-024, PRD-001 FR-8, ADR-0011 (JQ budget), `src/runtime/jsonpath.rs`

## Context
`apply_jsonpath` picks the result shape from the **number of matches**: 0 matches give `[]`,
1 match gives the bare value, and 2 or more give an array. So the type of `JQ(...)` depends on the data,
not on the expression. Measured on the release build:

```
=JQ({"a":[3,4]}, "$.a[*]")             -> Array([Number(3.0), Number(4.0)])
=JQ({"a":[3]},   "$.a[*]")             -> Number(3.0)
=JQ({"a":[]},    "$.a[*]")             -> Array([])
=JQ({"a":[3]},   "$.a[*]").sum()       -> Error: Unknown number method: sum
=[{"a":[3]},{"a":[1,2]}].map(JQ(:x, "$.a[*]").sum()) -> Error: Unknown number method: sum
=JQ({"a":[3,4]}, "$.a[0:1]")           -> Number(3.0)     (a slice, yet a scalar)
=JQ({"a":[{"p":1}]}, "$..p")           -> Number(1.0)
=JQ({"u":{"n":"Ann"}}, "$.u.n")        -> String("Ann")
=JQ({"a":5}, "$.b")                    -> Array([])       (missing key gives [], not null)
=SUM(JQ({"a":[3]}, "$.a[*]"))          -> Number(3.0)     (aggregates hide the problem)
```

Every documented example wraps `JQ` in `SUM`/`AVG`/`MIN`/`MAX`. Those functions accept a scalar or an
array, so the bug only shows up with methods (`.sum()`, `.length()`, `.map()`) or in per-row lambdas.
The workaround is `.to_a()`.

Precedent: RFC 9535 (JSONPath) always returns a *nodelist*. It defines a **singular query** (only
name and index selectors, no `..`) as one that yields at most one node. Goessner's JSONPath and `jq`
return a list for wildcards too. Postgres `jsonb_path_query_first` and `jsonb_path_query` split the
two cases into two functions.

## Options

| Expression | Current | A: always array | B: shape by path |
|---|---|---|---|
| `JQ({"a":[3]}, "$.a[*]")` | `3` | `[3]` | `[3]` |
| `JQ({"a":[3,4]}, "$.a[*]")` | `[3, 4]` | `[3, 4]` | `[3, 4]` |
| `JQ({"a":[3,4]}, "$.a[0:1]")` | `3` | `[3]` | `[3]` |
| `JQ({"a":[{"p":1}]}, "$..p")` | `1` | `[1]` | `[1]` |
| `JQ({"u":{"n":"Ann"}}, "$.u.n")` | `"Ann"` | `["Ann"]` | `"Ann"` |
| `JQ({"a":[3,4]}, "$.a[0]")` | `3` | `[3]` | `3` |
| `JQ({"a":5}, "$.b")` | `[]` | `[]` | `NULL` |
| `JQ({"a":[3]}, "$.a")` | `[3]` | `[[3]]` | `[3]` |
| `SUM(JQ(:x, "$.a[*]"))` | same | same | same |

- **A. Always an array** (RFC 9535 nodelist). Simple and predictable. But `JQ(:arguments, "$.name")`
  becomes `["acme"]`, which breaks every scalar lookup. Those would need `.first()` or `DIG`.
- **B. Shape by path.** A singular path (only `.name`, `['name']`, `[n]`, no `..`) returns the value,
  or `NULL` when it is missing. Any other path (`[*]`, `..`, slice, filter `?()`, union `[a,b]`)
  always returns an array, possibly `[]`. The type then follows from the expression alone.
- **C. Keep, and document `.to_a()`.** Not a breaking change, but the trap stays in the most common use.

## Decision
Adopt **B**. Singular-ness is decided from the parsed path, never from the data (RFC 9535 §2.3.5.1).
A singular path that matches nothing returns `NULL`. That fits ADR-0006, where a missing value is `null`,
and `SUM(NULL)` is already `0`. PRD-001 FR-8 gains: "`JQ` returns a value for a singular path
(`NULL` if absent) and an array for any other path."

## Compatibility
- Changed: wildcard, `..`, slice or filter paths with exactly one match (scalar becomes `[x]`), and
  singular paths that miss (`[]` becomes `NULL`).
- Unchanged: every test and doc example. They all aggregate (`SUM/AVG/MIN/MAX(JQ(...[*]...))`),
  match 0 or 2+ nodes, or use a singular path that hits. That covers `tests/{jsonpath,cli_integration,http_server_jsonpath,
  eval_budget,evaluator_parity}.rs`, `README.md`, `docs/guide/{user-guide,servers}.md`,
  `docs/reference/api.md` and ADR-0011's `JQ(:rows, "$[*].price").sum()`, which now also works
  for a single row.
- `docs/reference/api.md` §JQ should state the shape rule (docs follow-up).
- Add a 0.7.0 changelog note. Code that used `JQ(..."[*]"...)` as a scalar for one-row input must add
  `.first()`.

## Consequences
- + `.sum()`, `.length()` and `.map()` on `JQ` results work for every row.
- − Small parser-level cost: classify the path once per call. That is negligible next to the query.
- − A user who wants "first match" from a wildcard writes `JQ(...).first()`.

## Alternatives considered
- A: breaks scalar lookups that work today for no gain over B.
- C: leaves data-dependent types in place.
- A separate `JQ_ALL()` function: two names for one concept, which breaks the "one obvious way" rule.
