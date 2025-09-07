use crate::error::Error;
use crate::runtime::utils::is_blank;
use crate::types::Value;

/// Handle predicate method calls (methods ending with '?')
pub fn exec_predicate(name: &str, recv: &Value) -> Result<Value, Error> {
    let lname = name.to_lowercase();
    
    let result = match lname.as_str() {
        "positive" => recv.as_number().map(|n| n > 0.0).unwrap_or(false),
        "negative" => recv.as_number().map(|n| n < 0.0).unwrap_or(false),
        "zero" => recv.as_number().map(|n| n == 0.0).unwrap_or(false),
        "even" => recv.as_number()
            .map(|n| (n as i64) % 2 == 0)
            .unwrap_or(false),
        "odd" => recv.as_number()
            .map(|n| (n as i64) % 2 != 0)
            .unwrap_or(false),
        "numeric" => matches!(recv, Value::Number(_)),
        "array" => matches!(recv, Value::Array(_)),
        "nil" => matches!(recv, Value::Null),
        "blank" => is_blank(recv),
        "present" => !is_blank(recv),
        _ => return Err(Error::new(
            format!("Unknown predicate method: {}?", name),
            None,
        )),
    };
    
    Ok(Value::Boolean(result))
}