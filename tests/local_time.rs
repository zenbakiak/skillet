//! `DATE()` and `TIME()` without arguments use the local time zone, read from `TZ`
//! (ADR-0020: libc `localtime_r` instead of chrono's `clock` feature). Each case runs `sk` in a
//! fresh process with a pinned `TZ` and compares with an independent computation: UTC now plus
//! the zone's known fixed offset. Nothing here hard-codes `TIME`'s unit: the time check is
//! `(TIME() - TIME(h, m, s)) / TIME(0, 0, 1)`, a difference in seconds whatever the unit is.

use chrono::{Duration, NaiveDateTime, Timelike, Utc};
use std::process::Command;

/// (TZ value, offset east of UTC in seconds). POSIX strings need no zone database; their sign
/// is inverted (`XST-14` is UTC+14).
const ZONES: &[(&str, i64)] = &[
    ("UTC0", 0),
    ("XST-14", 14 * 3600),
    ("YST+12", -12 * 3600),
    ("IST-5:30", 5 * 3600 + 30 * 60),
];

/// `sk <expr>` with `TZ` set, no hooks, outside the repo; returns the last stdout line.
fn sk(tz: &str, expr: &str) -> String {
    let hooks = std::env::temp_dir().join("skillet-local-time-no-hooks");
    let out = Command::new(env!("CARGO_BIN_EXE_sk"))
        .arg(expr)
        .current_dir(std::env::temp_dir())
        .env("TZ", tz)
        .env("SKILLET_HOOKS_DIR", hooks)
        .output()
        .expect("run sk");
    assert!(out.status.success(), "sk {:?} failed: {:?}", expr, out);
    let stdout = String::from_utf8(out.stdout).unwrap();
    stdout.lines().last().unwrap_or_default().trim().to_string()
}

fn local_now(offset: i64) -> NaiveDateTime {
    Utc::now().naive_utc() + Duration::seconds(offset)
}

/// Waits until at least 30 s away from local midnight, so date and time-of-day cannot roll
/// over between the expected value and the one `sk` computes.
fn away_from_midnight(offset: i64) {
    let secs = local_now(offset).time().num_seconds_from_midnight();
    if secs > 86_400 - 30 {
        std::thread::sleep(std::time::Duration::from_secs(u64::from(86_400 - secs) + 1));
    }
}

#[test]
fn date_without_arguments_is_todays_date_in_tz() {
    for &(tz, offset) in ZONES {
        away_from_midnight(offset);
        let today = local_now(offset).date();
        let expected = sk(
            tz,
            &format!(
                "=DATE({}, {}, {})",
                today.format("%Y"),
                today.format("%-m"),
                today.format("%-d")
            ),
        );
        assert_eq!(sk(tz, "=DATE()"), expected, "TZ={}", tz);
    }
}

#[test]
fn time_without_arguments_is_the_time_of_day_in_tz() {
    for &(tz, offset) in ZONES {
        away_from_midnight(offset);
        let t = local_now(offset).time();
        let expr = format!(
            "=(TIME() - TIME({}, {}, {})) / TIME(0, 0, 1)",
            t.hour(),
            t.minute(),
            t.second()
        );
        let out = sk(tz, &expr);
        let secs: f64 = out
            .strip_prefix("Number(")
            .and_then(|s| s.strip_suffix(')'))
            .unwrap_or_else(|| panic!("TZ={}: unexpected output {:?}", tz, out))
            .parse()
            .unwrap();
        // `sk` runs after `t` was taken: 0 to a few seconds later, never an offset away.
        assert!(
            (-1.0..=10.0).contains(&secs),
            "TZ={}: TIME() is {} s from local time",
            tz,
            secs
        );
    }
}
