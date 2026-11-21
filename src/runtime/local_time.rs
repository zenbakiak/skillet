//! The current local wall-clock time, for `DATE()` and `TIME()` without arguments.
//!
//! On the Unix targets listed below, the UTC offset comes from libc's `localtime_r`, which reads
//! `TZ` and `/etc/localtime` — the same sources chrono's `Local` reads first. This keeps
//! chrono's `clock` feature (and through it `iana-time-zone` and, on macOS, CoreFoundation) out
//! of the build, which saves ~1.1–1.6 ms per `sk` start ([ADR-0020], BL-055). Other targets use
//! chrono's `Local` (Cargo.toml turns `clock` on for them).
//!
//! The target list here must match the `cfg` of the `clock` entry in `Cargo.toml`.
//!
//! [ADR-0020]: https://github.com/zenbakiak/skillet/blob/develop/docs/adr/0020-drop-corefoundation.md

use chrono::NaiveDateTime;

/// The current local date and time. Falls back to UTC if the local offset cannot be read.
#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
pub(crate) fn now() -> NaiveDateTime {
    let now = chrono::Utc::now();
    let offset = utc_offset_seconds(now.timestamp()).unwrap_or(0);
    now.naive_utc() + chrono::Duration::seconds(offset)
}

/// The local UTC offset in seconds (east positive) at Unix time `timestamp`, from
/// `localtime_r`'s `tm_gmtoff`. `None` if `timestamp` does not fit `time_t` or libc fails.
#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
))]
// `time_t` and `c_long` are `i64` here, but 32-bit on some of these targets.
#[allow(clippy::useless_conversion)]
fn utc_offset_seconds(timestamp: i64) -> Option<i64> {
    let time: libc::time_t = timestamp.try_into().ok()?;
    let mut tm = std::mem::MaybeUninit::<libc::tm>::zeroed();
    // SAFETY: `localtime_r` is the reentrant variant: it reads `*time` and writes only into the
    // caller-owned `tm` buffer, both valid for the whole call. `libc::tm` is plain old data, so
    // the zeroed buffer is already a valid value; on success libc has filled it in, and on
    // failure (null return) we never read it. Its `tm_zone` pointer is not dereferenced.
    let tm = unsafe {
        if libc::localtime_r(&time, tm.as_mut_ptr()).is_null() {
            return None;
        }
        tm.assume_init()
    };
    Some(i64::from(tm.tm_gmtoff))
}

/// The current local date and time (chrono `Local`; `clock` is on for these targets).
#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "dragonfly"
)))]
pub(crate) fn now() -> NaiveDateTime {
    chrono::Local::now().naive_local()
}
