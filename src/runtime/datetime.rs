use crate::error::Error;
use crate::types::Value;
use chrono::{DateTime, Datelike, Local, NaiveDate, NaiveTime, Timelike, Utc};

pub fn is_datetime_function(name: &str) -> bool {
    matches!(
        name,
        "NOW" | "DATE" | "TODAY" | "TIME" | "YEAR" | "MONTH" | "DAY" | "DATEADD" | "DATEDIFF"
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

/// Today's local date as midnight UTC (`DATE()` and `TODAY()`).
fn today() -> Value {
    let today = Local::now().date_naive();
    Value::DateTime(today.and_time(NaiveTime::MIN).and_utc().timestamp())
}

/// One `DATE` argument, truncated toward zero like Excel.
fn date_part(v: &Value, what: &str) -> Result<i64, Error> {
    match v {
        Value::Number(n) if n.is_finite() => Ok(n.trunc() as i64),
        _ => Err(Error::new(format!("DATE expects {} as number", what), None)),
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

/// `TIME(hour, minute, second)`: seconds since midnight, `hour*3600 + minute*60 + second`
/// (docs/reference/api.md). `TIME()` with no arguments keeps its legacy meaning: the current
/// local time as seconds since midnight.
fn exec_time(args: &[Value]) -> Result<Value, Error> {
    match args {
        [] => {
            let now = Local::now().time();
            Ok(Value::Number(now.num_seconds_from_midnight() as f64))
        }
        [hour, minute, second] => {
            let part = |v: &Value, what: &str| match v {
                Value::Number(n) if n.is_finite() => Ok(*n),
                _ => Err(Error::new(format!("TIME expects {} as number", what), None)),
            };
            let h = part(hour, "hour")?;
            let m = part(minute, "minute")?;
            let s = part(second, "second")?;
            Ok(Value::Number(h * 3600.0 + m * 60.0 + s))
        }
        _ => Err(Error::new(
            "TIME expects either no arguments or three arguments (hour, minute, second)",
            None,
        )),
    }
}
