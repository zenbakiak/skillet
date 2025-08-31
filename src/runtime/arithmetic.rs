use crate::types::Value;
use crate::error::Error;

pub fn exec_arithmetic(name: &str, args: &[Value]) -> Result<Value, Error> {
    match name {
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
        _ => Err(Error::new(format!("Unknown arithmetic function: {}", name), None)),
    }
}