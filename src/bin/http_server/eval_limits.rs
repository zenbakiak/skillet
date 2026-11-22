//! Per-request evaluation limits of `sk_http_server` and `sk_server` (ADR-0011 amendment 4),
//! shared by both (included in `sk_server` with `#[path]`).
//!
//! The deadline and the budget are set at start-up with `--eval-timeout <seconds>` /
//! `SKILLET_EVAL_TIMEOUT` and `--max-eval-bytes <bytes>` / `SKILLET_MAX_EVAL_BYTES`; the flag wins
//! over the env var. The library and `sk` keep [`skillet::MAX_EVAL_BYTES`] and no deadline.

use skillet::EvalLimits;
use std::sync::OnceLock;
use std::time::Duration;

/// Command-line flag for the deadline, in whole seconds.
pub const TIMEOUT_FLAG: &str = "--eval-timeout";
/// Environment variable for the deadline; the flag wins over it.
pub const TIMEOUT_ENV: &str = "SKILLET_EVAL_TIMEOUT";
/// Command-line flag for the budget, in bytes (suffixes `K`, `M`, `G` = KiB, MiB, GiB).
pub const BYTES_FLAG: &str = "--max-eval-bytes";
/// Environment variable for the budget; the flag wins over it.
pub const BYTES_ENV: &str = "SKILLET_MAX_EVAL_BYTES";

/// Default wall-clock limit for one server evaluation.
pub const DEFAULT_EVAL_TIMEOUT: Duration = Duration::from_secs(60);
/// Default server budget (the library default is 256 MiB).
pub const DEFAULT_MAX_EVAL_BYTES: u64 = 1024 * 1024 * 1024;
/// Largest accepted deadline (one day). A deadline too far away to represent would silently
/// mean no deadline at all.
const MAX_EVAL_TIMEOUT_SECS: u64 = 86_400;

const KIB: u64 = 1024;
const MIB: u64 = 1024 * KIB;
const GIB: u64 = 1024 * MIB;

static CONFIGURED: OnceLock<EvalLimits> = OnceLock::new();

/// The defaults: [`DEFAULT_MAX_EVAL_BYTES`] and [`DEFAULT_EVAL_TIMEOUT`].
pub fn defaults() -> EvalLimits {
    let mut limits = EvalLimits::default();
    limits.max_bytes = DEFAULT_MAX_EVAL_BYTES;
    limits.timeout = Some(DEFAULT_EVAL_TIMEOUT);
    limits
}

/// Resolve both limits from the flag values (if given) and the environment.
pub fn resolve(timeout_flag: Option<&str>, bytes_flag: Option<&str>) -> Result<EvalLimits, String> {
    resolve_with(timeout_flag, bytes_flag, |name| {
        std::env::var_os(name).map(|v| v.to_string_lossy().into_owned())
    })
}

/// [`resolve`] with the environment passed in (for tests). `0`, an empty value or anything that
/// is not a valid number is an error whose message names its source.
fn resolve_with(
    timeout_flag: Option<&str>,
    bytes_flag: Option<&str>,
    env: impl Fn(&str) -> Option<String>,
) -> Result<EvalLimits, String> {
    let mut limits = defaults();
    let timeout = pick(timeout_flag, TIMEOUT_FLAG, TIMEOUT_ENV, &env);
    if let Some((value, source)) = timeout {
        limits.timeout = Some(parse_timeout(&value, source)?);
    }
    if let Some((value, source)) = pick(bytes_flag, BYTES_FLAG, BYTES_ENV, &env) {
        limits.max_bytes = parse_bytes(&value, source)?;
    }
    Ok(limits)
}

/// The flag value if given, else the env var value, with the name of where it came from.
fn pick<'a>(
    flag: Option<&str>,
    flag_name: &'a str,
    env_name: &'a str,
    env: &impl Fn(&str) -> Option<String>,
) -> Option<(String, &'a str)> {
    match flag {
        Some(v) => Some((v.to_string(), flag_name)),
        None => env(env_name).map(|v| (v, env_name)),
    }
}

fn parse_timeout(value: &str, source: &str) -> Result<Duration, String> {
    not_empty(value, source, "a number of seconds")?;
    match value.parse::<u64>() {
        Ok(n) if (1..=MAX_EVAL_TIMEOUT_SECS).contains(&n) => Ok(Duration::from_secs(n)),
        _ => Err(format!(
            "{source} must be a whole number of seconds from 1 to {MAX_EVAL_TIMEOUT_SECS} (got {value:?})"
        )),
    }
}

fn parse_bytes(value: &str, source: &str) -> Result<u64, String> {
    not_empty(value, source, "a number of bytes")?;
    let invalid = || {
        format!(
            "{source} must be a whole number of bytes greater than 0, optionally followed by K, M or G (KiB, MiB, GiB) (got {value:?})"
        )
    };
    let (digits, unit) = match value.as_bytes().last() {
        Some(b'K' | b'k') => (&value[..value.len() - 1], KIB),
        Some(b'M' | b'm') => (&value[..value.len() - 1], MIB),
        Some(b'G' | b'g') => (&value[..value.len() - 1], GIB),
        _ => (value, 1),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(invalid());
    }
    match digits.parse::<u64>().ok().and_then(|n| n.checked_mul(unit)) {
        Some(n) if n > 0 => Ok(n),
        _ => Err(invalid()),
    }
}

fn not_empty(value: &str, source: &str, what: &str) -> Result<(), String> {
    if value.is_empty() {
        Err(format!(
            "{source} must not be empty; give {what} greater than 0 or unset it"
        ))
    } else {
        Ok(())
    }
}

/// Record the limits resolved at start-up. Called once, before any request is served.
pub fn install(limits: EvalLimits) {
    let _ = CONFIGURED.set(limits);
}

/// Limits for every server evaluation: what [`install`] recorded, else the defaults.
pub fn server_limits() -> EvalLimits {
    CONFIGURED.get().copied().unwrap_or_else(defaults)
}

/// The deadline for the start-up banner: `60 s`.
pub fn describe_timeout(limits: &EvalLimits) -> String {
    match limits.timeout {
        Some(t) => format!("{} s", t.as_secs()),
        None => "none".to_string(),
    }
}

/// The budget for the start-up banner: `1 GiB`, `512 MiB`, `64 KiB`, or `N bytes`.
pub fn describe_bytes(max: u64) -> String {
    for (unit, name) in [(GIB, "GiB"), (MIB, "MiB"), (KIB, "KiB")] {
        if max >= unit && max % unit == 0 {
            return format!("{} {name}", max / unit);
        }
    }
    format!("{max} bytes")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |name| map.get(name).cloned()
    }

    #[test]
    fn defaults_are_60_s_and_1_gib() {
        let limits = resolve_with(None, None, env(&[])).unwrap();
        assert_eq!(limits.timeout, Some(Duration::from_secs(60)));
        assert_eq!(limits.max_bytes, 1 << 30);
        assert_eq!(limits, defaults());
    }

    #[test]
    fn flags_and_env_vars_are_read() {
        let limits = resolve_with(Some("5"), Some("2M"), env(&[])).unwrap();
        assert_eq!(limits.timeout, Some(Duration::from_secs(5)));
        assert_eq!(limits.max_bytes, 2 * MIB);
        let e = env(&[(TIMEOUT_ENV, "7"), (BYTES_ENV, "1000")]);
        let limits = resolve_with(None, None, e).unwrap();
        assert_eq!(limits.timeout, Some(Duration::from_secs(7)));
        assert_eq!(limits.max_bytes, 1000);
    }

    #[test]
    fn the_flag_wins_over_the_env_var() {
        // An invalid env value is not even read when the flag is given (as for the tokens).
        let e = env(&[(TIMEOUT_ENV, "0"), (BYTES_ENV, "abc")]);
        let limits = resolve_with(Some("3"), Some("4K"), e).unwrap();
        assert_eq!(limits.timeout, Some(Duration::from_secs(3)));
        assert_eq!(limits.max_bytes, 4 * KIB);
        // Each limit falls back on its own.
        let e = env(&[(TIMEOUT_ENV, "9"), (BYTES_ENV, "1G")]);
        let limits = resolve_with(None, Some("512M"), e).unwrap();
        assert_eq!(limits.timeout, Some(Duration::from_secs(9)));
        assert_eq!(limits.max_bytes, 512 * MIB);
    }

    #[test]
    fn suffixes_are_binary_units_in_either_case() {
        for (text, want) in [
            ("1", 1),
            ("1k", KIB),
            ("3K", 3 * KIB),
            ("256m", 256 * MIB),
            ("1G", GIB),
            ("8g", 8 * GIB),
            ("1073741824", GIB),
        ] {
            assert_eq!(parse_bytes(text, BYTES_FLAG), Ok(want), "{text}");
        }
    }

    #[test]
    fn invalid_timeouts_are_errors_naming_their_source() {
        let empty = parse_timeout("", TIMEOUT_ENV).unwrap_err();
        assert!(
            empty.starts_with("SKILLET_EVAL_TIMEOUT must not be empty"),
            "{empty}"
        );
        for bad in ["0", "-1", "1.5", "10s", " 5", "abc", "86401"] {
            let err = parse_timeout(bad, TIMEOUT_FLAG).unwrap_err();
            assert!(
                err.starts_with("--eval-timeout must be a whole number"),
                "{err}"
            );
            assert!(err.contains(&format!("{bad:?}")), "{err}");
        }
        assert_eq!(
            parse_timeout("86400", TIMEOUT_FLAG),
            Ok(Duration::from_secs(86_400))
        );
    }

    #[test]
    fn invalid_budgets_are_errors_naming_their_source() {
        let empty = parse_bytes("", BYTES_ENV).unwrap_err();
        assert!(
            empty.starts_with("SKILLET_MAX_EVAL_BYTES must not be empty"),
            "{empty}"
        );
        for bad in [
            "0",
            "0K",
            "-1",
            "1.5G",
            "G",
            "1KB",
            "1T",
            " 1",
            "+1",
            "abc",
            "18446744073709551616",
            "17179869184G",
        ] {
            let err = parse_bytes(bad, BYTES_FLAG).unwrap_err();
            assert!(
                err.starts_with("--max-eval-bytes must be a whole number"),
                "{err}"
            );
            assert!(err.contains(&format!("{bad:?}")), "{err}");
        }
    }

    #[test]
    fn resolve_reports_the_first_invalid_value() {
        let err = resolve_with(None, None, env(&[(BYTES_ENV, "")])).unwrap_err();
        assert!(
            err.starts_with("SKILLET_MAX_EVAL_BYTES must not be empty"),
            "{err}"
        );
        let err = resolve_with(Some("0"), None, env(&[])).unwrap_err();
        assert!(err.starts_with("--eval-timeout must be"), "{err}");
    }

    #[test]
    fn banner_texts() {
        assert_eq!(describe_timeout(&defaults()), "60 s");
        assert_eq!(describe_bytes(GIB), "1 GiB");
        assert_eq!(describe_bytes(256 * MIB), "256 MiB");
        assert_eq!(describe_bytes(64 * KIB), "64 KiB");
        assert_eq!(describe_bytes(1500), "1500 bytes");
        assert_eq!(describe_bytes(GIB + MIB), "1025 MiB");
    }
}
