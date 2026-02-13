use crate::types::Value;
use crate::error::Error;
use crate::runtime::utils::values_equal;
use std::collections::BTreeSet;

pub fn exec_array(name: &str, args: &[Value]) -> Result<Value, Error> {
    match name {
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
        "IN" => {
            if args.len() != 2 {
                return Err(Error::new("IN expects 2 arguments: array, value", None));
            }
            if let Some(Value::Array(items)) = args.get(0) {
                let needle = &args[1];
                Ok(Value::Boolean(items.iter().any(|v| values_equal(v, needle))))
            } else { 
                Err(Error::new("IN expects array as first argument", None)) 
            }
        }
        "COUNT" => {
            if args.len() != 1 {
                return Err(Error::new("COUNT expects 1 argument: array", None));
            }
            match args.get(0) {
                Some(Value::Array(items)) => Ok(Value::Number(items.len() as f64)),
                Some(Value::Null) => Ok(Value::Number(0.0)),
                Some(_) => Err(Error::new("COUNT expects array", None)),
                None => Ok(Value::Number(0.0)),
            }
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
            Some(Value::Array(items)) => Ok(Value::Array(items.iter().rev().cloned().collect())),
            _ => Err(Error::new("REVERSE expects array", None))
        },
        "JOIN" => match args.get(0) {
            Some(Value::Array(items)) => {
                let sep = match args.get(1) { Some(Value::String(s)) => s.as_str(), _ => "," };
                let mut parts: Vec<String> = Vec::with_capacity(items.len());
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
        _ => Err(Error::new(format!("Unknown array function: {}", name), None)),
    }
}