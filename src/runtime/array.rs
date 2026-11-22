use crate::error::Error;
use crate::runtime::datetime::datetime_text;
use crate::runtime::utils::values_equal;
use crate::types::Value;
use std::cmp::Ordering;
use std::collections::HashSet;

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
            Some(Value::Array(items)) => unique(items).map(Value::Array),
            _ => Err(Error::new("UNIQUE expects array", None)),
        },
        "SORT" => match args.first() {
            Some(Value::Array(items)) => sort(items, args.get(1)),
            _ => Err(Error::new("SORT expects array", None)),
        },
        // Items in reverse order; a string reverses its characters (excel-compat B-11).
        "REVERSE" => match args.first() {
            Some(Value::Array(items)) => Ok(Value::Array(items.iter().rev().cloned().collect())),
            Some(Value::String(s)) => Ok(Value::String(s.chars().rev().collect())),
            _ => Err(Error::new("REVERSE expects array or string", None)),
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
                        Value::DateTime(ts) => parts.push(datetime_text(*ts)),
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

/// Excel `UNIQUE`: every distinct value, in first-seen order, with Skillet's `==` (ADR-0023:
/// same type and value, text case-sensitive). Scalars are found by hash; arrays and JSON objects
/// are compared with the values kept so far.
fn unique(items: &[Value]) -> Result<Vec<Value>, Error> {
    #[derive(PartialEq, Eq, Hash)]
    enum Key<'a> {
        Number(u64),
        Currency(u64),
        Text(&'a str),
        Boolean(bool),
        DateTime(i64),
        Null,
    }
    // `+ 0.0` makes -0.0 and 0.0 one key, as `==` does. NaN never equals itself, so each is kept.
    let bits = |n: f64| (n + 0.0).to_bits();
    let mut seen = HashSet::new();
    let mut out: Vec<Value> = Vec::new();
    for (i, it) in items.iter().enumerate() {
        crate::limits::tick_item(i)?;
        let key = match it {
            Value::Number(n) if !n.is_nan() => Some(Key::Number(bits(*n))),
            Value::Currency(n) if !n.is_nan() => Some(Key::Currency(bits(*n))),
            Value::String(s) => Some(Key::Text(s)),
            Value::Boolean(b) => Some(Key::Boolean(*b)),
            Value::DateTime(t) => Some(Key::DateTime(*t)),
            Value::Null => Some(Key::Null),
            Value::Number(_) | Value::Currency(_) => {
                out.push(it.clone());
                continue;
            }
            Value::Array(_) | Value::Json(_) => None,
        };
        let new = match key {
            Some(k) => seen.insert(k),
            None => !out.iter().any(|v| values_equal(v, it)),
        };
        if new {
            out.push(it.clone());
        }
    }
    Ok(out)
}

/// Excel `SORT(array, [order])` on one column: numbers (and currency) < text < booleans; text
/// ignores case (Unicode lowercase, then code point); `FALSE < TRUE`. Equal items keep their
/// order, in both directions. Other values (NULL, DateTime, arrays, JSON) and NaN are an error.
fn sort(items: &[Value], order: Option<&Value>) -> Result<Value, Error> {
    // Excel's sort_order: 1 ascending, -1 descending; "ASC"/"DESC" are Skillet's.
    let desc = match order {
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
    enum Key {
        Number(f64),
        Text(String),
        Boolean(bool),
    }
    let mut keyed = Vec::with_capacity(items.len());
    for it in items {
        crate::limits::tick_item(keyed.len())?;
        let key = match it {
            Value::Number(n) | Value::Currency(n) if n.is_nan() => {
                return Err(Error::new("Cannot sort NaN values", None))
            }
            Value::Number(n) | Value::Currency(n) => Key::Number(*n),
            Value::String(s) => Key::Text(s.to_lowercase()),
            Value::Boolean(b) => Key::Boolean(*b),
            _ => return Err(Error::new("SORT expects numbers, text or booleans", None)),
        };
        keyed.push((key, it));
    }
    let cmp = |a: &Key, b: &Key| match (a, b) {
        (Key::Number(x), Key::Number(y)) => x.partial_cmp(y).unwrap_or(Ordering::Equal),
        (Key::Text(x), Key::Text(y)) => x.cmp(y),
        (Key::Boolean(x), Key::Boolean(y)) => x.cmp(y),
        (Key::Number(_), _) | (Key::Text(_), Key::Boolean(_)) => Ordering::Less,
        _ => Ordering::Greater,
    };
    if desc {
        keyed.sort_by(|a, b| cmp(&b.0, &a.0));
    } else {
        keyed.sort_by(|a, b| cmp(&a.0, &b.0));
    }
    Ok(Value::Array(
        keyed.into_iter().map(|(_, v)| v.clone()).collect(),
    ))
}
