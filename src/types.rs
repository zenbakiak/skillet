#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(f64),
    Array(Vec<Value>),
    Boolean(bool),
    String(String),
    Null,
    Currency(f64),
    DateTime(i64),
    Json(String),
}

impl Value {
    pub fn as_number(&self) -> Option<f64> {
        match self {
            Value::Number(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Boolean(b) => Some(*b),
            _ => None,
        }
    }
}
