# 0010. Remove dead public modules in 0.7.0 without deprecation

- Status: Accepted
- Date: 2026-11-20
- Deciders: Maintainer
- Related: BL-002, BL-001, ADR-0002

## Context
`skillet::memory_pool`, `skillet::traits` and `skillet::concurrent_registry` (~780 lines) are
`pub` but unused by the crate, its binaries, tests and examples. They describe optimizations
(arena allocation, concurrent registry) that the engine never adopted. `skillet::runtime::evaluation`
is a re-export shim left by BL-001. Keeping them misleads contributors and agents (the
development guide documented them as live) and keeps the `dashmap` dependency alive.

The crate is pre-1.0. Under Cargo SemVer, `0.6.x → 0.7.0` may contain breaking changes.

## Decision
Delete these modules outright in the next minor release (0.7.0), with no `#[deprecated]` cycle:
- `skillet::memory_pool`, `skillet::traits`, `skillet::concurrent_registry`
- `skillet::runtime::evaluation` (use `skillet::runtime::evaluator` or the top-level `evaluate*` functions)

Also drop the dependencies left without users: `string-interner`, `typed-arena`, `beef`, `dashmap`.
The removal is listed under "Removed" in the 0.7.0 changelog.

## Consequences
- + ~1.8k fewer lines, four fewer dependencies, faster builds, docs match the code.
- − Any downstream crate importing these paths breaks on 0.7.0. Judged unlikely: nothing in the
  repo or docs examples uses them.
- Future removals of `pub` items before 1.0 may follow this ADR if they are demonstrably unused;
  otherwise deprecate for one minor release.

## Alternatives considered
- Deprecate in 0.7, remove in 0.8 — protects hypothetical users at the cost of two releases of dead code.
