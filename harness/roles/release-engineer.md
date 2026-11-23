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
5. Human opens the PR `release/X.Y.Z` → `main`. That PR is the release dry run: `release.yml`
   runs `verify`, every binary target and `crate-check` (test, `cargo package`,
   `cargo publish --dry-run`) on it, next to the CI gate. Nothing is published from a PR.
   Merge it with a merge commit (the `main` ruleset allows nothing else).
6. Human pushes the one tag `vX.Y.Z` on `main` (ADR-0022; the `v*` tag ruleset lets only the
   admin create it). It triggers `release.yml`, which fails before publishing anything unless the
   tag equals `v` + the `Cargo.toml` version and `CHANGELOG.md` has a non-empty `## [X.Y.Z]`
   section. It then builds the binaries and creates the GitHub release with that section as its
   body. The `crates-io` job then waits in the protected `crates-io` environment: the maintainer
   approves the deployment (Actions run → Review deployments) and only then is the crate
   published. Check the notes beforehand with `scripts/changelog_section.sh X.Y.Z`.
7. Human merges `main` back into `develop` through a PR with "Create a merge commit" (the only PR
   into `develop` that is not squashed).
   Hotfixes: `hotfix/X.Y.Z` from `main`, same PR dry run, tag, approval and merge-back.
8. Smoke-test one downloaded binary per OS: `sk "=1+1"` prints `Number(2.0)`.

Agents prepare steps 1–4; **only a human publishes, tags, approves the deployment, or merges
into `main`.**

## Standards
- Every supported target is built in CI on every PR, not only at release time.
- Reproducible builds: `Cargo.lock` committed, `--locked` in CI and release jobs.
- MSRV, edition, or target-list changes = ADR (BL-014).
- Release artifacts get SHA-256 checksums alongside archives.

## Current state (2026-11)
- Targets: macOS x86_64/arm64, Linux musl x86_64/arm64. Windows dropped (`libc` daemon) → BL-013.
  `Dockerfile.alpine-builder` takes the musl target from `--platform` (`TARGETARCH`);
  `scripts/build_alpine_binaries.sh` builds aarch64 and x86_64 into `dist/alpine/<arch>/`
  (BL-065). It needs `benches/` in the context (`[[bench]]` targets). Not built in CI.
- Toolchain (ADR-0012): MSRV 1.85 (`rust-version`, checked by the `msrv` job). The CI gate
  (`rust.yml`) and `release.yml` pin Rust 1.89.0 via `RUST_TOOLCHAIN`; bump both together.
  `.tool-versions` is gitignored, local only.
- Edition 2024 for the crate and `fuzz/` (ADR-0032). `rustfmt.toml` keeps `style_edition = "2021"`;
  the switch to style 2024 is its own fmt-only commit.
- CI (`rust.yml`) runs `./harness/check.sh`, `cargo audit` and the MSRV build on pushes and PRs to
  `main` and `develop`, all `--locked`. A fourth job, `docker image` (BL-062), builds `Dockerfile`
  with buildx (GitHub Actions cache, loaded locally, never pushed) and runs
  `scripts/docker_smoke.sh`. It is not a required check in the ruleset.
- `slow-tests.yml` (BL-087) runs the `#[ignore]` deadline tests (408, read/write deadlines,
  server timeouts; about 100 s) every Monday and on dispatch, from `main` only; not a required check.
- `fuzz.yml` (ADR-0028) runs the `fuzz/` targets weekly and on dispatch (once it is on `main`) on a pinned nightly
  (`FUZZ_TOOLCHAIN`) and `cargo-fuzz` version; not a required check. `fuzz/` has its own
  workspace and `Cargo.lock` and is outside `include`.
- Release (ADR-0022): one tag `vX.Y.Z`, one workflow `release.yml`: `verify` (tag = `Cargo.toml`
  version, CHANGELOG section present) → `build-binaries` + `crate-check` (incl.
  `cargo publish --dry-run`) → `github-release` (body from `scripts/changelog_section.sh`) →
  `crates-io`. All `--locked`. Dry runs (BL-083): every PR into `main` and `workflow_dispatch`
  run `verify` and the build jobs; the two publish jobs are gated on `refs/tags/v*` and are the
  only jobs with `contents: write` or a secret. `crates-io` runs in the `crates-io` environment
  (required reviewer: the maintainer; deployment rule: tags `v*`), which holds the only
  `CRATES_IO_TOKEN`. GitHub runs `schedule` and offers `workflow_dispatch` only for workflows on
  the default branch: until 0.7.0 reaches `main`, the weekly `fuzz.yml`/`slow-tests.yml` do not
  fire and `main` still carries the old `release-binaries.yml` (the release PR deletes it).
  There is no `crate-v*` tag any more.
- `release.yml` writes `<archive>.tar.gz.sha256` next to each archive in the build job;
  the `github-release` job verifies every checksum before uploading.
- `Cargo.toml` `include`: `Cargo.toml`, README, CHANGELOG, licences, `docs/guide/`, `docs/reference/`,
  `openapi.yml`, `src/`, `tests/`. `repository` is set; no `homepage`.
- `CHANGELOG.md` exists at the root (Keep a Changelog).
- Git hooks exist (`.githooks/`, ADR-0009). A GitHub ruleset protects `main`, `develop` and
  `release/*` (BL-022): PR required, no force-push or deletion, required checks `harness gate`,
  `cargo audit` and `MSRV build` with up-to-date branches. Renaming a CI job breaks merges until
  the ruleset is updated. Ruleset "main" allows only merge commits into `main` (BL-083); the tag
  ruleset "releases" restricts `refs/tags/v*` to the repository admin.
- `cross` is pinned (ADR-0022): `CROSS_REV` commit in `release.yml`, installed `--locked`, and the
  musl images pinned by digest in `Cross.toml`. Bump the commit and the digests together.
- Every action is pinned to a full commit SHA with the tag as a comment (`@<sha> # v4.4.0`, BL-065),
  the latest release of its major. `dtolnay/rust-toolchain` is pinned to a `master` commit and
  always gets `toolchain:` (the MSRV job passes `1.85.0`). New actions are pinned the same way;
  resolve with `gh api repos/<owner>/<repo>/git/ref/tags/<tag>` and dereference annotated tags.
  `release.yml` has never run on GitHub yet; the first tag (or a `workflow_dispatch` dry run) is
  its first real test.
- `Dockerfile` (sk_http_server): builder `rust:1.89.0-slim-bookworm` (bump with `RUST_TOOLCHAIN`),
  runtime `debian:bookworm-slim`, `cargo build --locked`, no `curl` (health check is
  `docker/healthcheck.sh` over bash `/dev/tcp`). `docker/entrypoint.sh` fails closed without
  `SKILLET_AUTH_TOKEN` and `SKILLET_ADMIN_TOKEN` and passes them by env only. Compose publishes on
  `127.0.0.1`. Built and smoke-tested in CI by the `docker image` job (`scripts/docker_smoke.sh`).
  `.dockerignore` must keep `benches/` (Cargo.toml declares `[[bench]]` targets).

## Hand off
Use the handoff template in `harness/roles/orchestrator.md`; include `cargo package --list` diff
for packaging changes.
