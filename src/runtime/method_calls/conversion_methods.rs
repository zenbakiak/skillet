use crate::error::Error;
use crate::runtime::text::{json_text, string_chars, to_text};
use crate::types::Value;

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

/// `.to_s()`: the text form of the value (ADR-0034); an array's JSON text is charged first.
fn to_string(value: &Value) -> Result<Value, Error> {
    to_text(value).map(Value::String)
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
        Value::String(s) => string_chars(s)?,
        other => vec![other.clone()],
    };
    Ok(Value::Array(result))
}

/// `.to_json()`: the value's JSON text (ADR-0034 §4); NULL is `{}` and an object stays as held.
#[allow(deprecated)] // `Value::Json` until objects become `Value::Object` in 0.8 (ADR-0030)
fn to_json(value: &Value) -> Result<Value, Error> {
    match value {
        Value::Null => Ok(Value::Json("{}".to_string())),
        Value::Json(s) => Ok(Value::Json(s.clone())),
        other => json_text(other).map(Value::Json),
    }
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
