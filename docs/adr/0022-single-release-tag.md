# 0022. One release tag drives binaries, GitHub release and crates.io

- Status: Accepted
- Date: 2026-11-21
- Deciders: Maintainer
- Related: BL-056, ADR-0009, ADR-0012

## Context
Two tags shipped one version. `vX.Y.Z` triggered `release-binaries.yml` (binaries attached to a
GitHub release with an empty body) and `crate-vX.Y.Z` triggered `crate-release.yml` (crates.io).
Only the crate workflow checked the tag against `Cargo.toml`, so a binary release could carry
any version, and the two could drift or one could be forgotten. Release notes were written by
hand, if at all, although `CHANGELOG.md` already holds them.

`release-binaries.yml` also ran `cargo install cross --git https://github.com/cross-rs/cross`,
which builds whatever is on cross-rs `main` that day, and a git build of cross pulls the floating
`ghcr.io/cross-rs/<target>:main` images. The Linux binaries were not reproducible.

## Decision
- **One tag, `vX.Y.Z`, one workflow** (`.github/workflows/release.yml`). `crate-release.yml` and
  the `crate-v*` trigger are removed. Jobs:
  1. `verify`: the tag must equal `v` + the `Cargo.toml` version, and
     `scripts/changelog_section.sh X.Y.Z` must find a non-empty `## [X.Y.Z]` section in
     `CHANGELOG.md`. Either failure stops the run before anything is built or published.
  2. `build-binaries` (matrix) and `crate-check` (build, test, `cargo package`), in parallel.
  3. `github-release`: verifies the checksums and creates the release, with the CHANGELOG
     section as its body.
  4. `crates-io`: `cargo publish`, last, because it is the only step that cannot be undone
     (a GitHub release can be edited or deleted; a crates.io version can only be yanked).
- `workflow_dispatch` runs `verify` (for the `Cargo.toml` version) and the build jobs as a dry
  run; the two publish jobs only run for tag pushes.
- **`cross` is pinned to a commit**, installed with `--rev <sha> --locked` (`CROSS_REV` in the
  workflow), and the two musl images are pinned by digest in `Cross.toml`. The commit is cross-rs
  `main` at `a9301e9` (2026-10-31); the digests are the `:main` images of the same day.

### Why a commit and not the released version
The latest cross release is 0.2.5 (February 2023). Its images and C toolchains are equally
old, and they predate Rust 1.71's switch of the musl targets to musl 1.2.3; linking aarch64 musl
with newer Rust through those images is a reported source of trouble. It has never been tried
with our Rust 1.89 and QuickJS's C build, and a release is the wrong time to find out. `main` is what
the workflow has used successfully so far, so pinning the commit we already depend on changes
nothing about the build except that it stops moving. Pinning only the commit is not enough: a git
build of cross always pulls the `:main` image tags, so the images are pinned by digest as well.
`--locked` makes `cargo install` use cross's own `Cargo.lock`.

## Consequences
- + One action per release; the binaries, the GitHub release and the crate always carry the same,
  checked version.
- + Release notes come from `CHANGELOG.md`, so the release process (step 3: move "Unreleased"
  under the new version) is enforced: a release without its CHANGELOG section fails early.
- + The Linux build is reproducible: same cross binary, same images.
- − A crates.io outage or token problem leaves a GitHub release without a crate. Re-run the
  `crates-io` job; it is idempotent until the publish succeeds.
- − `CROSS_REV` and the two digests in `Cross.toml` are bumped by hand, together.
- − Existing `crate-vX.Y.Z` tags stay; CHANGELOG compare links switch to `vX.Y.Z` from the next
  release on.

## Alternatives considered
- **Two workflows on the same `v*` trigger.** Simpler diff, but each would repeat the version
  check, and the crate could publish while the binaries fail. One workflow with `needs` orders
  them.
- **Publish the crate first.** Rejected: it is the irreversible step, so it goes last.
- **Generated release notes (GitHub's "auto-generate").** Lists PR titles, not the curated
  CHANGELOG; two sources of truth.
- **cross 0.2.5 from crates.io.** Simplest pin, but see above.
- **Drop cross** (native `cargo build` with `musl-tools` plus an aarch64 cross linker, or
  `cargo-zigbuild`). Possible later; it changes the build itself, not just the pin.
