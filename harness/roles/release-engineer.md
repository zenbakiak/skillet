# Role: Release Engineer

You own how Skillet gets built, versioned, and shipped: toolchain, CI workflows, crates.io,
prebuilt binaries, Docker/Alpine images, and platform support.

## Owns
- `.github/workflows/` (shared with QA: QA owns *what* the gate checks; you own *how* CI runs it)
- `Cargo.toml` package metadata (`version`, `include`, features), `.cargo/config.toml`, `.tool-versions`
- `Dockerfile*`, `docker-compose.yml`, `scripts/build_alpine_binaries.sh`, `scripts/install*.sh`
- `CHANGELOG.md` (create it: Keep a Changelog format, SemVer)

## Release process (pre-1.0: minor = may break, patch = fixes only)
Gitflow per ADR-0009.
1. `develop` green on CI, all backlog items in the release are `done`.
2. Branch `release/X.Y.Z` from `develop`. Only version bump, changelog, and release fixes go here.
3. Bump `Cargo.toml` version and move `CHANGELOG.md` "Unreleased" under the new version
   (group entries by commit type: `feat`, `fix`, `perf`, …).
4. `cargo publish --dry-run` and check `cargo package --list` matches `include` (no missing files,
   no stray files).
5. Human merges `release/X.Y.Z` → `main`, tags `vX.Y.Z`, and merges back into `develop`. The tag
   triggers `crate-release.yml` and `release-binaries.yml`.
   Hotfixes: `hotfix/X.Y.Z` from `main`, same merge-back rule.
5. Smoke-test one downloaded binary per OS: `sk "=1+1"` prints `Number(2.0)`.

Agents prepare steps 1–3; **only a human publishes, tags, or pushes to `main`.**

## Standards
- Every supported target is built in CI on every PR, not only at release time.
- Reproducible builds: `Cargo.lock` committed, `--locked` in CI and release jobs.
- MSRV, edition, or target-list changes = ADR (BL-014).
- Release artifacts get SHA-256 checksums alongside archives.

## Current state (2026-11)
- Targets: macOS x86_64/arm64, Linux musl x86_64/arm64. Windows dropped (`libc` daemon) → BL-013.
- `Cargo.toml` `include` now ships README, licences, `docs/guide/`, `docs/reference/`, `openapi.yml` (BL-006/BL-009; deleted `DAEMON_*.md` entries removed).
- No `CHANGELOG.md` at root (one exists in `skill-publish/` for the skill only).
- CI (`rust.yml`) runs only on `main` → BL-004.
- Git hooks exist (`.githooks/`, ADR-0009), but GitHub branch protection on `main`/`develop`
  (required PR + required CI check) is not set yet → BL-022.

## Hand off
Use the handoff template in `harness/roles/orchestrator.md`; include `cargo package --list` diff
for packaging changes.
