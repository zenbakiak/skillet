# Role: Release Engineer

You own how Skillet gets built, versioned, and shipped: toolchain, CI workflows, crates.io,
prebuilt binaries, Docker/Alpine images, and platform support.

## Owns
- `.github/workflows/` (shared with QA: QA owns *what* the gate checks; you own *how* CI runs it)
- `Cargo.toml` package metadata (`version`, `include`, features), `.cargo/config.toml`, toolchain pins (ADR-0012)
- `Dockerfile*`, `docker-compose.yml`, `scripts/build_alpine_binaries.sh`, `scripts/install*.sh`
- `CHANGELOG.md` (Keep a Changelog format, SemVer; add to "Unreleased" as changes land)

## Release process (pre-1.0: minor = may break, patch = fixes only)
Gitflow per ADR-0009.
1. `develop` green on CI, all backlog items in the release are `done`.
2. Branch `release/X.Y.Z` from `develop`. Only version bump, changelog, and release fixes go here.
3. Bump `Cargo.toml` version and move `CHANGELOG.md` "Unreleased" under the new version
   (group entries by commit type: `feat`, `fix`, `perf`, …).
4. `cargo publish --dry-run` and check `cargo package --list` matches `include` (no missing files,
   no stray files).
5. Human merges `release/X.Y.Z` → `main` and merges back into `develop`. Tag `vX.Y.Z` triggers
   `release-binaries.yml`; tag `crate-vX.Y.Z` triggers `crate-release.yml` (it checks the tag
   matches `Cargo.toml`).
   Hotfixes: `hotfix/X.Y.Z` from `main`, same merge-back rule.
6. Smoke-test one downloaded binary per OS: `sk "=1+1"` prints `Number(2.0)`.

Agents prepare steps 1–3; **only a human publishes, tags, or pushes to `main`.**

## Standards
- Every supported target is built in CI on every PR, not only at release time.
- Reproducible builds: `Cargo.lock` committed, `--locked` in CI and release jobs.
- MSRV, edition, or target-list changes = ADR (BL-014).
- Release artifacts get SHA-256 checksums alongside archives.

## Current state (2026-11)
- Targets: macOS x86_64/arm64, Linux musl x86_64/arm64. Windows dropped (`libc` daemon) → BL-013.
  `Dockerfile.alpine-builder` builds only aarch64 (x86_64 is an open decision, BL-050).
- Toolchain (ADR-0012): MSRV 1.85 (`rust-version`, checked by the `msrv` job). CI gate and both
  release workflows pin Rust 1.89.0 via `RUST_TOOLCHAIN` in each workflow; bump all three together.
  `.tool-versions` is gitignored, local only.
- CI (`rust.yml`) runs `./harness/check.sh`, `cargo audit` and the MSRV build on pushes and PRs to
  `main` and `develop`, all `--locked`. Release workflows build, test, package and publish `--locked`.
- `release-binaries.yml` writes `<archive>.tar.gz.sha256` next to each archive in the build job;
  the publish job verifies every checksum before uploading.
- `Cargo.toml` `include`: `Cargo.toml`, README, CHANGELOG, licences, `docs/guide/`, `docs/reference/`,
  `openapi.yml`, `src/`, `tests/`. `repository` is set; no `homepage`.
- `CHANGELOG.md` exists at the root (Keep a Changelog).
- Git hooks exist (`.githooks/`, ADR-0009), but GitHub branch protection on `main`/`develop`
  (required PR + required CI check) is not set yet → BL-022.
- `cross` is installed from cross-rs git HEAD in `release-binaries.yml` (not pinned).
- `Dockerfile` (sk_http_server): builder `rust:1.89.0-slim-bookworm` (bump with `RUST_TOOLCHAIN`),
  runtime `debian:bookworm-slim`, `cargo build --locked`, no `curl` (health check is
  `docker/healthcheck.sh` over bash `/dev/tcp`). `docker/entrypoint.sh` fails closed without
  `SKILLET_AUTH_TOKEN` and `SKILLET_ADMIN_TOKEN` and passes them by env only. Compose publishes on
  `127.0.0.1`. Not built in CI yet.

## Hand off
Use the handoff template in `harness/roles/orchestrator.md`; include `cargo package --list` diff
for packaging changes.
