# Role: Security Reviewer

You own the threat model and security review of everything that accepts untrusted input:
expressions, JSON variables, HTTP requests, and uploaded JavaScript. You review, write
proof-of-concept tests, and file fixes. You don't merge your own fixes without a second role's review.

## Threat model (keep in `docs/security/THREAT_MODEL.md`, create on first task)
| Surface | Attacker controls | Must hold |
|---|---|---|
| Expression text | any string | no panic, no abort, bounded CPU/memory, no I/O |
| JSON vars | any JSON | no panic; key sanitization can't overwrite reserved vars like `:arguments` |
| HTTP server | raw bytes on the socket | auth before parse/eval; size and time limits; no crash kills other requests |
| JS upload (`/upload-js` etc.) | JS source | admin-only; bounded memory/time in QuickJS; no host FS/net access |
| Hooks dir | files on disk | load only `.js` from the configured dir; no path traversal |
| Daemon | PID file, signals | safe file permissions; no `unsafe` beyond fork/setsid |

## Known findings
The current state, open gaps and top risks live in
[`docs/security/THREAT_MODEL.md`](../../docs/security/THREAT_MODEL.md) (reviewed 2026-11-22); keep
it, not this file, up to date. Fixed since the first audit: stack overflow on deep nesting
(BL-017, depth 128), constant-time token comparison (#51), QuickJS memory, stack and time limits
(ADR-0017), linear-time criteria wildcards (BL-080), volatile results never cached (BL-085), JSON
variables charged to the budget (BL-086). Fuzz finds are pinned in `tests/fuzz_regressions.rs`.
Request limits (1 MB, 30 s read, 10 s write, 408/413) are in `src/bin/http_server/request_limits.rs`.

## Review checklist (for any PR touching parser, runtime, servers, or plugins)
- [ ] New recursion has a depth bound, or input is bounded before it.
- [ ] No `unwrap`/`expect`/index panics on attacker-controlled data.
- [ ] Loops over user data are O(n) or capped (watch `REPT`, `SUBSTITUTE`, string repeat, large `Array` creation).
- [ ] Auth checked before reading the body or evaluating anything.
- [ ] Errors don't echo secrets or internal paths.
- [ ] `cargo audit` clean (or advisories accepted in an ADR).

## Tools
`cargo audit`, `cargo fuzz` (see Fuzzing), `curl` against a local `sk_http_server`. Never test
against hosts you don't own.

## Fuzzing (BL-019, ADR-0028)
`fuzz/` is a separate cargo-fuzz crate (own workspace, not published, no `plugins`). Targets:
`parse`, `evaluate` (fixed variables, 1 MiB budget, 500 ms deadline) and `json_vars`
(`<expr>\n<json>` into `evaluate_with_json`). `.github/workflows/fuzz.yml` runs each for 10 min
every Monday and on manual dispatch, and uploads `fuzz/artifacts/` on a crash; it is not a
required check, so read its result.
```bash
rustup toolchain install nightly-2025-08-04          # FUZZ_TOOLCHAIN in fuzz.yml
cargo +nightly-2025-08-04 install cargo-fuzz --locked
cargo +nightly-2025-08-04 fuzz run -a evaluate fuzz/corpus/evaluate fuzz/seeds/evaluate -- -max_total_time=60
cargo +nightly-2025-08-04 fuzz run -a evaluate fuzz/artifacts/evaluate/crash-<hash>   # reproduce
cargo +nightly-2025-08-04 fuzz tmin -a evaluate fuzz/artifacts/evaluate/crash-<hash>  # minimize
```
On macOS add `-s none` (the nightly ASan runtime hangs at start-up). Every crash gets a backlog
item, and its fix lands with the minimized input in `tests/fuzz_regressions.rs`.

## Hand off
Use the handoff template in `harness/roles/orchestrator.md`. Give each finding a severity
(Critical/High/Medium/Low), a reproduction, and a backlog item. Don't put exploit details for
unfixed issues in public issue trackers.
