# PRD-002. Embedding, CLI and servers

- Status: Baseline (retroactive — describes v0.6.3 behavior)
- Owner: Orchestrator
- Related: ADR-0007, ADR-0008, `openapi.yml`, `SERVER_USAGE_GUIDE.md`

## Problem
Skillet must be usable from Rust directly and from other runtimes (Ruby/Rails, Node, shell)
with low per-evaluation overhead.

## Requirements
| ID | Requirement | Acceptance |
|---|---|---|
| FR-1 | Library API: `parse`, `evaluate`, `evaluate_with`, `evaluate_with_json`, `*_custom`, `*_assignments*` | `src/lib.rs`, `tests/*` |
| FR-2 | Custom Rust functions via `CustomFunction` + global registry | `tests/custom_functions.rs` |
| FR-3 | JS functions loaded from `SKILLET_HOOKS_DIR` (default `hooks/`) | `hook_examples/rust_tests/` |
| FR-4 | CLI `sk "<expr>" [:k=v ...]`, `--json` vars, JSON output | `tests/cli_integration.rs` |
| FR-5 | TCP server `sk_server` + `sk_client` with thread pool | `tests/sk_concurrency.rs` |
| FR-6 | HTTP server: `GET/POST /eval`, `/health`, `/` (docs), `/cache`, JS mgmt (`/upload-js`, `/update-js`, `/delete-js`, `/list-js`, `/reload-hooks`) | `openapi.yml`, `tests/http_server_jsonpath.rs` |
| FR-7 | Token auth for eval; separate admin token for JS management | `src/bin/http_server/auth.rs` |
| FR-8 | `--daemon` mode with PID file (Unix) | `src/bin/http_server/daemon.rs` |
| FR-9 | Distribution: crates.io, prebuilt Linux/macOS binaries, Alpine/musl, Docker | `.github/workflows/*`, `Dockerfile*` |

## Non-functional requirements
| ID | Requirement |
|---|---|
| NFR-1 | HTTP p50 latency and throughput tracked in `benches/BASELINE.md` (BL-005) |
| NFR-2 | Request size limits and auth enforced before parsing/eval |
| NFR-3 | No secrets logged; tokens only via env/flags |

## Open questions
- Windows support (BL-013).
- Keep hand-rolled HTTP vs adopt a framework (ADR-0008 alternatives).
