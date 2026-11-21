# 0020. Drop CoreFoundation from the macOS binaries

- Status: Accepted
- Date: 2026-11-21
- Deciders: Maintainer (dependency feature change, BL-055)
- Related: BL-040 (start-up profile), BL-042 (`scalar-doc`, closed by this ADR), BL-055,
  `src/runtime/local_time.rs`, `src/runtime/datetime.rs`, `src/bin/http_server/api_docs.html`,
  `benches/BASELINE.md` ("BL-055: what CoreFoundation costs `sk`")

## Context

On macOS every Skillet binary linked `CoreFoundation.framework`. `otool -L target/release/sk` at
`3f59ae1` listed CoreFoundation and libSystem, and loading the framework cost ~1.1–1.6 ms per
`sk` call (BL-040 found it; numbers below). The path (`cargo tree -i iana-time-zone -e
features`) was chrono's `clock` feature → `iana-time-zone` → `core-foundation-sys`. Two crates
turned `clock` on:

1. `skillet` itself, through chrono's default features. The code needed them: with no
   arguments, `DATE()` returns midnight of today's **local** date and `TIME()` returns the
   **local** time of day (`chrono::Local`). Both are documented that way. `NOW()` already used
   `Utc::now()`.
2. `tera` (chrono `std` + `clock`), pulled in by `scalar-doc`, which only built the `/docs` page
   of `sk_http_server`. It was a normal dependency of the library, so Cargo's feature
   unification turned `clock` on for every binary, `sk` included.

Removing only one of the two saves nothing: a prototype with skillet's chrono reduced to
`["std", "now", "serde"]` still linked CoreFoundation through `tera`.

## Options considered

Local time for `DATE()` / `TIME()`:

- A. Switch to UTC. Smallest change, but user-visible: outside UTC, `DATE()` and `TIME()`
  change value for part of the day. Rejected.
- **B. Read the local UTC offset from libc (`localtime_r`, `tm_gmtoff`) on Unix.** libc honours
  `TZ` and `/etc/localtime`, the same sources chrono's `Local` reads first (chrono only asks
  `iana-time-zone`, and so CoreFoundation, when both fail). Results stay the same. Costs one
  small `unsafe` block.
- C. UTC unless `TZ` is set, parsing `TZ` ourselves. A behavior change for most users, and more
  code than B. Rejected.

`tera` (through `scalar-doc`):

- **D. Remove `scalar-doc` and serve the Scalar page as static HTML (`include_str!`).** The page
  that `scalar-doc` rendered was fixed (title, `/openapi.yml`, default theme), so a static copy
  is equivalent. Also removes `tera` and ~30 crates from the tree, and most `cargo audit`
  warnings (BL-042).
- E. Feature-gate `scalar-doc`. Builds that enable it would bring CoreFoundation back. Rejected.

## Decision

**B + D.**

- chrono becomes `{ version = "0.4.43", default-features = false, features = ["std", "now",
  "serde"] }`. `now` provides `Utc::now()`. chrono 0.4.43 is what `Cargo.lock` already had; its
  `rust-version` is 1.62, so the MSRV (1.85, ADR-0012) is unaffected.
- `src/runtime/local_time.rs` provides `now()`, the local date-time, used by `DATE()` and
  `TIME()`. On Linux, Android, macOS, iOS, FreeBSD, NetBSD, OpenBSD and DragonFly it is
  `Utc::now()` plus `tm_gmtoff` from `libc::localtime_r`, and falls back to UTC if libc fails.
  On every other target it is `chrono::Local::now()`, and `Cargo.toml` turns chrono's `clock`
  on for exactly those targets (`[target.'cfg(not(any(...)))'.dependencies]`; the two target
  lists must match).
- **`unsafe` exception.** The rule "no `unsafe` outside `src/bin/http_server/daemon.rs` without
  an ADR" (`harness/roles/rust-engineer.md`) gets one more allowed place:
  `utc_offset_seconds` in `src/runtime/local_time.rs`. It is one block around the
  `localtime_r` call and `MaybeUninit::assume_init`, with a `SAFETY` comment. Any further
  `unsafe` in the library still needs its own ADR.
- `GET /docs` serves `src/bin/http_server/api_docs.html` (the page `scalar-doc` rendered, with
  its entity-escaped `/` written plainly). `tests/http_server_accept.rs` pins it.

## Consequences

Measured from release builds of `3f59ae1` and of this change, same session: Apple M3 Pro,
macOS 27.0.1, rustc 1.89.0, 5 interleaved rounds of 300 calls (`benches/cli_startup.py`),
1-minute load average 20–27:

| | before (`3f59ae1`) | after |
|---|---|---|
| `otool -L sk` | CoreFoundation, libSystem | libSystem, libiconv |
| `sk "=1+1"` median (p95) | 4.55 ms (6.88) | 3.29 ms (4.90) |
| `/usr/bin/true` (floor) | 2.14 ms | 2.14 ms |
| `sk` size | 4 382 000 B | 4 340 480 B |
| `sk_http_server` size | 7 989 680 B | 4 833 568 B |

- `sk` start-up drops by ~1.2 ms on macOS, about half of its time above the process-start floor.
- `libiconv` is linked now because the library calls into the `libc` crate, which declares
  `#[link(name = "iconv")]` on Apple targets. It is in the dyld shared cache and costs nothing
  measurable: a prototype without it ran 3.35 ms in the same session.
- `sk_server` and `sk_http_server` still load Foundation (and so CoreFoundation) through
  `ctrlc` → `dispatch2` → `objc2`. This is unrelated to chrono and was already the case
  before; those binaries are long-running, so the start-up cost does not matter there.
- A regression test (`tests/local_time.rs`) runs `sk` with pinned POSIX `TZ` values and checks
  `DATE()` and `TIME()` against UTC plus the known offset, so a later silent switch to UTC
  fails. It is unit-free for `TIME`, so changing `TIME`'s unit does not break it.
- On targets outside the libc list, behavior and dependencies are as before.
