use crate::types::Value;
use crate::error::Error;

pub fn exec_logical(name: &str, args: &[Value]) -> Result<Value, Error> {
    match name {
        "__CONST_TRUE__" => Ok(Value::Boolean(true)),
        "__CONST_FALSE__" => Ok(Value::Boolean(false)),
        "__TERNARY__" => {
            if args.len() != 3 { return Err(Error::new("Ternary expects 3 args", None)); }
            let cond = args[0].as_bool().ok_or_else(|| Error::new("Ternary condition must be boolean", None))?;
            Ok(if cond { args[1].clone() } else { args[2].clone() })
        }
        "XOR" => {
            if args.len() != 2 { return Err(Error::new("XOR expects 2 arguments", None)); }
            let a = match &args[0] { Value::Boolean(b) => *b, Value::Number(n) => *n != 0.0, _ => false };
            let b = match &args[1] { Value::Boolean(b) => *b, Value::Number(n) => *n != 0.0, _ => false };
            Ok(Value::Boolean(a != b))
        }
        "AND" => {
            let mut result = true;
            for arg in args {
                let val = match arg { Value::Boolean(b) => *b, Value::Number(n) => *n != 0.0, _ => false };
                result = result && val;
                if !result { break; }
            }
            Ok(Value::Boolean(result))
        }
        "OR" => {
            let mut result = false;
            for arg in args {
                let val = match arg { Value::Boolean(b) => *b, Value::Number(n) => *n != 0.0, _ => false };
                result = result || val;
                if result { break; }
            }
            Ok(Value::Boolean(result))
        }
        "NOT" => {
            let val = match args.get(0) { Some(Value::Boolean(b)) => *b, Some(Value::Number(n)) => *n != 0.0, _ => false };
            Ok(Value::Boolean(!val))
        }
        "IF" => {
            if args.len() < 2 { return Err(Error::new("IF expects at least 2 arguments", None)); }
            let cond = match &args[0] { Value::Boolean(b) => *b, Value::Number(n) => *n != 0.0, _ => false };
            if cond {
                Ok(args[1].clone())
            } else {
                Ok(args.get(2).cloned().unwrap_or(Value::Boolean(false)))
            }
        }
        "IFS" => {
            if args.len() % 2 != 0 { return Err(Error::new("IFS expects pairs of condition,value arguments", None)); }
            for chunk in args.chunks(2) {
                if chunk.len() == 2 {
                    let cond = match &chunk[0] { Value::Boolean(b) => *b, Value::Number(n) => *n != 0.0, _ => false };
                    if cond {
                        return Ok(chunk[1].clone());
                    }
                }
            }
            Ok(Value::Boolean(false))
        }
        _ => Err(Error::new(format!("Unknown logical function: {}", name), None)),
    }
}