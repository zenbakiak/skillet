use crate::error::Error;
use crate::types::Value;
use std::collections::HashMap;

pub fn exec_statistical(name: &str, args: &[Value]) -> Result<Value, Error> {
    match name {
        "MEDIAN" => {
            let mut nums: Vec<f64> = Vec::new();
            fn collect_nums(v: &Value, nums: &mut Vec<f64>) {
                match v {
                    Value::Number(n) => nums.push(*n),
                    Value::Currency(n) => nums.push(*n),
                    Value::Array(items) => {
                        for item in items {
                            collect_nums(item, nums);
                        }
                    }
                    _ => {}
                }
            }
            for arg in args {
                collect_nums(arg, &mut nums);
            }
            if nums.is_empty() {
                return Ok(Value::Number(0.0));
            }
            crate::runtime::utils::sort_numbers(&mut nums, false)?;
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
                    Value::Array(items) => {
                        for item in items {
                            collect_nums(item, nums);
                        }
                    }
                    _ => {}
                }
            }
            for arg in args {
                collect_nums(arg, &mut nums);
            }
            if nums.is_empty() {
                return Ok(Value::Number(0.0));
            }

            let mut counts = HashMap::new();
            let mut first_occurrence = HashMap::new();
            for (index, &n) in nums.iter().enumerate() {
                let bits = n.to_bits();
                *counts.entry(bits).or_insert(0) += 1;
                first_occurrence.entry(bits).or_insert(index);
            }

            let max_count = *counts.values().max().unwrap();
            let mode_bits = counts
                .into_iter()
                .filter(|(_, count)| *count == max_count)
                .min_by_key(|(bits, _)| first_occurrence[bits])
                .unwrap()
                .0;

            Ok(Value::Number(f64::from_bits(mode_bits)))
        }
        "STDEV.P" | "STDEVP" | "STDEV_P" => {
            let mut nums: Vec<f64> = Vec::new();
            fn collect_nums(v: &Value, nums: &mut Vec<f64>) {
                match v {
                    Value::Number(n) => nums.push(*n),
                    Value::Currency(n) => nums.push(*n),
                    Value::Array(items) => {
                        for item in items {
                            collect_nums(item, nums);
                        }
                    }
                    _ => {}
                }
            }
            for arg in args {
                collect_nums(arg, &mut nums);
            }
            if nums.is_empty() {
                return Ok(Value::Number(0.0));
            }

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
                    Value::Array(items) => {
                        for item in items {
                            collect_nums(item, nums);
                        }
                    }
                    _ => {}
                }
            }
            for arg in args {
                collect_nums(arg, &mut nums);
            }
            if nums.is_empty() {
                return Ok(Value::Number(0.0));
            }

            let mean = nums.iter().sum::<f64>() / nums.len() as f64;
            let variance = nums.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / nums.len() as f64;
            Ok(Value::Number(variance))
        }
        "PERCENTILE.INC" | "PERCENTILEINC" | "PERCENTILE_INC" => {
            if args.len() < 2 {
                return Err(Error::new(
                    "PERCENTILE.INC expects array and percentile",
                    None,
                ));
            }
            let mut nums: Vec<f64> = Vec::new();
            fn collect_nums(v: &Value, nums: &mut Vec<f64>) {
                match v {
                    Value::Number(n) => nums.push(*n),
                    Value::Currency(n) => nums.push(*n),
                    Value::Array(items) => {
                        for item in items {
                            collect_nums(item, nums);
                        }
                    }
                    _ => {}
                }
            }
            for arg in &args[..args.len() - 1] {
                collect_nums(arg, &mut nums);
            }
            let percentile = match args.last() {
                Some(Value::Number(p)) => *p,
                _ => return Err(Error::new("Percentile must be a number", None)),
            };

            if nums.is_empty() {
                return Ok(Value::Number(0.0));
            }
            // NaN must pass through as before; `!contains` would reject it.
            #[allow(clippy::manual_range_contains)]
            if percentile < 0.0 || percentile > 1.0 {
                return Err(Error::new("Percentile must be between 0 and 1", None));
            }

            crate::runtime::utils::sort_numbers(&mut nums, false)?;
            let len = nums.len() as f64;
            let rank = percentile * (len - 1.0);
            let rank_floor = rank.floor() as usize;
            let rank_ceil = rank.ceil() as usize;

            if rank_floor == rank_ceil || rank_ceil >= nums.len() {
                Ok(Value::Number(nums[rank_floor.min(nums.len() - 1)]))
            } else {
                let weight = rank - rank_floor as f64;
                Ok(Value::Number(
                    nums[rank_floor] * (1.0 - weight) + nums[rank_ceil] * weight,
                ))
            }
        }
        "QUARTILE.INC" | "QUARTILEINC" | "QUARTILE_INC" => {
            if args.len() < 2 {
                return Err(Error::new("QUARTILE.INC expects array and quartile", None));
            }
            let mut nums: Vec<f64> = Vec::new();
            fn collect_nums(v: &Value, nums: &mut Vec<f64>) {
                match v {
                    Value::Number(n) => nums.push(*n),
                    Value::Currency(n) => nums.push(*n),
                    Value::Array(items) => {
                        for item in items {
                            collect_nums(item, nums);
                        }
                    }
                    _ => {}
                }
            }
            for arg in &args[..args.len() - 1] {
                collect_nums(arg, &mut nums);
            }
            let quartile = match args.last() {
                Some(Value::Number(q)) => *q as i32,
                _ => return Err(Error::new("Quartile must be a number", None)),
            };

            if nums.is_empty() {
                return Ok(Value::Number(0.0));
            }
            if !(0..=4).contains(&quartile) {
                return Err(Error::new("Quartile must be 0-4", None));
            }

            let percentile = quartile as f64 / 4.0;
            crate::runtime::utils::sort_numbers(&mut nums, false)?;
            let len = nums.len() as f64;
            let rank = percentile * (len - 1.0);
            let rank_floor = rank.floor() as usize;
            let rank_ceil = rank.ceil() as usize;

            if rank_floor == rank_ceil || rank_ceil >= nums.len() {
                Ok(Value::Number(nums[rank_floor.min(nums.len() - 1)]))
            } else {
                let weight = rank - rank_floor as f64;
                Ok(Value::Number(
                    nums[rank_floor] * (1.0 - weight) + nums[rank_ceil] * weight,
                ))
            }
        }
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

/// Numbers and currency in `v`, arrays flattened; other values are skipped.
fn collect_numbers(v: &Value, out: &mut Vec<f64>) {
    match v {
        Value::Number(n) | Value::Currency(n) => out.push(*n),
        Value::Array(items) => items.iter().for_each(|item| collect_numbers(item, out)),
        _ => {}
    }
}

/// Sample variance (divides by n - 1), as Excel `VAR.S`. Fewer than two numbers is an error
/// (Excel `#DIV/0!`).
fn sample_variance(name: &str, args: &[Value]) -> Result<f64, Error> {
    let mut nums = Vec::new();
    args.iter().for_each(|a| collect_numbers(a, &mut nums));
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
