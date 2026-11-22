use crate::error::Error;
use crate::limits;
use crate::runtime::datetime::{datetime_text, write_datetime};
use crate::types::Value;
use std::fmt::Write;

/// Handle conversion method calls for all types (Ruby-style)
pub fn exec_conversion_method(name: &str, recv: &Value) -> Result<Value, Error> {
    let lname = name.to_lowercase();

    match lname.as_str() {
        "to_s" | "to_string" => to_string(recv),
        "to_i" | "to_int" => to_int(recv),
        "to_f" | "to_float" => to_float(recv),
        "to_a" | "to_array" => to_array(recv),
        "to_json" => to_json(recv),
        "to_bool" | "to_boolean" => to_boolean(recv),
        _ => Err(Error::new(
            format!("Unknown conversion method: {}", name),
            None,
        )),
    }
}

/// Convert any value to string. Arrays are counted first and pre-charged, since a number
/// can print far longer than it costs (`1e308` is 309 characters).
#[allow(deprecated)] // `Value::Currency` is read as a Number (ADR-0033)
fn to_string(value: &Value) -> Result<Value, Error> {
    if let Value::Array(_) = value {
        limits::charge_formatted(|c| write_s(c, value))?;
        let mut out = String::new();
        write_s(&mut out, value);
        return Ok(Value::String(out));
    }
    let result = match value {
        Value::Null => "".to_string(),
        Value::String(s) => s.clone(),
        Value::Number(n) | Value::Currency(n) => {
            if n.fract() == 0.0 {
                format!("{:.0}", n)
            } else {
                n.to_string()
            }
        }
        Value::Boolean(b) => b.to_string(),
        Value::Json(s) => s.clone(),
        Value::DateTime(dt) => datetime_text(*dt),
        Value::Array(_) => unreachable!("arrays are handled above"),
    };
    Ok(Value::String(result))
}

/// Write the `.to_s()` text of `value` (arrays as `[a, b]`, recursively).
#[allow(deprecated)] // `Value::Currency` is read as a Number (ADR-0033)
fn write_s(w: &mut impl Write, value: &Value) {
    let _ = match value {
        Value::Null => Ok(()),
        Value::String(s) | Value::Json(s) => w.write_str(s),
        Value::Number(n) | Value::Currency(n) if n.fract() == 0.0 => write!(w, "{:.0}", n),
        Value::Number(n) | Value::Currency(n) => write!(w, "{}", n),
        Value::Boolean(b) => write!(w, "{}", b),
        Value::DateTime(dt) => write_datetime(w, *dt),
        Value::Array(arr) => {
            let _ = w.write_char('[');
            for (i, v) in arr.iter().enumerate() {
                if i > 0 {
                    let _ = w.write_str(", ");
                }
                write_s(w, v);
            }
            w.write_char(']')
        }
    };
}

/// Convert any value to integer
#[allow(deprecated)] // `Value::Currency` is read as a Number (ADR-0033)
fn to_int(value: &Value) -> Result<Value, Error> {
    let result = match value {
        Value::Null => 0.0,
        Value::Number(n) | Value::Currency(n) => n.trunc(),
        Value::Boolean(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        Value::String(s) => s.trim().parse::<f64>().unwrap_or(0.0).trunc(),
        Value::Array(arr) => arr.len() as f64,
        Value::Json(_) => 1.0,     // JSON objects are truthy
        Value::DateTime(_) => 1.0, // DateTime values are truthy
    };
    Ok(Value::Number(result))
}

/// Convert any value to float
#[allow(deprecated)] // `Value::Currency` is read as a Number (ADR-0033)
fn to_float(value: &Value) -> Result<Value, Error> {
    let result = match value {
        Value::Null => 0.0,
        Value::Number(n) | Value::Currency(n) => *n,
        Value::Boolean(b) => {
            if *b {
                1.0
            } else {
                0.0
            }
        }
        Value::String(s) => s.trim().parse::<f64>().unwrap_or(0.0),
        Value::Array(arr) => arr.len() as f64,
        Value::Json(_) => 1.0,
        Value::DateTime(_) => 1.0,
    };
    Ok(Value::Number(result))
}

/// Convert any value to array
fn to_array(value: &Value) -> Result<Value, Error> {
    let result = match value {
        Value::Null => Vec::new(),
        Value::Array(arr) => arr.clone(),
        Value::String(s) => {
            // One string per character: 48 + its bytes each, charged before building.
            let chars = s.chars().count() as u64;
            limits::charge(32 + limits::STRING_COST * chars + s.len() as u64)?;
            s.chars().map(|c| Value::String(c.to_string())).collect()
        }
        other => vec![other.clone()],
    };
    Ok(Value::Array(result))
}

/// Convert any value to JSON
#[allow(deprecated)] // `Value::Currency` is read as a Number (ADR-0033)
fn to_json(value: &Value) -> Result<Value, Error> {
    let json_str = match value {
        Value::Null => "{}".to_string(),
        Value::Json(s) => s.clone(),
        Value::String(s) => {
            // Escaping can grow the text up to 6x (`\u0001`): charge the exact size first.
            limits::charge_json_string(s)?;
            serde_json::to_string(s)
                .map_err(|e| Error::new(format!("Failed to convert to JSON: {}", e), None))?
        }
        Value::Number(n) | Value::Currency(n) => {
            let json_val = serde_json::Value::Number(
                serde_json::Number::from_f64(*n)
                    .ok_or_else(|| Error::new("Invalid number for JSON", None))?,
            );
            serde_json::to_string(&json_val)
                .map_err(|e| Error::new(format!("Failed to convert to JSON: {}", e), None))?
        }
        Value::Boolean(b) => {
            let json_val = serde_json::Value::Bool(*b);
            serde_json::to_string(&json_val)
                .map_err(|e| Error::new(format!("Failed to convert to JSON: {}", e), None))?
        }
        Value::Array(arr) => {
            let json_array: Result<Vec<serde_json::Value>, Error> =
                arr.iter().map(value_to_json_value).collect();
            let json_val = serde_json::Value::Array(json_array?);
            // Size the escaped text without building it, then charge it before allocating.
            limits::charge_json_text(&json_val)?;
            serde_json::to_string(&json_val)
                .map_err(|e| Error::new(format!("Failed to convert to JSON: {}", e), None))?
        }
        Value::DateTime(dt) => {
            let json_val = serde_json::Value::String(datetime_text(*dt));
            serde_json::to_string(&json_val)
                .map_err(|e| Error::new(format!("Failed to convert to JSON: {}", e), None))?
        }
    };
    Ok(Value::Json(json_str))
}

/// Convert any value to boolean
#[allow(deprecated)] // `Value::Currency` is read as a Number (ADR-0033)
fn to_boolean(value: &Value) -> Result<Value, Error> {
    let result = match value {
        Value::Null => false,
        Value::Boolean(b) => *b,
        Value::Number(n) | Value::Currency(n) => *n != 0.0,
        Value::String(s) => !s.is_empty(),
        Value::Array(arr) => !arr.is_empty(),
        Value::Json(_) => true,
        Value::DateTime(_) => true,
    };
    Ok(Value::Boolean(result))
}

/// Helper function to convert Value to serde_json::Value
#[allow(deprecated)] // `Value::Currency` is read as a Number (ADR-0033)
fn value_to_json_value(value: &Value) -> Result<serde_json::Value, Error> {
    match value {
        Value::Null => Ok(serde_json::Value::Null),
        Value::String(s) => Ok(serde_json::Value::String(s.clone())),
        Value::Number(n) | Value::Currency(n) => serde_json::Number::from_f64(*n)
            .map(serde_json::Value::Number)
            .ok_or_else(|| Error::new("Invalid number for JSON", None)),
        Value::Boolean(b) => Ok(serde_json::Value::Bool(*b)),
        Value::Array(arr) => {
            let json_array: Result<Vec<serde_json::Value>, Error> =
                arr.iter().map(value_to_json_value).collect();
            Ok(serde_json::Value::Array(json_array?))
        }
        Value::Json(s) => {
            crate::limits::charge_json_parse(s)?;
            serde_json::from_str(s).map_err(|e| Error::new(format!("Invalid JSON: {}", e), None))
        }
        Value::DateTime(dt) => Ok(serde_json::Value::String(datetime_text(*dt))),
    }
}
