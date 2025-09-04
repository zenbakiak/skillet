use crate::ast::TypeName;
use crate::error::Error;
use crate::types::Value;

pub fn cast_value(v: Value, ty: &TypeName) -> Result<Value, Error> {
    Ok(match ty {
        TypeName::Float => match v {
            Value::Number(n) => Value::Number(n),
            Value::Currency(n) => Value::Number(n),
            Value::String(s) => Value::Number(
                s.parse::<f64>()
                    .map_err(|_| Error::new("Cannot cast String to Float", None))?,
            ),
            Value::Boolean(b) => Value::Number(if b { 1.0 } else { 0.0 }),
            Value::Null => Value::Number(0.0),
            _ => return Err(Error::new("Cannot cast to Float", None)),
        },
        TypeName::Integer => match v {
            Value::Number(n) => Value::Number((n as i64) as f64),
            Value::Currency(n) => Value::Number((n as i64) as f64),
            Value::String(s) => {
                let mut clean_s = String::new();
                let mut has_dot = false;
                for (i, c) in s.chars().enumerate() {
                    if i == 0 && (c == '-' || c == '+') {
                        clean_s.push(c);
                    } else if c.is_ascii_digit() {
                        clean_s.push(c);
                    } else if c == '.' && !has_dot {
                        clean_s.push(c);
                        has_dot = true;
                    } else {
                        break;
                    }
                }
                Value::Number(
                    clean_s.parse::<f64>()
                        .unwrap_or(0.0)
                        .trunc(),
                )
            },
            Value::Boolean(b) => Value::Number(if b { 1.0 } else { 0.0 }),
            Value::Null => Value::Number(0.0),
            _ => return Err(Error::new("Cannot cast to Integer", None)),
        },
        TypeName::String => match v {
            Value::String(s) => Value::String(s),
            Value::Number(n) => Value::String(n.to_string()),
            Value::Boolean(b) => Value::String(if b { "TRUE".into() } else { "FALSE".into() }),
            Value::Null => Value::String(String::new()),
            Value::Array(items) => Value::String(format!("{:?}", items)),
            Value::Currency(n) => Value::String(format!("{:.4}", n)),
            Value::DateTime(ts) => Value::String(ts.to_string()),
            Value::Json(s) => Value::String(s),
        },
        TypeName::Boolean => match v {
            Value::Boolean(b) => Value::Boolean(b),
            Value::Number(n) => Value::Boolean(n != 0.0),
            Value::Currency(n) => Value::Boolean(n != 0.0),
            Value::String(s) => Value::Boolean(!s.trim().is_empty()),
            Value::Array(items) => Value::Boolean(!items.is_empty()),
            Value::Null => Value::Boolean(false),
            Value::DateTime(ts) => Value::Boolean(ts != 0),
            Value::Json(s) => Value::Boolean(!s.trim().is_empty()),
        },
        TypeName::Array => match v {
            Value::Array(items) => Value::Array(items),
            other => Value::Array(vec![other]),
        },
        TypeName::Currency => match v {
            Value::Currency(n) => Value::Currency(n),
            Value::Number(n) => Value::Currency(n),
            Value::String(s) => Value::Currency(
                s.parse::<f64>()
                    .map_err(|_| Error::new("Cannot cast String to Currency", None))?,
            ),
            Value::Boolean(b) => Value::Currency(if b { 1.0 } else { 0.0 }),
            Value::Null => Value::Currency(0.0),
            _ => return Err(Error::new("Cannot cast to Currency", None)),
        },
        TypeName::DateTime => match v {
            Value::DateTime(ts) => Value::DateTime(ts),
            Value::Number(n) => Value::DateTime(n as i64),
            Value::String(s) => Value::DateTime(
                s.parse::<i64>()
                    .map_err(|_| Error::new("Cannot cast String to DateTime", None))?,
            ),
            _ => return Err(Error::new("Cannot cast to DateTime", None)),
        },
        TypeName::Json => match v {
            Value::Json(s) => Value::Json(s),
            Value::String(s) => Value::Json(s),
            Value::Number(n) => Value::Json(n.to_string()),
            Value::Boolean(b) => Value::Json(if b {
                "true".to_string()
            } else {
                "false".to_string()
            }),
            Value::Null => Value::Json("null".to_string()),
            Value::Currency(n) => Value::Json(n.to_string()),
            Value::DateTime(ts) => Value::Json(ts.to_string()),
            Value::Array(items) => {
                let json_items: Result<Vec<String>, Error> = items
                    .iter()
                    .map(|item| cast_value(item.clone(), &TypeName::Json))
                    .map(|result| {
                        result.map(|v| match v {
                            Value::Json(s) => s,
                            _ => unreachable!(),
                        })
                    })
                    .collect();
                match json_items {
                    Ok(strings) => Value::Json(format!("[{}]", strings.join(","))),
                    Err(e) => return Err(e),
                }
            }
        },
    })
}
