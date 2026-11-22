# Changelog

All notable changes to the `skillet` crate and its binaries (`sk`, `sk_server`, `sk_client`,
`sk_http_server`, `sk_http_bench`) are recorded here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). Before 1.0, a minor release
(`0.x.0`) may contain breaking changes and a patch release (`0.x.y`) contains fixes only.
Breaking changes are marked **BREAKING**.

Releases are tagged `vX.Y.Z` (binaries, GitHub release and crates.io, ADR-0022). Tags for 0.6.3
and earlier are `crate-vX.Y.Z`. Entries for 0.6.3 and earlier were reconstructed from git tags and
commit messages, so they are brief.

## [Unreleased]

## [0.7.0] - YYYY-MM-DD

0.7.0 makes the UPPERCASE builtins and the operators follow Excel, and it breaks behaviour in
many places. The
[migration guide](https://github.com/zenbakiak/skillet/blob/develop/docs/guide/migration-0.7.md)
lists every breaking change with the old and the new result; read its "Check these first" list
before you upgrade.

### Breaking changes
Each item is marked **BREAKING** in the sections below. In short:

- **Operators**: Excel precedence (`-2^2` is 4, `2^3^2` is 64); `%` before `+`/`-` is a percent;
  `1e3` is 1000; parsing requires the end of the input; the `AND`/`OR` operators work
  ([operators](https://github.com/zenbakiak/skillet/blob/develop/docs/guide/migration-0.7.md#operators-adr-0018-d-10),
  [parsing](https://github.com/zenbakiak/skillet/blob/develop/docs/guide/migration-0.7.md#parsing-requires-the-end-of-input-bl-069-93)).
- **Builtins follow Excel**: `COUNT`, `MOD`, `TRIM`, `ROUND`, `CEILING`/`FLOOR`, `TIME`, `IPMT`,
  `DB`, `SUBSTITUTE`, `SORT`, criteria in `COUNTIF`/`SUMIF`, `LEFT`/`MID`/`REPLACE`, `DATE`
  rollover; errors instead of 0, NaN or infinity; `.round()` is `ROUND`
  ([builtins](https://github.com/zenbakiak/skillet/blob/develop/docs/guide/migration-0.7.md#builtins-that-now-match-excel-adr-0018),
  [errors](https://github.com/zenbakiak/skillet/blob/develop/docs/guide/migration-0.7.md#errors-instead-of-numbers-adr-0018-d-1)).
- **Comparisons and conditions**: `==` compares arrays, objects and dates by value, `NULL == NULL`
  is true, and one condition rule (`IF`'s) for every form
  ([comparisons](https://github.com/zenbakiak/skillet/blob/develop/docs/guide/migration-0.7.md#comparisons-adr-0023-88),
  [logical functions](https://github.com/zenbakiak/skillet/blob/develop/docs/guide/migration-0.7.md#logical-functions-and-operators-adr-0018-d-8-adr-0029)).
- **Types and text**: one number type (`::Currency` is a conversion); one text form per value
  type; dates as ISO 8601 text; dates in aggregates, sorting and criteria; VBA date unit codes
  ([currency](https://github.com/zenbakiak/skillet/blob/develop/docs/guide/migration-0.7.md#currency-is-a-conversion-adr-0033),
  [values as text](https://github.com/zenbakiak/skillet/blob/develop/docs/guide/migration-0.7.md#values-as-text-adr-0034),
  [date units](https://github.com/zenbakiak/skillet/blob/develop/docs/guide/migration-0.7.md#date-units-follow-excel-b-15)).
- **Servers, JSON variables and plugins**: `Content-Type: application/json` required, CORS off in
  development mode, the Docker image needs both tokens, 408/413 status codes, JSON variables are
  charged to the budget, `:arguments` is always the whole object, hooks cannot use builtin names,
  `httpGet` is gone
  ([servers](https://github.com/zenbakiak/skillet/blob/develop/docs/guide/migration-0.7.md#servers),
  [JSON variables](https://github.com/zenbakiak/skillet/blob/develop/docs/guide/migration-0.7.md#json-variables-bl-077),
  [JavaScript plugins](https://github.com/zenbakiak/skillet/blob/develop/docs/guide/migration-0.7.md#javascript-plugins-adr-0017)).
- **Rust API**: `Value` and the AST and token enums are `#[non_exhaustive]`, `&` is
  `BinaryOp::Concat`, dead modules are removed, `Value::Json` and `Value::Currency` are
  deprecated ([Rust API](https://github.com/zenbakiak/skillet/blob/develop/docs/guide/migration-0.7.md#rust-api)).

### Added
- `Value::from_json_text` and `Value::to_json_text` read and write JSON text the way the
  `evaluate_with_json*` entry points and `.to_json()` do; `Value::as_str`, `Value::as_array`;
  `From<String | &str | Vec<Value> | f64 | bool> for Value`; `impl serde::Serialize for Value`
  (ADR-0030 step 2, BL-104, #132).
- `skillet::charge_variables(&vars)` charges a variable map to the current evaluation budget;
  `evaluate_with_json*` call it, a `HashMap` passed to `evaluate_with` is charged only when the
  caller does (ADR-0011 amendment 5, BL-086, #124).
- `skillet::is_volatile(&Expr)`: TRUE when the expression reads the clock anywhere inside it
  (`NOW()`, `TODAY()`, `DATE()`/`TIME()` with no arguments) (BL-085, #122).
- Casts chain left to right: `DATE(2026,1,1)::String::DateTime` works (was a parse error), and
  cast errors give the position of the type name (ADR-0031 amendment 1, BL-078, #112).
- `::DateTime` reads the ISO 8601 text that dates print as (`2026-01-01T00:00:00Z`); integer text
  still works (ADR-0031, BL-076, #108).
- `sk_http_server` and `sk_server`: `--eval-timeout <seconds>` / `SKILLET_EVAL_TIMEOUT` (1–86400)
  and `--max-eval-bytes <bytes>` (`K`/`M`/`G`) / `SKILLET_MAX_EVAL_BYTES`; the flag wins, and `0`
  or an invalid value stops start-up. The banner shows both and the worst-case memory
  (ADR-0011 amendment 4, BL-075, #106).
- `DATEDIF(start, end, unit)` with Excel's worksheet codes `y m d md ym yd` (BL-074, #103,
  ADR-0035).
- `NOT` prefix keyword, any case, with the same result as `NOT(x)` (ADR-0029, #102).
- `/health` reports the parse cache (hits, misses, hit rate, entries) over all workers, and
  `DELETE /cache` clears every worker's parse cache too (BL-067, #99).
- The Alpine builder builds x86_64 as well as aarch64, `--locked`, on a pinned
  `rust:1.89.0-bookworm` image (BL-065, #95).
- Scientific notation number literals: `1e3`, `2.5e-4`, `1E+2` (BL-058, #89).
- `sk_http_server` and `sk_server`: optional limit on concurrent connections per IP address,
  `--max-connections-per-ip N` or `SKILLET_MAX_CONNECTIONS_PER_IP`; over the limit, HTTP 429 or a
  JSON error line (ADR-0025, #87).
- New Excel builtins (ADR-0018): `LEN`, `TEXTJOIN`, `TODAY`, `PPMT`, `PV`, `NPV`, `COUNTA`,
  `AVERAGEIF`, the sample statistics `STDEV.S`/`STDEV` and `VAR.S`/`VAR`, and the names `MODE`,
  `PERCENTILE` and `QUARTILE`. Dotted names such as `STDEV.P(…)` parse (#82, #83).
- `&` joins text, and a `%` suffix divides by 100 (`15%` is 0.15) (#82).
- `FIND(text, within, [start])` is Excel's 1-based, case-sensitive text search;
  `FIND(array, lambda)` is unchanged (#83).
- Date arithmetic in days: `date ± n` and `n + date` give a DateTime, `date - date` the number
  of days (#83).
- `evaluate_parsed`, `evaluate_parsed_with`, `evaluate_parsed_with_custom`,
  `evaluate_parsed_with_assignments` and `evaluate_parsed_with_assignments_and_context` evaluate
  the `Expr` returned by `parse`, with the same results, errors and limits (ADR-0021, #80).
- `sk_http_bench --expr` selects the expression for the sweep (#80).
- `sk_http_server --cors-origin` / `SKILLET_CORS_ORIGIN` sets the CORS origin, and
  `SKILLET_ADMIN_TOKEN` sets the admin token (the flag wins) (ADR-0019, #78).
- Docs: threat model (#73), migration guide (#77), limits (#49), a generated function reference
  (#58), the Excel comparison (#56) and rustdoc examples (#60).
- Release archives come with a `.tar.gz.sha256` checksum file, verified before upload (#53).
- The binaries build without the `plugins` feature (`cargo build --no-default-features`); the
  HTTP JS management endpoints then answer 404 after the admin-token check (BL-025).
- Per-evaluation memory budget (ADR-0011): `skillet::MAX_EVAL_BYTES` (256 MB of created values),
  `skillet::EvalLimits { max_bytes, timeout }` and `skillet::with_limits(limits, f)`; nested calls
  can only tighten limits (#46).
- `skillet::MAX_NESTING_DEPTH` (128): deeper expressions and runtime-built arrays fail with
  `Expression nested too deeply (max 128)` instead of overflowing the stack (#44).
- Benchmarks: `benches/baseline.rs` and `benches/BASELINE.md` for the library, CLI and HTTP
  layers (#48); `benches/per_item.rs` for per-item costs on 100k numbers (BL-088, #127).

### Changed
BREAKING changes first, newest first; then the other changes, newest first.

- **BREAKING** (Rust API): `Value`, `lexer::Token`, `ast::Expr`, `ast::BinaryOp`, `ast::UnaryOp`
  and `ast::TypeName` are `#[non_exhaustive]`, so a `match` on them outside the crate needs a `_`
  arm; later variants (`Value::Object` in 0.8.0) will not break (BL-092, #131). Earlier in 0.7.0,
  `&` became `Expr::Binary(_, BinaryOp::Concat, _)` (was a `__CONCAT__` call, ADR-0034, #129) and
  `Token` gained `Ampersand` (#82).
- **BREAKING:** one text form per value type in `::String`, `.to_s()`, `CONCAT`, `&`, `JOIN`,
  `.join()`, `TEXTJOIN`, `LEN`/`FIND` and `.to_json()`: numbers with 15 significant digits
  (`"Total: " & 3*0.1` is `"Total: 0.3"`), booleans `TRUE`/`FALSE`, an array or object as its
  JSON text, `.length()` counts characters; `__CONCAT__` is an unknown function
  (ADR-0034, BL-091, #129).
- **BREAKING:** dates in aggregates, sorting and criteria: `MIN`/`MAX`, `SORT`, `.sort()`,
  `.min()`/`.max()`, `BETWEEN` and `COUNTIF`/`SUMIF`/`AVERAGEIF` criteria compare dates by instant
  (`COUNTIF(r, ">" & DATE(2026,6,1))` works); `SUM`, `AVERAGE` and the other arithmetic aggregates
  reject a date with `#VALUE!` (dates were skipped) (ADR-0031 amendment 2, BL-090, #128).
- **BREAKING:** `.round([digits])` is `ROUND`: `1.005.round(2)` is 1.01 (was 1.0); about 3% of
  two-decimal halves change (ADR-0018, BL-088, #127).
- **BREAKING:** JSON variables count against the evaluation budget; under the default 256 MiB no
  request under the 1 MiB cap is affected (ADR-0011 amendment 5, BL-086, #124).
- **BREAKING:** `sk --json` exits 1 for invalid JSON, a non-object or colliding keys, on every
  path (the plain path exited 2); evaluation errors still exit 2 (BL-096, #123).
- **BREAKING:** one condition rule (`IF`'s) for `IF`, `IFS`, `? :`, the logical operators,
  `filter`/`find`/`FILTER`/`FIND` and lambda criteria: a number is TRUE when not 0, `NULL` is
  FALSE, text, dates and arrays are `#VALUE!`. `[0,1,2].filter(:x)` is `[1,2]` (was `[]`). The
  `AND()`/`OR()`/`XOR()` builtins still skip `NULL` (ADR-0029 amendment 1, BL-089, #120).
- **BREAKING:** one number type: `x::Currency` rounds to 4 decimals and returns a Number, which
  prints, compares and computes as one; responses say `"type": "Number"` (ADR-0033, BL-097, #119).
  Currency rules from earlier development builds (#97, #108, #112, #115) did not ship.
- **BREAKING:** JSON variables: `:arguments` is always the whole object (a key named `arguments`
  is `:arguments.arguments`), and two keys that map to the same variable (`a-b`, `a_b`) are an
  error, HTTP 400 on the server. `sk_server` sanitizes `variables` keys the same way and builds
  no `:arguments` (BL-077, #111; BL-096, #123).
- **BREAKING:** `/reload-hooks` removes custom functions whose hook file is gone, registers the
  new set before removing anything, changes nothing when the directory cannot be read, and clears
  the result cache (BL-077, #111).
- **BREAKING:** dates as text are ISO 8601 UTC: `DATE(2026,1,1)::String` is
  `"2026-01-01T00:00:00Z"` (was `"1767225600"`); server wire formats are unchanged
  (ADR-0031, BL-076, #108).
- **BREAKING:** `DATEADD` and `DATEDIFF` read VBA's unit codes, any case: `"m"` is months (was
  minutes), `"y"` is days (was years), `DATEADD`'s `"w"` adds a day (was a week); the long words
  are unchanged. `DATEADD` clamps to the end of the month (ADR-0035, BL-074, B-15, #103).
- **BREAKING:** Excel logic: the operators short-circuit; the `AND`/`OR`/`NOT`/`XOR` builtins skip
  text and `NULL` in arrays and a `NULL` argument (`AND(TRUE, NULL)` is TRUE, `AND()` is
  `#VALUE!`); `IFS` with no true condition is `#N/A` (ADR-0029, ADR-0018 D-8, B-18, #102).
- **BREAKING** (wire format): `sk_http_server` answers an oversized request with HTTP 413 and a
  slow one with 408 (both were 400), after reading the rest of the request (bounded) (BL-049, #99).
- **BREAKING:** date arithmetic like Excel: in `DateTime ± x`, `x` converts with the ADR-0023 rule
  (`+ NULL` is the date, `+ "1"` or `+ TRUE` the next day, other text `#VALUE!`)
  (ADR-0027, BL-070, #97).
- **BREAKING:** text builtins follow Excel: `LEFT`, `RIGHT`, `MID` and `REPLACE` give `#VALUE!` for
  a negative length or a start below 1; `SORT` sorts text case-insensitively, orders mixed values
  numbers < text < booleans, and is stable (ADR-0018, #96).
- **BREAKING:** parsing requires the end of the input (`=1 2` is an error; the rest was dropped),
  and error positions count a leading `=`. So the `AND`/`OR` operators work: `TRUE AND FALSE`
  was TRUE and is FALSE (BL-069, BL-060, #93).
- **BREAKING:** arithmetic converts like Excel (`"3"+1` is 4, `1 + NULL` is 1, other text
  `#VALUE!`); `%` before `+`/`-` is a percent (`10 % -3` is -2.9); `1x` is an
  `Invalid number literal` (ADR-0023, #89).
- **BREAKING:** `==` and `!=` compare arrays, JSON objects and dates by value: `[1,2] == [1,2]` is
  TRUE (was FALSE), so `filter(:x != [...])` changes result without an error. `FALSE < TRUE` is
  TRUE (was a wrong FALSE) and dates order with `<`/`>`. `==` stays strict and case-sensitive
  (ADR-0023, BL-059, #88).
- **BREAKING:** JS hooks cannot use a builtin name or alias (any case): such a hook is skipped
  with a warning on load, and `/upload-js` and `/update-js` answer 400. Rust `CustomFunction`s
  may still shadow builtins (ADR-0024, #86).
- **BREAKING:** existing builtins follow Excel (ADR-0018, #83): criteria in `COUNTIF`/`SUMIF`;
  `TIME`, `IPMT`, `COUNT`, `MOD`, `TRIM`, `ROUND`, `CEILING`/`FLOOR`, `SUBSTITUTE` with
  `instance_num`, `SORT(arr, -1)`, and `DATE` rollover.
- **BREAKING:** errors instead of 0, NaN or infinity: `x/0`, `0^0`, `SQRT(-1)`, `MOD(n, 0)`, empty
  `AVERAGE`/`MEDIAN`/…, wrong argument types, NaN or out-of-range percentiles, and any non-finite
  operator result (ADR-0018 D-1, ADR-0015, #83).
- **BREAKING:** Excel precedence: negation, then `%`, then `^` (left-associative), then `* /`,
  `+ -`, `&` and comparisons; `-2^2` is 4 (was -4), `2^3^2` is 64 (was 512) (ADR-0018 D-10, #82).
- **BREAKING:** `NULL == NULL` is TRUE and `NULL != NULL` FALSE (ADR-0013, #82).
- **BREAKING:** custom and JS functions are visible in builtin lambdas, method arguments and
  assignment formulas, so they can shadow a builtin there too (ADR-0016, #82).
- **BREAKING:** `sk_http_server` requires `Content-Type: application/json` on request bodies (415
  otherwise; multipart still on `/upload-js` and `/update-js`), and sends CORS headers only when
  an origin is configured or a token is set (ADR-0019, #78).
- **BREAKING:** `JQ` result shape follows the path: a singular path returns the value, or `NULL`
  when it misses (was `[]`); any other path returns an array, even with one match (ADR-0014, #76).
- **BREAKING:** the Docker image refuses to start (exit 64) unless `SKILLET_AUTH_TOKEN` and
  `SKILLET_ADMIN_TOKEN` are both set; Compose publishes on `127.0.0.1` by default (#74).
- **BREAKING:** `/upload-js`, `/update-js` and `/delete-js` accept only file names matching
  `[A-Za-z0-9_-]+.js` (400 otherwise); hooks in subdirectories still load but cannot be updated
  or deleted through the API (#63).
- **BREAKING:** servers (#62): writing one response has a 10 s limit in total (a slower reader
  gets a truncated response); `sk_server` closes the connection after a wrong or missing token;
  both servers refuse to start with an empty token (exit code 1).
- **BREAKING:** evaluation budget (ADR-0011, #46): formulas that copy a large variable once per
  item (`:rows.map(SUM(:rows))`) can fail with `Evaluation budget exceeded (max 256 MB)`; the
  servers stop an evaluation at a deadline (20 s by default since #106).
- **BREAKING** (`skillet::runtime::method_calls`, public but not documented API): the
  `exec_filter`/`exec_map`/`exec_find`/`exec_reduce`, `exec_string_method` and
  `exec_array_method` handlers take `&dyn EvaluationContext` (#45).
- Date to text is 3.5–11x faster (written directly instead of through chrono) (BL-091, #129).
- `ROUND` is about 3x faster on large arrays with bit-identical results, and `AND()`/`OR()`/`XOR()`
  no longer allocate per call (11–14% faster in row filters); `.round()` is 6–7% slower because it
  now rounds like `ROUND` (BL-088, #127).
- `.join()` writes `NULL` as empty text, the same as `JOIN` (was an error) (BL-078, #112).
- The crate is edition 2024 (ADR-0032); MSRV unchanged at 1.85 (BL-014, #110).
- Servers: the evaluation timeout default is 20 s (was a fixed 10 s in development builds), the
  budget stays 256 MiB but is configurable, and the response-size cap follows it. The Docker
  image runs 4 threads (was 2) (ADR-0011 amendment 4, BL-075, #106).
- Evaluation budget: strings and JSON cost `48 + len` bytes (was `32 + len`), the real size of a
  short string, so `with_limits` boundaries for string results move by 16 bytes per string
  (ADR-0011 amendment 3, BL-035, #105).
- ADR-0030 (`Value::Object` and `Arc`-shared values) is accepted for 0.8.0 (BL-011, BL-036, #104).
- `sk_server` daemonizes through the shared `daemon.rs`, with the same behaviour (BL-064, #99).
- `docs/reference/excel-compat.md` re-run on develop, and the threat model re-reviewed (BL-071, #94).
- The Excel precedence parser makes short expressions 10–18% slower to parse; accepted, see
  `benches/BASELINE.md` (#82).
- Both servers keep a per-worker cache of parsed expressions (256 entries, ADR-0021); on a 1.8 KB
  expression the HTTP server handles 41% more requests per second on one connection (#80).
- No CoreFoundation: `sk "=1+1"` starts about 1.3 ms faster on macOS and `sk_http_server` is 3 MB
  smaller; `GET /docs` is static HTML (ADR-0020, #81).
- JavaScript errors carry the real exception: `JS function NAME failed: …` (#79).
- One `vX.Y.Z` tag drives the release: binaries, GitHub release (notes from this file) and
  crates.io (ADR-0022). `release-binaries.yml` is renamed to `release.yml`, and `cross` and the
  musl images are pinned (#75).
- Builtin functions are dispatched from one table; lookups take 14–28 ns instead of 20–75 ns
  (#58). Evaluation-budget charges are cheaper, recovering part of the slowdown from #46 (#64).
- Release workflows use Rust 1.89.0 and build with `--locked` (#53).
- `sk_http_server` blocks in `accept()` instead of polling every 1 ms, which removes about 1.1 ms
  from each request (#54).
- The minimum supported Rust version is 1.85 (`rust-version` in `Cargo.toml`, ADR-0012) (#50).
- Documentation moved into `docs/guide/`, `docs/reference/` and `docs/ops/`; the published crate
  includes the licence files (#49).
- The parser uses about 4x less stack per nesting level, and parsing is 4–35% faster (#48).
- Server results (#46): the HTTP result cache stores entries up to 64 KiB and never errors, and
  `sk_server` streams nested arrays as JSON (wire format unchanged).
- Lambdas borrow the caller's variables instead of copying them for each item:
  `:nums.map(:x.abs()).sum()` on 100k numbers drops from about 15 s to about 5 ms (#45).
- Server worker threads get a 16 MB stack instead of the default 2 MB (#44).
- One evaluator: method and lambda calls use the same evaluator as the top level, so `JQ`, `DIG`,
  `REDUCE` with the default `:acc` and similar cases behave inside lambdas as at the top level
  (`[1,2].map(DIG(...))` no longer fails with `Unknown function: DIG`) (#41).

### Deprecated
- `Value::Json` (`#[deprecated(since = "0.7.0")]`): JSON objects become the structured
  `Value::Object` in 0.8.0 (ADR-0030, BL-011) and the variant is removed then. Behaviour is
  unchanged; build objects with `Value::from_json_text` (BL-092, #131; BL-104, #132).
- `Value::Currency` (`#[deprecated(since = "0.7.0")]`), removed in 0.8; use `Value::Number`. A
  Currency passed in from Rust is read as a Number, and results never contain one
  (ADR-0033, #119).
- `runtime::builtin_functions::exec_builtin`; use `runtime::function_dispatch::exec_builtin_fast`
  or `evaluate` (BL-057, #99).
- `SUBSTITUTEM`, in favour of `SUBSTITUTE`; it still works (#96).

### Removed
- Direct dependencies `lazy_static` and `once_cell`; statics use `std::sync::LazyLock`
  (ADR-0032, #110).
- `scalar-doc` and `tera` (about 30 crates) and `iana-time-zone` (ADR-0020, #81).
- **BREAKING:** the undocumented `httpGet` global for JavaScript hooks, which ran `curl`
  (ADR-0017, #79).
- `crate-release.yml` and the `crate-v*` release trigger (ADR-0022, #75).
- Unused dependencies `dashmap`, `string-interner`, `typed-arena` and `beef` (#43).
- **BREAKING** (ADR-0010): the unused public modules `skillet::memory_pool`, `skillet::traits`,
  `skillet::concurrent_registry` and the `skillet::runtime::evaluation` re-export shim; use
  `skillet::runtime::evaluator` or the top-level `evaluate*` functions (#41, #43).

### Fixed
- `sk_http_server` result cache: results that read the clock are never cached, so they no longer
  repeat an old value, and a cached result has the shape `include_variables` asks for
  (BL-085, #122).
- A lambda criterion that reads `:x` is evaluated per item even when the caller defines `x`
  (BL-081, #116).
- `DATEADD` with a huge amount gives `#NUM!`; before, it panicked or looped (#103).
- `CONCAT` and `&` no longer drop a `::Currency` result or a DateTime. `DB` matches Microsoft's
  7-period example and gives `#NUM!` for bad arguments instead of NaN or 0. `UNIQUE` keeps every
  type and `REVERSE` reverses strings (B-7, B-8, B-11, #96).
- `/health` evictions count real LRU evictions, and the HTTP request buffer goes back to the pool
  (BL-057, #99).
- Two fuzz findings that panicked in debug builds: an input starting with `==` in the lexer, and
  `[1][-1e300]` in index clamping (F-1, F-2, #98).
- The Docker image could never build: `.dockerignore` excluded `benches/`, which Cargo needs to
  read `Cargo.toml`. Same fix for the Alpine builder (BL-062, #92; BL-065, #95).
- `scripts/daemon_manager.sh` drives the `skillet-http` Compose service; it used `skillet-server`,
  which does not exist (BL-063, #91).
- `=1e3` evaluated to 1; `1e308*10` now gives the `#NUM!` error (BL-058, #89).
- A lexer error on the first token is reported instead of `Unexpected token: Eof` (#89).
- `DateTime == DateTime` compares timestamps (it was always FALSE) (BL-059, #88).
- `TIME(h, m, s)` used its arguments only from #61 on (it returned the current time); since #83 the
  result is a fraction of a day (see Changed). `TIME()` still returns the current local time.
- NaN in sorted data no longer panics: `MEDIAN`, `PERCENTILE_INC`, `QUARTILE_INC`, `SORT` and
  `.sort()` return `Cannot sort NaN values` (#59).
- The "Range operations" case in `sk_http_bench` failed every request (#57).
- `sk_http_server` serves its built-in documentation page on `GET /` from any working directory
  (#54).
- `sk_http_server` reported a request read timeout as "500 Internal Server Error"; it now
  reports "408 Request Timeout" (#51).
- Accepted sockets were inherited as non-blocking on macOS: `sk_server` dropped requests that
  arrived in pieces and `sk_http_server` cut large responses short (#46). `sk_http_server` builds
  each response once, at its exact size (#46).
- Deeply nested input (`((((…))))`, long `+` or `.a.b…` chains) no longer crashes the process
  with a stack overflow (#44). JavaScript arguments and results nested deeper than 128, or
  cyclic, no longer abort the process (#79).

### Security
- JSON request variables are charged to the evaluation budget, so one request can no longer use far
  more memory than `--max-eval-bytes` allows (BL-086, #124).
- Two different requests can no longer share an HTTP result-cache entry (BL-085, #122).
- `COUNTIF`/`SUMIF`/`AVERAGEIF` wildcard criteria match in linear time and honour the evaluation
  deadline (BL-080, #116).
- Request variables can no longer replace the reserved `:arguments` or silently overwrite each
  other through key renaming (BL-077, #111).
- `SORT`, `UNIQUE`, `JOIN`, `TEXTJOIN` and `.sort`/`.unique`/`.join` check the deadline inside
  their loops, and `UPPER`/`LOWER` and the methods check that case-mapped text fits the budget
  before building it (BL-035, #105).
- Every GitHub Action in `rust.yml` and `release.yml` is pinned by commit SHA (BL-065, #95, #101).
- `.env` and `.env.*` are excluded from the Docker build contexts (#92).
- JS hooks cannot replace a builtin, so a stray hooks folder can only add functions (ADR-0024,
  #86).
- Each JavaScript call runs with a 64 MiB heap, a 256 KiB stack and a time limit (the request
  deadline, or 10 s per call); the upload `@example` check runs under the same limits
  (ADR-0017, #79).
- Web pages can no longer drive a `sk_http_server` that runs without a token: no CORS headers,
  and admin requests with an `Origin` header get 403 when no admin token is set; an origin with
  whitespace or control characters is refused (ADR-0019, #78).
- The Docker image fails closed without tokens, never prints them, and runs a root-owned binary
  with only `/app/hooks` writable (#74).
- Hook files stay inside the hooks directory, a slow custom function no longer blocks
  `/reload-hooks`, and the PID file is not written through a symlink (#63). Empty tokens no
  longer turn auth off, and slow readers cannot hold a worker past the write limit (#62).
- `lru` 0.19 fixes RUSTSEC-2026-0253 (#57).
- `sk_server` limits each request line to 1 MB, both servers limit reading one request to 30 s in
  total, and auth tokens are compared in constant time (#51).
- `crossbeam-epoch` updated to 0.9.21 for RUSTSEC-2026-0204 (#50).
- One evaluation is bounded by the budget and, on the servers, the deadline (#46); nesting depth
  can no longer overflow the stack (#44). The release workflow has read-only `contents`.
- The threat model is published (`docs/security/THREAT_MODEL.md`, #73).

### Testing
- `tests/golden/evaluator_parity.txt` pins the result of every expression in the parity corpus
  (1190 lines; rewrite with `UPDATE_GOLDEN=1`). The full gate also runs
  `cargo test --no-default-features`. A weekly `slow-tests.yml` workflow runs the `#[ignore]`
  deadline tests. Test servers start with `SKILLET_*` variables cleared, tight wall-clock bounds
  became hang guards, and `--fast` skips the tests that wait out fixed limits (BL-087, #125).
- Server integration tests start each server on a port it owns and wait for its own start-up
  line (0 of 360 suite runs failed under load, against 11 of 130 before) (BL-061, #100). A test
  fails on `unsafe` outside `daemon.rs` and `local_time.rs` (BL-064, #99).
- A weekly `fuzz.yml` workflow fuzzes `parse`, `evaluate` and `json_vars` with cargo-fuzz and ASan
  (`fuzz/`, ADR-0028, BL-019, #98).
- CI builds and smoke-tests the Docker image (`scripts/docker_smoke.sh`); not a required check yet
  (BL-062, #92).
- CI runs the full `./harness/check.sh` gate (both feature sets, strict clippy and fmt) and
  `cargo audit` on every push and pull request to `main` and `develop` (BL-004, BL-019).
- Two timing-sensitive tests no longer fail on fast or loaded machines; the accept-floor test
  runs only with `--ignored` (#65, #66).

## [0.6.3] - 2026-03-25

### Removed
- Windows release binaries (the daemon code depends on `libc`).

### Fixed
- Release build task.

## [0.6.2] - 2026-03-25

Includes the untagged 0.6.1.

### Added
- Comments and free whitespace in expressions.
- GitHub workflow that builds and publishes release binaries for macOS (x86_64, arm64) and Linux
  musl (x86_64, arm64).
- Docker image for GCP deployment, with a threads parameter and environment variable (default 2
  threads).
- Formula-builder agent skill and `docs/DEVELOPMENT_GUIDE.md`.

## [0.6.0] - 2026-03-12

### Added
- `MERGE(args)` and `array.merge(args)` to merge arrays or append elements.
- Linux Alpine (musl) builds via Docker.

### Changed
- Faster runtime (about 3x in the project's own benchmarks).
- `lru` 0.12 → 0.16.

## [0.5.3] - 2025-10-06

### Added
- JSONPath queries (`JQ`).

## [0.5.2] - 2025-09-23

### Added
- `DIG`, `REPLACE`, `SUBSTITUTE`.
- `SUMIF`, `PRODUCT`, `MULTIPLY`.
- `openapi.yml` shipped in the crate.

### Changed
- HTTP server improvements; evaluation module refactor.

## [0.4.1] - 2025-09-06

### Added
- Null-safe navigation operator `&.`.
- `INCLUDES(:string, :substring)` and `.includes(:substring)`.

### Changed
- Speed improvements; Dockerfile for any host.

## [0.3.0] - 2025-09-03

### Added
- `sk_http_server` (HTTP server), with daemon mode, bind address, auth token and admin token.
- HTTP management of JavaScript functions.
- `LEFT`, `MID`, `RIGHT`; `.find()`/`FIND()`; `BETWEEN()`/`.between()`.
- `to_i`, `to_boolean`, `to_s` methods; JSON key sanitization.
- `include_variables` request parameter; request field `variables` renamed to `arguments`.
- Variable assignment with objects.

### Fixed
- Datetime functions, JSON parsing, client parameter parsing.

## [0.2.0] - 2025-08-31

### Changed
- Faster server responses; clearer docs and README.

## [0.1.2] - 2025-08-31

### Added
- `sk_server` (TCP evaluation server).

### Fixed
- Ctrl+C handling in `sk_server`.

## [0.1.1] - 2025-08-31

### Fixed
- Version number in `Cargo.toml`.

## [0.1.0] - 2025-08-31

### Changed
- Builtins split by category (arithmetic, financial, logical, …); lexer improvements.

### Fixed
- Method chains such as `[].length().zero?`.

## [0.0.2] - 2025-08-23

### Added
- Custom Rust functions and JavaScript plugins (hooks directory).

### Removed
- SQLite support.

### Fixed
- JSON variables in method chains such as `=:user.name.upper()`.

## [0.0.1] - 2025-08-19

- First release (tag `v0.1.0-alpha`).

[Unreleased]: https://github.com/zenbakiak/skillet/compare/v0.7.0...develop
[0.7.0]: https://github.com/zenbakiak/skillet/compare/crate-v0.6.3...v0.7.0
[0.6.3]: https://github.com/zenbakiak/skillet/compare/crate-v0.6.2...crate-v0.6.3
[0.6.2]: https://github.com/zenbakiak/skillet/compare/crate-v0.6.0...crate-v0.6.2
[0.6.0]: https://github.com/zenbakiak/skillet/compare/crate-v0.5.3...crate-v0.6.0
[0.5.3]: https://github.com/zenbakiak/skillet/compare/crate-v0.5.2...crate-v0.5.3
[0.5.2]: https://github.com/zenbakiak/skillet/compare/crate-v0.4.1...crate-v0.5.2
[0.4.1]: https://github.com/zenbakiak/skillet/compare/crate-v0.3.0...crate-v0.4.1
[0.3.0]: https://github.com/zenbakiak/skillet/compare/crate-v0.2.0...crate-v0.3.0
[0.2.0]: https://github.com/zenbakiak/skillet/compare/crate-v0.1.2...crate-v0.2.0
[0.1.2]: https://github.com/zenbakiak/skillet/compare/crate-v0.1.1...crate-v0.1.2
[0.1.1]: https://github.com/zenbakiak/skillet/compare/crate-v0.1.0...crate-v0.1.1
[0.1.0]: https://github.com/zenbakiak/skillet/compare/crate-v0.0.2...crate-v0.1.0
[0.0.2]: https://github.com/zenbakiak/skillet/compare/v0.1.0-alpha...crate-v0.0.2
[0.0.1]: https://github.com/zenbakiak/skillet/releases/tag/v0.1.0-alpha
