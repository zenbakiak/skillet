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
5. Human merges `release/X.Y.Z` → `main` and merges back into `develop`, then pushes the one tag
   `vX.Y.Z` on `main` (ADR-0022). It triggers `release.yml`, which fails before publishing
   anything unless the tag equals `v` + the `Cargo.toml` version and `CHANGELOG.md` has a
   non-empty `## [X.Y.Z]` section. It then builds the binaries, creates the GitHub release with
   that section as its body, and publishes the crate last. Check the notes beforehand with
   `scripts/changelog_section.sh X.Y.Z`.
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
- Toolchain (ADR-0012): MSRV 1.85 (`rust-version`, checked by the `msrv` job). The CI gate
  (`rust.yml`) and `release.yml` pin Rust 1.89.0 via `RUST_TOOLCHAIN`; bump both together.
  `.tool-versions` is gitignored, local only.
- CI (`rust.yml`) runs `./harness/check.sh`, `cargo audit` and the MSRV build on pushes and PRs to
  `main` and `develop`, all `--locked`.
- Release (ADR-0022): one tag `vX.Y.Z`, one workflow `release.yml`: `verify` (tag = `Cargo.toml`
  version, CHANGELOG section present) → `build-binaries` + `crate-check` → `github-release`
  (body from `scripts/changelog_section.sh`) → `crates-io`. All `--locked`. `workflow_dispatch`
  is a dry run (verify + builds, no publishing). There is no `crate-v*` tag any more.
- `release.yml` writes `<archive>.tar.gz.sha256` next to each archive in the build job;
  the `github-release` job verifies every checksum before uploading.
- `Cargo.toml` `include`: `Cargo.toml`, README, CHANGELOG, licences, `docs/guide/`, `docs/reference/`,
  `openapi.yml`, `src/`, `tests/`. `repository` is set; no `homepage`.
- `CHANGELOG.md` exists at the root (Keep a Changelog).
- Git hooks exist (`.githooks/`, ADR-0009), but GitHub branch protection on `main`/`develop`
  (required PR + required CI check) is not set yet → BL-022.
- `cross` is pinned (ADR-0022): `CROSS_REV` commit in `release.yml`, installed `--locked`, and the
  musl images pinned by digest in `Cross.toml`. Bump the commit and the digests together.
- Actions are pinned by tag (`@v4`, `@v2`, `dtolnay/rust-toolchain@master`), not by SHA (BL-056
  follow-up).

## Hand off
Use the handoff template in `harness/roles/orchestrator.md`; include `cargo package --list` diff
for packaging changes.
