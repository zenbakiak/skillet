# Skillet threat model

- Owner: Security Reviewer (`harness/roles/security-reviewer.md`)
- Last reviewed: 2026-11-20, against `develop` d3e6a5d (version 0.6.3 + unreleased 0.7.0 changes)
- Scope: the `skillet` library, the five binaries, the hooks directory, the Docker image and the
  release workflows. Update this file whenever a surface, limit or finding changes.

This page describes the system **as built today**: what an attacker controls on each surface,
what already holds (with the tests that prove it), and what does not hold yet (with backlog ids).
Gaps that are not fixed yet are described by what is missing, not by how to exploit them.

## 1. Assets and trust levels

| Who | Trust | Can do |
|---|---|---|
| Expression author / eval client | **Untrusted.** Holds the eval token if one is set. | Send any expression text and any JSON variables, many times, concurrently. |
| Admin | Trusted to run code **inside the sandbox** (ADR-0007) but not to own the host. Holds the admin token. | Upload, update, delete and reload JS hooks; clear the cache. |
| Hook author | Same as admin (writes files into the hooks directory). | Any JS the sandbox allows. |
| Operator | Fully trusted. | Starts the binaries, picks flags, tokens, hooks dir, PID file. |
| Network peer without a token | **Untrusted.** | Raw bytes on the socket; with no token configured, everything an eval client and an admin can do. |
| Web page in the operator's browser | **Untrusted.** | Cross-origin requests to `127.0.0.1` (simple requests need no preflight). |

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
    Tests: [`tests/security_limits.rs`](../../tests/security_limits.rs) (38 subprocess tests: every
    nesting shape, 20k parens, 60k-term chains, runtime-built depth),
    [`tests/nesting_stack.rs`](../../tests/nesting_stack.rs) (depth 128 fits a 2 MB debug thread, BL-029).
  - Per-evaluation budget of 256 MiB of values created, charged before amplifying builtins
    allocate (ADR-0011, BL-027). Tests: [`tests/eval_budget.rs`](../../tests/eval_budget.rs)
    (rows P1–P12, A1–A12, C1–C6, peak heap within 2× budget).
  - Servers: 10 s evaluation deadline checked inside the evaluator; response size bounded by the
    budget (ADR-0011). Tests: `c6_timeout`, `bl027c_*_meet_the_deadline`,
    `f3_servers_reject_responses_larger_than_the_budget` in `tests/eval_budget.rs`.
  - Lambdas share the parent scope (BL-030), so formulas over big variables are no longer O(n·m).
- **Gaps:**
  - The library and `sk` have **no deadline** by default (by design, ADR-0011). Uncharged
    borrowed reads can still cost O(n²) CPU there; embedders must use `with_limits`.
  - A single builtin runs to completion before the next deadline check; small cost-model
    undercounts (BL-035). Shared values would remove the read copies (BL-036).
  - No fuzzing yet (BL-019; `cargo fuzz` over `parse` + `evaluate_with_json`).
  - Custom (Rust or JS) functions are looked up **before** builtins, so a hook named `SUM`
    replaces `SUM` for every expression (see 2.6).

### 2.2 JSON variables (`--json`, HTTP `arguments`, `sk_server` `variables`)
- **Attacker controls:** any JSON (HTTP: within the 1 MB request).
- **Must hold:** no panic; key sanitizing cannot overwrite reserved variables such as `:arguments`.
- **Holds now:** `serde_json`'s depth limit (128) matches `MAX_NESTING_DEPTH`
  (`deep_json_vars_*` in `tests/security_limits.rs`); Json parsing is pre-charged to the budget.
- **Gaps:**
  - **Low, new:** a JSON key named `arguments` replaces the reserved `:arguments` variable that
    `sk_http_server` builds for `JQ` (it is inserted first, then overwritten by the loop in
    `http_server/eval.rs`). Only the same caller is affected, so the impact is confusion, not
    escalation; still a broken invariant. Two keys that sanitize to the same name (`a-b`, `a_b`)
    silently collide.
  - Values passed in through the Rust API (`evaluate_with(vars)`) are not depth-checked until
    they are wrapped (BL-017 follow-up).

### 2.3 `sk_http_server`
- **Attacker controls:** raw bytes on the socket.
- **Must hold:** auth before parse/eval; size and time limits; one request cannot crash or stall
  the server for others.
- **Holds now:**
  - 1 MB request cap; the whole request must arrive within 30 s (BL-037); 10 s write timeout per
    write; responses are written completely (BL-034). Tests:
    [`tests/server_limits.rs`](../../tests/server_limits.rs)
    (`bl037_sk_http_server_trickling_client_times_out`, `bl034_*`).
  - Tokens compared in constant time; only the exact token is accepted
    (`bl018_servers_accept_only_the_exact_token`).
  - Separate eval and admin tokens; JS management and `DELETE /cache` need the admin token.
  - Default bind `127.0.0.1`; 16 MB worker stacks (BL-017); result cache entries capped at 64 KiB.
- **Gaps:**
  - **No token = no auth at all** ("development mode"), including the JS management endpoints.
    An empty token (`--token ""`) also disables auth (BL-048).
  - **Medium, new:** every response carries `Access-Control-Allow-Origin: *`, and request bodies
    are parsed as JSON whatever the `Content-Type`. In development mode a web page in the
    operator's browser can therefore send "simple" cross-origin requests (no preflight) to
    `127.0.0.1` that upload a hook, reload hooks and read evaluation results.
  - No total deadline for writing a response (slow reader, BL-047); all read errors answer
    HTTP 400 and the connection may reset before the error is read (BL-049).
  - `documentation.html` is read from the working directory at run time (BL-045).
  - 1 ms accept-loop sleep (BL-039) is a latency issue, not a security one.

### 2.4 `sk_server` (TCP, newline-delimited JSON)
- **Attacker controls:** raw bytes on the socket.
- **Holds now:** 1 MB line cap and a read deadline (BL-033, tests `bl033_*`, `bl037_sk_server_*`);
  constant-time token check; 10 s evaluation deadline; default bind `127.0.0.1`.
- **Gaps:** the deadline resets on blank, invalid or wrong-token lines, so an unauthenticated
  client can hold a worker (BL-046); no total write deadline (BL-047); empty token disables auth
  (BL-048). The token travels inside the JSON body, in clear text (no TLS on either server).

### 2.5 JavaScript plugins: upload and management endpoints, and execution
Endpoints: `POST /upload-js`, `PUT /update-js`, `DELETE /delete-js`, `GET /list-js`,
`POST /reload-hooks` (only with the default `plugins` feature).

- **Attacker controls:** JS source (admin or hook author). Indirectly, an eval client controls the
  **arguments** of every installed hook, so a hook whose work depends on its input is reachable
  by untrusted users.
- **Must hold:** admin-only; bounded memory and time in QuickJS; no host file system or network
  access; a hook cannot crash or freeze the server.
- **Holds now:** admin token check before the body is parsed; a fresh QuickJS runtime per call
  (no state shared between calls); QuickJS's default 256 KiB stack limit turns runaway JS
  recursion into an error; custom function results are charged to the evaluation budget and
  depth-checked.
- **Gaps** (measured on 2026-11-20, release build; details in
  [ADR-0017](../adr/0017-quickjs-resource-limits.md)):
  - **High (BL-018):** no interrupt handler and no memory limit. A looping hook keeps its worker
    at 100% CPU forever, far past the 10 s deadline; two such requests made a 2-worker server
    stop answering, `/health` included. A hook allocated a 400 MB string with nothing stopping it.
    The `@example` in an upload is executed during validation, with the same lack of limits.
  - **High, new:** converting a JS result back to a `Value` recurses without a depth bound. A
    hook that returns a self-referencing or very deeply nested array overflows the worker's stack
    and **aborts the whole process** (both `sk_http_server` and `sk`). The depth limit only runs
    after the conversion.
  - **High, new:** every runtime gets an undocumented global `httpGet(url)` that runs the `curl`
    binary with the JS-supplied string as its argument. That gives hooks: reads of any local file
    the server user can read (`file://`), requests to internal and loopback addresses (SSRF),
    curl option injection (an argument starting with `-` is parsed as an option), and no timeout
    (a silent peer held a worker for as long as we waited, past the 10 s deadline, and the
    interrupt handler cannot stop a blocked host call). This breaks "no host FS/net access" and
    is a maintainer decision (ADR-0017 §6).
  - **Medium, new:** `filename` in upload/update/delete is joined to the hooks directory without
    validation. Relative (`../`) and absolute paths are accepted, so an admin can write or delete
    any `*.js` file the server user can write, outside the hooks directory.
  - **Medium, new:** the registry read lock is held while a custom function runs. A long-running
    hook plus a waiting `POST /reload-hooks` (writer) blocks every new, uncached evaluation that
    calls any function, so the whole server stalls until the hook returns.
  - **Low, new:** JS errors reach the client as `JS function execution failed: Exception generated
    by QuickJS`; the actual exception message is lost. `/reload-hooks` adds and replaces functions
    but never removes deleted ones; upload "syntax validation" only parses the comment header.

### 2.6 Hooks directory
- **Attacker controls:** files in the directory (hook author / admin), or the directory itself
  when a binary runs in an untrusted working directory.
- **Must hold:** load only `.js` from the configured directory; no path traversal.
- **Holds now:** only `*.js` files are loaded (recursively); loading parses the comment header
  only, JS runs when the function is called.
- **Gaps:**
  - **Medium, new:** the default hooks directory is the **relative** path `hooks`, resolved
    against the working directory, for `sk`, `sk_server` and `sk_http_server`, and hooks may
    shadow builtins. Running `sk "=SUM(1, 2)"` inside a checkout that contains `hooks/sum.js`
    runs that file's JS (verified: the result was the hook's). With `httpGet` that JS can read
    files and reach the network. `load_functions` also creates the directory when it is missing.
  - Symlinks inside the directory are followed (`is_dir`/`read_to_string`), so the "configured
    directory" boundary is only as strong as its permissions.
  - Uploads write outside the directory (path traversal, 2.5).

### 2.7 Daemon mode and PID file (Unix)
- **Attacker controls:** a local user who can write to the directory that holds the PID file.
- **Must hold:** safe file permissions; no `unsafe` beyond fork/setsid/dup2.
- **Holds now:** `unsafe` is limited to `fork`, `setsid`, `close` and `dup2` in
  `http_server/daemon.rs` (and the copy in `sk_server.rs`); signal handling uses `ctrlc`.
- **Gaps:**
  - **Low, new:** the PID file is written with `File::create`, which follows symlinks and
    truncates. A symlink planted at the PID path makes the daemon overwrite the link target with
    its PID (verified in a temp dir). Default path is `skillet-http-server.pid` in the working
    directory; the daemon keeps the working directory and umask (needed for the relative hooks
    path). Fix: `O_CREAT|O_EXCL|O_NOFOLLOW`, or write a temp file and rename.
  - `--log-file` is accepted and ignored; daemon output goes to `/dev/null`.

### 2.8 Docker image
- **Gaps:**
  - **High, new:** the image defaults to `HOST=0.0.0.0` with `AUTH_TOKEN=""` and `ADMIN_TOKEN=""`,
    so a container started without tokens runs in development mode on all interfaces: anyone who
    reaches the port can evaluate, upload JS, and (through `httpGet`, with `curl` installed in the
    image) read files and reach internal addresses, cloud metadata services included. The
    generated `start.sh` echoes the full command line, tokens included, into the container log.
  - Builder `rustlang/rust:nightly-slim` and runtime `debian:sid-slim` are unpinned, unstable
    tags (reproducibility and supply chain); the image runs as a non-root `skillet` user (good).

### 2.9 Release artifacts and dependencies
- **Holds now:** `cargo audit` runs in CI (BL-019, PR #50); release binaries ship with
  `.sha256` files generated in the same job; MSRV and the gate toolchain are pinned for CI
  (ADR-0012).
- **Gaps:** release workflows use floating `stable` without `--locked` (BL-044); checksums come
  from the same pipeline, so they detect corruption, not tampering (no signatures or build
  provenance); `lru` 0.16 has RUSTSEC-2026-0253 (BL-043); most other advisories come through
  `scalar-doc` (BL-042); client-side git hooks only, no branch protection yet (BL-022).

## 3. Top risks (2026-11-20)

| # | Risk | Severity | Tracking |
|---|---|---|---|
| 1 | Docker default runs unauthenticated on `0.0.0.0`; with JS upload and `httpGet` that is remote file read and SSRF for anyone who reaches the port | High | new (suggested backlog item) + ADR-0017 §6 |
| 2 | A JS hook result that is cyclic or deeply nested aborts the whole process | High | new; fix proposed in ADR-0017 §4 |
| 3 | No CPU or memory limit inside QuickJS: looping or allocating hooks pin workers and memory past every server limit | High | BL-018, ADR-0017 |
| 4 | `httpGet` gives hooks local file read, SSRF, curl option injection and unbounded blocking | High | new; ADR-0017 §6 (maintainer decision) |
| 5 | Development mode + `Access-Control-Allow-Origin: *` lets any web page drive a local server, JS upload included | Medium | new (suggested backlog item) |

Next in line: upload path traversal (Medium), registry lock held during hook execution (Medium),
relative `hooks/` default plus builtin shadowing (Medium), BL-046/BL-048 (Medium).

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
