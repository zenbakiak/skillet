use crate::error::Error;
use crate::runtime::utils::type_name;
use crate::types::Value;

/// The one condition rule (ADR-0018 D-8, B-18; ADR-0029 amendment 1): TRUE/FALSE, a number
/// (non-zero is TRUE, as in Excel) or NULL (FALSE, a blank cell, ADR-0006). NaN is Excel's
/// `#NUM!`; anything else, text included, is `#VALUE!`. Used by `IF`, `IFS`, `NOT`/`!`, `? :`,
/// the `AND`/`OR` operators, `filter`/`find`/`FILTER`/`FIND` and lambda criteria; `name` is the
/// form named in the error.
#[inline]
#[allow(deprecated)] // `Value::Currency` is read as a Number (ADR-0033)
pub(crate) fn condition(name: &str, v: &Value) -> Result<bool, Error> {
    match v {
        Value::Boolean(b) => Ok(*b),
        Value::Number(n) | Value::Currency(n) if n.is_nan() => Err(not_a_number(name)),
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

/// A NaN condition (Excel `#NUM!`). Expressions cannot make NaN; it comes from a host value.
fn not_a_number(name: &str) -> Error {
    Error::new(format!("{name} condition is not a number (#NUM!)"), None)
}

/// `NOT x`, `NOT(x)` and `!x` (ADR-0029 §5, amendment 1 A2): the negated condition, with
/// `NOT`'s own message for a value that is not a condition.
#[allow(deprecated)] // `Value::Currency` is read as a Number (ADR-0033)
pub(crate) fn not(v: &Value) -> Result<bool, Error> {
    match v {
        Value::Boolean(_) | Value::Number(_) | Value::Currency(_) | Value::Null => {
            Ok(!condition("NOT", v)?)
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

/// `IF` with more than 3 arguments (ADR-0029 amendment 1 A7), checked before any is evaluated.
#[inline]
pub(crate) fn if_arity(len: usize) -> Result<(), Error> {
    if len < 2 {
        return Err(Error::new("IF expects at least 2 arguments", None));
    }
    if len > 3 {
        return Err(Error::new("IF expects at most 3 arguments", None));
    }
    Ok(())
}

/// `IFS` takes condition/value pairs, at least one (ADR-0029 amendment 1 A7).
#[inline]
pub(crate) fn ifs_arity(len: usize) -> Result<(), Error> {
    if len == 0 || len % 2 != 0 {
        return Err(Error::new(
            "IFS expects pairs of condition,value arguments",
            None,
        ));
    }
    Ok(())
}

/// `IFS` when no condition is TRUE (Excel `#N/A`).
pub(crate) fn ifs_no_match() -> Error {
    Error::new("IFS: no condition is TRUE (#N/A)", None)
}

/// Counts the logical values of `AND`/`OR`/`XOR` arguments, Excel style (ADR-0029): booleans,
/// and numbers as non-zero. Arrays are flattened and their text, NULL and other non-logical items
/// are ignored, like a range's text and empty cells. A NULL argument is ignored too (a blank
/// cell). Text or another type passed directly is `#VALUE!`, and so is having no logical value.
/// Every argument is read (no short-circuit), so the first error wins. Returns
/// `(values, trues)` without collecting them (PERF-2, BL-088).
#[allow(deprecated)] // `Value::Currency` is read as a Number (ADR-0033)
fn logical_counts(name: &str, args: &[Value]) -> Result<(usize, usize), Error> {
    #[derive(Default)]
    struct Counts {
        values: usize,
        trues: usize,
    }
    impl Counts {
        #[inline]
        fn push(&mut self, b: bool) {
            self.values += 1;
            self.trues += usize::from(b);
        }
    }
    fn from_array(name: &str, items: &[Value], out: &mut Counts) -> Result<(), Error> {
        for item in items {
            match item {
                Value::Boolean(b) => out.push(*b),
                Value::Number(n) | Value::Currency(n) if n.is_nan() => {
                    return Err(not_a_number(name))
                }
                Value::Number(n) | Value::Currency(n) => out.push(*n != 0.0),
                Value::Array(inner) => from_array(name, inner, out)?,
                _ => {}
            }
        }
        Ok(())
    }
    let mut out = Counts::default();
    for arg in args {
        match arg {
            Value::Boolean(b) => out.push(*b),
            Value::Number(n) | Value::Currency(n) if n.is_nan() => return Err(not_a_number(name)),
            Value::Number(n) | Value::Currency(n) => out.push(*n != 0.0),
            Value::Null => {}
            Value::Array(items) => from_array(name, items, &mut out)?,
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
    if out.values == 0 {
        return Err(Error::new(
            format!("{name}: no logical values (#VALUE!)"),
            None,
        ));
    }
    Ok((out.values, out.trues))
}

#[allow(deprecated)] // `Value::Currency` is read as a Number (ADR-0033)
pub fn exec_logical(name: &str, args: &[Value]) -> Result<Value, Error> {
    match name {
        "__CONST_TRUE__" => Ok(Value::Boolean(true)),
        "__CONST_FALSE__" => Ok(Value::Boolean(false)),
        "__TERNARY__" => {
            if args.len() != 3 {
                return Err(Error::new("Ternary expects 3 args", None));
            }
            Ok(if condition("Ternary", &args[0])? {
                args[1].clone()
            } else {
                args[2].clone()
            })
        }
        // Excel evaluates every argument (no short-circuit, unlike the `AND`/`OR` operators).
        "AND" => {
            let (values, trues) = logical_counts(name, args)?;
            Ok(Value::Boolean(trues == values))
        }
        "OR" => Ok(Value::Boolean(logical_counts(name, args)?.1 > 0)),
        // TRUE when an odd number of the values are TRUE.
        "XOR" => Ok(Value::Boolean(logical_counts(name, args)?.1 % 2 == 1)),
        "NOT" => {
            let [v] = args else {
                return Err(Error::new("NOT expects 1 argument", None));
            };
            Ok(Value::Boolean(not(v)?))
        }
        "IF" => {
            if_arity(args.len())?;
            if condition(name, &args[0])? {
                Ok(args[1].clone())
            } else {
                Ok(args.get(2).cloned().unwrap_or(Value::Boolean(false)))
            }
        }
        "IFS" => {
            ifs_arity(args.len())?;
            for chunk in args.chunks(2) {
                if condition(name, &chunk[0])? {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eager_ternary_uses_the_condition_rule() {
        let n = Value::Number;
        let pick = |c: Value| exec_logical("__TERNARY__", &[c, n(1.0), n(2.0)]);
        assert_eq!(pick(n(3.0)).unwrap(), n(1.0));
        assert_eq!(pick(Value::Null).unwrap(), n(2.0));
        assert_eq!(
            pick(Value::String("a".into())).unwrap_err().message,
            "Ternary condition must be TRUE, FALSE or a number, got string (#VALUE!)"
        );
        assert_eq!(
            pick(n(f64::NAN)).unwrap_err().message,
            "Ternary condition is not a number (#NUM!)"
        );
    }

    /// AND/OR/XOR read every argument (no short-circuit), flatten nested arrays, skip NULL and
    /// text inside arrays, and report the first error in argument order.
    #[test]
    fn and_or_xor_count_every_logical_value() {
        let b = Value::Boolean;
        let n = Value::Number;
        let arr = Value::Array;
        let run = |f: &str, args: Vec<Value>| exec_logical(f, &args);
        let nested = arr(vec![
            b(true),
            Value::Null,
            Value::String("x".into()),
            arr(vec![n(2.0), b(true)]),
        ]);
        assert_eq!(
            run("AND", vec![nested.clone(), Value::Null]).unwrap(),
            b(true)
        );
        assert_eq!(run("XOR", vec![nested.clone()]).unwrap(), b(true));
        assert_eq!(
            run("XOR", vec![nested.clone(), n(0.0), b(true)]).unwrap(),
            b(false)
        );
        assert_eq!(
            run("OR", vec![n(0.0), arr(vec![b(false)])]).unwrap(),
            b(false)
        );
        assert_eq!(
            run("OR", vec![b(true), Value::String("a".into())])
                .unwrap_err()
                .message,
            "OR expects logical values, got string (#VALUE!)"
        );
        assert_eq!(
            run(
                "AND",
                vec![b(false), arr(vec![n(f64::NAN)]), Value::String("a".into())]
            )
            .unwrap_err()
            .message,
            "AND condition is not a number (#NUM!)"
        );
        assert_eq!(
            run(
                "AND",
                vec![Value::Null, arr(vec![Value::String("a".into())])]
            )
            .unwrap_err()
            .message,
            "AND: no logical values (#VALUE!)"
        );
    }
}
