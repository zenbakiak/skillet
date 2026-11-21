use crate::error::Error;
use crate::types::Value;
use chrono::{DateTime, Datelike, Local, NaiveDate, Timelike, Utc};

pub fn is_datetime_function(name: &str) -> bool {
    matches!(
        name,
        "NOW" | "DATE" | "TIME" | "YEAR" | "MONTH" | "DAY" | "DATEADD" | "DATEDIFF"
    )
}

pub fn exec_datetime(name: &str, args: &[Value]) -> Result<Value, Error> {
    match name {
        "NOW" => {
            let now = Utc::now();
            Ok(Value::DateTime(now.timestamp()))
        }
        "DATE" => {
            if args.is_empty() {
                // No arguments - return today's date
                let today = Local::now().date_naive();
                let timestamp = today.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp();
                Ok(Value::DateTime(timestamp))
            } else if args.len() == 3 {
                // Three arguments - year, month, day
                let year = match args.first() {
                    Some(Value::Number(n)) => *n as i32,
                    _ => return Err(Error::new("DATE expects year as number", None)),
                };
                let month = match args.get(1) {
                    Some(Value::Number(n)) => *n as u32,
                    _ => return Err(Error::new("DATE expects month as number", None)),
                };
                let day = match args.get(2) {
                    Some(Value::Number(n)) => *n as u32,
                    _ => return Err(Error::new("DATE expects day as number", None)),
                };

                // Validate and create the date
                let date = NaiveDate::from_ymd_opt(year, month, day)
                    .ok_or_else(|| Error::new("Invalid date", None))?;
                let timestamp = date.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp();
                Ok(Value::DateTime(timestamp))
            } else {
                Err(Error::new(
                    "DATE expects either no arguments or three arguments (year, month, day)",
                    None,
                ))
            }
        }
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
        "DATEADD" => {
            if args.len() < 3 {
                return Err(Error::new("DATEADD expects date, interval, unit", None));
            }
            let timestamp = match args.first() {
                Some(Value::DateTime(ts)) => *ts,
                _ => {
                    return Err(Error::new(
                        "DATEADD expects datetime as first argument",
                        None,
                    ))
                }
            };
            let interval = match args.get(1) {
                Some(Value::Number(n)) => *n as i64,
                _ => {
                    return Err(Error::new(
                        "DATEADD expects number as second argument",
                        None,
                    ))
                }
            };
            let unit = match args.get(2) {
                Some(Value::String(s)) => s.to_lowercase(),
                _ => {
                    return Err(Error::new(
                        "DATEADD expects string unit as third argument",
                        None,
                    ))
                }
            };

            let dt = DateTime::from_timestamp(timestamp, 0)
                .ok_or_else(|| Error::new("Invalid timestamp", None))?;

            let new_dt = match unit.as_str() {
                "days" | "day" | "d" => dt + chrono::Duration::days(interval),
                "hours" | "hour" | "h" => dt + chrono::Duration::hours(interval),
                "minutes" | "minute" | "m" => dt + chrono::Duration::minutes(interval),
                "seconds" | "second" | "s" => dt + chrono::Duration::seconds(interval),
                "weeks" | "week" | "w" => dt + chrono::Duration::weeks(interval),
                "months" | "month" => {
                    let mut year = dt.year();
                    let mut month = dt.month() as i32;
                    month += interval as i32;
                    while month > 12 {
                        year += 1;
                        month -= 12;
                    }
                    while month < 1 {
                        year -= 1;
                        month += 12;
                    }
                    let new_date = NaiveDate::from_ymd_opt(year, month as u32, dt.day())
                        .unwrap_or_else(|| NaiveDate::from_ymd_opt(year, month as u32, 28).unwrap());
                    new_date.and_time(dt.time()).and_utc()
                }
                "years" | "year" | "y" => {
                    let new_year = dt.year() + interval as i32;
                    let new_date = NaiveDate::from_ymd_opt(new_year, dt.month(), dt.day())
                        .unwrap_or_else(|| NaiveDate::from_ymd_opt(new_year, dt.month(), 28).unwrap());
                    new_date.and_time(dt.time()).and_utc()
                }
                _ => return Err(Error::new("DATEADD unit must be one of: days, hours, minutes, seconds, weeks, months, years", None)),
            };

            Ok(Value::DateTime(new_dt.timestamp()))
        }
        "DATEDIFF" => {
            if args.len() < 3 {
                return Err(Error::new("DATEDIFF expects date1, date2, unit", None));
            }
            let timestamp1 = match args.first() {
                Some(Value::DateTime(ts)) => *ts,
                _ => {
                    return Err(Error::new(
                        "DATEDIFF expects datetime as first argument",
                        None,
                    ))
                }
            };
            let timestamp2 = match args.get(1) {
                Some(Value::DateTime(ts)) => *ts,
                _ => {
                    return Err(Error::new(
                        "DATEDIFF expects datetime as second argument",
                        None,
                    ))
                }
            };
            let unit = match args.get(2) {
                Some(Value::String(s)) => s.to_lowercase(),
                _ => {
                    return Err(Error::new(
                        "DATEDIFF expects string unit as third argument",
                        None,
                    ))
                }
            };

            let dt1 = DateTime::from_timestamp(timestamp1, 0)
                .ok_or_else(|| Error::new("Invalid timestamp1", None))?;
            let dt2 = DateTime::from_timestamp(timestamp2, 0)
                .ok_or_else(|| Error::new("Invalid timestamp2", None))?;

            let duration = dt2.signed_duration_since(dt1);

            let diff = match unit.as_str() {
                "days" | "day" | "d" => duration.num_days() as f64,
                "hours" | "hour" | "h" => duration.num_hours() as f64,
                "minutes" | "minute" | "m" => duration.num_minutes() as f64,
                "seconds" | "second" | "s" => duration.num_seconds() as f64,
                "weeks" | "week" | "w" => duration.num_weeks() as f64,
                "months" | "month" => {
                    let years_diff = dt2.year() - dt1.year();
                    let months_diff = dt2.month() as i32 - dt1.month() as i32;
                    (years_diff * 12 + months_diff) as f64
                }
                "years" | "year" | "y" => (dt2.year() - dt1.year()) as f64,
                _ => return Err(Error::new("DATEDIFF unit must be one of: days, hours, minutes, seconds, weeks, months, years", None)),
            };

            Ok(Value::Number(diff))
        }
        _ => Err(Error::new(
            format!("Unknown datetime function: {}", name),
            None,
        )),
    }
}

/// Seconds in a day: `TIME` returns a fraction of a day.
const SECONDS_PER_DAY: f64 = 86400.0;

/// `TIME(hour, minute, second)`: the time as a fraction of a day (Excel, ADR-0018), with Excel's
/// truncation, limits and 24-hour wrap: `TIME(12, 0, 0)` is 0.5. `TIME()` with no arguments is
/// the current local time in the same unit.
fn exec_time(args: &[Value]) -> Result<Value, Error> {
    match args {
        [] => {
            let now = Local::now().time();
            Ok(Value::Number(
                f64::from(now.num_seconds_from_midnight()) / SECONDS_PER_DAY,
            ))
        }
        [hour, minute, second] => {
            // Excel TIME: each part is truncated and must be 0..=32767 (a negative minute or
            // second is allowed while the total stays non-negative). The total wraps at 24
            // hours, so TIME(25, 0, 0) = TIME(1, 0, 0). The result is a fraction of a day, as in
            // Excel (ADR-0018), so DATE(...) + TIME(...) adds the time to a date.
            let part = |v: &Value, what: &str| match v {
                Value::Number(n) if n.is_finite() && *n <= 32767.0 => Ok(n.trunc()),
                Value::Number(_) => Err(Error::new(
                    format!("TIME {} must be at most 32767 (#NUM!)", what),
                    None,
                )),
                _ => Err(Error::new(format!("TIME expects {} as number", what), None)),
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
