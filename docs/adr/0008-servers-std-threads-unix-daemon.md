# 0008. Servers on `std::net` + thread pool; Unix-only daemon

- Status: Accepted (retroactive, v0.6.3)
- Date: 2026-11-20
- Related: BL-012, BL-013, PRD-002

## Context
Non-Rust callers (Ruby, Node, shell) need to evaluate expressions without per-call process spawn.

## Decision
- `sk_server` (line-based TCP, JSON) and `sk_http_server` (HTTP/1.1) are hand-written on
  `std::net::TcpListener` with a `threadpool::ThreadPool`; no async runtime, no HTTP framework.
- HTTP server adds token auth, an LRU result cache (`lru`), stats, OpenAPI docs page (`scalar-doc`),
  and JS management endpoints (`openapi.yml`).
- `--daemon` uses `libc::fork`/`setsid`, which is why Windows builds were dropped (commit `169e007`).

## Consequences
- + Tiny dependency tree, predictable latency, easy static/musl builds (Alpine).
- − Hand-rolled HTTP parsing is an attack surface and lacks keep-alive/HTTP2 robustness.
- − Unix-only until daemon code is `cfg(unix)`-gated (BL-013).

## Alternatives considered
- `tokio` + `axum`/`hyper` — robust HTTP, but large dependency and async contagion; revisit if
  HTTP correctness/security issues appear.
