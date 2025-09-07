use crate::error::Error;
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
        _ => Err(Error::new(format!("Unknown conversion method: {}", name), None)),
    }
}

/// Convert any value to string
fn to_string(value: &Value) -> Result<Value, Error> {
    let result = match value {
        Value::Null => "".to_string(),
        Value::String(s) => s.clone(),
        Value::Number(n) => {
            if n.fract() == 0.0 {
                format!("{:.0}", n)
            } else {
                n.to_string()
            }
        }
        Value::Boolean(b) => b.to_string(),
        Value::Currency(c) => format!("{:.2}", c),
        Value::Array(arr) => {
            let string_parts: Result<Vec<String>, Error> = arr
                .iter()
                .map(|v| match to_string(v)? {
                    Value::String(s) => Ok(s),
                    _ => unreachable!(),
                })
                .collect();
            format!("[{}]", string_parts?.join(", "))
        }
        Value::Json(s) => s.clone(),
        Value::DateTime(dt) => dt.to_string(),
    };
    Ok(Value::String(result))
}

/// Convert any value to integer
fn to_int(value: &Value) -> Result<Value, Error> {
    let result = match value {
        Value::Null => 0.0,
        Value::Number(n) => n.trunc(),
        Value::Currency(c) => c.trunc(),
        Value::Boolean(b) => if *b { 1.0 } else { 0.0 },
        Value::String(s) => {
            s.trim().parse::<f64>().unwrap_or(0.0).trunc()
        }
        Value::Array(arr) => arr.len() as f64,
        Value::Json(_) => 1.0, // JSON objects are truthy
        Value::DateTime(_) => 1.0, // DateTime values are truthy
    };
    Ok(Value::Number(result))
}

/// Convert any value to float
fn to_float(value: &Value) -> Result<Value, Error> {
    let result = match value {
        Value::Null => 0.0,
        Value::Number(n) => *n,
        Value::Currency(c) => *c,
        Value::Boolean(b) => if *b { 1.0 } else { 0.0 },
        Value::String(s) => {
            s.trim().parse::<f64>().unwrap_or(0.0)
        }
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
            // Convert string to array of characters
            s.chars().map(|c| Value::String(c.to_string())).collect()
        }
        other => vec![other.clone()],
    };
    Ok(Value::Array(result))
}

/// Convert any value to JSON
fn to_json(value: &Value) -> Result<Value, Error> {
    let json_str = match value {
        Value::Null => "{}".to_string(),
        Value::Json(s) => s.clone(),
        Value::String(s) => {
            let json_val = serde_json::Value::String(s.clone());
            serde_json::to_string(&json_val)
                .map_err(|e| Error::new(format!("Failed to convert to JSON: {}", e), None))?
        }
        Value::Number(n) => {
            let json_val = serde_json::Value::Number(
                serde_json::Number::from_f64(*n)
                    .ok_or_else(|| Error::new("Invalid number for JSON", None))?
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
            let json_array: Result<Vec<serde_json::Value>, Error> = arr
                .iter()
                .map(|v| value_to_json_value(v))
                .collect();
            let json_val = serde_json::Value::Array(json_array?);
            serde_json::to_string(&json_val)
                .map_err(|e| Error::new(format!("Failed to convert to JSON: {}", e), None))?
        }
        Value::Currency(c) => {
            let json_val = serde_json::Value::Number(
                serde_json::Number::from_f64(*c)
                    .ok_or_else(|| Error::new("Invalid currency for JSON", None))?
            );
            serde_json::to_string(&json_val)
                .map_err(|e| Error::new(format!("Failed to convert to JSON: {}", e), None))?
        }
        Value::DateTime(dt) => {
            let json_val = serde_json::Value::String(dt.to_string());
            serde_json::to_string(&json_val)
                .map_err(|e| Error::new(format!("Failed to convert to JSON: {}", e), None))?
        }
    };
    Ok(Value::Json(json_str))
}

/// Convert any value to boolean
fn to_boolean(value: &Value) -> Result<Value, Error> {
    let result = match value {
        Value::Null => false,
        Value::Boolean(b) => *b,
        Value::Number(n) => *n != 0.0,
        Value::Currency(c) => *c != 0.0,
        Value::String(s) => !s.is_empty(),
        Value::Array(arr) => !arr.is_empty(),
        Value::Json(_) => true,
        Value::DateTime(_) => true,
    };
    Ok(Value::Boolean(result))
}

/// Helper function to convert Value to serde_json::Value
fn value_to_json_value(value: &Value) -> Result<serde_json::Value, Error> {
    match value {
        Value::Null => Ok(serde_json::Value::Null),
        Value::String(s) => Ok(serde_json::Value::String(s.clone())),
        Value::Number(n) => {
            serde_json::Number::from_f64(*n)
                .map(serde_json::Value::Number)
                .ok_or_else(|| Error::new("Invalid number for JSON", None))
        }
        Value::Boolean(b) => Ok(serde_json::Value::Bool(*b)),
        Value::Currency(c) => {
            serde_json::Number::from_f64(*c)
                .map(serde_json::Value::Number)
                .ok_or_else(|| Error::new("Invalid currency for JSON", None))
        }
        Value::Array(arr) => {
            let json_array: Result<Vec<serde_json::Value>, Error> = arr
                .iter()
                .map(value_to_json_value)
                .collect();
            Ok(serde_json::Value::Array(json_array?))
        }
        Value::Json(s) => {
            serde_json::from_str(s)
                .map_err(|e| Error::new(format!("Invalid JSON: {}", e), None))
        }
        Value::DateTime(dt) => Ok(serde_json::Value::String(dt.to_string())),
    }
}