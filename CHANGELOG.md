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

Planned as 0.7.0. It contains breaking changes, marked **BREAKING** below; the
[migration guide](docs/guide/migration-0.7.md) lists each one with the old and the new result.

### Added
- `DATEDIF(start, end, unit)` with Excel's worksheet codes `y m d md ym yd`; `#NUM!` when start is
  after end or the unit is unknown (BL-074, #103).
- `NOT` prefix keyword, any case, with the same result as `NOT(x)` (ADR-0029, #102).
- `/health` reports `parse_cache` (hits, misses, hit rate, entries) over all workers, and
  `DELETE /cache` now clears every worker's parse cache too (BL-067, #99).
- Fuzzing: a `fuzz/` crate with cargo-fuzz targets `parse`, `evaluate` and `json_vars`, and a
  weekly `fuzz.yml` workflow; the root crate and the gate are unchanged (ADR-0028, BL-019, #98).
- CI builds and smoke-tests the Docker image (`scripts/docker_smoke.sh`): exit 64 without tokens,
  healthy with both, 401 without a token, no token in the log. Not a required check yet
  (BL-062, #92).
- The Alpine builder builds x86_64 as well as aarch64, `--locked`, on a pinned
  `rust:1.89.0-bookworm` image (BL-065, #95).
- `sk_http_server` and `sk_server`: optional limit on concurrent connections per IP address,
  `--max-connections-per-ip N` or `SKILLET_MAX_CONNECTIONS_PER_IP` (flag wins; no limit when unset).
  Connections over the limit get HTTP 429 or a JSON error line (ADR-0025, #87).
- Scientific notation number literals: `1e3`, `2.5e-4`, `1E+2` (BL-058, #89).
- `benches/baseline.rs` and `benches/BASELINE.md`: a reproducible performance baseline for the
  library, CLI and HTTP layers (#48).
- `docs/guide/limits.md`: one place for every limit (nesting depth, evaluation budget, server
  deadlines, request and response sizes) (#49).
- `skillet::MAX_NESTING_DEPTH` (128): expressions and runtime-built arrays deeper than this fail
  with `Expression nested too deeply (max 128)` instead of overflowing the stack (#44).
- Per-evaluation memory budget (ADR-0011): `skillet::MAX_EVAL_BYTES` (256 MB of created values),
  `skillet::EvalLimits { max_bytes, timeout }` and `skillet::with_limits(limits, f)`. Nested
  `with_limits` calls can only tighten limits (#46).
- The binaries build without the `plugins` feature (`cargo build --no-default-features`): the CLI
  and both servers run without JavaScript support, and the HTTP JS management endpoints answer
  404 `JavaScript plugins are not enabled in this build` after the admin-token check (BL-025).
- CI runs the full `./harness/check.sh` gate (both feature sets, strict clippy and fmt) and
  `cargo audit` on every push and pull request to `main` and `develop` (BL-004, BL-019).
- Release archives come with a `.tar.gz.sha256` checksum file, verified before upload (#53).
- `docs/reference/functions.md` is generated from the builtin table, and a test fails when it is
  stale (#58). `docs/reference/excel-compat.md` lists how each builtin compares with Excel (#56).
  Rustdoc examples for the public API (#60).
- `evaluate_parsed`, `evaluate_parsed_with`, `evaluate_parsed_with_custom`,
  `evaluate_parsed_with_assignments` and `evaluate_parsed_with_assignments_and_context` evaluate
  the `Expr` returned by `parse` (ADR-0021). The `evaluate*` functions call them, so results,
  errors and limits are the same (#80).
- New Excel builtins (ADR-0018): `LEN`, `TEXTJOIN`, `TODAY`, `PPMT`, `PV`, `NPV`, `COUNTA`,
  `AVERAGEIF`, the sample statistics `STDEV.S`/`STDEV` and `VAR.S`/`VAR`, and the names `MODE`,
  `PERCENTILE` and `QUARTILE` (#82, #83, #84).
- Dotted function names parse, so `STDEV.P(…)`, `PERCENTILE.INC(…)` and the like work (#82).
- `&` joins text, and a `%` suffix divides by 100 (`15%` is 0.15); `%` followed by an operand is
  still modulo (#82).
- `FIND(text, within, [start])` is Excel's 1-based, case-sensitive text search;
  `FIND(array, lambda)` is unchanged (#83).
- Date arithmetic in days: `date ± n` and `n + date` give a DateTime, and `date - date` gives the
  number of days (#83).
- `sk_http_server --cors-origin` / `SKILLET_CORS_ORIGIN` sets the CORS origin, and
  `SKILLET_ADMIN_TOKEN` sets the admin token (the flag wins) (ADR-0019, #78).
- `docs/security/THREAT_MODEL.md`: the threat model for the library, servers and JS plugins (#73).
  `docs/guide/migration-0.7.md`: every 0.7.0 breaking change (#77).
- `sk_http_bench --expr` selects the expression for the sweep (#80).

### Changed
- **BREAKING:** parsing requires the end of the input. Tokens after the expression are an error,
  `Unexpected token after end of expression`, at the first extra token (`=1 2`,
  `=42 as string`); before, they were dropped silently. One trailing `;` is still allowed. Error
  positions count from the start of the caller's input, including a leading `=` (BL-069, #93).
- **BREAKING:** the `AND` and `OR` operators work. They were never lexed, so everything from
  `AND` on was dropped: `TRUE AND FALSE` was TRUE and is now FALSE. `AND(...)`/`OR(...)` before an
  operand are still the builtins (BL-069, #93). `^` followed by unary minus already matched Excel
  (`2^-2` is 0.25, `0^-1` is `#DIV/0!`); tests now pin it (BL-060, #93).
- **BREAKING:** Excel logic (ADR-0029, ADR-0018 D-8, #102; see
  [logical functions](docs/guide/migration-0.7.md#logical-functions-and-operators-adr-0018-d-8-adr-0029)):
  - The operators `AND`, `OR`, `&&` and `||` short-circuit (`FALSE AND 1/0` is FALSE). The
    builtins `AND()`/`OR()` still evaluate every argument.
  - The `AND`/`OR`/`NOT`/`XOR` builtins skip text and `NULL` inside arrays and a `NULL` argument;
    text passed directly, or no logical value at all, is `#VALUE!`. `AND(TRUE, NULL)` is now TRUE
    (was FALSE) and `AND()` is `#VALUE!` (was TRUE). `XOR` takes any number of arguments.
- **BREAKING:** `DATEADD` and `DATEDIFF` read VBA's unit codes, case-insensitively
  (`yyyy q m y d w ww h n s`). Three codes change silently: `"m"` is months (was minutes), `"y"`
  is days (was years) and `DATEADD`'s `"w"` adds a day (was a week). The long words are unchanged.
  `DATEADD` clamps months to the end of the month, as VBA does (BL-074, B-15, #103; see
  [date units](docs/guide/migration-0.7.md#date-units-follow-excel-b-15)).
- **BREAKING:** date and currency arithmetic like Excel (ADR-0027, amends ADR-0023, BL-070, #97;
  see [dates](docs/guide/migration-0.7.md#dates-adr-0018-d-5) and
  [currency](docs/guide/migration-0.7.md#currency-in-arithmetic-adr-0027)):
  - In `DateTime ± x` and `x + DateTime`, `x` converts with the ADR-0023 rule:
    `DATE(2026,1,1) + NULL` is the date, `+ "1"` or `+ TRUE` the next day, other text `#VALUE!`.
  - `+ - * /` and unary minus keep Currency; `Currency / Currency`, `^` and `%` give a Number.
    Results are not rounded. Some error messages change to `#VALUE!`.
  - Methods are unchanged: `[1,"2"].sum()` still errors.
- **BREAKING:** text builtins follow Excel (ADR-0018, #96): `LEFT`, `RIGHT`, `MID` and `REPLACE`
  give `#VALUE!` for a negative length or a start below 1 instead of clamping (`LEFT(x,-1)`,
  `MID(x,0,n)`). `SORT` sorts text case-insensitively, orders mixed values numbers < text <
  booleans, and is stable; sorting text used to be an error.
- **BREAKING** (wire format): `sk_http_server` answers an oversized request with HTTP 413 and a
  slow one with 408 (both were 400). It reads the rest of the request first (bounded), so the
  client gets the error instead of a reset (BL-049, #99; see
  [servers](docs/guide/migration-0.7.md#servers)).
- Evaluation budget: strings and JSON cost `48 + len` bytes (was `32 + len`), which matches what a
  1-char string really takes. `with_limits` boundaries for string results move by 16 bytes per
  string (ADR-0011 amendment 3, BL-035, #105).
- `sk_server` daemonizes through the shared `daemon.rs`, with the same behaviour; a test fails on
  `unsafe` outside `daemon.rs` and `local_time.rs` (BL-064, #99).
- Server integration tests start each server on a port it owns and wait for its own start-up
  line; under heavy load 0 of 360 suite runs failed, against 11 of 130 before (BL-061, #100).
- `docs/reference/excel-compat.md` re-run on develop and recounted, and the threat model
  re-reviewed (BL-071, #94). ADR-0030 (`Value::Object` and `Arc`-shared values) is accepted for
  0.8.0; no code yet (BL-011, BL-036, #104).
- **BREAKING:** like Excel, the arithmetic operators and scalar math builtins convert numeric
  text and `TRUE`/`FALSE` to numbers (`ABS("-3")` = 3, `"3"+1` = 4, `TRUE+1` = 2), and the operators
  read `NULL` as 0. Other text is `Cannot convert "abc" to a number (#VALUE!)` (ADR-0023, #89).
- **BREAKING:** `%` before `+` or `-` is the percent suffix: `10%-3` and `10 % -3` are -2.9. `%` is
  modulo only when an operand follows it directly (`10 % 3` = 1) (ADR-0023, #89).
- **BREAKING:** a number run straight into a letter (`1x`, `1_000`) is an `Invalid number literal`
  error instead of evaluating to the leading number (#89).
- `==` compares arrays, JSON objects and Currency by value (`[1,2] == [1,2]` is true); booleans and
  Currency can be ordered (`FALSE < TRUE`). `==` stays strict and case-sensitive; comparisons never
  convert between types (ADR-0023, #88).
- **BREAKING:** JS hooks can no longer override builtins. A hook whose `@name` is a builtin name or
  alias (any case) is skipped with a warning on load; `/upload-js` and `/update-js` answer 400.
  Rust `CustomFunction`s may still shadow builtins (ADR-0024, #86).
- The parser uses about 4x less stack per nesting level, so debug builds handle the maximum
  depth on a default 2 MB thread; parsing is 4-35% faster (#48).
- Documentation moved into `docs/guide/`, `docs/reference/` and `docs/ops/`; the published crate
  now includes the licence files (#49).
- `evaluate_with_json` and `evaluate_with_json_custom` share one internal helper; no behaviour
  change (#47).
- The minimum supported Rust version is 1.85 (`rust-version` in `Cargo.toml`, ADR-0012) (#50).
- One evaluator: method and lambda calls now use the same evaluator as the top level. Inside
  lambdas, `JQ`, `DIG`, `REDUCE` with the default `:acc`, custom parameter names and similar
  cases now behave as they do at the top level; for example, `[1,2].map(DIG(...))` no longer fails
  with `Unknown function: DIG` (#41).
- Lambdas no longer copy the whole variable map for each item; they borrow the caller's
  variables. `:nums.map(:x.abs()).sum()` on 100k numbers drops from about 15 s to about 5 ms (#45).
- **BREAKING** (`skillet::runtime::method_calls`, publicly visible but not part of the documented
  API): the `exec_filter`/`exec_map`/`exec_find`/`exec_reduce`, `exec_string_method` and
  `exec_array_method` handlers take `&dyn EvaluationContext`. `exec_method`,
  `exec_method_with_custom` and the `EvaluationContext` trait are unchanged (#45).
- Evaluation budget behaviour (#46):
  - Reads that copy a value are charged against the budget; borrowing reads such as
    `:rows.length()` are free. Formulas that copy a large variable once per item can now fail
    with `Evaluation budget exceeded (max 256 MB)`, for example `:rows.map(SUM(:rows))` on large
    data. Compute the value once instead: `:t := SUM(:rows); :rows.map(:t)`.
  - Quadratic builds hit the budget: `reduce(MERGE(:acc, [:x]), [])` fails above about 2,800 items.
- Servers (#46):
  - `sk_server` and `sk_http_server` run each evaluation with a 10 s deadline and return
    `Evaluation timed out (max 10 s)` when it passes.
  - Results that would exceed the budget once serialized are rejected before the response is built.
  - The HTTP result cache only stores entries up to 64 KiB and never stores errors.
  - `sk_server` streams nested arrays as JSON instead of building the full text first (wire
    format unchanged).
- Server worker threads get a 16 MB stack instead of the default 2 MB (#44).
- Servers (#62):
  - Writing one response has a 10 s limit in total, instead of 10 s per socket write. A client
    that reads more slowly gets a truncated response (an HTTP body shorter than `Content-Length`,
    or an `sk_server` line without its trailing newline). Reading the largest results in time
    needs about 25 MB/s; see `docs/guide/limits.md`.
  - `sk_server`: blank lines and invalid JSON no longer restart the 30 s connection deadline;
    only a real, authorized request does. After a wrong or missing token the server answers
    `Unauthorized: invalid token` and closes the connection.
  - Both servers refuse to start with an empty token and exit with code 1: `--token ""` on
    both, `--admin-token ""` on `sk_http_server`, an empty `SKILLET_AUTH_TOKEN` on `sk_server`.
- `/upload-js`, `/update-js` and `/delete-js` accept only plain file names matching
  `[A-Za-z0-9_-]+.js` and answer 400 otherwise. Hooks in subdirectories still load and are
  listed by `/list-js`, but can no longer be updated or deleted through the API (#63).
- `sk_http_server` blocks in `accept()` instead of polling every 1 ms, which removes about 1.1 ms
  from each request (#54).
- Builtin functions are dispatched from one table; lookups take 14–28 ns instead of 20–75 ns
  (#58). Evaluation-budget charges are cheaper, recovering part of the slowdown from #46 (#64).
- Release workflows use Rust 1.89.0 and build with `--locked` (#53). `lru` updated to 0.19 (#57).
- Two timing-sensitive tests no longer fail on fast or loaded machines; the accept-floor test
  runs only with `--ignored` (#65, #66).
- The crate is formatted with `rustfmt` and clippy-clean (#42).
- **BREAKING** Excel precedence (ADR-0018 D-10): negation, then `%`, then `^` (left-associative),
  then `* /`, `+ -`, `&` and comparisons. `-2^2` is 4 (was -4) and `2^3^2` is 64 (was 512) (#82).
- **BREAKING** `NULL == NULL` is `true` and `NULL != NULL` is `false` (ADR-0013); ordering with
  `NULL` is still an error (#82).
- **BREAKING** Custom and JS functions are visible in builtin lambdas, method arguments and
  assignment formulas, so a custom function can shadow a builtin there too (ADR-0016) (#82).
- `IF` and `IFS` evaluate only the chosen branch (#82). **BREAKING**: the condition must be
  TRUE/FALSE, a number or `NULL` (false); anything else (`IF("x", 1, 2)`) is `#VALUE!`, and `IFS`
  with no true condition is `#N/A` (ADR-0018 D-8, B-18, #102). An earlier version of this entry
  credited #84, which did not implement it.
- **BREAKING** `JQ` result shape follows the path (ADR-0014): a singular path (`.name`, `['name']`,
  `[n]`) returns the value, or `NULL` when it misses (was `[]`); any other path returns an array,
  even with one match. `JQ({"a":[3]}, "$.a[*]")` is `[3]` (was `3`) (#76).
- **BREAKING** Existing builtins follow Excel (ADR-0018) (#84):
  - `COUNTIF`, `SUMIF` and `AVERAGEIF` share Excel's criteria matcher: operators (`">2"`),
    case-insensitive text, `*`/`?`/`~` wildcards, blanks, and a scalar range.
  - `TIME(h, m, s)` returns a fraction of a day, with Excel's truncation and wrap
    (`TIME(14, 30, 0)` is 0.6041666666666666).
  - `IPMT` uses Excel's sign; `COUNT` is variadic and counts numbers only.
  - `MOD` takes the sign of the divisor, `TRIM` collapses inner spaces, `ROUND` accepts negative
    digits, `CEILING`/`FLOOR` round to a significance, `SUBSTITUTE` honours `instance_num`, and
    `SORT(arr, -1)` sorts descending.
  - Methods (`.trim()`, `.round()`, `.int()`, …) keep Ruby semantics.
- **BREAKING** Errors instead of 0, NaN or infinity (ADR-0018 D-1, ADR-0015): `x/0`, `0^0`,
  `SQRT(-1)`, `MOD(n, 0)`, `POWER(0, 0)`, empty `AVERAGE`/`MEDIAN`/…, arguments of the wrong type,
  NaN or out-of-range percentiles, and any non-finite operator result (#84).
- **BREAKING** `DATE` rolls months and days over like Excel (`DATE(2026, 13, 1)` is 2027-01-01),
  adds 1900 to years 0–1899, and fails outside 1900–9999 (#83).
- **BREAKING** `sk_http_server` requires `Content-Type: application/json` on POST, PUT and DELETE
  requests with a body and answers 415 otherwise; multipart is still accepted on `/upload-js` and
  `/update-js` (ADR-0019, #78).
- **BREAKING** `sk_http_server` sends `Access-Control-Allow-Origin` only when an origin is
  configured, or `*` when a token is set. Without a token (development mode) it sends no CORS
  headers (ADR-0019, #78).
- **BREAKING** The Docker image refuses to start (exit 64) unless `SKILLET_AUTH_TOKEN` and
  `SKILLET_ADMIN_TOKEN` are both set; the old `AUTH_TOKEN`/`ADMIN_TOKEN` names get a rename hint.
  It builds on `rust:1.89.0-slim-bookworm` with `--locked`, runs on `debian:bookworm-slim` without
  `curl`, and Compose publishes on `127.0.0.1` by default (`SKILLET_PUBLISH_ADDR`) (#74).
- JavaScript errors carry the real exception: `JS function NAME failed: …` (#79).
- Both servers keep a per-worker cache of parsed expressions (256 entries, ADR-0021). On a 1.8 KB
  expression the HTTP server handles 41% more requests per second on one connection (#80).
- `DATE()` and `TIME()` read the local offset from libc on unix instead of CoreFoundation, and
  chrono is built without default features; results are unchanged (ADR-0020). `sk "=1+1"` starts
  about 1.3 ms faster on macOS and `sk_http_server` is 3 MB smaller (#81).
- `GET /docs` serves the Scalar API page as static HTML (#81).
- One `vX.Y.Z` tag drives the release: binaries, GitHub release (notes from this file) and
  crates.io (ADR-0022). `release-binaries.yml` is renamed to `release.yml`, and `cross` and the
  musl images are pinned (#75).
- Public enum `lexer::Token` gains `Ampersand`, and `runtime::evaluator` gains
  `eval_with_assignments_and_custom` (#82).

### Deprecated
- `SUBSTITUTEM`, in favour of `SUBSTITUTE`; it still works (#96).
- `runtime::builtin_functions::exec_builtin`; use `runtime::function_dispatch::exec_builtin_fast`
  or `evaluate` (BL-057, #99; see [Rust API](docs/guide/migration-0.7.md#rust-api)).

### Removed
- **BREAKING** (ADR-0010): the unused public modules `skillet::memory_pool`, `skillet::traits`,
  `skillet::concurrent_registry` and the `skillet::runtime::evaluation` re-export shim. Use
  `skillet::runtime::evaluator` or the top-level `evaluate*` functions instead (#41, #43).
- Unused dependencies `dashmap`, `string-interner`, `typed-arena` and `beef` (#43).
- **BREAKING** The undocumented `httpGet` global for JavaScript hooks, which ran `curl`
  (ADR-0017, #79).
- `scalar-doc` and `tera` (about 30 crates) and `iana-time-zone` (ADR-0020, #81).
- `crate-release.yml` and the `crate-v*` release trigger (ADR-0022, #75).

### Fixed
- `CONCAT` and `&` no longer drop Currency or DateTime: Currency is written as its number and a
  DateTime as its `::String` text. `DB` matches Microsoft's 7-period example and gives `#NUM!` for
  bad arguments instead of NaN or 0. `UNIQUE` keeps every type and `REVERSE` reverses strings
  (B-7, B-8, B-11, #96).
- `DATEADD` with a huge amount gives `#NUM!`; before, it panicked or looped (#103).
- Two fuzz findings: an input starting with `==` overflowed in the lexer (release builds reported
  position `usize::MAX`, now 0), and `[1][-1e300]` overflowed in index clamping. Both panicked in
  debug builds only (F-1, F-2, #98).
- `/health` evictions count real LRU evictions, and the HTTP request buffer goes back to the pool
  (BL-057, #99).
- The Docker image could never build: `.dockerignore` excluded `benches/`, which Cargo needs to
  read `Cargo.toml`. Same fix for the Alpine builder (BL-062, #92; BL-065, #95).
- `scripts/daemon_manager.sh` drives the `skillet-http` Compose service; it used `skillet-server`,
  which does not exist (BL-063, #91).
- `=1e3` evaluated to 1; `1e308*10` now gives the `#NUM!` error (BL-058, #89).
- `DateTime == DateTime` compares timestamps, and `<`/`>` work between dates (BL-059, #88).
- A lexer error on the first token is reported instead of `Unexpected token: Eof` (#89).
- `sk_http_server` reported a request read timeout as "500 Internal Server Error"; it now
  reports "408 Request Timeout" (#51).
- Deeply nested input (`((((…))))`, `1+1+…`, long `.a.b…` chains, `.reduce([:acc], 0)`) no longer
  crashes the process with a stack overflow; on `sk_http_server` one request could take the
  server down (#44).
- Accepted sockets were inherited as non-blocking on macOS: `sk_server` dropped requests that
  arrived in pieces and `sk_http_server` cut large responses short (#46).
- `sk_http_server` builds each response once, at its exact size (#46).
- NaN in sorted data no longer panics: `MEDIAN`, `PERCENTILE_INC`, `QUARTILE_INC`, `SORT` and
  `.sort()` return `Cannot sort NaN values`. Before, the evaluator panicked (a server request got
  no response), and `.sort()` returned arrays of up to 20 items containing NaN unsorted (#59).
- `TIME(h, m, s)` returns `h*3600 + m*60 + s`; it used to ignore its arguments and return the
  current time. `TIME()` still returns the current local time, and one or two arguments are an
  error (#61). From #84 the result is a fraction of a day (see Changed).
- `sk_http_server` serves its built-in documentation page on `GET /` from any working directory
  (#54).
- The "Range operations" case in `sk_http_bench` failed every request (#57).
- JavaScript arguments and results nested deeper than 128, or cyclic, aborted the process; they
  now give `Expression nested too deeply (max 128)` (#79).

### Security
- `SORT`, `UNIQUE`, `JOIN`, `TEXTJOIN` and `.sort`/`.unique`/`.join` check the deadline inside
  their loops: with a 1 ms deadline and 3M items `.unique` stops after 3.8 ms instead of 1064 ms
  and returns the timeout error. `UPPER`/`LOWER` and the methods check that case-mapped text fits
  the budget before building it; peak memory under a 4 MiB budget fell from 24 MB to 3.4 MB
  (BL-035, #105).
- The parser and evaluator are fuzzed weekly in CI with ASan (ADR-0028, #98).
- `.env` and `.env.*` are excluded from the Docker build contexts (#92).
- Every GitHub Action in `rust.yml` and `release.yml` is pinned by commit SHA (BL-065, #95, #101).
- `sk_server` limits each request line to 1 MB and both servers limit reading one request to
  30 s in total (#51).
- Auth tokens are compared in constant time on both servers (#51).
- `crossbeam-epoch` updated to 0.9.21 for RUSTSEC-2026-0204 (#50).
- Stack-overflow DoS through nesting depth is closed (#44).
- CPU and allocation DoS through lambdas copying the scope is closed (#45).
- Memory and CPU exhaustion from a single evaluation is bounded by the evaluation budget and, on
  the servers, the 10 s deadline (#46).
- The crate release workflow runs with read-only `contents` permission.
- An empty token no longer turns authentication off while reporting it as enabled; the servers
  refuse to start (#62).
- A slow-reading client can no longer hold a server worker past the 10 s write limit (#62).
- `sk_server` allows one token attempt per connection (#62).
- Hook management endpoints only touch files directly in the hooks directory, and hook loading
  only reads files inside it (#63).
- Values returned by JavaScript hooks are limited to nesting depth 128 (#63).
- A slow custom function no longer blocks function registration or `/reload-hooks` (#63).
- `sk_http_server` writes its daemon PID file without following symlinks (#63).
- `lru` 0.19 fixes RUSTSEC-2026-0253 (#57).
- Each JavaScript call runs with a 64 MiB heap, a 256 KiB stack and a time limit (the request
  deadline, or 10 s per call); the upload `@example` check runs under the same limits
  (ADR-0017, #79).
- Web pages can no longer drive a `sk_http_server` that runs without a token: there are no CORS
  headers, and admin requests with an `Origin` header get 403 when no admin token is set. An
  origin with whitespace or control characters is refused (ADR-0019, #78).
- The Docker image fails closed without tokens, never prints them, and runs a root-owned binary
  with only `/app/hooks` writable (#74).
- The threat model is published (`docs/security/THREAT_MODEL.md`, #73).

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

[Unreleased]: https://github.com/zenbakiak/skillet/compare/crate-v0.6.3...develop
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
