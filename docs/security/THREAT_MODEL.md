# Skillet threat model

- Owner: Security Reviewer (`harness/roles/security-reviewer.md`)
- Last reviewed: 2026-11-21, against `develop` 824df14 (version 0.6.3 + unreleased 0.7.0 changes)
- Scope: the `skillet` library, the five binaries, the hooks directory, the Docker image and the
  release workflows. Update this file whenever a surface, limit or finding changes.

This page describes the system **as built today**: what an attacker controls on each surface,
what already holds (with the tests that prove it), and what does not hold yet (with backlog ids).
Gaps that are not fixed yet are described by what is missing, not by how to exploit them.

**Status 2026-11-21.** Every High and Medium finding of the 2026-11-20 review is fixed:

- QuickJS memory, stack and time limits, and bounded conversion of JS arguments and results
  (ADR-0017, #63, #79).
- The `httpGet` global is removed; hooks get no host functions (ADR-0017 §6, #79).
- Upload, update and delete accept only plain file names, and loading skips entries that resolve
  outside the hooks directory (#63).
- CORS only when configured or a token is set, `415` for request bodies that are not JSON, and
  `403` for browser requests to admin endpoints in development mode (ADR-0019, #78).
- `SKILLET_ADMIN_TOKEN` sets the admin token (#78).
- The Docker image fails closed without both tokens and no longer logs them (#74).
- JS hooks cannot shadow builtins (ADR-0024, #86).
- Optional per-IP connection limit (ADR-0025, #87).
- The registry lock is released before a custom function runs (#63).
- Also fixed: the PID file no longer follows symlinks (#63, #72); BL-046, BL-047, BL-048 (#62);
  BL-045 (#54); BL-043 (#57); BL-044 (#53); `scalar-doc` removed (BL-042, #81).

The risks still open are in §3. Run servers with both tokens set.

## 1. Assets and trust levels

| Who | Trust | Can do |
|---|---|---|
| Expression author / eval client | **Untrusted.** Holds the eval token if one is set. | Send any expression text and any JSON variables, many times, concurrently. |
| Admin | Trusted to run code **inside the sandbox** (ADR-0007) but not to own the host. Holds the admin token. | Upload, update, delete and reload JS hooks; clear the cache. |
| Hook author | Same as admin (writes files into the hooks directory). | Any JS the sandbox allows. |
| Operator | Fully trusted. | Starts the binaries, picks flags, tokens, hooks dir, PID file. |
| Network peer without a token | **Untrusted.** | Raw bytes on the socket; with no token configured, everything an eval client and an admin can do (browser requests to admin endpoints excepted, 2.3). |
| Web page in the operator's browser | **Untrusted.** | Cross-origin requests to `127.0.0.1`. |

Assets: availability of the server process and its workers; the host file system and network
reachable by the server user; the tokens; the integrity of the hooks directory; the integrity of
release artifacts.

Design invariants that matter for security (AGENTS.md §7): the language has no loops and no I/O;
limits return an ordinary `Error`, never abort the process.

## 2. Surfaces

### 2.1 Expression text (library, `sk`, both servers)
- **Attacker controls:** any string (HTTP: up to 1 MB per request).
- **Must hold:** no panic, no abort, bounded CPU and memory, no I/O.
- **Holds now:**
  - Nesting depth 128 in the parser and for arrays built at run time (BL-017).
    Tests: [`tests/security_limits.rs`](../../tests/security_limits.rs) (subprocess tests: every
    nesting shape, 20k parens, 60k-term chains, runtime-built depth),
    [`tests/nesting_stack.rs`](../../tests/nesting_stack.rs) (depth 128 fits a 2 MB debug thread, BL-029).
  - Per-evaluation budget of 256 MiB of values created, charged before amplifying builtins
    allocate (ADR-0011, BL-027). Tests: [`tests/eval_budget.rs`](../../tests/eval_budget.rs)
    (rows P1–P12, A1–A12, C1–C6, peak heap within 2× budget). Comparing two JSON objects with
    `==` parses both and is charged too (ADR-0023).
  - Servers: 10 s evaluation deadline checked inside the evaluator; response size bounded by the
    budget (ADR-0011). Tests: `c6_timeout`, `bl027c_*_meet_the_deadline`,
    `f3_servers_reject_responses_larger_than_the_budget` in `tests/eval_budget.rs`.
  - Lambdas share the parent scope (BL-030), so formulas over big variables are no longer O(n·m).
  - Weekly coverage-guided fuzzing of `parse`, `evaluate` and `evaluate_with_json`
    (`fuzz/`, `.github/workflows/fuzz.yml`, ADR-0028). Found inputs are pinned in
    [`tests/fuzz_regressions.rs`](../../tests/fuzz_regressions.rs).
  - Operators and builtins return an error instead of `NaN` or infinity (ADR-0018 D-1), so
    results serialize as JSON numbers.
  - JS hooks cannot replace a builtin: a hook whose `@name` is a builtin name or alias (any case)
    is skipped on load (ADR-0024, #86). Rust `CustomFunction`s registered by the embedder may
    still shadow builtins; that is embedder code, by design (ADR-0016).
- **Gaps:**
  - The library and `sk` have **no deadline** by default (by design, ADR-0011). Uncharged
    borrowed reads can still cost O(n²) CPU there; embedders must use `with_limits`.
  - A single builtin runs to completion before the next deadline check; small cost-model
    undercounts (BL-035). Shared values would remove the read copies (BL-036).

### 2.2 JSON variables (`--json`, HTTP `arguments`, `sk_server` `variables`)
- **Attacker controls:** any JSON (HTTP: within the 1 MB request).
- **Must hold:** no panic; key sanitizing cannot overwrite reserved variables such as `:arguments`.
- **Holds now:** `serde_json`'s depth limit (128) matches `MAX_NESTING_DEPTH`
  (`deep_json_vars_*` in `tests/security_limits.rs`); JSON parsing is pre-charged to the budget.
  - **Mitigated (BL-077):** wherever `:arguments` is built (the library `*_json*` entry points,
    `sk --json`, `sk_http_server`) it is inserted after the keys, so a key named `arguments`
    cannot replace it; the key's value stays readable as `:arguments.arguments`. Two keys that
    sanitize to the same name (`a-b`, `a_b`) are an error naming both keys (HTTP 400 on
    `sk_http_server`), not a silent overwrite; one sanitizer, `src/json_keys.rs`, serves all
    of them and, since BL-096, `sk_server` too (`tests/json_variable_keys.rs`,
    `tests/server_reserved_arguments.rs`). `sk_server` builds no `:arguments`.
- **Gaps:**
  - Values passed in through the Rust API (`evaluate_with(vars)`) are not depth-checked until
    they are wrapped (BL-017 follow-up; documented on the public API in `src/lib.rs`).

### 2.3 `sk_http_server`
- **Attacker controls:** raw bytes on the socket.
- **Must hold:** auth before parse/eval; size and time limits; one request cannot crash or stall
  the server for others.
- **Holds now:**
  - 1 MB request cap; the whole request must arrive within 30 s (BL-037); writing one response
    has a 10 s limit in total (BL-047, #62); responses are written completely (BL-034). Tests:
    [`tests/server_limits.rs`](../../tests/server_limits.rs)
    (`bl037_sk_http_server_trickling_client_times_out`, `bl034_*`).
  - Tokens compared in constant time; only the exact token is accepted
    (`bl018_servers_accept_only_the_exact_token`). An empty `--token` or `--admin-token` is a
    start-up error (BL-048, #62).
  - Separate eval and admin tokens; JS management and `DELETE /cache` need the admin token.
    `SKILLET_ADMIN_TOKEN` sets it without putting it on the command line (#78).
  - CORS (ADR-0019, #78): `Access-Control-Allow-Origin` is the configured origin
    (`--cors-origin` / `SKILLET_CORS_ORIGIN`), else `*` when a token is set, else absent. POST,
    PUT and DELETE bodies must be `application/json` (`415` otherwise; multipart only on
    `/upload-js` and `/update-js`), so a cross-origin JSON request needs a preflight. In
    development mode a request with an `Origin` header to an admin endpoint gets `403` before any
    handler runs (`src/bin/http_server/cors.rs`).
  - Optional limit on concurrent connections per IP address, `--max-connections-per-ip` /
    `SKILLET_MAX_CONNECTIONS_PER_IP`; over the limit the accept thread answers `429` (ADR-0025, #87).
  - Default bind `127.0.0.1`; 16 MB worker stacks (BL-017); result cache entries capped at 64 KiB;
    the documentation page is embedded in the binary (BL-045, #54).
- **Gaps:**
  - **No token = no auth** ("development mode"). Any non-browser client that reaches the port can
    evaluate and use the JS management endpoints. Only browser requests (with `Origin`) to admin
    endpoints are refused. The server prints a development-mode warning at start-up.
  - The per-IP limit is **off by default**, so one client can hold every worker of the fixed pool
    with idle connections until the 30 s read deadline (ADR-0025). Behind a proxy all clients
    share one address, and many addresses are not stopped; that belongs in a proxy or firewall.
  - All read errors answer HTTP 400, and the connection may reset before the error is read
    (BL-049).
  - No TLS (§4); the tokens travel in clear text in the `Authorization` header.

### 2.4 `sk_server` (TCP, newline-delimited JSON)
- **Attacker controls:** raw bytes on the socket.
- **Holds now:** 1 MB line cap and a read deadline (BL-033, tests `bl033_*`, `bl037_sk_server_*`);
  only an authorized request restarts the 30 s connection deadline, and a wrong or missing token
  closes the connection (BL-046, #62); total write deadline (BL-047, #62); an empty token is a
  start-up error (BL-048, #62); constant-time token check; 10 s evaluation deadline; default bind
  `127.0.0.1`; optional per-IP connection limit (ADR-0025, #87).
- **Gaps:** without a token there is no auth; the per-IP limit is off by default (as in 2.3);
  the token travels inside the JSON body, in clear text (no TLS on either server).

### 2.5 JavaScript plugins: upload and management endpoints, and execution
Endpoints: `POST /upload-js`, `PUT /update-js`, `DELETE /delete-js`, `GET /list-js`,
`POST /reload-hooks` (only with the default `plugins` feature).

- **Attacker controls:** JS source (admin or hook author). Indirectly, an eval client controls the
  **arguments** of every installed hook, so a hook whose work depends on its input is reachable
  by untrusted users.
- **Must hold:** admin-only; bounded memory and time in QuickJS; no host file system or network
  access; a hook cannot crash or freeze the server.
- **Holds now:**
  - Admin token check before the body is parsed; a fresh QuickJS runtime per call (no state
    shared between calls).
  - Limits per call (ADR-0017 option B, #79): 64 MiB heap, 256 KiB stack, and an interrupt handler
    that stops the call at the evaluation deadline or after 10 s, whichever comes first. The
    `@example` run during upload validation uses the same limits.
  - No host functions: `httpGet` is removed (ADR-0017 §6, #79); `src/js_plugin.rs` tests that
    `typeof httpGet` is undefined.
  - JS arguments and results nest at most 128 levels; deeper or cyclic values give
    `Expression nested too deeply (max 128)` instead of aborting the process (#63, #79). Results are charged
    to the evaluation budget.
  - Upload, update and delete accept only `^[A-Za-z0-9_-]+\.js$` and answer 400 otherwise, so
    they touch only files directly inside the hooks directory (#63).
  - A reserved builtin name in `@name` gets 400 and nothing is written (ADR-0024, #86).
  - The registry lock is released before a custom function runs, so a slow hook no longer
    blocks `/reload-hooks` or other evaluations (#63).
  - JS errors carry the real exception text (#79).
  - **Mitigated (BL-077):** `/reload-hooks` makes the registry match the hooks that load from
    disk, so a hook whose file was deleted is removed; a reload that cannot read the directory
    changes nothing (`reload_hooks_*` in `tests/hook_files.rs`).
- **Gaps:**
  - **Low:** upload "syntax validation" only parses the comment header and runs the
    `@example`. Hooks in subdirectories load but cannot be updated or deleted through the API.
  - Each hook call may use up to its full time and memory limit, so an installed hook whose cost
    depends on its input lets eval clients spend that much per call (bounded by the limits above
    and by the worker pool).

### 2.6 Hooks directory
- **Attacker controls:** files in the directory (hook author / admin), or the directory itself
  when a binary runs in an untrusted working directory.
- **Must hold:** load only `.js` from the configured directory; no path traversal.
- **Holds now:** only `*.js` files are loaded (recursively); loading parses the comment header
  only, and JS runs when the function is called, inside the limits of 2.5. Each entry is resolved
  first and skipped when it resolves outside the hooks directory, and each directory is read once
  (#63). Hooks cannot shadow builtins (ADR-0024, #86), so a stray hooks folder can only **add**
  functions; it can no longer change what an existing expression returns.
- **Gaps:**
  - **Low:** the default hooks directory is still the **relative** path `hooks`, resolved
    against the working directory, for `sk`, `sk_server` and `sk_http_server` (maintainer
    decision, BL-066). A hooks folder in an untrusted working directory still loads its new
    function names. `load_functions` creates the directory when it is missing (BL-057).
  - Symlinks that resolve inside the directory are followed, so the "configured directory"
    boundary is only as strong as its permissions.

### 2.7 Daemon mode and PID file (Unix)
- **Attacker controls:** a local user who can write to the directory that holds the PID file.
- **Must hold:** safe file permissions; no `unsafe` beyond fork/setsid/dup2.
- **Holds now:** the PID file is opened with `O_NOFOLLOW`, so a symlink at the PID path makes the
  write fail (`sk_http_server` #63, `sk_server` #72). `unsafe` is limited to `fork`, `setsid`,
  `close` and `dup2` in `http_server/daemon.rs` and the copy in `sk_server.rs`; signal handling
  uses `ctrlc`.
- **Gaps:** `--log-file` is accepted and ignored, and daemon output goes to `/dev/null`. The
  daemon code in `sk_server.rs` sits outside the files the Rust Engineer rule allows `unsafe` in
  (BL-064).

### 2.8 Docker image
- **Holds now** (#74): `docker/entrypoint.sh` refuses to start (exit 64) unless
  `SKILLET_AUTH_TOKEN` and `SKILLET_ADMIN_TOKEN` are both set and non-empty, passes neither on the
  command line and never prints them. The builder is `rust:1.89.0-slim-bookworm` with
  `--locked`; the runtime is `debian:bookworm-slim` without `curl`; the server runs as a non-root
  `skillet` user that can write only the hooks directory. Compose publishes on `127.0.0.1` unless
  `SKILLET_PUBLISH_ADDR` says otherwise.
- **Gaps:**
  - CI does not build the image, so #74 has not been checked with a real build or start (BL-062).
  - The image does not set a per-IP connection limit; the container binds `0.0.0.0` inside, and
    exposure depends on the published address.
  - `Dockerfile.alpine-builder` uses the floating `rust:latest` tag and builds aarch64 only
    (BL-065).

### 2.9 Release artifacts and dependencies
- **Holds now:** `cargo audit` runs in CI (BL-019, PR #50); release workflows pin Rust 1.89.0 and
  build with `--locked` (BL-044, #53); `cross` and the musl images are pinned (#75); release
  binaries ship with `.sha256` files generated in the same job; `lru` updated past
  RUSTSEC-2026-0253 (BL-043, #57); `scalar-doc` and its dependencies removed (BL-042, #81);
  branch protection on `main`, `develop` and `release/*` with required checks (BL-022).
- **Gaps:** checksums come from the same pipeline, so they detect corruption, not tampering (no
  signatures or build provenance); GitHub Actions are referenced by tag or branch, not by commit
  SHA (BL-065).

## 3. Top risks (2026-11-21)

| # | Risk | Severity | Tracking |
|---|---|---|---|
| 1 | Development mode: without a token, both servers accept evaluation, and `sk_http_server` accepts JS management, from any non-browser client that reaches the port. The default bind is `127.0.0.1` and the Docker image refuses to start without tokens, so this needs an operator to bind elsewhere without tokens | Medium | accepted risk (maintainer, 2026-11-21); ADR-0019; `sk_http_server` warns at start-up, `sk_server` only reports when auth is on |
| 2 | Worker-pool exhaustion: the per-IP limit is off by default, so one client can hold every worker until the 30 s read deadline; many addresses are not stopped at all | Medium | ADR-0025 (opt-in limit); proxy or firewall in front |
| 3 | No deadline in the library and `sk` by default, and one builtin runs to completion between deadline checks | Medium (embedders) | ADR-0011 (`with_limits`), BL-035, BL-036 |
| 4 | Release and image integrity: no signatures or provenance (Actions are pinned by SHA, the Docker image is built and smoke-tested in CI) | Low | BL-062, BL-065 done; signing not tracked yet |

Next in line (all Low): the relative `hooks` default (2.6, BL-057), real 408/413 codes
(BL-049), Rust API values not depth-checked (2.2).

## 4. Out of scope
- Confidentiality of traffic: neither server speaks TLS; put a TLS proxy in front.
- Multi-tenant isolation: the function registry is process-global (ADR-0007), so all clients of
  one server share the same hooks.
- A malicious operator or anyone who can write the binary, its flags or its environment.
- Side channels beyond token comparison.

## 5. Reporting a vulnerability
Please do not open a public issue for a security problem. Open a **private GitHub security
advisory** for this repository (the repository's "Security" tab, then "Report a vulnerability").
Include the affected version or commit, the binary or API involved, and the smallest reproduction
you have. We will acknowledge the report in the advisory thread, agree on a fix and a disclosure
date with you, and credit you unless you prefer otherwise. Test only against servers you run
yourself.
