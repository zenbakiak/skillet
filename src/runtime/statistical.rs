//! Statistical builtins. Excel is the spec (BL-054): no numbers → the Excel error (`#NUM!`,
//! `#DIV/0!`, `#N/A`) as a Skillet `Error`. ADR-0015 option B: a NaN or out-of-range `k`/`quart`
//! is an error, NaN data in an order statistic is an error, and nothing panics.

use crate::error::Error;
use crate::types::Value;
use std::collections::HashMap;

pub fn exec_statistical(name: &str, args: &[Value]) -> Result<Value, Error> {
    match name {
        "MEDIAN" => median(args),
        "MODE.SNGL" | "MODESNGL" | "MODE_SNGL" | "MODE" => mode(args),
        "STDEV.P" | "STDEVP" | "STDEV_P" => {
            Ok(Value::Number(variance(&numbers(args), "STDEV.P")?.sqrt()))
        }
        "VAR.P" | "VARP" | "VAR_P" => Ok(Value::Number(variance(&numbers(args), "VAR.P")?)),
        "PERCENTILE.INC" | "PERCENTILEINC" | "PERCENTILE_INC" | "PERCENTILE" => percentile(args),
        "QUARTILE.INC" | "QUARTILEINC" | "QUARTILE_INC" | "QUARTILE" => quartile(args),
        "STDEV.S" | "STDEV_S" | "STDEV" => {
            sample_variance(name, args).map(|v| Value::Number(v.sqrt()))
        }
        "VAR.S" | "VAR_S" | "VAR" => sample_variance(name, args).map(Value::Number),
        _ => Err(Error::new(
            format!("Unknown statistical function: {}", name),
            None,
        )),
    }
}

/// Every number and currency in the arguments, arrays flattened; other values are ignored.
fn numbers(args: &[Value]) -> Vec<f64> {
    fn collect(v: &Value, out: &mut Vec<f64>) {
        match v {
            Value::Number(n) | Value::Currency(n) => out.push(*n),
            Value::Array(items) => items.iter().for_each(|i| collect(i, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    args.iter().for_each(|a| collect(a, &mut out));
    out
}

fn median(args: &[Value]) -> Result<Value, Error> {
    let mut nums = numbers(args);
    if nums.is_empty() {
        return Err(Error::new("MEDIAN of no numbers (#NUM!)", None));
    }
    crate::runtime::utils::sort_numbers(&mut nums, false)?;
    let len = nums.len();
    Ok(Value::Number(if len % 2 == 0 {
        (nums[len / 2 - 1] + nums[len / 2]) / 2.0
    } else {
        nums[len / 2]
    }))
}

/// Most frequent number; the first one seen wins a tie. Excel: no repeated value → `#N/A`.
fn mode(args: &[Value]) -> Result<Value, Error> {
    let nums = numbers(args);
    let mut counts: HashMap<u64, (usize, usize)> = HashMap::new();
    for (index, n) in nums.iter().enumerate() {
        counts.entry(n.to_bits()).or_insert((0, index)).0 += 1;
    }
    let best = counts
        .into_iter()
        .filter(|(_, (count, _))| *count > 1)
        .max_by(|(_, (ca, fa)), (_, (cb, fb))| ca.cmp(cb).then(fb.cmp(fa)));
    match best {
        Some((bits, _)) => Ok(Value::Number(f64::from_bits(bits))),
        None => Err(Error::new("MODE.SNGL: no value repeats (#N/A)", None)),
    }
}

/// Population variance (divides by n). No numbers → `#DIV/0!`.
fn variance(nums: &[f64], name: &str) -> Result<f64, Error> {
    if nums.is_empty() {
        return Err(Error::new(format!("{name} of no numbers (#DIV/0!)"), None));
    }
    let len = nums.len() as f64;
    let mean = nums.iter().sum::<f64>() / len;
    Ok(nums.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / len)
}

/// Splits `values…, k`: the data and the last argument as a number.
fn data_and_last(args: &[Value], name: &str, what: &str) -> Result<(Vec<f64>, f64), Error> {
    let Some((last, data)) = args.split_last().filter(|_| args.len() >= 2) else {
        return Err(Error::new(format!("{name} expects array and {what}"), None));
    };
    let k = match last {
        Value::Number(k) | Value::Currency(k) => *k,
        _ => return Err(Error::new(format!("{name} {what} must be a number"), None)),
    };
    Ok((numbers(data), k))
}

/// Inclusive percentile of sorted, non-empty data, interpolated at rank `p * (n - 1)`.
fn interpolate(mut nums: Vec<f64>, p: f64, name: &str) -> Result<Value, Error> {
    if nums.is_empty() {
        return Err(Error::new(format!("{name} of no numbers (#NUM!)"), None));
    }
    crate::runtime::utils::sort_numbers(&mut nums, false)?;
    let rank = p * (nums.len() as f64 - 1.0);
    let lo = rank.floor() as usize;
    let hi = (rank.ceil() as usize).min(nums.len() - 1);
    let weight = rank - lo as f64;
    Ok(Value::Number(if lo == hi {
        nums[lo]
    } else {
        nums[lo] * (1.0 - weight) + nums[hi] * weight
    }))
}

fn percentile(args: &[Value]) -> Result<Value, Error> {
    let (nums, p) = data_and_last(args, "PERCENTILE.INC", "percentile")?;
    // `contains` is false for NaN, so NaN is rejected too (ADR-0015).
    if !(0.0..=1.0).contains(&p) {
        return Err(Error::new(
            "Percentile must be between 0 and 1 (#NUM!)",
            None,
        ));
    }
    interpolate(nums, p, "PERCENTILE.INC")
}

fn quartile(args: &[Value]) -> Result<Value, Error> {
    let (nums, q) = data_and_last(args, "QUARTILE.INC", "quartile")?;
    // Range-check before truncating, as Excel does: -0.5 and 4.9 are errors, 2.7 is quartile 2.
    if !(0.0..=4.0).contains(&q) {
        return Err(Error::new("Quartile must be 0-4 (#NUM!)", None));
    }
    interpolate(nums, q.trunc() / 4.0, "QUARTILE.INC")
}

/// Sample variance (divides by n - 1), as Excel `VAR.S`. Fewer than two numbers is an error
/// (Excel `#DIV/0!`).
fn sample_variance(name: &str, args: &[Value]) -> Result<f64, Error> {
    let nums = numbers(args);
    if nums.len() < 2 {
        return Err(Error::new(
            format!("{name} needs at least two numbers"),
            None,
        ));
    }
    let n = nums.len() as f64;
    let mean = nums.iter().sum::<f64>() / n;
    Ok(nums.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0))
}
