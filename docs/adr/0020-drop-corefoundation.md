# 0020. Drop CoreFoundation from the macOS binaries

- Status: Proposed
- Date: 2026-11-21
- Deciders: Maintainer (dependency feature change, BL-055)
- Related: BL-040 (start-up profile), BL-042 (`scalar-doc`), BL-055, `src/runtime/datetime.rs`,
  `benches/BASELINE.md` ("BL-055: what CoreFoundation costs `sk`")

The maintainer approved removing CoreFoundation. This ADR stays Proposed because the removal
needs two choices the approval did not cover: what `DATE()` and `TIME()` return, and what
happens to `scalar-doc`.

## Context

Every binary on macOS links `CoreFoundation.framework`. `otool -L target/release/sk` at
`3f59ae1` lists CoreFoundation and libSystem. The path (`cargo tree -i iana-time-zone -e
features`) is `chrono` feature `clock` → `iana-time-zone` → `core-foundation-sys`. Two crates
turn `clock` on:

1. `skillet` itself, through chrono's default features. The code needs them: `DATE()` with no
   arguments returns midnight of today's **local** date (`Local::now().date_naive()`), and
   `TIME()` with no arguments returns seconds since **local** midnight (`Local::now().time()`).
   Both are documented that way (`docs/reference/functions.md`, `docs/reference/api.md`).
   `NOW()` already uses `Utc::now()`.
2. `tera` (chrono `std` + `clock`), pulled in by `scalar-doc`, which only builds the `/docs`
   page of `sk_http_server`. It is a normal dependency of the library, so Cargo's feature
   unification turns `clock` on for every binary, `sk` included.

Measured with prototypes (not merged), 5 interleaved rounds of 300 calls, release build:

| Build | Links CoreFoundation | `sk` size | `sk "=1+1"` median |
|---|---|---|---|
| `3f59ae1` | yes | 4 382 000 B | 5.84 ms |
| `skillet` chrono without `clock` (`Local` → `Utc`) | yes (through `tera`) | 4 340 400 B | 5.95 ms |
| same, `scalar-doc` removed | no | 4 340 096 B | 4.31 ms |
| `/usr/bin/true` | | | 2.94 ms |

So removing CoreFoundation saves 1.1–1.6 ms per `sk` call, but only if **both** sources go.
Machine, load and method are in `benches/BASELINE.md`.

## Options

### For `DATE()` / `TIME()` (local time)

A. **Use UTC.** `Utc::now()` in both. Smallest change. Behavior change: outside UTC, `DATE()`
   and `TIME()` change value near midnight (`DATE()` is off by one day for part of the day,
   `TIME()` by the UTC offset). Needs docs and a CHANGELOG "Changed" entry. Inconsistent with
   Excel, where `TODAY()`/`NOW()` are local.

B. **Read the local offset from libc** (`localtime_r`, `tm_gmtoff`) on Unix. `libc` is already a
   dependency. libc honors `TZ` and `/etc/localtime`, the same sources chrono's `Local` reads
   first; chrono only calls `iana-time-zone` (and so CoreFoundation) as a fallback when both
   fail. No behavior change on the supported targets (release binaries are macOS and Linux
   only). Cost: one small `unsafe` block. On non-Unix targets, keep chrono `clock` through a
   `[target.'cfg(not(unix))'.dependencies]` entry, or fall back to UTC.

C. **UTC unless `TZ` is set**, parsing `TZ` ourselves. Behavior change for users without `TZ`
   (most desktop users); more code than B for no gain. Not recommended.

### For `tera` (through `scalar-doc`)

D. **Remove `scalar-doc`** (BL-042) and serve a static Scalar page from `include_str!` (it is
   an HTML page that loads Scalar and points it at `/openapi.yml`). Also removes most
   `cargo audit` warnings (BL-042).

E. **Feature-gate `scalar-doc`** behind an optional `docs-page` feature, off by default or on
   only for `sk_http_server` builds. Default builds drop CoreFoundation; builds that turn the
   feature on keep it.

## Decision (proposed)

B + D: local time through libc, so `DATE()` and `TIME()` keep their values, and `scalar-doc`
removed. `skillet`'s chrono becomes
`{ version = "0.4", default-features = false, features = ["std", "now", "serde"] }` (`now` gives
`Utc::now()`). The `now` feature needs a recent chrono 0.4 (`Cargo.lock` has 0.4.43, whose
`rust-version` is far below our MSRV 1.85); set the requirement to `0.4.43` to make that explicit.

## Consequences

- `sk` start-up drops by ~1.1–1.6 ms on macOS; `otool -L` shows only libSystem.
- A regression test pins `DATE()`/`TIME()` against `TZ` (for example `TZ=Etc/GMT-14`), so a
  later switch to UTC cannot happen silently.
- With D, `/docs` keeps working without `tera`; with E, the docs feature brings
  CoreFoundation back for the builds that enable it.
- If the maintainer picks A instead of B, it is a user-visible change and needs a CHANGELOG
  "Changed" entry and updates to `docs/reference/functions.md` and `docs/reference/api.md`.
