use crate::types::Value;
use crate::error::Error;
use crate::runtime::utils::{is_blank, values_equal};
use chrono::{DateTime, Local, NaiveDate, Utc, Datelike, Timelike};
use std::collections::{BTreeSet, HashMap};

pub fn exec_builtin(name: &str, args: &[Value]) -> Result<Value, Error> {
    match name {
        "__CONST_TRUE__" => Ok(Value::Boolean(true)),
        "__CONST_FALSE__" => Ok(Value::Boolean(false)),
        "__TERNARY__" => {
            if args.len() != 3 { return Err(Error::new("Ternary expects 3 args", None)); }
            let cond = args[0].as_bool().ok_or_else(|| Error::new("Ternary condition must be boolean", None))?;
            Ok(if cond { args[1].clone() } else { args[2].clone() })
        }
        "SUM" => {
            let mut acc = 0.0;
            fn sum_value(v: &Value, acc: &mut f64) {
                match v {
                    Value::Number(n) => *acc += *n,
                    Value::Array(items) => {
                        for it in items { sum_value(it, acc); }
                    }
                    Value::Boolean(_) => {}
                    Value::String(_) => {}
                    Value::Null => {}
                    Value::Currency(n) => *acc += *n,
                    Value::DateTime(_) => {}
                    Value::Json(_) => {}
                }
            }
            for a in args { sum_value(a, &mut acc); }
            Ok(Value::Number(acc))
        }
        "AVG" | "AVERAGE" => {
            let mut acc = 0.0;
            let mut count = 0usize;
            fn visit(v: &Value, acc: &mut f64, count: &mut usize) {
                match v {
                    Value::Number(n) => { *acc += *n; *count += 1; }
                    Value::Array(items) => for it in items { visit(it, acc, count); },
                    Value::Boolean(_) => {}
                    Value::String(_) => {}
                    Value::Null => {}
                    Value::Currency(n) => { *acc += *n; *count += 1; }
                    Value::DateTime(_) => {}
                    Value::Json(_) => {}
                }
            }
            for a in args { visit(a, &mut acc, &mut count); }
            let avg = if count == 0 { 0.0 } else { acc / count as f64 };
            Ok(Value::Number(avg))
        }
        "MIN" => {
            let mut cur: Option<f64> = None;
            fn visit(v: &Value, cur: &mut Option<f64>) {
                match v {
                    Value::Number(n) => { *cur = Some(cur.map_or(*n, |c| c.min(*n))); }
                    Value::Array(items) => for it in items { visit(it, cur); },
                    Value::Boolean(_) => {}
                    Value::String(_) => {}
                    Value::Null => {}
                    Value::Currency(n) => { *cur = Some(cur.map_or(*n, |c| c.min(*n))); }
                    Value::DateTime(_) => {}
                    Value::Json(_) => {}
                }
            }
            for a in args { visit(a, &mut cur); }
            Ok(Value::Number(cur.unwrap_or(0.0)))
        }
        "MAX" => {
            let mut cur: Option<f64> = None;
            fn visit(v: &Value, cur: &mut Option<f64>) {
                match v {
                    Value::Number(n) => { *cur = Some(cur.map_or(*n, |c| c.max(*n))); }
                    Value::Array(items) => for it in items { visit(it, cur); },
                    Value::Boolean(_) => {}
                    Value::String(_) => {}
                    Value::Null => {}
                    Value::Currency(n) => { *cur = Some(cur.map_or(*n, |c| c.max(*n))); }
                    Value::DateTime(_) => {}
                    Value::Json(_) => {}
                }
            }
            for a in args { visit(a, &mut cur); }
            Ok(Value::Number(cur.unwrap_or(0.0)))
        }
        "ROUND" => {
            if args.is_empty() { return Ok(Value::Number(0.0)); }
            let n = match args[0] { Value::Number(n) => n, _ => return Err(Error::new("ROUND expects number", None)) };
            let decimals = if args.len() > 1 { match args[1] { Value::Number(d) => d as i32, _ => 0 } } else { 0 };
            let factor = 10f64.powi(decimals.max(0));
            Ok(Value::Number((n * factor).round() / factor))
        }
        "CEIL" => {
            let n = match args.get(0) { Some(Value::Number(n)) => *n, _ => 0.0 };
            Ok(Value::Number(n.ceil()))
        }
        "FLOOR" => {
            let n = match args.get(0) { Some(Value::Number(n)) => *n, _ => 0.0 };
            Ok(Value::Number(n.floor()))
        }
        "ABS" => {
            let n = match args.get(0) { Some(Value::Number(n)) => *n, _ => 0.0 };
            Ok(Value::Number(n.abs()))
        }
        "SQRT" => {
            let n = match args.get(0) { Some(Value::Number(n)) => *n, _ => 0.0 };
            Ok(Value::Number(n.sqrt()))
        }
        "POW" | "POWER" => {
            let a = match args.get(0) { Some(Value::Number(n)) => *n, _ => 0.0 };
            let b = match args.get(1) { Some(Value::Number(n)) => *n, _ => 0.0 };
            Ok(Value::Number(a.powf(b)))
        }
        "MOD" => {
            let a = match args.get(0) { Some(Value::Number(n)) => *n, _ => 0.0 };
            let b = match args.get(1) { Some(Value::Number(n)) => *n, _ => 1.0 };
            Ok(Value::Number(a % b))
        }
        "INT" => {
            let n = match args.get(0) { Some(Value::Number(n)) => *n, _ => 0.0 };
            Ok(Value::Number(n.floor()))
        }
        "CEILING" => {
            let n = match args.get(0) { Some(Value::Number(n)) => *n, _ => 0.0 };
            let _significance = match args.get(1) { Some(Value::Number(s)) => *s, _ => 1.0 };
            Ok(Value::Number(n.ceil()))
        }
        "XOR" => {
            if args.len() != 2 { return Err(Error::new("XOR expects 2 arguments", None)); }
            let a = match &args[0] { Value::Boolean(b) => *b, Value::Number(n) => *n != 0.0, _ => false };
            let b = match &args[1] { Value::Boolean(b) => *b, Value::Number(n) => *n != 0.0, _ => false };
            Ok(Value::Boolean(a != b))
        }
        "AND" => {
            let mut result = true;
            for arg in args {
                let val = match arg { Value::Boolean(b) => *b, Value::Number(n) => *n != 0.0, _ => false };
                result = result && val;
                if !result { break; }
            }
            Ok(Value::Boolean(result))
        }
        "OR" => {
            let mut result = false;
            for arg in args {
                let val = match arg { Value::Boolean(b) => *b, Value::Number(n) => *n != 0.0, _ => false };
                result = result || val;
                if result { break; }
            }
            Ok(Value::Boolean(result))
        }
        "NOT" => {
            let val = match args.get(0) { Some(Value::Boolean(b)) => *b, Some(Value::Number(n)) => *n != 0.0, _ => false };
            Ok(Value::Boolean(!val))
        }
        "IF" => {
            if args.len() < 2 { return Err(Error::new("IF expects at least 2 arguments", None)); }
            let cond = match &args[0] { Value::Boolean(b) => *b, Value::Number(n) => *n != 0.0, _ => false };
            if cond {
                Ok(args[1].clone())
            } else {
                Ok(args.get(2).cloned().unwrap_or(Value::Boolean(false)))
            }
        }
        "IFS" => {
            if args.len() % 2 != 0 { return Err(Error::new("IFS expects pairs of condition,value arguments", None)); }
            for chunk in args.chunks(2) {
                if chunk.len() == 2 {
                    let cond = match &chunk[0] { Value::Boolean(b) => *b, Value::Number(n) => *n != 0.0, _ => false };
                    if cond {
                        return Ok(chunk[1].clone());
                    }
                }
            }
            Ok(Value::Boolean(false))
        }
        "LENGTH" => {
            match args.get(0) {
                Some(Value::Array(items)) => Ok(Value::Number(items.len() as f64)),
                Some(Value::String(s)) => Ok(Value::Number(s.chars().count() as f64)),
                Some(Value::Null) => Ok(Value::Number(0.0)),
                Some(_) | None => Err(Error::new("LENGTH expects array or string", None)),
            }
        }
        "CONCAT" => {
            let mut out = String::new();
            fn push_val(s: &mut String, v: &Value) -> Result<(), Error> {
                match v {
                    Value::String(st) => { s.push_str(st); Ok(()) }
                    Value::Number(n) => { s.push_str(&n.to_string()); Ok(()) }
                    Value::Array(arr) => { for it in arr { push_val(s, it)?; } Ok(()) }
                    Value::Boolean(b) => { s.push_str(if *b {"TRUE"} else {"FALSE"}); Ok(()) }
                    Value::Null => Ok(()),
                    Value::Currency(_) => Ok(()),
                    Value::DateTime(_) => Ok(()),
                    Value::Json(_) => Ok(())
                }
            }
            for a in args { if let Value::Null = a { /* skip */ } else { push_val(&mut out, a)?; } }
            Ok(Value::String(out))
        }
        "UPPER" => match args.get(0) { Some(Value::String(s)) => Ok(Value::String(s.to_uppercase())), _ => Err(Error::new("UPPER expects string", None)) },
        "LOWER" => match args.get(0) { Some(Value::String(s)) => Ok(Value::String(s.to_lowercase())), _ => Err(Error::new("LOWER expects string", None)) },
        "TRIM" => match args.get(0) { Some(Value::String(s)) => Ok(Value::String(s.trim().to_string())), _ => Err(Error::new("TRIM expects string", None)) },
        "SUBSTRING" => {
            if args.len() < 2 {
                return Err(Error::new("SUBSTRING expects string, start, [length]", None));
            }
            let string = match args.get(0) {
                Some(Value::String(s)) => s,
                _ => return Err(Error::new("SUBSTRING expects string as first argument", None)),
            };
            let start = match args.get(1) {
                Some(Value::Number(n)) => *n as usize,
                _ => return Err(Error::new("SUBSTRING expects number as second argument", None)),
            };
            
            // Convert to characters for proper Unicode handling
            let chars: Vec<char> = string.chars().collect();
            let string_len = chars.len();
            
            // Handle optional length parameter
            let end = if let Some(Value::Number(len)) = args.get(2) {
                let length = *len as usize;
                start.saturating_add(length).min(string_len)
            } else {
                string_len
            };
            
            // Clamp start to string bounds
            let start = start.min(string_len);
            let end = end.max(start);
            
            if start >= string_len {
                Ok(Value::String(String::new()))
            } else {
                let substring: String = chars[start..end].iter().collect();
                Ok(Value::String(substring))
            }
        }
        "ISBLANK" => {
            let v = args.get(0).cloned().unwrap_or(Value::Null);
            Ok(Value::Boolean(is_blank(&v)))
        }
        "ISNUMBER" => {
            let v = args.get(0).cloned().unwrap_or(Value::Null);
            Ok(Value::Boolean(matches!(v, Value::Number(_) | Value::Currency(_))))
        }
        "ISTEXT" => {
            let v = args.get(0).cloned().unwrap_or(Value::Null);
            Ok(Value::Boolean(matches!(v, Value::String(_))))
        }
        "ARRAY" => Ok(Value::Array(args.to_vec())),
        "FLATTEN" => {
            fn flatten(v: &Value, out: &mut Vec<Value>) {
                match v {
                    Value::Array(items) => { for it in items { flatten(it, out); } }
                    other => out.push(other.clone()),
                }
            }
            let mut out = Vec::new();
            for a in args { flatten(a, &mut out); }
            Ok(Value::Array(out))
        }
        "FIRST" => match args.get(0) { Some(Value::Array(items)) => items.first().cloned().ok_or_else(|| Error::new("FIRST on empty array", None)), _ => Err(Error::new("FIRST expects array", None)) },
        "LAST" => match args.get(0) { Some(Value::Array(items)) => items.last().cloned().ok_or_else(|| Error::new("LAST on empty array", None)), _ => Err(Error::new("LAST expects array", None)) },
        "CONTAINS" => {
            if let Some(Value::Array(items)) = args.get(0) {
                let needle = args.get(1).cloned().unwrap_or(Value::Null);
                Ok(Value::Boolean(items.iter().any(|v| values_equal(v, &needle))))
            } else { Err(Error::new("CONTAINS expects array, value", None)) }
        }
        "UNIQUE" => match args.get(0) {
            Some(Value::Array(items)) => {
                let mut set = BTreeSet::new();
                let mut out = Vec::new();
                for it in items { if let Value::Number(n) = it { if set.insert(n.to_bits()) { out.push(Value::Number(*n)); } } }
                Ok(Value::Array(out))
            }
            _ => Err(Error::new("UNIQUE expects array", None))
        },
        "SORT" => match args.get(0) {
            Some(Value::Array(items)) => {
                let desc = matches!(args.get(1), Some(Value::String(s)) if s.eq_ignore_ascii_case("DESC"));
                let mut nums: Vec<f64> = Vec::new();
                for it in items { if let Value::Number(n) = it { nums.push(*n); } else { return Err(Error::new("SORT expects numeric array", None)); } }
                nums.sort_by(|a,b| a.partial_cmp(b).unwrap());
                if desc { nums.reverse(); }
                Ok(Value::Array(nums.into_iter().map(Value::Number).collect()))
            }
            _ => Err(Error::new("SORT expects array", None))
        },
        "REVERSE" => match args.get(0) {
            Some(Value::Array(items)) => { let mut v = items.clone(); v.reverse(); Ok(Value::Array(v)) }
            Some(Value::String(s)) => Ok(Value::String(s.chars().rev().collect())),
            _ => Err(Error::new("REVERSE expects array or string", None))
        },
        "SPLIT" => match (args.get(0), args.get(1)) {
            (Some(Value::String(s)), Some(Value::String(sep))) => Ok(Value::Array(s.split(sep).map(|p| Value::String(p.to_string())).collect())),
            (Some(Value::String(s)), None) => Ok(Value::Array(s.split(',').map(|p| Value::String(p.trim().to_string())).collect())),
            _ => Err(Error::new("SPLIT expects string, [separator]", None))
        },
        "REPLACE" => match (args.get(0), args.get(1), args.get(2)) {
            (Some(Value::String(s)), Some(Value::String(from)), Some(Value::String(to))) => Ok(Value::String(s.replace(from, to))),
            _ => Err(Error::new("REPLACE expects string, search, replace", None))
        },
        "JOIN" => match args.get(0) {
            Some(Value::Array(items)) => {
                let sep = match args.get(1) { Some(Value::String(s)) => s.as_str(), _ => "," };
                let mut parts: Vec<String> = Vec::new();
                for it in items {
                    match it {
                        Value::String(s) => parts.push(s.clone()),
                        Value::Number(n) => parts.push(n.to_string()),
                        Value::Boolean(b) => parts.push(if *b {"TRUE".into()} else {"FALSE".into()}),
                        Value::Null => parts.push(String::new()),
                        Value::Currency(n) => parts.push(format!("{:.4}", n)),
                        Value::DateTime(ts) => parts.push(ts.to_string()),
                        Value::Json(s) => parts.push(s.clone()),
                        Value::Array(_) => return Err(Error::new("JOIN does not flatten nested arrays", None)),
                    }
                }
                Ok(Value::String(parts.join(sep)))
            }
            _ => Err(Error::new("JOIN expects array, [separator]", None))
        },
        "MEDIAN" => {
            let mut nums: Vec<f64> = Vec::new();
            fn collect_nums(v: &Value, nums: &mut Vec<f64>) {
                match v {
                    Value::Number(n) => nums.push(*n),
                    Value::Currency(n) => nums.push(*n),
                    Value::Array(items) => for item in items { collect_nums(item, nums); },
                    _ => {}
                }
            }
            for arg in args { collect_nums(arg, &mut nums); }
            if nums.is_empty() { return Ok(Value::Number(0.0)); }
            nums.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let len = nums.len();
            Ok(Value::Number(if len % 2 == 0 {
                (nums[len / 2 - 1] + nums[len / 2]) / 2.0
            } else {
                nums[len / 2]
            }))
        }
        "MODE.SNGL" | "MODESNGL" | "MODE_SNGL" => {
            let mut nums: Vec<f64> = Vec::new();
            fn collect_nums(v: &Value, nums: &mut Vec<f64>) {
                match v {
                    Value::Number(n) => nums.push(*n),
                    Value::Currency(n) => nums.push(*n),
                    Value::Array(items) => for item in items { collect_nums(item, nums); },
                    _ => {}
                }
            }
            for arg in args { collect_nums(arg, &mut nums); }
            if nums.is_empty() { return Ok(Value::Number(0.0)); }
            
            let mut counts = HashMap::new();
            let mut first_occurrence = HashMap::new();
            for (index, &n) in nums.iter().enumerate() {
                let bits = n.to_bits();
                *counts.entry(bits).or_insert(0) += 1;
                first_occurrence.entry(bits).or_insert(index);
            }
            
            let max_count = *counts.values().max().unwrap();
            let mode_bits = counts.into_iter()
                .filter(|(_, count)| *count == max_count)
                .min_by_key(|(bits, _)| first_occurrence[bits])
                .unwrap().0;
            
            Ok(Value::Number(f64::from_bits(mode_bits)))
        }
        "STDEV.P" | "STDEVP" | "STDEV_P" => {
            let mut nums: Vec<f64> = Vec::new();
            fn collect_nums(v: &Value, nums: &mut Vec<f64>) {
                match v {
                    Value::Number(n) => nums.push(*n),
                    Value::Currency(n) => nums.push(*n),
                    Value::Array(items) => for item in items { collect_nums(item, nums); },
                    _ => {}
                }
            }
            for arg in args { collect_nums(arg, &mut nums); }
            if nums.is_empty() { return Ok(Value::Number(0.0)); }
            
            let mean = nums.iter().sum::<f64>() / nums.len() as f64;
            let variance = nums.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / nums.len() as f64;
            Ok(Value::Number(variance.sqrt()))
        }
        "VAR.P" | "VARP" | "VAR_P" => {
            let mut nums: Vec<f64> = Vec::new();
            fn collect_nums(v: &Value, nums: &mut Vec<f64>) {
                match v {
                    Value::Number(n) => nums.push(*n),
                    Value::Currency(n) => nums.push(*n),
                    Value::Array(items) => for item in items { collect_nums(item, nums); },
                    _ => {}
                }
            }
            for arg in args { collect_nums(arg, &mut nums); }
            if nums.is_empty() { return Ok(Value::Number(0.0)); }
            
            let mean = nums.iter().sum::<f64>() / nums.len() as f64;
            let variance = nums.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / nums.len() as f64;
            Ok(Value::Number(variance))
        }
        "PERCENTILE.INC" | "PERCENTILEINC" | "PERCENTILE_INC" => {
            if args.len() < 2 { return Err(Error::new("PERCENTILE.INC expects array and percentile", None)); }
            let mut nums: Vec<f64> = Vec::new();
            fn collect_nums(v: &Value, nums: &mut Vec<f64>) {
                match v {
                    Value::Number(n) => nums.push(*n),
                    Value::Currency(n) => nums.push(*n),
                    Value::Array(items) => for item in items { collect_nums(item, nums); },
                    _ => {}
                }
            }
            for i in 0..args.len()-1 { collect_nums(&args[i], &mut nums); }
            let percentile = match args.last() { Some(Value::Number(p)) => *p, _ => return Err(Error::new("Percentile must be a number", None)) };
            
            if nums.is_empty() { return Ok(Value::Number(0.0)); }
            if percentile < 0.0 || percentile > 1.0 { return Err(Error::new("Percentile must be between 0 and 1", None)); }
            
            nums.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let len = nums.len() as f64;
            let rank = percentile * (len - 1.0);
            let rank_floor = rank.floor() as usize;
            let rank_ceil = rank.ceil() as usize;
            
            if rank_floor == rank_ceil || rank_ceil >= nums.len() {
                Ok(Value::Number(nums[rank_floor.min(nums.len() - 1)]))
            } else {
                let weight = rank - rank_floor as f64;
                Ok(Value::Number(nums[rank_floor] * (1.0 - weight) + nums[rank_ceil] * weight))
            }
        }
        "QUARTILE.INC" | "QUARTILEINC" | "QUARTILE_INC" => {
            if args.len() < 2 { return Err(Error::new("QUARTILE.INC expects array and quartile", None)); }
            let mut nums: Vec<f64> = Vec::new();
            fn collect_nums(v: &Value, nums: &mut Vec<f64>) {
                match v {
                    Value::Number(n) => nums.push(*n),
                    Value::Currency(n) => nums.push(*n),
                    Value::Array(items) => for item in items { collect_nums(item, nums); },
                    _ => {}
                }
            }
            for i in 0..args.len()-1 { collect_nums(&args[i], &mut nums); }
            let quartile = match args.last() { Some(Value::Number(q)) => *q as i32, _ => return Err(Error::new("Quartile must be a number", None)) };
            
            if nums.is_empty() { return Ok(Value::Number(0.0)); }
            if quartile < 0 || quartile > 4 { return Err(Error::new("Quartile must be 0-4", None)); }
            
            let percentile = quartile as f64 / 4.0;
            nums.sort_by(|a, b| a.partial_cmp(b).unwrap());
            let len = nums.len() as f64;
            let rank = percentile * (len - 1.0);
            let rank_floor = rank.floor() as usize;
            let rank_ceil = rank.ceil() as usize;
            
            if rank_floor == rank_ceil || rank_ceil >= nums.len() {
                Ok(Value::Number(nums[rank_floor.min(nums.len() - 1)]))
            } else {
                let weight = rank - rank_floor as f64;
                Ok(Value::Number(nums[rank_floor] * (1.0 - weight) + nums[rank_ceil] * weight))
            }
        }
        
        // Date/Time Functions
        "NOW" => {
            let now = Utc::now();
            Ok(Value::DateTime(now.timestamp()))
        }
        "DATE" => {
            let today = Local::now().date_naive();
            let timestamp = today.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp();
            Ok(Value::DateTime(timestamp))
        }
        "TIME" => {
            let now = Local::now().time();
            let seconds_since_midnight = now.num_seconds_from_midnight() as f64;
            Ok(Value::Number(seconds_since_midnight))
        }
        "YEAR" => {
            if let Some(Value::DateTime(timestamp)) = args.get(0) {
                let dt = DateTime::from_timestamp(*timestamp, 0)
                    .ok_or_else(|| Error::new("Invalid timestamp", None))?;
                Ok(Value::Number(dt.year() as f64))
            } else {
                Err(Error::new("YEAR expects datetime", None))
            }
        }
        "MONTH" => {
            if let Some(Value::DateTime(timestamp)) = args.get(0) {
                let dt = DateTime::from_timestamp(*timestamp, 0)
                    .ok_or_else(|| Error::new("Invalid timestamp", None))?;
                Ok(Value::Number(dt.month() as f64))
            } else {
                Err(Error::new("MONTH expects datetime", None))
            }
        }
        "DAY" => {
            if let Some(Value::DateTime(timestamp)) = args.get(0) {
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
            let timestamp = match args.get(0) {
                Some(Value::DateTime(ts)) => *ts,
                _ => return Err(Error::new("DATEADD expects datetime as first argument", None)),
            };
            let interval = match args.get(1) {
                Some(Value::Number(n)) => *n as i64,
                _ => return Err(Error::new("DATEADD expects number as second argument", None)),
            };
            let unit = match args.get(2) {
                Some(Value::String(s)) => s.to_lowercase(),
                _ => return Err(Error::new("DATEADD expects string unit as third argument", None)),
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
                    // Handle months specially since Duration doesn't support months
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
            let timestamp1 = match args.get(0) {
                Some(Value::DateTime(ts)) => *ts,
                _ => return Err(Error::new("DATEDIFF expects datetime as first argument", None)),
            };
            let timestamp2 = match args.get(1) {
                Some(Value::DateTime(ts)) => *ts,
                _ => return Err(Error::new("DATEDIFF expects datetime as second argument", None)),
            };
            let unit = match args.get(2) {
                Some(Value::String(s)) => s.to_lowercase(),
                _ => return Err(Error::new("DATEDIFF expects string unit as third argument", None)),
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
                    // Approximate months calculation
                    let years_diff = dt2.year() - dt1.year();
                    let months_diff = dt2.month() as i32 - dt1.month() as i32;
                    (years_diff * 12 + months_diff) as f64
                }
                "years" | "year" | "y" => (dt2.year() - dt1.year()) as f64,
                _ => return Err(Error::new("DATEDIFF unit must be one of: days, hours, minutes, seconds, weeks, months, years", None)),
            };
            
            Ok(Value::Number(diff))
        }
        
        // Financial Functions
        "PMT" => {
            // PMT(rate, nper, pv, [fv], [type])
            // Calculates the payment for a loan based on constant payments and a constant interest rate
            if args.len() < 3 || args.len() > 5 {
                return Err(Error::new("PMT expects 3-5 arguments: rate, nper, pv, [fv], [type]", None));
            }
            
            let rate = args[0].as_number().ok_or_else(|| Error::new("PMT rate must be a number", None))?;
            let nper = args[1].as_number().ok_or_else(|| Error::new("PMT nper must be a number", None))?;
            let pv = args[2].as_number().ok_or_else(|| Error::new("PMT pv must be a number", None))?;
            let fv = args.get(3).and_then(|v| v.as_number()).unwrap_or(0.0);
            let payment_type = args.get(4).and_then(|v| v.as_number()).unwrap_or(0.0);
            
            // Validate inputs
            if nper <= 0.0 {
                return Err(Error::new("PMT nper must be positive", None));
            }
            
            let payment_at_beginning = payment_type != 0.0;
            
            let pmt = if rate == 0.0 {
                // Special case: no interest
                -(pv + fv) / nper
            } else {
                // Standard PMT formula
                let pvif = (1.0 + rate).powf(nper);
                let payment = -(pv * pvif + fv) / (((pvif - 1.0) / rate) * if payment_at_beginning { 1.0 + rate } else { 1.0 });
                payment
            };
            
            Ok(Value::Number(pmt))
        }
        
        // SUMIF/AVGIF/COUNTIF handled in FunctionCall branch to preserve lambda expr
        _ => Err(Error::new(format!("Unknown function: {}", name), None)),
    }
}