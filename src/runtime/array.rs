use crate::error::Error;
use crate::runtime::utils::values_equal;
use crate::types::Value;
use std::collections::BTreeSet;

pub fn exec_array(name: &str, args: &[Value]) -> Result<Value, Error> {
    match name {
        "ARRAY" => Ok(Value::Array(args.to_vec())),
        "FLATTEN" => {
            fn flatten(v: &Value, out: &mut Vec<Value>) {
                match v {
                    Value::Array(items) => {
                        for it in items {
                            flatten(it, out);
                        }
                    }
                    other => out.push(other.clone()),
                }
            }
            let mut out = Vec::new();
            for a in args {
                flatten(a, &mut out);
            }
            Ok(Value::Array(out))
        }
        "FIRST" => match args.first() {
            Some(Value::Array(items)) => items
                .first()
                .cloned()
                .ok_or_else(|| Error::new("FIRST on empty array", None)),
            _ => Err(Error::new("FIRST expects array", None)),
        },
        "LAST" => match args.first() {
            Some(Value::Array(items)) => items
                .last()
                .cloned()
                .ok_or_else(|| Error::new("LAST on empty array", None)),
            _ => Err(Error::new("LAST expects array", None)),
        },
        "CONTAINS" => {
            if let Some(Value::Array(items)) = args.first() {
                let needle = args.get(1).cloned().unwrap_or(Value::Null);
                Ok(Value::Boolean(
                    items.iter().any(|v| values_equal(v, &needle)),
                ))
            } else {
                Err(Error::new("CONTAINS expects array, value", None))
            }
        }
        "IN" => {
            if args.len() != 2 {
                return Err(Error::new("IN expects 2 arguments: array, value", None));
            }
            if let Some(Value::Array(items)) = args.first() {
                let needle = &args[1];
                Ok(Value::Boolean(
                    items.iter().any(|v| values_equal(v, needle)),
                ))
            } else {
                Err(Error::new("IN expects array as first argument", None))
            }
        }
        // Excel COUNT: how many numbers (currency and datetime count too); arrays are flattened
        // and every other value is ignored. COUNTA: how many values are not NULL.
        "COUNT" | "COUNTA" if args.is_empty() => Err(Error::new(
            format!("{name} expects at least 1 argument"),
            None,
        )),
        "COUNT" => Ok(Value::Number(count_where(args, &|v| {
            matches!(
                v,
                Value::Number(_) | Value::Currency(_) | Value::DateTime(_)
            )
        }) as f64)),
        "COUNTA" => Ok(Value::Number(
            count_where(args, &|v| !matches!(v, Value::Null)) as f64,
        )),
        "UNIQUE" => match args.first() {
            Some(Value::Array(items)) => {
                let mut set = BTreeSet::new();
                let mut out = Vec::new();
                for (i, it) in items.iter().enumerate() {
                    crate::limits::tick_item(i)?;
                    if let Value::Number(n) = it {
                        if set.insert(n.to_bits()) {
                            out.push(Value::Number(*n));
                        }
                    }
                }
                Ok(Value::Array(out))
            }
            _ => Err(Error::new("UNIQUE expects array", None)),
        },
        "SORT" => match args.first() {
            Some(Value::Array(items)) => {
                // Excel's sort_order: 1 ascending, -1 descending; "ASC"/"DESC" are Skillet's.
                let desc = match args.get(1) {
                    None => false,
                    Some(Value::String(s)) if s.eq_ignore_ascii_case("DESC") => true,
                    Some(Value::String(s)) if s.eq_ignore_ascii_case("ASC") => false,
                    Some(Value::Number(n)) if *n == 1.0 => false,
                    Some(Value::Number(n)) if *n == -1.0 => true,
                    Some(_) => {
                        return Err(Error::new(
                            "SORT order must be 1, -1, \"ASC\" or \"DESC\" (#VALUE!)",
                            None,
                        ))
                    }
                };
                let mut nums: Vec<f64> = Vec::new();
                for it in items {
                    crate::limits::tick_item(nums.len())?;
                    if let Value::Number(n) = it {
                        nums.push(*n);
                    } else {
                        return Err(Error::new("SORT expects numeric array", None));
                    }
                }
                crate::runtime::utils::sort_numbers(&mut nums, false)?;
                if desc {
                    nums.reverse();
                }
                Ok(Value::Array(nums.into_iter().map(Value::Number).collect()))
            }
            _ => Err(Error::new("SORT expects array", None)),
        },
        "REVERSE" => match args.first() {
            Some(Value::Array(items)) => Ok(Value::Array(items.iter().rev().cloned().collect())),
            _ => Err(Error::new("REVERSE expects array", None)),
        },
        "JOIN" => match args.first() {
            Some(Value::Array(items)) => {
                let sep = match args.get(1) {
                    Some(Value::String(s)) => s.as_str(),
                    _ => ",",
                };
                let mut parts: Vec<String> = Vec::with_capacity(items.len());
                for it in items {
                    crate::limits::tick_item(parts.len())?;
                    match it {
                        Value::String(s) => parts.push(s.clone()),
                        Value::Number(n) => parts.push(n.to_string()),
                        Value::Boolean(b) => {
                            parts.push(if *b { "TRUE".into() } else { "FALSE".into() })
                        }
                        Value::Null => parts.push(String::new()),
                        Value::Currency(n) => parts.push(format!("{:.4}", n)),
                        Value::DateTime(ts) => parts.push(ts.to_string()),
                        Value::Json(s) => parts.push(s.clone()),
                        Value::Array(_) => {
                            return Err(Error::new("JOIN does not flatten nested arrays", None))
                        }
                    }
                }
                crate::limits::charge_join(&parts, sep)?;
                Ok(Value::String(parts.join(sep)))
            }
            _ => Err(Error::new("JOIN expects array, [separator]", None)),
        },
        "MERGE" => {
            // Charged before building; the evaluator does not charge the result again.
            crate::limits::charge_merge(args)?;
            // Estimate capacity: count array lengths + scalar elements
            let mut capacity = 0;
            for arg in args {
                capacity += match arg {
                    Value::Array(items) => items.len(),
                    _ => 1,
                };
            }

            let mut result = Vec::with_capacity(capacity);
            for arg in args {
                match arg {
                    Value::Array(items) => result.extend_from_slice(items),
                    other => result.push(other.clone()),
                }
            }
            Ok(Value::Array(result))
        }
        _ => Err(Error::new(
            format!("Unknown array function: {}", name),
            None,
        )),
    }
}

/// Count the values in `args` (arrays flattened) for which `keep` holds.
fn count_where(args: &[Value], keep: &dyn Fn(&Value) -> bool) -> usize {
    args.iter()
        .map(|a| match a {
            Value::Array(items) => count_where(items, keep),
            other => usize::from(keep(other)),
        })
        .sum()
}
