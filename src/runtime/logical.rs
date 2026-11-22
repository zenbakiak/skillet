use crate::error::Error;
use crate::runtime::utils::type_name;
use crate::types::Value;

/// A condition of `IF`/`IFS` (ADR-0018 D-8, B-18): TRUE/FALSE, a number (non-zero is TRUE, as
/// in Excel; currency counts as a number) or NULL (FALSE, a blank cell, ADR-0006). Anything
/// else, text included, is Excel's `#VALUE!`.
pub(crate) fn condition(name: &str, v: &Value) -> Result<bool, Error> {
    match v {
        Value::Boolean(b) => Ok(*b),
        Value::Number(n) | Value::Currency(n) => Ok(*n != 0.0),
        Value::Null => Ok(false),
        other => Err(Error::new(
            format!(
                "{name} condition must be TRUE, FALSE or a number, got {} (#VALUE!)",
                type_name(other)
            ),
            None,
        )),
    }
}

/// `IFS` when no condition is TRUE (Excel `#N/A`).
pub(crate) fn ifs_no_match() -> Error {
    Error::new("IFS: no condition is TRUE (#N/A)", None)
}

/// The logical values of `AND`/`OR`/`XOR` arguments, Excel style (ADR-0029): booleans, and
/// numbers as non-zero. Arrays are flattened and their text, NULL and other non-logical items
/// are ignored, like a range's text and empty cells. A NULL argument is ignored too (a blank
/// cell). Text or another type passed directly is `#VALUE!`, and so is having no logical value.
fn logical_values(name: &str, args: &[Value]) -> Result<Vec<bool>, Error> {
    fn from_array(items: &[Value], out: &mut Vec<bool>) {
        for item in items {
            match item {
                Value::Boolean(b) => out.push(*b),
                Value::Number(n) | Value::Currency(n) => out.push(*n != 0.0),
                Value::Array(inner) => from_array(inner, out),
                _ => {}
            }
        }
    }
    let mut out = Vec::with_capacity(args.len());
    for arg in args {
        match arg {
            Value::Boolean(b) => out.push(*b),
            Value::Number(n) | Value::Currency(n) => out.push(*n != 0.0),
            Value::Null => {}
            Value::Array(items) => from_array(items, &mut out),
            other => {
                return Err(Error::new(
                    format!(
                        "{name} expects logical values, got {} (#VALUE!)",
                        type_name(other)
                    ),
                    None,
                ))
            }
        }
    }
    if out.is_empty() {
        return Err(Error::new(
            format!("{name}: no logical values (#VALUE!)"),
            None,
        ));
    }
    Ok(out)
}

pub fn exec_logical(name: &str, args: &[Value]) -> Result<Value, Error> {
    match name {
        "__CONST_TRUE__" => Ok(Value::Boolean(true)),
        "__CONST_FALSE__" => Ok(Value::Boolean(false)),
        "__TERNARY__" => {
            if args.len() != 3 {
                return Err(Error::new("Ternary expects 3 args", None));
            }
            let cond = args[0]
                .as_bool()
                .ok_or_else(|| Error::new("Ternary condition must be boolean", None))?;
            Ok(if cond {
                args[1].clone()
            } else {
                args[2].clone()
            })
        }
        // Excel evaluates every argument (no short-circuit, unlike the `AND`/`OR` operators).
        "AND" => Ok(Value::Boolean(
            logical_values(name, args)?.into_iter().all(|b| b),
        )),
        "OR" => Ok(Value::Boolean(
            logical_values(name, args)?.into_iter().any(|b| b),
        )),
        // TRUE when an odd number of the values are TRUE.
        "XOR" => Ok(Value::Boolean(
            logical_values(name, args)?
                .into_iter()
                .filter(|b| *b)
                .count()
                % 2
                == 1,
        )),
        "NOT" => {
            let [v] = args else {
                return Err(Error::new("NOT expects 1 argument", None));
            };
            match v {
                Value::Boolean(_) | Value::Number(_) | Value::Currency(_) | Value::Null => {
                    Ok(Value::Boolean(!condition(name, v)?))
                }
                other => Err(Error::new(
                    format!(
                        "NOT expects a logical value, got {} (#VALUE!)",
                        type_name(other)
                    ),
                    None,
                )),
            }
        }
        "IF" => {
            if args.len() < 2 {
                return Err(Error::new("IF expects at least 2 arguments", None));
            }
            if condition(name, &args[0])? {
                Ok(args[1].clone())
            } else {
                Ok(args.get(2).cloned().unwrap_or(Value::Boolean(false)))
            }
        }
        "IFS" => {
            if args.len() % 2 != 0 {
                return Err(Error::new(
                    "IFS expects pairs of condition,value arguments",
                    None,
                ));
            }
            for chunk in args.chunks(2) {
                if chunk.len() == 2 && condition(name, &chunk[0])? {
                    return Ok(chunk[1].clone());
                }
            }
            Err(ifs_no_match())
        }
        "BETWEEN" => {
            if args.len() != 3 {
                return Err(Error::new(
                    "BETWEEN expects 3 arguments: (min, max, value)",
                    None,
                ));
            }
            let min = args[0]
                .as_number()
                .ok_or_else(|| Error::new("BETWEEN min must be a number", None))?;
            let max = args[1]
                .as_number()
                .ok_or_else(|| Error::new("BETWEEN max must be a number", None))?;
            let value = args[2]
                .as_number()
                .ok_or_else(|| Error::new("BETWEEN value must be a number", None))?;
            Ok(Value::Boolean(value >= min && value <= max))
        }
        _ => Err(Error::new(
            format!("Unknown logical function: {}", name),
            None,
        )),
    }
}
