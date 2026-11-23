# Skillet documentation index

One topic, one home. Start with the [README](../README.md) for the pitch, install and examples.

## Guide (`docs/guide/`)
- [User guide](guide/user-guide.md) — CLI, Rust library, JSON, plugins, language walkthrough
- [Installation](guide/installation.md) — prebuilt binaries, install script, build from source
- [Servers](guide/servers.md) — `sk_server` / `sk_client` (TCP) usage and clients in other languages
- [Limits](guide/limits.md) — nesting depth, evaluation budget, server deadline, `with_limits`
- [Migrating to 0.7](guide/migration-0.7.md) — every breaking change in 0.7.0, before and after

## Reference (`docs/reference/`)
- [Functions, methods and operators](reference/api.md)
- [Builtin functions](reference/functions.md) — generated from the builtin table in code (BL-010)
- HTTP API: [`openapi.yml`](../openapi.yml) (also served by `sk_http_server` at `/openapi.yml`)

## Operations (`docs/ops/`)
- [Docker](ops/docker.md) — container deployment
- [Alpine / musl](ops/alpine.md) — static builds and adding Skillet to Alpine images
- [Binaries](ops/binaries.md) — packaging and distributing release binaries, systemd

## Security (`docs/security/`)
- [Threat model](security/THREAT_MODEL.md) — surfaces, limits that hold, known gaps, how to report a vulnerability

## Decisions and requirements
- [ADRs](adr/README.md) — architecture decisions
- [PRDs](prd/README.md) — product requirements
- [Development guide](DEVELOPMENT_GUIDE.md) — architecture and contributor guide

## Where the old root files went (BL-009)

| Old path | New path |
|---|---|
| `DOCUMENTATION.md` | [`docs/guide/user-guide.md`](guide/user-guide.md) |
| `API_REFERENCE.md` | [`docs/reference/api.md`](reference/api.md) |
| `SERVER_USAGE_GUIDE.md` | [`docs/guide/servers.md`](guide/servers.md) |
| `INSTALLATION.md` | [`docs/guide/installation.md`](guide/installation.md) |
| `DOCKER_DEPLOYMENT_GUIDE.md` | [`docs/ops/docker.md`](ops/docker.md) |
| `BINARY_DISTRIBUTION_GUIDE.md` | [`docs/ops/binaries.md`](ops/binaries.md) |
| `ALPINE_INTEGRATION_GUIDE.md`, `ALPINE_BUILD_README.md`, `QUICK_ALPINE_SETUP.md` | [`docs/ops/alpine.md`](ops/alpine.md) (merged) |
| `SETUP_COMPLETE.md` | removed (BL-015); still-useful content is in [`docs/ops/alpine.md`](ops/alpine.md) |

Root keeps only `README.md`, `AGENTS.md`, `CLAUDE.md`, `CHANGELOG.md` and the licence files.
