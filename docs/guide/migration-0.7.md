# Migrating from 0.6 to 0.7

Skillet 0.7.0 contains breaking changes. Before 1.0, a minor release may break behaviour
(ADR-0010). This page lists every breaking change, with the old and the new result. The
[changelog](../../CHANGELOG.md) has the full list of changes, including the ones that do not break.

Check these first. They change results without an error:

1. **`-x^2` and `a^b^c`** (operator precedence, below). Search your formulas for `^`.
2. **`COUNT`** now counts numbers only.
3. **`MOD`** with negative numbers, **`TRIM`** with inner spaces, **`ROUND`** with negative digits.
4. **`JQ`** with a wildcard path that matches one item now returns an array.

## Expressions

### Operators (ADR-0018 D-10)
Skillet now uses Excel's precedence. Unary minus binds tighter than `^`, and `^` is
left-associative.

| Expression | 0.6 | 0.7 | Keep the old result with |
|---|---|---|---|
| `-2^2` | `-4` | `4` | `-(2^2)` |
| `-:x^2` (`x` = 3) | `-9` | `9` | `-(:x^2)` |
| `2*-3^2` | `-18` | `18` | `2*-(3^2)` |
| `2^3^2` | `512` | `64` | `2^(3^2)` |
| `0-2^2`, `1-2^2`, `2^-2` | `-4`, `-3`, `0.25` | unchanged | |

### Errors instead of numbers (ADR-0018 D-1)
Where Excel returns an error value (`#DIV/0!`, `#NUM!`, `#VALUE!`, `#N/A`), Skillet now returns an
`Error`. Aggregates still skip `NULL` items, like blank cells.

| Expression | 0.6 | 0.7 |
|---|---|---|
| `10 / 0`, `0 / 0` | `inf`, `NaN` | `Error` |
| `MOD(5, 0)` | `NaN` | `Error` |
| `SQRT(-1)`, `POWER(0, -1)` | `NaN`, `inf` | `Error` |
| `ABS("x")`, `SQRT("x")`, `FLOOR("3.7")` | `0` | `Error` |
| `AVERAGE()`, `MEDIAN()`, `STDEV_P()`, `VAR_P()` | `0` | `Error` |
| `PERCENTILE_INC([], 0.5)`, `QUARTILE_INC([], 1)` | `0` | `Error` |
| `MODE_SNGL(1, 2, 3)` (no repeated value) | `1` | `Error` |
| `AVGIF([1, 2], :x > 5)` (no match) | `0` | `Error` |
| `ROUND(2.567, "x")`, `PMT(0.05, 10, 1000, "x")` | argument ignored | `Error` |
| `COUNTIF([1,2,3], :x)` (lambda returns a non-boolean) | `0` | `Error` |
| `SUM()`, `MAX()`, `SUM([1, NULL])` | `0`, `0`, `1` | unchanged |

To keep a default, test first: `IF(:n == 0, 0, :total / :n)` (now lazy, see below).

### Builtins that now match Excel (ADR-0018)
| Expression | 0.6 | 0.7 |
|---|---|---|
| `COUNT([1, "a", TRUE, NULL])` | `4` | `1` (numbers only) |
| `COUNT(1, 2, 3)` | `Error` | `3` |
| `COUNTIF([1,2,3,4], ">2")`, `COUNTIF([1,2,3], 2)` | `0`, `0` | `2`, `1` |
| `SUMIF(["A","b"], "a", [1,2])` | `0` | `1` (text ignores case) |
| `SUMIF(["apple","avocado"], "a*", [1,2])` | `0` | `3` (wildcards) |
| `MOD(-10, 3)`, `MOD(10, -3)` | `-1`, `1` | `2`, `-2` |
| `TRIM("  a   b  ")` | `"a   b"` | `"a b"` |
| `ROUND(1234.567, -2)`, `ROUND(15, -1)` | `1235`, `15` | `1200`, `20` |
| `ROUND(1.005, 2)` | `1.0` | `1.01` |
| `CEILING(2.1, 0.5)`, `CEILING(7, 5)` | `3`, `7` | `2.5`, `10` |
| `FLOOR(7, 5)` | `7` | `5` |
| `TIME(14, 30, 0)` | current time (0.6.3) | `0.6041666666666666` (fraction of a day) |
| `IPMT(0.1, 1, 3, 1000)`, `IPMT(0.1, 2, 3, 1000)` | `100`, `49.79…` | `-100`, `-69.79…` |
| `FIND("b", "abc")` | `Error` | `2` (text search) |
| `IF(TRUE, 1, UPPER(1))` | `Error` | `1` (only the chosen branch runs) |
| `IF("x", 1, 2)` | `2` | `Error` |
| `IFS(FALSE, 1)` (no true condition) | `false` | `Error` |

The methods keep Ruby semantics and do not change: `"  a   b  ".trim()` is still `"a   b"`, and
`(-3.7).int()` is still `-3` (while `INT(-3.7)` is `-4`).

### Dates (ADR-0018 D-5)
| Expression | 0.6 | 0.7 |
|---|---|---|
| `DATE(2026, 13, 1)` | `Error: Invalid date` | 2027-01-01 |
| `DATE(2026, 2, 30)`, `DATE(2026, 1, 0)` | `Error` | 2026-03-02, 2025-12-31 |
| `DATE(26, 1, 1)` | year 26 | 1926-01-01 (years 0–1899 get 1900 added) |
| `DATE(2026, 1, 1) + 1` | `Error` | 2026-01-02 (numbers count days) |
| `DATE(2026, 1, 2) - DATE(2026, 1, 1)` | `Error` | `1` |
| `"2026-02-30"::DateTime` | `Error` | `Error` (casts never roll over) |

### New names (additive)
These used to fail with `Unknown function` or a parse error: `STDEV.P`, `STDEV.S`, `STDEV`,
`VAR.P`, `VAR.S`, `VAR`, `MODE.SNGL`, `MODE`, `PERCENTILE.INC`, `PERCENTILE`, `QUARTILE.INC`,
`QUARTILE`, `COUNTA`, `LEN`, `TODAY`, `PPMT`, `PV`, `NPV`, `TEXTJOIN`, `AVERAGEIF`.

### Null equality (ADR-0013)
| Expression | 0.6 | 0.7 |
|---|---|---|
| `NULL == NULL` | `false` | `true` |
| `NULL != NULL` | `true` | `false` |
| `[NULL, 1, 2].filter(:x != NULL)` | `[NULL, 1, 2]` | `[1, 2]` |
| `NULL == 0`, `NULL < 1` | `false`, `Error` | unchanged |

### `JQ` result shape (ADR-0014)
The shape now follows the path, not the number of matches. A singular path (`.name`, `[n]`)
returns the value, or `NULL` when it is missing. Any other path returns an array.

| Expression | 0.6 | 0.7 |
|---|---|---|
| `JQ({"a":[3]}, "$.a[*]")` | `3` | `[3]` |
| `JQ({"a":[3,4]}, "$.a[0:1]")` | `3` | `[3]` |
| `JQ({"a":[{"p":1}]}, "$..p")` | `1` | `[1]` |
| `JQ({"a":5}, "$.b")` | `[]` | `NULL` |
| `JQ({"a":[3,4]}, "$.a[*]")`, `JQ({"u":{"n":"Ann"}}, "$.u.n")` | `[3, 4]`, `"Ann"` | unchanged |

If you used a wildcard path as a scalar, add `.first()`.

### NaN in statistics (ADR-0015)
| Expression | 0.6 | 0.7 |
|---|---|---|
| `PERCENTILE_INC([1,2,3], 0/0)` | `1` | `Error` |
| `QUARTILE_INC([1,2,3,4], -0.5)`, `QUARTILE_INC([1,2,3,4], 4.9)` | `1`, `4` | `Error` |
| `MEDIAN([1, 0/0, 3])` | crash (panic) | `Error` |

### Custom functions (ADR-0016)
Custom and JS functions are now visible in every sub-expression.

| Expression | 0.6 | 0.7 |
|---|---|---|
| `MAP([21000], MX_ISR(:x, "semanal").isr)` | `Unknown function` | works |
| `FILTER`, `REDUCE`, `SUMIF`, `FIND` lambdas, method arguments | `Unknown function` | works |
| `:v := MX_ISR(21000, "semanal").isr; :v` | `Unknown function` | works |
| A custom `SUM` inside a `FILTER` lambda | builtin `SUM` | the custom `SUM` |

## Evaluation limits (ADR-0011)
Each evaluation has a budget of 256 MB of created values, and the servers stop an evaluation after
10 s. Details: [Limits](limits.md).

| Situation | 0.6 | 0.7 |
|---|---|---|
| A formula that builds huge values (`[…].reduce([:acc, :acc], 0)`) | runs until memory is gone | `Evaluation budget exceeded (max 256 MB)` |
| Copying a large variable once per item (`:rows.map(SUM(:rows))`) | slow | may hit the budget; hoist it: `:t := SUM(:rows); :rows.map(:t)` |
| `reduce(MERGE(:acc, [:x]), [])` over more than about 2,800 items | runs | budget error |
| A server evaluation that runs longer than 10 s | runs | `Evaluation timed out (max 10 s)` (HTTP 400) |
| Expressions nested deeper than 128 | stack overflow (crash) | `Expression nested too deeply (max 128)` |

## JavaScript plugins (ADR-0017)
| Situation | 0.6 | 0.7 |
|---|---|---|
| A hook calls `httpGet(url)` | runs `curl` | `httpGet` is not defined, so the call fails. Hooks are pure functions of their arguments |
| A hook runs longer than 10 s | runs forever | `JS function NAME timed out (max 10 s)`, or the server's `Evaluation timed out (max 10 s)` |
| A hook uses more than 64 MiB | runs | `JS function NAME ran out of memory (max 64 MB)` |
| Deep recursion in a hook | `Exception generated by QuickJS` | `JS function NAME failed: RangeError: Maximum call stack size exceeded` |
| A hook returns an array deeper than 128, or a cyclic array | process abort | `Expression nested too deeply (max 128)` |
| Any other JS exception | `Exception generated by QuickJS` | `JS function NAME failed: <message>` |

## Rust API
| Item | 0.6 | 0.7 |
|---|---|---|
| `skillet::memory_pool`, `skillet::traits`, `skillet::concurrent_registry` | public | removed (ADR-0010) |
| `skillet::runtime::evaluation` | re-export shim | removed; use `skillet::runtime::evaluator` or the top-level `evaluate*` functions (ADR-0010) |
| `runtime::method_calls::exec_filter`/`exec_map`/`exec_find`/`exec_reduce`, `exec_string_method`, `exec_array_method` | take a variable map | take `&dyn EvaluationContext` |
| Minimum Rust version | not stated | 1.85 (ADR-0012) |

## Servers
| Change | 0.6 | 0.7 |
|---|---|---|
| `sk_http_server` request bodies | parsed as JSON whatever the `Content-Type` | `Content-Type: application/json` is required; anything else gets `415 Unsupported Media Type` |
| CORS on `sk_http_server` | every response has `Access-Control-Allow-Origin: *` | no CORS header when the server runs without a token (development mode), so web pages cannot drive a local server. Rules in [ADR-0019](../adr/0019-server-cors-and-development-mode.md) |
| Admin token from the environment | only `--admin-token` | `SKILLET_ADMIN_TOKEN` is read too, like `SKILLET_AUTH_TOKEN` |
| Docker image without tokens | starts unauthenticated on `0.0.0.0` | refuses to start; set `AUTH_TOKEN` and `ADMIN_TOKEN` ([Docker](../ops/docker.md)) |
| Empty token (`--token ""`, `--admin-token ""`, empty `SKILLET_AUTH_TOKEN`) | auth silently off | the server exits with code 1 |
| `sk_server` wrong or missing token | can retry on the same connection | `Unauthorized: invalid token`, then the connection closes |
| `/upload-js`, `/update-js`, `/delete-js` file names | any path | only `[A-Za-z0-9_-]+.js`, otherwise 400 |
| Writing one response | 10 s per socket write | 10 s in total; a slower reader gets a truncated response |

Update clients first: send `Content-Type: application/json` on every `POST` and `PUT`, and set
tokens on every deployment.
