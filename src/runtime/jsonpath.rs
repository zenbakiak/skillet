use crate::types::Value;
use crate::error::Error;
use jsonpath_rust::JsonPathQuery;
use serde_json;

/// Apply JSONPath query to a JSON string or JSON Value
pub fn apply_jsonpath(json_data: &Value, path: &str) -> Result<Value, Error> {
    // Convert Value to serde_json::Value
    let json_value = value_to_json(json_data)?;

    // Apply JSONPath query - this returns a serde_json::Value directly
    let result = json_value.path(path)
        .map_err(|e| Error::new(format!("JSONPath error: {}", e), None))?;

    // Convert result back to our Value type
    let converted = json_to_value(result)?;

    // Handle special cases for better usability
    match converted {
        Value::Null => Ok(Value::Array(vec![])), // No matches -> empty array
        Value::Array(ref arr) if arr.len() == 1 => {
            // Single-element array -> unwrap to the element for easier arithmetic
            Ok(arr[0].clone())
        }
        other => Ok(other)
    }
}

/// Convert our Value type to serde_json::Value
fn value_to_json(value: &Value) -> Result<serde_json::Value, Error> {
    match value {
        Value::Number(n) => {
            if n.fract() == 0.0 && *n >= i64::MIN as f64 && *n <= i64::MAX as f64 {
                Ok(serde_json::Value::Number(serde_json::Number::from(*n as i64)))
            } else {
                serde_json::Number::from_f64(*n)
                    .map(serde_json::Value::Number)
                    .ok_or_else(|| Error::new("Invalid number for JSON conversion", None))
            }
        }
        Value::String(s) => Ok(serde_json::Value::String(s.clone())),
        Value::Boolean(b) => Ok(serde_json::Value::Bool(*b)),
        Value::Null => Ok(serde_json::Value::Null),
        Value::Array(arr) => {
            let mut json_arr = Vec::new();
            for item in arr {
                json_arr.push(value_to_json(item)?);
            }
            Ok(serde_json::Value::Array(json_arr))
        }
        Value::Currency(n) => {
            serde_json::Number::from_f64(*n)
                .map(serde_json::Value::Number)
                .ok_or_else(|| Error::new("Invalid currency for JSON conversion", None))
        }
        Value::DateTime(ts) => Ok(serde_json::Value::Number(serde_json::Number::from(*ts))),
        Value::Json(json_str) => {
            serde_json::from_str(json_str)
                .map_err(|e| Error::new(format!("Invalid JSON string: {}", e), None))
        }
    }
}

/// Convert serde_json::Value to our Value type
fn json_to_value(json: serde_json::Value) -> Result<Value, Error> {
    match json {
        serde_json::Value::Null => Ok(Value::Null),
        serde_json::Value::Bool(b) => Ok(Value::Boolean(b)),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(Value::Number(i as f64))
            } else if let Some(f) = n.as_f64() {
                Ok(Value::Number(f))
            } else {
                Err(Error::new("Invalid number in JSON", None))
            }
        }
        serde_json::Value::String(s) => Ok(Value::String(s)),
        serde_json::Value::Array(arr) => {
            let mut result = Vec::new();
            for item in arr {
                result.push(json_to_value(item)?);
            }
            Ok(Value::Array(result))
        }
        serde_json::Value::Object(_) => {
            // For objects, convert back to JSON string to maintain compatibility
            let json_str = serde_json::to_string(&json)
                .map_err(|e| Error::new(format!("Failed to serialize JSON object: {}", e), None))?;
            Ok(Value::Json(json_str))
        }
    }
}

/// Check if a string looks like a JSONPath expression
pub fn is_jsonpath(s: &str) -> bool {
    s.starts_with('$')
}

/// Extract values from JSONPath result for aggregation functions like SUM
pub fn extract_numeric_values(value: &Value) -> Vec<f64> {
    let mut numbers = Vec::new();

    fn collect_numbers(v: &Value, numbers: &mut Vec<f64>) {
        match v {
            Value::Number(n) => numbers.push(*n),
            Value::Currency(n) => numbers.push(*n),
            Value::Array(items) => {
                for item in items {
                    collect_numbers(item, numbers);
                }
            }
            _ => {}
        }
    }

    collect_numbers(value, &mut numbers);
    numbers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jsonpath_basic() {
        let json_str = r#"{"accounts": [{"amount": 100.0}, {"amount": 200.0}]}"#;
        let json_value = Value::Json(json_str.to_string());

        let result = apply_jsonpath(&json_value, "$.accounts[*].amount").unwrap();
        if let Value::Array(values) = result {
            assert_eq!(values.len(), 2);
            assert_eq!(values[0], Value::Number(100.0));
            assert_eq!(values[1], Value::Number(200.0));
        } else {
            panic!("Expected array result");
        }
    }

    #[test]
    fn test_extract_numeric_values() {
        let values = vec![Value::Number(100.0), Value::Number(200.0)];
        let array_value = Value::Array(values);

        let numbers = extract_numeric_values(&array_value);
        assert_eq!(numbers, vec![100.0, 200.0]);
    }

    #[test]
    fn test_is_jsonpath() {
        assert!(is_jsonpath("$.accounts[*].amount"));
        assert!(is_jsonpath("$"));
        assert!(!is_jsonpath("accounts"));
        assert!(!is_jsonpath("normal_variable"));
    }
}