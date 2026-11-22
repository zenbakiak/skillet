use super::local_time;
use super::utils::coerce_to_number;
use crate::error::Error;
use crate::types::Value;
use chrono::{DateTime, Datelike, NaiveDate, NaiveTime, TimeDelta, Timelike, Utc};

pub fn is_datetime_function(name: &str) -> bool {
    matches!(
        name,
        "NOW"
            | "DATE"
            | "TODAY"
            | "TIME"
            | "YEAR"
            | "MONTH"
            | "DAY"
            | "DATEADD"
            | "DATEDIFF"
            | "DATEDIF"
    )
}

pub fn exec_datetime(name: &str, args: &[Value]) -> Result<Value, Error> {
    match name {
        "NOW" => {
            let now = Utc::now();
            Ok(Value::DateTime(now.timestamp()))
        }
        "DATE" => match args {
            [] => Ok(today()),
            [year, month, day] => exec_date(year, month, day),
            _ => Err(Error::new(
                "DATE expects either no arguments or three arguments (year, month, day)",
                None,
            )),
        },
        "TODAY" => match args {
            [] => Ok(today()),
            _ => Err(Error::new("TODAY expects no arguments", None)),
        },
        "TIME" => exec_time(args),
        "YEAR" => {
            if let Some(Value::DateTime(timestamp)) = args.first() {
                let dt = DateTime::from_timestamp(*timestamp, 0)
                    .ok_or_else(|| Error::new("Invalid timestamp", None))?;
                Ok(Value::Number(dt.year() as f64))
            } else {
                Err(Error::new("YEAR expects datetime", None))
            }
        }
        "MONTH" => {
            if let Some(Value::DateTime(timestamp)) = args.first() {
                let dt = DateTime::from_timestamp(*timestamp, 0)
                    .ok_or_else(|| Error::new("Invalid timestamp", None))?;
                Ok(Value::Number(dt.month() as f64))
            } else {
                Err(Error::new("MONTH expects datetime", None))
            }
        }
        "DAY" => {
            if let Some(Value::DateTime(timestamp)) = args.first() {
                let dt = DateTime::from_timestamp(*timestamp, 0)
                    .ok_or_else(|| Error::new("Invalid timestamp", None))?;
                Ok(Value::Number(dt.day() as f64))
            } else {
                Err(Error::new("DAY expects datetime", None))
            }
        }
        "DATEADD" => exec_dateadd(args),
        "DATEDIFF" => exec_datediff(args),
        "DATEDIF" => exec_datedif(args),
        _ => Err(Error::new(
            format!("Unknown datetime function: {}", name),
            None,
        )),
    }
}

/// Today's local date as midnight UTC (`DATE()` and `TODAY()`).
fn today() -> Value {
    let today = local_time::now().date();
    Value::DateTime(today.and_time(NaiveTime::MIN).and_utc().timestamp())
}

/// One `DATE` argument, truncated toward zero like Excel.
/// Numeric text and booleans convert as in Excel (BL-066); NULL stays an error.
fn date_part(v: &Value, what: &str) -> Result<i64, Error> {
    match part_number(v, "DATE", what)? {
        Some(n) if n.is_finite() => Ok(n.trunc() as i64),
        _ => Err(Error::new(format!("DATE expects {} as number", what), None)),
    }
}

/// A `DATE`/`TIME` part as a number: a number, or numeric text or a boolean (Excel coercion).
/// `None` for other types.
fn part_number(v: &Value, func: &str, what: &str) -> Result<Option<f64>, Error> {
    match v {
        Value::Number(n) => Ok(Some(*n)),
        other => coerce_to_number(other, &format!("{func} {what}: ")),
    }
}

/// Excel `DATE(year, month, day)`: years 0-1899 get 1900 added; months and days outside their
/// range roll over into the next or previous month/year (`DATE(2026, 13, 1)` = 2027-01-01,
/// `DATE(2026, 1, 0)` = 2025-12-31). Like Excel, a year outside 0-9999 or a result outside
/// 1900-01-01..=9999-12-31 is an error.
fn exec_date(year: &Value, month: &Value, day: &Value) -> Result<Value, Error> {
    let out_of_range = || Error::new("DATE result is outside 1900-01-01..9999-12-31", None);
    let mut y = date_part(year, "year")?;
    let m = date_part(month, "month")?;
    let d = date_part(day, "day")?;
    if !(0..=9999).contains(&y) {
        return Err(Error::new("DATE year must be between 0 and 9999", None));
    }
    if y < 1900 {
        y += 1900;
    }
    // Months since year 0, then split back into year and month (0-based).
    let months = y
        .checked_mul(12)
        .and_then(|v| v.checked_add(m.checked_sub(1)?))
        .ok_or_else(out_of_range)?;
    let (y, m0) = (months.div_euclid(12), months.rem_euclid(12));
    let first = i32::try_from(y)
        .ok()
        .and_then(|y| NaiveDate::from_ymd_opt(y, m0 as u32 + 1, 1))
        .ok_or_else(out_of_range)?;
    let date = d
        .checked_sub(1)
        .and_then(chrono::TimeDelta::try_days)
        .and_then(|delta| first.checked_add_signed(delta))
        .ok_or_else(out_of_range)?;
    if !(1900..=9999).contains(&date.year()) {
        return Err(out_of_range());
    }
    Ok(Value::DateTime(
        date.and_time(NaiveTime::MIN).and_utc().timestamp(),
    ))
}

/// Seconds in a day: `TIME` returns a fraction of a day.
const SECONDS_PER_DAY: f64 = 86400.0;

/// `TIME(hour, minute, second)`: the time as a fraction of a day (Excel, ADR-0018), with Excel's
/// truncation, limits and 24-hour wrap: `TIME(12, 0, 0)` is 0.5. `TIME()` with no arguments is
/// the current local time in the same unit.
fn exec_time(args: &[Value]) -> Result<Value, Error> {
    match args {
        [] => {
            let now = local_time::now().time();
            Ok(Value::Number(
                f64::from(now.num_seconds_from_midnight()) / SECONDS_PER_DAY,
            ))
        }
        [hour, minute, second] => {
            // Excel TIME: each part is truncated and must be 0..=32767 (a negative minute or
            // second is allowed while the total stays non-negative). The total wraps at 24
            // hours, so TIME(25, 0, 0) = TIME(1, 0, 0). The result is a fraction of a day, as in
            // Excel (ADR-0018), so DATE(...) + TIME(...) adds the time to a date.
            let part = |v: &Value, what: &str| match part_number(v, "TIME", what)? {
                Some(n) if n.is_finite() && n <= 32767.0 => Ok(n.trunc()),
                Some(_) => Err(Error::new(
                    format!("TIME {} must be at most 32767 (#NUM!)", what),
                    None,
                )),
                None => Err(Error::new(format!("TIME expects {} as number", what), None)),
            };
            let h = part(hour, "hour")?;
            let m = part(minute, "minute")?;
            let s = part(second, "second")?;
            if h < 0.0 {
                return Err(Error::new("TIME hour must not be negative (#NUM!)", None));
            }
            let total = h * 3600.0 + m * 60.0 + s;
            if total < 0.0 {
                return Err(Error::new("TIME is before midnight (#NUM!)", None));
            }
            Ok(Value::Number((total % SECONDS_PER_DAY) / SECONDS_PER_DAY))
        }
        _ => Err(Error::new(
            "TIME expects either no arguments or three arguments (hour, minute, second)",
            None,
        )),
    }
}

/// A `DATEADD`/`DATEDIFF` unit (B-15). The codes are VBA's `DateAdd`/`DateDiff` codes, the
/// closest Excel functions, so `"m"` is months and `"n"` minutes. The long words are Skillet's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Interval {
    Year,
    Quarter,
    Month,
    /// `"d"`, `"y"` (day of year) and the long words.
    Day,
    /// `"w"`: adds days, counts whole weeks (VBA).
    Weekday,
    /// `"ww"`: adds weeks, counts Sundays (VBA).
    CalendarWeek,
    /// `"weeks"`: adds and counts whole weeks.
    Week,
    Hour,
    Minute,
    Second,
}

const INTERVAL_UNITS: &str = "yyyy, q, m, y, d, w, ww, h, n, s, years, months, weeks, days, \
hours, minutes, seconds (#VALUE!)";

/// The unit argument, case-insensitive. Unknown units are `#VALUE!`, as VBA rejects them.
fn interval(func: &str, unit: Option<&Value>) -> Result<Interval, Error> {
    let Some(Value::String(unit)) = unit else {
        return Err(Error::new(
            format!("{func} expects string unit as third argument"),
            None,
        ));
    };
    Ok(match unit.to_ascii_lowercase().as_str() {
        "yyyy" | "years" | "year" => Interval::Year,
        "q" => Interval::Quarter,
        "m" | "months" | "month" => Interval::Month,
        "y" | "d" | "days" | "day" => Interval::Day,
        "w" => Interval::Weekday,
        "ww" => Interval::CalendarWeek,
        "weeks" | "week" => Interval::Week,
        "h" | "hours" | "hour" => Interval::Hour,
        "n" | "minutes" | "minute" => Interval::Minute,
        "s" | "seconds" | "second" => Interval::Second,
        _ => {
            return Err(Error::new(
                format!("{func} unit must be one of: {INTERVAL_UNITS}"),
                None,
            ))
        }
    })
}

fn datetime_arg(v: Option<&Value>, func: &str, which: &str) -> Result<DateTime<Utc>, Error> {
    let Some(Value::DateTime(ts)) = v else {
        return Err(Error::new(
            format!("{func} expects datetime as {which} argument"),
            None,
        ));
    };
    DateTime::from_timestamp(*ts, 0).ok_or_else(|| Error::new("Invalid timestamp", None))
}

/// `DATEADD(datetime, amount, unit)`, like VBA `DateAdd`: `amount` is truncated; months,
/// quarters and years keep the day, clamped to the end of a shorter month.
fn exec_dateadd(args: &[Value]) -> Result<Value, Error> {
    if args.len() < 3 {
        return Err(Error::new("DATEADD expects date, interval, unit", None));
    }
    let dt = datetime_arg(args.first(), "DATEADD", "first")?;
    let amount = match args.get(1) {
        Some(Value::Number(n)) if n.is_finite() => n.trunc() as i64,
        _ => {
            return Err(Error::new(
                "DATEADD expects number as second argument",
                None,
            ))
        }
    };
    let seconds = |per: i64| {
        amount
            .checked_mul(per)
            .and_then(TimeDelta::try_seconds)
            .and_then(|d| dt.checked_add_signed(d))
    };
    let result = match interval("DATEADD", args.get(2))? {
        Interval::Year => amount.checked_mul(12).and_then(|m| add_months(dt, m)),
        Interval::Quarter => amount.checked_mul(3).and_then(|m| add_months(dt, m)),
        Interval::Month => add_months(dt, amount),
        Interval::Day | Interval::Weekday => seconds(86_400),
        Interval::Week | Interval::CalendarWeek => seconds(7 * 86_400),
        Interval::Hour => seconds(3_600),
        Interval::Minute => seconds(60),
        Interval::Second => seconds(1),
    };
    result
        .map(|dt| Value::DateTime(dt.timestamp()))
        .ok_or_else(|| Error::new("DATEADD result is out of range (#NUM!)", None))
}

/// `dt` plus `months` calendar months, the day clamped to the last day of the target month.
fn add_months(dt: DateTime<Utc>, months: i64) -> Option<DateTime<Utc>> {
    let total = i64::from(dt.year())
        .checked_mul(12)?
        .checked_add(i64::from(dt.month0()))?
        .checked_add(months)?;
    let year = i32::try_from(total.div_euclid(12)).ok()?;
    let month = total.rem_euclid(12) as u32 + 1;
    let date = (dt.day().min(28)..=dt.day())
        .rev()
        .find_map(|day| NaiveDate::from_ymd_opt(year, month, day))?;
    Some(date.and_time(dt.time()).and_utc())
}

/// `DATEDIFF(date1, date2, unit)`: `date2 - date1` with VBA `DateDiff`'s codes. Years,
/// quarters, months and `"ww"` count calendar boundaries (`"ww"` counts Sundays, as in VBA);
/// the other units count whole elapsed units.
fn exec_datediff(args: &[Value]) -> Result<Value, Error> {
    if args.len() < 3 {
        return Err(Error::new("DATEDIFF expects date1, date2, unit", None));
    }
    let dt1 = datetime_arg(args.first(), "DATEDIFF", "first")?;
    let dt2 = datetime_arg(args.get(1), "DATEDIFF", "second")?;
    let elapsed = dt2.signed_duration_since(dt1);
    let months = |dt: DateTime<Utc>| i64::from(dt.year()) * 12 + i64::from(dt.month0());
    let sundays = |dt: DateTime<Utc>| i64::from(dt.date_naive().num_days_from_ce()).div_euclid(7);
    let diff = match interval("DATEDIFF", args.get(2))? {
        Interval::Year => i64::from(dt2.year() - dt1.year()),
        Interval::Quarter => months(dt2).div_euclid(3) - months(dt1).div_euclid(3),
        Interval::Month => months(dt2) - months(dt1),
        Interval::Day => elapsed.num_days(),
        Interval::Weekday | Interval::Week => elapsed.num_weeks(),
        Interval::CalendarWeek => sundays(dt2) - sundays(dt1),
        Interval::Hour => elapsed.num_hours(),
        Interval::Minute => elapsed.num_minutes(),
        Interval::Second => elapsed.num_seconds(),
    };
    Ok(Value::Number(diff as f64))
}

/// Excel `DATEDIF(start_date, end_date, unit)`: complete years (`"y"`), months (`"m"`) or days
/// (`"d"`), and `"md"`, `"ym"`, `"yd"`, which ignore the larger units. Times are ignored. An
/// unknown unit or `start_date` after `end_date` is `#NUM!`, as in Excel.
fn exec_datedif(args: &[Value]) -> Result<Value, Error> {
    let [start, end, unit] = args else {
        return Err(Error::new(
            "DATEDIF expects start_date, end_date, unit",
            None,
        ));
    };
    let start = datetime_arg(Some(start), "DATEDIF", "first")?.date_naive();
    let end = datetime_arg(Some(end), "DATEDIF", "second")?.date_naive();
    let Value::String(unit) = unit else {
        return Err(Error::new(
            "DATEDIF expects string unit as third argument",
            None,
        ));
    };
    if start > end {
        return Err(Error::new(
            "DATEDIF start_date is after end_date (#NUM!)",
            None,
        ));
    }
    let out_of_range = || Error::new("DATEDIF result is out of range (#NUM!)", None);
    let mut months = (i64::from(end.year()) - i64::from(start.year())) * 12
        + i64::from(end.month0())
        - i64::from(start.month0());
    if end.day() < start.day() {
        months -= 1;
    }
    let days = match unit.to_ascii_lowercase().as_str() {
        "y" => months / 12,
        "m" => months,
        "ym" => months % 12,
        "d" => (end - start).num_days(),
        "md" if end.day() >= start.day() => i64::from(end.day() - start.day()),
        // Excel: end - DATE(YEAR(end), MONTH(end) - 1, DAY(start)), which can be negative.
        "md" => {
            let from = rollover_date(end.year(), i64::from(end.month0()) - 1, start.day())
                .ok_or_else(out_of_range)?;
            (end - from).num_days()
        }
        // Excel: days since the last anniversary of start_date on or before end_date.
        "yd" => {
            let anniversary = |year| rollover_date(year, i64::from(start.month0()), start.day());
            let mut from = anniversary(end.year()).ok_or_else(out_of_range)?;
            if from > end {
                from = anniversary(end.year() - 1).ok_or_else(out_of_range)?;
            }
            (end - from).num_days()
        }
        _ => {
            return Err(Error::new(
                "DATEDIF unit must be one of: y, m, d, md, ym, yd (#NUM!)",
                None,
            ))
        }
    };
    Ok(Value::Number(days as f64))
}

/// Excel `DATE(year, month0 + 1, day)` with month and day rolling over.
fn rollover_date(year: i32, month0: i64, day: u32) -> Option<NaiveDate> {
    let total = i64::from(year).checked_mul(12)?.checked_add(month0)?;
    let first = NaiveDate::from_ymd_opt(
        i32::try_from(total.div_euclid(12)).ok()?,
        total.rem_euclid(12) as u32 + 1,
        1,
    )?;
    first.checked_add_signed(TimeDelta::try_days(i64::from(day) - 1)?)
}

/// Write a DateTime as text (ADR-0031): ISO 8601 UTC with whole seconds,
/// `2026-01-01T00:00:00Z`. Years outside 0000–9999 get a sign (`+10000`); a timestamp beyond
/// chrono's range falls back to its Unix seconds, which `parse_datetime_text` also reads.
///
/// Years 0000–9999 are written by hand (PERF-4, ADR-0034 §7): no chrono format parsing, one
/// write of 20 bytes. Other years take chrono's path.
pub(crate) fn write_datetime(w: &mut impl std::fmt::Write, ts: i64) -> std::fmt::Result {
    if let Some(text) = iso_four_digit_year(ts) {
        if let Ok(s) = std::str::from_utf8(&text) {
            return w.write_str(s);
        }
    }
    match DateTime::from_timestamp(ts, 0) {
        Some(dt) => write!(w, "{}", dt.format("%Y-%m-%dT%H:%M:%SZ")),
        None => write!(w, "{}", ts),
    }
}

/// `0000-01-01T00:00:00Z` and `9999-12-31T23:59:59Z`: the range of the 20-byte form.
const FOUR_DIGIT_YEARS: std::ops::RangeInclusive<i64> = -62_167_219_200..=253_402_300_799;

/// `YYYY-MM-DDTHH:MM:SSZ` for a timestamp in [`FOUR_DIGIT_YEARS`], else `None`.
fn iso_four_digit_year(ts: i64) -> Option<[u8; 20]> {
    if !FOUR_DIGIT_YEARS.contains(&ts) {
        return None;
    }
    let (days, secs) = (ts.div_euclid(86_400), ts.rem_euclid(86_400));
    let (year, month, day) = civil_from_days(days);
    let mut out = *b"0000-00-00T00:00:00Z";
    let mut put = |at: usize, v: i64| {
        out[at] = b'0' + (v / 10) as u8;
        out[at + 1] = b'0' + (v % 10) as u8;
    };
    put(0, year / 100);
    put(2, year % 100);
    put(5, month);
    put(8, day);
    put(11, secs / 3600);
    put(14, secs / 60 % 60);
    put(17, secs % 60);
    Some(out)
}

/// Proleptic Gregorian (year, month, day) of a day count from 1970-01-01 (H. Hinnant's
/// `civil_from_days`), integer arithmetic only.
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(month <= 2), month, day)
}

/// `write_datetime` into a new `String`.
pub(crate) fn datetime_text(ts: i64) -> String {
    let mut s = String::with_capacity(20);
    let _ = write_datetime(&mut s, ts);
    s
}

/// The DateTime a `::DateTime` cast reads from text: Unix seconds (`"1767225600"`) or the
/// ISO 8601 UTC form `write_datetime` writes. Strict: no other offsets, no fractions, no
/// rollover (`"2026-02-30T00:00:00Z"` is `None`).
pub(crate) fn parse_datetime_text(s: &str) -> Option<i64> {
    if let Ok(ts) = s.parse::<i64>() {
        return Some(ts);
    }
    // "<year>-MM-DDTHH:MM:SSZ": a fixed 16-byte tail after the year.
    let split = s.len().checked_sub(16)?;
    let (year, tail) = (s.get(..split)?, s.get(split..)?.as_bytes());
    let digits = year.strip_prefix(['+', '-']).unwrap_or(year);
    let shape = b"-dd-ddTdd:dd:ddZ";
    let fits = tail.iter().zip(shape).all(|(c, p)| match p {
        b'd' => c.is_ascii_digit(),
        _ => c == p,
    });
    if digits.len() < 4 || !digits.bytes().all(|b| b.is_ascii_digit()) || !fits {
        return None;
    }
    let field = |i: usize| u32::from(tail[i] - b'0') * 10 + u32::from(tail[i + 1] - b'0');
    let date = NaiveDate::from_ymd_opt(year.parse().ok()?, field(1), field(4))?;
    let time = NaiveTime::from_hms_opt(field(7), field(10), field(13))?;
    Some(date.and_time(time).and_utc().timestamp())
}

#[cfg(test)]
mod text_tests {
    use super::{datetime_text, parse_datetime_text};

    #[test]
    fn iso_text_round_trips() {
        for ts in [
            0,
            1_767_225_600,
            -1,
            253_402_300_799,
            253_402_300_800,
            -62_167_219_201,
            i64::MAX,
        ] {
            assert_eq!(parse_datetime_text(&datetime_text(ts)), Some(ts), "{ts}");
        }
        assert_eq!(datetime_text(1_767_225_600), "2026-01-01T00:00:00Z");
    }

    /// The hand-written 0000–9999 form is chrono's text, at the edges and in between.
    #[test]
    fn four_digit_years_match_chrono() {
        let chrono_text = |ts: i64| {
            chrono::DateTime::from_timestamp(ts, 0)
                .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
                .unwrap_or_default()
        };
        let (lo, hi) = (
            *super::FOUR_DIGIT_YEARS.start(),
            *super::FOUR_DIGIT_YEARS.end(),
        );
        let mut edges = vec![lo, lo + 1, hi - 1, hi, 0, -1, 951_782_400, 4_107_542_399];
        let mut x: u64 = 0x2545_F491_4F6C_DD1D;
        for _ in 0..100_000 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            edges.push(lo + (x % (hi - lo + 1) as u64) as i64);
        }
        for ts in edges {
            assert_eq!(datetime_text(ts), chrono_text(ts), "{ts}");
        }
        assert_eq!(datetime_text(lo - 1), "-0001-12-31T23:59:59Z");
        assert_eq!(datetime_text(hi + 1), "+10000-01-01T00:00:00Z");
    }

    #[test]
    fn rejects_other_shapes() {
        for s in [
            "2026-01-01",
            "2026-01-01T00:00:00",
            "2026-01-01 00:00:00Z",
            "2026-01-01T00:00:00+00:00",
            "2026-01-01T00:00:00.5Z",
            "2026-02-30T00:00:00Z",
            "2026-01-01T24:00:00Z",
            "2026-01-01T23:59:60Z",
            "26-01-01T00:00:00Z",
            "2026-1-01T00:00:00Z",
            "+-2026-01-01T00:00:00Z",
            "2026-01-01t00:00:00z",
            "",
            "héllo-01-01T00:00:00Z",
        ] {
            assert_eq!(parse_datetime_text(s), None, "{s}");
        }
    }
}
