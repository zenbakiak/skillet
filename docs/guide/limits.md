# Limits

Skillet evaluates untrusted formulas, so every evaluation runs under fixed limits. A formula that
goes over a limit returns an ordinary `Error`; it never crashes the process or the server.

| Limit | Value | Applies to | Error text |
|---|---|---|---|
| Nesting depth | 128 (`skillet::MAX_NESTING_DEPTH`) | library, CLI, servers | `Expression nested too deeply (max 128)` |
| Evaluation budget | 256 MiB of values created (`skillet::MAX_EVAL_BYTES`) in the library and CLI; 1 GiB by default on the servers, set with `--max-eval-bytes` or `SKILLET_MAX_EVAL_BYTES` | library, CLI, servers | `Evaluation budget exceeded (max 256 MB)` (servers: `max 1024 MB`, or the configured value) |
| Deadline | 60 s per request by default, set with `--eval-timeout` or `SKILLET_EVAL_TIMEOUT` | servers only | `Evaluation timed out (max 60 s)` (or the configured value) |
| Response size | serialized result must fit the server's budget | servers only | `Evaluation budget exceeded (max 1024 MB)` (or the configured value) |
| Request size | 1 MB (1,048,576 bytes) | both servers | HTTP 413 `Request body too large (max 1MB)` / `Request too large (max 1MB)` (`sk_server`) |
| Request read time | 30 s for the whole request | both servers | HTTP 408 `Request read timed out (max 30 s)` / the same message (`sk_server`) |
| Response write | 10 s for the whole response | both servers | none (the response is cut short and the connection closed) |
| Connections per IP address | off by default; `--max-connections-per-ip N` or `SKILLET_MAX_CONNECTIONS_PER_IP` | both servers | HTTP 429 `Too many connections from this address (max N)` / the same JSON line (`sk_server`); see [servers](servers.md#connection-limit-per-ip-address) |
| Cache entry | 64 KiB | `sk_http_server` result cache | none (bigger results are just not cached) |
| Parse cache | 256 expressions, 128 KiB of text, 32 KiB per expression, per worker thread | both servers | none (longer expressions are parsed on every request) |
| JS plugin heap | 64 MiB per call | library, CLI, servers (`plugins` feature) | `JS function NAME ran out of memory (max 64 MB)` |
| JS plugin time | 10 s per call, or the request deadline if sooner | library, CLI, servers (`plugins` feature) | `JS function NAME timed out (max 10 s)` / `Evaluation timed out (max N s)` |
| JS plugin stack | 256 KiB | library, CLI, servers (`plugins` feature) | `JS function NAME failed: RangeError: Maximum call stack size exceeded` |

Request limits are described in [Server request limits](#server-request-limits-1-mb-30-s-10-s).

Decisions and the full cost model: [ADR-0011](../adr/0011-evaluation-budget.md) (budget, deadline,
cache), BL-017 (depth) and [ADR-0017](../adr/0017-quickjs-resource-limits.md) (JavaScript plugins). Requirements: PRD-001 NFR-4 to NFR-7.

All examples below were run with the release CLI (`cargo build --release --bin sk`, then
`target/release/sk`). The CLI prints the result on stdout and errors on stderr, and exits with code
2 on an error. The `Loaded N custom JavaScript function(s)` line the CLI prints on stderr when
`hooks/` has plugins is left out.

## Nesting depth (128)

The parser counts its own recursion (parentheses, brackets, call arguments, unary operators,
ternaries) and the depth of the tree it builds, including long operator chains (`1+1+1+…`) and
method/property chains (`:a.b.c…`, `.to_s().to_s()…`). The evaluator applies the same limit to
arrays it builds at run time. 128 is also `serde_json`'s limit, so JSON variables passed with
`--json` or over HTTP share the same number.

```console
$ sk "$(printf '(%.0s' {1..128})1$(printf ')%.0s' {1..128})"
Number(1.0)

$ sk "$(printf '(%.0s' {1..129})1$(printf ')%.0s' {1..129})"
Error: Expression nested too deeply (max 128) at position 129

$ sk "$(printf '[%.0s' {1..129})1$(printf ']%.0s' {1..129})"
Error: Expression nested too deeply (max 128) at position 129
```

Arrays built at run time are checked too. These have no position, because no single token is at
fault:

```console
$ sk "[1,1,1].reduce([:acc], 0)"
Array([Array([Array([Number(0.0)])])])

$ sk ":a := [1]; :a := [:a]; …(200 times)…; :a"
Error: Expression nested too deeply (max 128)
```

Values returned by a JavaScript plugin get the same check when they are converted to Skillet values.
`NEST(n)` below is a test plugin that returns `n` nested arrays around `1`:

```console
$ sk "=NEST(128).flatten()"
Array([Number(1.0)])

$ sk "=NEST(129).flatten()"
Error: Expression nested too deeply (max 128)
```

The depth limit cannot be changed. It protects the thread stack. Release builds handle depth 128 on
a 1 MB stack. Debug builds need about 20 KB of stack per level, so depth 128 can overflow a default
2 MB `std::thread` in a **debug** build (BL-029). The servers run evaluations on 16 MB worker
threads.

## Evaluation budget (256 MiB; servers 1 GiB)

Each evaluation has a budget of 256 MiB (`MAX_EVAL_BYTES = 268435456`) in the library and the `sk`
CLI. The servers use 1 GiB by default and can be given another value at start-up (see
[Server limits are configurable](#server-limits-are-configurable)). Every value the evaluator
creates is charged against it, at a fixed cost:

```
cost(Number | Boolean | Null | Currency | DateTime) = 32
cost(String(s) | Json(s))                           = 32 + byte length of s
cost(Array(items))                                  = 32 + sum of cost(item)
```

The count only goes up. Freeing a value gives nothing back, so the budget limits the total work as
well as memory. The count is deterministic: the same formula with the same variables always passes
or always fails, on any machine and under any load.

What is charged:

- results of array and object literals, function calls, method and safe-method calls, casts,
  property and safe-property access, index and slice;
- every assignment (`:a := v` stores a copy);
- every lambda iteration (`filter`, `map`, `find`, `reduce`, `SUMIF`, `AVGIF`, `COUNTIF`, in method
  and function form), at the cost of the item it binds;
- **copying variable reads**: a variable used as a function argument, an array or object element,
  an assignment value, a ternary or `IF` branch, a spread, or a lambda result is copied, so it is
  charged `cost(value)`.

What is free:

- **borrowed variable reads**: the operands of operators (`:s == :t`, `:x + 1`) and the targets of
  methods, properties, index and slice (`:rows.length()`, `:u.name`, `:a[0]`). These read the value
  in place. Each use still counts towards the deadline check;
- literals such as `"abcd"` and `42` themselves (only the results built from them are charged).

Builtins that can make far more than their inputs are charged **before** they allocate, with the
exact size or an upper bound: `SUBSTITUTE`, `SUBSTITUTEM`, `.replace`, `JOIN`, `.join`, `SPLIT`,
`.split`, `JQ` (each match), `CONCAT`, `MERGE`, `.to_a()` on strings, `.to_json()`, `.to_s()` and
`::String` on arrays, object literals (their exact JSON size), and every parse of a JSON object
(property access, `DIG`, `.keys()`, `.values()`, `.dig()`, `JQ` input).

Normal formulas use a small part of the budget. A 10,000-item `map` is fine:

```console
$ sk ':nums.map(:x * 2).length()' --json '{"nums": [0, 1, …, 9999]}'
Number(10000.0)
```

Formulas whose size doubles per step hit the budget after about 20 steps:

```console
$ sk "[1,1,1,1,1,1,1,1,1,1].reduce([:acc, :acc], 0).flatten().length()"
Number(1024.0)

$ sk "[1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1].reduce([:acc, :acc], 0)"
Error: Evaluation budget exceeded (max 256 MB)

$ sk "[1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1].reduce(MERGE(:acc, :acc), [1]).length()"
Number(1048576.0)

$ sk "[1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1].reduce(MERGE(:acc, :acc), [1]).length()"
Error: Evaluation budget exceeded (max 256 MB)
```

### Quadratic formulas

The cost is cumulative, so a formula that copies something of size *n* once per item costs
*n* × items. Two patterns are affected.

**1. Building an array with `reduce` + `MERGE`.** Each step copies the accumulator, so the total is
quadratic. Since the budget was added (0.7.0), this fails above about 2,800 items, where it used
to return a value. This is the one intended behaviour change from ADR-0011 (row A11):

```console
$ sk ':nums.reduce(MERGE(:acc, [:x]), []).length()' --json '{"nums": [0, 1, …, 2799]}'
Number(2800.0)

$ sk ':nums.reduce(MERGE(:acc, [:x]), []).length()' --json '{"nums": [0, 1, …, 9999]}'
Error: Evaluation budget exceeded (max 256 MB)
```

Use `map`, `filter` or `FLATTEN` instead. They build the array once.

**2. Copying a large variable inside a lambda.** A variable passed as a function *argument* is
copied, once per item:

```console
$ sk ':nums.map(SUM(:nums)).length()' --json '{"nums": [0, 1, …, 4999]}'
Error: Evaluation budget exceeded (max 256 MB)
```

Hoist the work out of the lambda: compute it once, then use the small result per item:

```console
$ sk ':t := SUM(:nums); :nums.map(:t).length()' --json '{"nums": [0, 1, …, 4999]}'
Number(5000.0)
```

The method form also works, because a method target is borrowed, not copied:

```console
$ sk ':nums.map(:nums.sum()).length()' --json '{"nums": [0, 1, …, 4999]}'
Number(5000.0)

$ sk ':nums.map(:x / :nums.length()).sum()' --json '{"nums": [0, 1, …, 9999]}'
Number(4999.499999999991)

$ sk ':n := :nums.length(); :nums.map(:x / :n).sum()' --json '{"nums": [0, 1, …, 9999]}'
Number(4999.499999999991)
```

Hoisting is still the better habit: the method form recomputes the sum for every item.

## Server deadline (60 s by default)

`sk_http_server` and `sk_server` run every evaluation with a deadline, 60 s unless set with
`--eval-timeout` or `SKILLET_EVAL_TIMEOUT` (see [below](#server-limits-are-configurable)). The
evaluator reads the clock every 256 charges or ticks, so a request stops shortly after the deadline
and the worker thread is free for the next request. The library and the `sk` CLI have no deadline
unless you set one (see [Library API](#library-api-evallimits-and-with_limits)).

The deadline is only checked between operations. One builtin call that is already running (for
example `SORT` or `UNIQUE` on millions of items) runs to completion before the next check, so a
request can run past 10 s by the time that one call takes. Measured in BL-027c: `SORT`/`UNIQUE` of
3M items takes about 0.12 s in a release build, so today the overrun is small. A builtin that is
slower than O(n log n) would make it larger (tracked in BL-035).

Example (HTTP, one worker, started with `--eval-timeout 10`; the expression does 1M × 4
comparisons of two 400 KB strings passed as `arguments`):

```console
POST /eval {"expression": "[1×1000].map([1×1000].map(((:s == :t) == (:t == :s)) == ((:s == :t) == (:t == :s))).length()).sum()", "arguments": {"s": "x…(400,000)", "t": "x…(400,000)"}}
400 {"success":false,"result":null,"error":"Evaluation timed out (max 10 s)","execution_time_ms":10006.224333,"request_id":0}

POST /eval {"expression": "2 + 3"}
200 {"success":true,"result":5.0,"error":null,"execution_time_ms":0.015708,"request_id":1}
```

### Server limits are configurable

Since ADR-0011 amendment 4 both servers take the deadline and the budget at start-up. The flag
wins over the environment variable, as for the tokens and the connection limit, and `0`, an empty
value or an invalid number stops start-up with exit code 1 and a message naming the setting.

| Setting | Value | Default |
|---|---|---|
| `--eval-timeout <seconds>` / `SKILLET_EVAL_TIMEOUT` | whole seconds, 1 to 86400 | `60` |
| `--max-eval-bytes <bytes>` / `SKILLET_MAX_EVAL_BYTES` | whole bytes, or with `K`, `M`, `G` (KiB, MiB, GiB, either case) | `1G` |

The error texts show the configured values: `Evaluation timed out (max 60 s)` and
`Evaluation budget exceeded (max 1024 MB)` with the defaults (whole MiB are shown as `MB`, as in the
library). The start-up banner prints both. The library's `MAX_EVAL_BYTES` (256 MiB) and the CLI do
not change, and neither has a deadline by default (ADR-0011 §2).

These limits do not change with the deadline: the request read deadline stays 30 s, the
response write deadline 10 s, and a JavaScript plugin call is still capped at 10 s or the
evaluation deadline, whichever is sooner ([ADR-0017](../adr/0017-quickjs-resource-limits.md) §3).
With the 60 s default a looping plugin therefore stops at 10 s with
`JS function NAME timed out (max 10 s)`.

## Server errors and response size

Limit errors are ordinary evaluation errors. `sk_http_server` answers HTTP **400** with
`success: false` and the error text; `sk_server` answers `success: false` with the same text.
The examples below were captured with `--max-eval-bytes 256M`.

```console
POST /eval {"expression": "(((…129 levels…1)))"}
400 {"success":false,"result":null,"error":"Expression nested too deeply (max 128) at position 129","execution_time_ms":0.117167,"request_id":0}

POST /eval {"expression": "[1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1,1].reduce([:acc, :acc], 0)"}
400 {"success":false,"result":null,"error":"Evaluation budget exceeded (max 256 MB)","execution_time_ms":79.39325,"request_id":1}
```

JSON escaping can make a response up to 6 times larger than the value (a control character such as
`\x01` becomes `\u0001`). Before building a response, both servers compute an upper bound on its
size. If it is over the budget, they return the budget error instead of the result. A result just
under the budget can be rejected slightly early, because the bound is conservative.

```console
POST /eval {"expression": "SUBSTITUTE(SUBSTITUTE(\"aaaaaaaaaa\", \"a\", :h), \"a\", :c).length()", "arguments": {"h": "a…(100)", "c": "\u0001…(46,000)"}}
200 {"success":true,"result":46000000.0,"error":null,"execution_time_ms":19.080208000000002,"request_id":2}

POST /eval {"expression": "SUBSTITUTE(SUBSTITUTE(\"aaaaaaaaaa\", \"a\", :h), \"a\", :c)", "arguments": {"h": "a…(100)", "c": "\u0001…(46,000)"}}
400 {"success":false,"result":null,"error":"Evaluation budget exceeded (max 256 MB)","execution_time_ms":13.677583,"request_id":3}
```

The 46 MB string fits the 256 MiB budget, but its escaped JSON (276 MB) does not. With the 1 GiB
default both requests succeed.

### Result cache

`sk_http_server` caches results in an LRU cache. An entry is stored only when its key, result and
returned variables together cost at most 64 KiB (same cost model as the budget), so the cache stays
small however big the results are. Errors, including budget and timeout errors, are never cached.

### Parse cache

Both servers also keep the parsed form of recent expressions, one cache per worker thread, keyed
by the exact expression text: at most 256 expressions and 128 KiB of expression text per thread,
and an expression longer than 32 KiB is not stored. A parsed expression takes 31-76 times the bytes
of its text, so one worker holds at most ~10 MiB of parsed expressions. Parse errors, including the
nesting-depth error, are never cached. Details: [servers](servers.md#parse-cache) and
[ADR-0021](../adr/0021-parsed-expression-api-and-server-cache.md).

### Sizing a server

Every worker thread can run one evaluation at the full budget at the same time, so the values the
budget counts can reach `threads × max-eval-bytes`: **8 threads × 1 GiB = 8 GiB** with the default
budget. Live memory per evaluation is at most about twice the budget (the budget plus the value
that crossed it), so the heap can peak near `threads × 2 × max-eval-bytes`. Size the host for that,
or lower `--threads` or `--max-eval-bytes`. The Docker image runs 2 threads by default (`SK_THREADS`).

A JavaScript plugin adds at most 64 MiB per call on top of that (see the next section), so the
worst case is about `threads × (2 × max-eval-bytes + 64 MB)`.

## JavaScript plugins (64 MiB, 10 s, 256 KiB)

Each call to a JavaScript plugin runs in a fresh QuickJS runtime with its own limits
([ADR-0017](../adr/0017-quickjs-resource-limits.md)). They apply in the library, the CLI, both
servers, `/reload-hooks` and the `@example` check of `/upload-js` and `/update-js`. They cannot be
changed.

| Limit | Value | What happens |
|---|---|---|
| Heap | 64 MiB per call (arguments converted into JS count too) | `JS function NAME ran out of memory (max 64 MB)` |
| Time | 10 s per call | `JS function NAME timed out (max 10 s)` |
| Time, inside a server request | the evaluation deadline, if it comes first (only with `--eval-timeout` below 10) | `Evaluation timed out (max N s)` |
| Stack | 256 KiB | `JS function NAME failed: RangeError: Maximum call stack size exceeded` |
| Result depth | 128 nested arrays; cyclic arrays hit it too | `Expression nested too deeply (max 128)` |

`NAME` is the plugin's `@name`. The time limit cannot be caught inside JavaScript: a loop wrapped
in `try`/`catch` is stopped the same way. When the request deadline stops a plugin, the request
fails with the timeout error even if the formula would have ignored the plugin's error. Any other
exception is reported with its own text, `JS function NAME failed: <exception>`. All of these are
ordinary evaluation errors (HTTP 400, `success: false` on the servers).

Release CLI, with test plugins in `SKILLET_HOOKS_DIR`:

```console
$ sk "=BIG()"          # return "x".repeat(4e8).length;
Error: JS function BIG ran out of memory (max 64 MB)

$ sk "=SPIN()"         # while (true) {}   (returns after 10 s)
Error: JS function SPIN timed out (max 10 s)

$ sk "=RECURSE()"      # function f(n) { return f(n + 1) + 1; }
Error: JS function RECURSE failed: RangeError: Maximum call stack size exceeded

$ sk "=CYCLE()"        # var a = [1]; a.push(a); return a;
Error: Expression nested too deeply (max 128)
```

Plugins have no host functions: no file system, no network, no processes. The undocumented
`httpGet(url)` global was removed (ADR-0017 §6); a plugin that calls it now fails with
`JS function NAME failed: ReferenceError: httpGet is not defined`.

## Server request limits (1 MB, 30 s, 10 s)

Both servers limit how much a client can send and how long it can take, before any evaluation
starts. The values are constants in `src/bin/http_server/request_limits.rs`; there is no flag or
environment variable to change them.

| Limit | Value | `sk_http_server` | `sk_server` |
|---|---|---|---|
| Request size | 1 MB (1,048,576 bytes) | headers + body; a `Content-Length` over 1 MB is rejected before the body is read | one request line, without the trailing newline |
| Read deadline | 30 s | for the whole request, from the first read | from when the connection opened or the last real request was answered |
| Write deadline | 10 s | for the whole response, from its first write | for the whole response line, from its first write |

The read deadline is a total, not a per-read timeout, so a client that sends one byte at a time
cannot hold a worker thread for longer than 30 s (BL-033, BL-037).

`sk_http_server` answers the size limit with HTTP status **413** and the read deadline with
**408**, on any path, with a JSON body whose `error` names the condition (BL-049; before 0.7.0
both were 400). It then reads and discards the rest of the request until the client closes or
the 30 s pass, so the client receives the error instead of a connection reset (output of a
debug build of this change):

```console
POST /eval  (body: {"expression": "1", "arguments": {"s": "x…(1,100,000)"}})
413 {"error":"Request body too large (max 1MB)","success":false}

POST /eval  (headers say Content-Length: 100, then 6 bytes, then nothing)
… 30.0 s later …
408 {"error":"Request read timed out (max 30 s)","success":false}
```

Headers over 1 MB without the closing blank line get 413 with `Request too large (max 1MB)`.

`sk_server` answers with an ordinary error response and then closes the connection, because the
rest of the stream can no longer be split into lines:

```console
{"expression": "1", "variables": {"s": "x…(1,100,000)"}}
{"success":false,"result":null,"error":"Request too large (max 1MB)","execution_time_ms":0.0,"request_id":0}

{"expression": "1        (no newline, then nothing)
… 30.0 s later …
{"success":false,"result":null,"error":"Request read timed out (max 30 s)","execution_time_ms":0.0,"request_id":1}
```

An `sk_server` connection that gets no new request within 30 s is closed without a response. The
30 s count from when the connection opened, or from the end of the last real, authorized request
(BL-046). Clients that keep connections open (see the pooling example in [servers](servers.md))
must reconnect after that.

Only real requests restart that count. Blank and whitespace-only lines are ignored, and invalid
JSON gets an `Invalid JSON request: …` error, but neither keeps a connection open past 30 s.
After a wrong or missing token the server answers `Unauthorized: invalid token` and closes the
connection; the client must reconnect to try again. See
[servers](servers.md#connections-and-authentication) for captured output.

### Response write deadline (10 s)

Writing one response may take at most 10 s in total, on both servers (BL-047). The deadline counts
from the first byte written, not per socket write. A client that reads more slowly than that gets a
truncated response and no error message:

- `sk_http_server`: the body is shorter than its `Content-Length` header, then the connection
  closes.
- `sk_server`: the JSON line has no trailing newline, then the connection closes.

A fast client is not affected. Measured with a release build of `develop` at 21a37cc on
127.0.0.1, a 20 MB result (`SUBSTITUTE(SUBSTITUTE("aaaaaaaaaa", "a", :h), "a", :c)` with `h` =
100 × `a` and `c` = 20,000 × `x`), read at full speed and then limited to 1 MB/s:

```console
$ curl -s -o /dev/null -w 'HTTP %{http_code} size_download=%{size_download}\n' … /eval
HTTP 200 size_download=20000085

$ curl -s -D slow.hdr -o slow.body --limit-rate 1M -w '… exit=%{exitcode} %{errormsg}\n' … /eval
HTTP 200 size_download=11886592 exit=18 end of response with 8113493 bytes missing
$ grep -i content-length slow.hdr
Content-Length: 20000085

sk_server, same expression, full speed:  20000084 bytes in 0.0s, ends with newline: True
sk_server, same expression, 1 MB/s:      9310234 bytes in 11.0s, ends with newline: False
```

The slow client received more than 10 MB because the operating system's socket buffers hold data
the server already wrote.

What this means for sizing: a response can be as large as the evaluation budget (1 GiB by
default), and sending 1 GiB within 10 s needs about 100 MB/s (256 MiB: about 25 MB/s). Clients on slower links, or results in the tens of
megabytes over a WAN, can hit the deadline. Check that a response is complete (HTTP: body length
equals `Content-Length`; `sk_server`: the line ends with `\n`) and keep large results small, for
example by returning `.length()`, a sum or `LEFT(text, n)` instead of the whole value.

## Library API: `EvalLimits` and `with_limits`

Library calls (`skillet::evaluate*`) use the default budget and no deadline. To change them, run the
evaluation inside `with_limits`:

```rust
use std::time::Duration;
use skillet::{evaluate, evaluate_with_json, with_limits, EvalLimits};

// A deadline, as the servers use (they default to 60 s and a 1 GiB budget).
let mut limits = EvalLimits::default(); // max_bytes = MAX_EVAL_BYTES, timeout = None
limits.timeout = Some(Duration::from_secs(10));
let r = with_limits(limits, || evaluate_with_json(":nums.sum()", r#"{"nums": [1, 2, 3]}"#));
// Ok(Number(6.0))

// A smaller budget.
let mut small = EvalLimits::default();
small.max_bytes = 160;
with_limits(small, || evaluate("[1, 2, 3, 4]"));    // Ok(Array([1, 2, 3, 4])), cost 160
with_limits(small, || evaluate("[1, 2, 3, 4, 5]")); // Err("Evaluation budget exceeded (max 160 bytes)")
```

Notes (all checked against the code):

- `EvalLimits` is `#[non_exhaustive]`: start from `EvalLimits::default()` and set the fields.
- Error texts show the active limit: `max N MB` when it is a whole number of MiB, otherwise
  `max N bytes`; `max N s` for a whole number of seconds, otherwise `max N ms` (for example
  `Evaluation timed out (max 1 ms)`).
- **One scope is one budget.** Every evaluation inside the same `with_limits` call shares it. With
  `max_bytes = 160`, `with_limits(small, || (evaluate("[1, 2]"), evaluate("[1, 2]")))` returns
  `Ok` for the first and `Evaluation budget exceeded (max 160 bytes)` for the second. Call
  `with_limits` once per evaluation if each one should get its own budget.
- Scopes nest, but an inner scope can only **tighten**: it gets the smaller budget (compared with
  what is left of the outer one) and the earlier deadline. A custom function or plugin cannot raise
  the limits of the evaluation that called it.
- A custom function that calls `skillet::evaluate` joins the caller's budget; it cannot reset it.
- The budget and the deadline are thread-local. Work a custom function moves to another thread
  (for example with `std::thread::spawn`, a thread pool, or an async runtime) is outside them: it is
  not charged, the deadline does not stop it, and a `skillet::evaluate` call made on that other
  thread opens a new default scope (256 MiB, no deadline) instead of joining the caller's. If a
  custom function must use other threads, it has to bound that work itself.
- A custom function's own Rust code is not metered either, on any thread. Only the values it
  returns are charged, and the deadline is checked only after it returns (same as a long builtin,
  see [Server deadline](#server-deadline-60-s-by-default)).
- A limit error is reported even if the formula swallowed it (for example inside `SUMIF`).

## Changes in 0.7.0

- Added: `MAX_NESTING_DEPTH`, `MAX_EVAL_BYTES`, `EvalLimits`, `with_limits`.
- Changed: very deep expressions and runtime arrays deeper than 128 now return an error (before,
  they could crash the process with a stack overflow).
- Changed: quadratic `reduce(MERGE(:acc, [:x]), [])` fails above about 2,800 items, and copying a
  large variable once per item (`:rows.map(SUM(:rows))`) can hit the budget; hoist it.
- Changed (servers): a deadline per evaluation (60 s by default, `--eval-timeout`), a 1 GiB budget
  by default (`--max-eval-bytes`), oversized responses are rejected with the budget error, and
  cache entries are capped at 64 KiB.
