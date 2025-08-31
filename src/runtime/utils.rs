use crate::error::Error;
use crate::types::Value;

pub fn is_blank(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.trim().is_empty(),
        Value::Array(items) => items.is_empty(),
        _ => false,
    }
}

pub fn clamp_index(len: usize, idx: isize) -> Option<usize> {
    if idx >= 0 {
        let i = idx as usize;
        if i < len {
            Some(i)
        } else {
            None
        }
    } else {
        let neg = (-idx) as usize; // idx is negative
        if neg <= len {
            Some(len - neg)
        } else {
            None
        }
    }
}

pub fn index_array(items: Vec<Value>, idx: isize) -> Result<Value, Error> {
    match clamp_index(items.len(), idx) {
        Some(i) => Ok(items[i].clone()),
        None => Err(Error::new("Index out of bounds", None)),
    }
}

pub fn slice_array(
    items: Vec<Value>,
    start: Option<Value>,
    end: Option<Value>,
) -> Result<Value, Error> {
    let len = items.len() as isize;
    let s = match start {
        Some(Value::Number(n)) => n as isize,
        None => 0,
        Some(_) => return Err(Error::new("Slice bounds must be numbers", None)),
    };
    let e = match end {
        Some(Value::Number(n)) => n as isize,
        None => len,
        Some(_) => return Err(Error::new("Slice bounds must be numbers", None)),
    };
    let s_norm = if s < 0 { len + s } else { s };
    let e_norm = if e < 0 { len + e } else { e };
    let s_idx = s_norm.max(0).min(len) as usize;
    let e_idx = e_norm.max(0).min(len) as usize;
    if s_idx > e_idx {
        return Ok(Value::Array(Vec::new()));
    }
    Ok(Value::Array(items[s_idx..e_idx].to_vec()))
}

pub fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x == y,
        (Value::Currency(x), Value::Currency(y)) => x == y,
        (Value::Boolean(x), Value::Boolean(y)) => x == y,
        (Value::String(x), Value::String(y)) => x == y,
        (Value::DateTime(x), Value::DateTime(y)) => x == y,
        (Value::Json(x), Value::Json(y)) => x == y,
        (Value::Null, Value::Null) => true,
        // Arrays: shallow equality by elements
        (Value::Array(ax), Value::Array(ay)) => {
            ax.len() == ay.len() && ax.iter().zip(ay.iter()).all(|(u, v)| values_equal(u, v))
        }
        _ => false,
    }
}
