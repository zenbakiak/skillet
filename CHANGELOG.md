# Changelog

All notable changes to the `skillet` crate and its binaries (`sk`, `sk_server`, `sk_client`,
`sk_http_server`, `sk_http_bench`) are recorded here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). Before 1.0, a minor release
(`0.x.0`) may contain breaking changes and a patch release (`0.x.y`) contains fixes only.
Breaking changes are marked **BREAKING**.

Releases are tagged `crate-vX.Y.Z` (crates.io) in this repository. Entries for 0.6.3 and earlier
were reconstructed from git tags and commit messages, so they are brief.

## [Unreleased]

Planned as 0.7.0 (it contains breaking changes, see "Removed").

### Added
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

### Changed
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

### Removed
- **BREAKING** (ADR-0010): the unused public modules `skillet::memory_pool`, `skillet::traits`,
  `skillet::concurrent_registry` and the `skillet::runtime::evaluation` re-export shim. Use
  `skillet::runtime::evaluator` or the top-level `evaluate*` functions instead (#41, #43).
- Unused dependencies `dashmap`, `string-interner`, `typed-arena` and `beef` (#43).

### Fixed
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
  error (#61).
- `sk_http_server` serves its built-in documentation page on `GET /` from any working directory
  (#54).
- The "Range operations" case in `sk_http_bench` failed every request (#57).

### Security
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
