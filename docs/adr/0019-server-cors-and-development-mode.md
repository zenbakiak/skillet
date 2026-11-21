# 0019. Server CORS and development mode

- Status: Accepted
- Date: 2026-11-21
- Deciders: Maintainer
- Related: BL-051, BL-048, BL-052, threat model §2.3, ADR-0008

## Context
`sk_http_server` sent `Access-Control-Allow-Origin: *` on every response and parsed request
bodies as JSON whatever their `Content-Type`. Without tokens ("development mode") every
endpoint is open, the JavaScript upload included. A page in the operator's browser could
therefore send "simple" cross-origin requests (`text/plain` or form bodies, no preflight) to
`127.0.0.1`, upload a hook, reload hooks, and read evaluation results. The admin token was only
settable by flag, so containers had to put it on the command line.

## Decision
- `SKILLET_ADMIN_TOKEN` sets the admin token, with the rules of `SKILLET_AUTH_TOKEN`: the
  `--admin-token` flag wins, an empty value is a start-up error. Both names are a contract
  (the Docker image passes tokens through them).
- `--cors-origin <origin>` / `SKILLET_CORS_ORIGIN` (flag wins; empty or containing whitespace
  or control characters is a start-up error) sets `Access-Control-Allow-Origin` exactly, `*`
  included. Without it the header is `*` when a token is configured and absent in development
  mode. Preflight `OPTIONS` follows the same rule and allows the `Authorization` and
  `Content-Type` headers.
- A request with a body (not `GET`/`HEAD`/`OPTIONS`, `Content-Length` over 0) must be
  `application/json` (parameters allowed, case-insensitive), otherwise HTTP 415, checked before
  dispatch. `/upload-js` and `/update-js` also accept `multipart/form-data`, but not from a
  browser (an `Origin` header) while no admin token is configured, because browsers send
  multipart without a preflight.

## Consequences
- A cross-origin page needs a preflight for every body request, and in development mode the
  preflight fails, so it can neither post JSON nor read responses.
- With a token, `*` stays the default: requests still need the token, which a page on another
  origin does not have. Operators who want a narrower policy set `--cors-origin`.
- Breaking for clients that post JSON without `Content-Type: application/json` (for example
  `curl -d` without `-H`); they now get 415. The bundled clients, benches, tests and docs send
  the header.
- Left open: body-less simple requests still reach the server in development mode (for
  example `POST /reload-hooks` with no body, `GET /eval`), though their responses cannot be
  read. Development mode remains meant for a trusted machine only.

## Alternatives considered
- Keep `*` and rely on tokens — leaves development mode drivable by any page.
- Reflect the request `Origin` — same exposure as `*`, plus credentialed-request pitfalls.
- Reject every request with an `Origin` header in development mode — breaks same-origin pages
  and tools that send `Origin`; the Content-Type rule closes the hole with less breakage.
- Require a token always — removes the zero-configuration local mode the maintainer keeps.
