# 0025. Optional limit on concurrent connections per IP address

- Status: Accepted
- Date: 2026-11-21
- Deciders: Maintainer
- Related: BL-066, BL-057, threat model (residual risk: fixed worker pool)

## Context
Both servers serve each connection on one thread of a fixed pool. One client that opens as many
idle connections as there are workers holds the whole pool until the 30 s read deadline, and can
then do it again. The request limits (ADR-0011, BL-033, BL-037) bound how long one connection
holds a worker, not how many connections one client holds.

## Decision
- `sk_http_server` and `sk_server` take `--max-connections-per-ip N` and
  `SKILLET_MAX_CONNECTIONS_PER_IP`. The flag wins over the variable. `0`, an empty value or
  non-numeric text is a start-up error (exit 1), like an empty token.
- **Not set means no limit** (the behaviour before this ADR).
- Open connections are counted per peer IP address (`IpAddr`; the port is ignored; an
  IPv4-mapped IPv6 address counts as its IPv4 address) in one `Mutex<HashMap<IpAddr, usize>>`.
  The count goes up in the accept loop, before the connection is queued for a worker, and down
  through a guard dropped when the connection closes, so a panic or early return cannot leak a
  slot. Entries at zero are removed, so the map holds at most one entry per open socket.
- Over the limit, the accept thread writes the rejection with a 500 ms write timeout and closes
  the connection; it never reaches a worker. `sk_http_server` answers
  `HTTP/1.1 429 Too Many Requests` with `Connection: close` and
  `{"success":false,"error":"Too many connections from this address (max N)"}`; `sk_server`
  writes the same JSON as one line.
- The code lives in `src/bin/http_server/conn_limit.rs`, included by `sk_server` with `#[path]`.

## Consequences
- Operators who expose a server directly can stop one address from taking the whole pool.
- Behind a reverse proxy or load balancer every client has the proxy's address, so a per-IP
  limit in the server would limit all clients together. Set the limit at the proxy instead and
  leave this setting unset (or set it above the proxy's own connection count).
- Many addresses (a botnet, or IPv6 clients rotating addresses) are not stopped by this; a
  firewall or proxy is still the place for that.
- No new dependency; one short lock per accepted and per closed connection.
