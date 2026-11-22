//! Arithmetic builtins. Excel's documented behaviour is the spec (BL-054): where Excel returns
//! `#VALUE!`, `#NUM!` or `#DIV/0!` these return an `Error`, and where Excel returns a value
//! (for example `MAX` of no numbers is 0) they return the same value.

use crate::ast::BinaryOp;
use crate::error::Error;
use crate::runtime::utils::{number_arg, optional_number_arg};
use crate::types::Value;

pub fn exec_arithmetic(name: &str, args: &[Value]) -> Result<Value, Error> {
    match name {
        "SUM" => Ok(fold_numbers(args, 0.0, |acc, n| acc + n)),
        "PRODUCT" | "MULTIPLY" => Ok(fold_numbers(args, 1.0, |acc, n| acc * n)),
        "AVG" | "AVERAGE" => average(args),
        "MIN" => Ok(extreme(args, f64::min)),
        "MAX" => Ok(extreme(args, f64::max)),
        "ROUND" => round(args),
        "CEIL" | "CEILING" => ceiling(name, args),
        "FLOOR" => floor(args),
        "ABS" => unary(name, args, |n| Ok(n.abs())),
        "INT" => unary(name, args, |n| Ok(n.floor())),
        "SQRT" => unary(name, args, |n| {
            if n < 0.0 {
                Err(Error::new("SQRT of a negative number (#NUM!)", None))
            } else {
                Ok(n.sqrt())
            }
        }),
        "POW" | "POWER" => power(name, args),
        "MOD" => modulo(args),
        _ => Err(Error::new(
            format!("Unknown arithmetic function: {}", name),
            None,
        )),
    }
}

/// Visit every number in the arguments, flattening arrays. Other values are ignored, as Excel
/// ignores text and logical values in a range (excel-compat X-3). A deprecated `Currency` is a
/// number (ADR-0033).
#[allow(deprecated)]
fn for_each_number(args: &[Value], f: &mut impl FnMut(f64)) {
    for a in args {
        match a {
            Value::Number(n) | Value::Currency(n) => f(*n),
            Value::Array(items) => for_each_number(items, f),
            _ => {}
        }
    }
}

fn fold_numbers(args: &[Value], init: f64, op: impl Fn(f64, f64) -> f64) -> Value {
    let mut acc = init;
    for_each_number(args, &mut |n| acc = op(acc, n));
    Value::Number(acc)
}

fn average(args: &[Value]) -> Result<Value, Error> {
    let (mut sum, mut count) = (0.0, 0usize);
    for_each_number(args, &mut |n| {
        sum += n;
        count += 1;
    });
    if count == 0 {
        return Err(Error::new("AVERAGE of no numbers (#DIV/0!)", None));
    }
    Ok(Value::Number(sum / count as f64))
}

/// `MIN`/`MAX`: Excel returns 0 when the arguments contain no numbers.
fn extreme(args: &[Value], pick: fn(f64, f64) -> f64) -> Value {
    let mut cur: Option<f64> = None;
    for_each_number(args, &mut |n| cur = Some(cur.map_or(n, |c| pick(c, n))));
    Value::Number(cur.unwrap_or(0.0))
}

fn arity(name: &str, args: &[Value], min: usize, max: usize, sig: &str) -> Result<(), Error> {
    if args.len() < min || args.len() > max {
        return Err(Error::new(format!("{} expects ({})", name, sig), None));
    }
    Ok(())
}

fn unary(
    name: &str,
    args: &[Value],
    f: impl Fn(f64) -> Result<f64, Error>,
) -> Result<Value, Error> {
    arity(name, args, 1, 1, "number")?;
    Ok(Value::Number(f(number_arg(name, args, 0, "number")?)?))
}

/// `ROUND(number, [num_digits])`: half away from zero, on the number's 15-significant-digit
/// decimal form (so `ROUND(1.005, 2)` is 1.01, as in Excel). Negative digits round to the left
/// of the decimal point. `num_digits` is truncated and defaults to 0 (a Skillet extension; Excel
/// requires it).
fn round(args: &[Value]) -> Result<Value, Error> {
    arity("ROUND", args, 1, 2, "number, [num_digits]")?;
    let n = number_arg("ROUND", args, 0, "number")?;
    let digits = optional_number_arg("ROUND", args, 1, "num_digits", 0.0)?;
    if digits.is_nan() {
        return Err(Error::new("ROUND num_digits is not a number (#NUM!)", None));
    }
    let r = round_half_away(n, digits.trunc().clamp(-400.0, 400.0) as i32);
    if n.is_finite() && !r.is_finite() {
        return Err(Error::new("ROUND result is out of range (#NUM!)", None));
    }
    Ok(Value::Number(r))
}

/// The amount of `x::Currency` (ADR-0033 §3): `ROUND(n, 4)`, half away from zero, with `-0` as
/// `0`. It never fails: a magnitude where 4 decimals cannot change an `f64` (`|n| >= 2^53 / 10^4`)
/// and a non-finite number are returned as they are.
pub(crate) fn round_currency(n: f64) -> f64 {
    const UNCHANGED_FROM: f64 = 9_007_199_254_740_992.0 / 10_000.0;
    if !n.is_finite() || n.abs() >= UNCHANGED_FROM {
        return n;
    }
    let r = round_half_away(n, 4);
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

pub(crate) fn round_half_away(n: f64, digits: i32) -> f64 {
    if !n.is_finite() || n == 0.0 {
        return n;
    }
    // "d.dddddddddddddde<exp>": the 15 significant digits Excel works with.
    let s = format!("{:.14e}", n.abs());
    let Some((mantissa, exp)) = s.split_once('e') else {
        return n;
    };
    let Ok(exp) = exp.parse::<i64>() else {
        return n;
    };
    let ds: Vec<u8> = mantissa.bytes().filter(u8::is_ascii_digit).collect();
    let keep = exp + 1 + i64::from(digits);
    if keep >= ds.len() as i64 {
        return n;
    }
    let sign = n.signum();
    if keep < 0 {
        return 0.0;
    }
    let keep = keep as usize;
    let mut m: u64 = ds[..keep]
        .iter()
        .fold(0, |acc, d| acc * 10 + u64::from(d - b'0'));
    if ds[keep] >= b'5' {
        m += 1;
    }
    let r = if digits >= 0 {
        m as f64 / 10f64.powi(digits)
    } else {
        m as f64 * 10f64.powi(-digits)
    };
    sign * r
}

/// The quotient `n / sig`, snapped to the nearest integer when it is within rounding error of
/// one, so `CEILING(2.1, 0.1)` is 2.1 and not 2.2.
fn quotient(n: f64, sig: f64) -> f64 {
    let q = n / sig;
    let r = q.round();
    if (q - r).abs() <= 1e-9 * r.abs().max(1.0) {
        r
    } else {
        q
    }
}

/// `CEILING(number, [significance])` (alias `CEIL`): round up to a multiple of `significance`
/// (default 1, a Skillet extension). Excel rules: significance 0 → 0; a positive number with a
/// negative significance → `#NUM!`; a negative number with a negative significance rounds away
/// from zero.
fn ceiling(name: &str, args: &[Value]) -> Result<Value, Error> {
    arity(name, args, 1, 2, "number, [significance]")?;
    let n = number_arg(name, args, 0, "number")?;
    let sig = optional_number_arg(name, args, 1, "significance", 1.0)?;
    if sig == 0.0 {
        return Ok(Value::Number(0.0));
    }
    if n > 0.0 && sig < 0.0 {
        return Err(Error::new(
            format!("{name}: a positive number needs a positive significance (#NUM!)"),
            None,
        ));
    }
    Ok(Value::Number(quotient(n, sig).ceil() * sig))
}

/// `FLOOR(number, [significance])`: round down to a multiple of `significance` (default 1, a
/// Skillet extension). Excel rules: significance 0 → `#DIV/0!`; a positive number with a negative
/// significance → `#NUM!`; a negative number with a negative significance rounds toward zero.
fn floor(args: &[Value]) -> Result<Value, Error> {
    arity("FLOOR", args, 1, 2, "number, [significance]")?;
    let n = number_arg("FLOOR", args, 0, "number")?;
    let sig = optional_number_arg("FLOOR", args, 1, "significance", 1.0)?;
    if sig == 0.0 {
        return Err(Error::new("FLOOR significance is 0 (#DIV/0!)", None));
    }
    if n > 0.0 && sig < 0.0 {
        return Err(Error::new(
            "FLOOR: a positive number needs a positive significance (#NUM!)",
            None,
        ));
    }
    Ok(Value::Number(quotient(n, sig).floor() * sig))
}

/// `POWER(number, power)`. Excel: `0^0` and results that are not real numbers → `#NUM!`;
/// `0` to a negative power → `#DIV/0!`.
fn power(name: &str, args: &[Value]) -> Result<Value, Error> {
    arity(name, args, 2, 2, "number, power")?;
    let a = number_arg(name, args, 0, "number")?;
    let b = number_arg(name, args, 1, "power")?;
    if a == 0.0 && b < 0.0 {
        return Err(Error::new(
            format!("{name}: 0 to a negative power (#DIV/0!)"),
            None,
        ));
    }
    if a == 0.0 && b == 0.0 {
        return Err(Error::new(
            format!("{name}(0, 0) is undefined (#NUM!)"),
            None,
        ));
    }
    let r = a.powf(b);
    if a.is_finite() && b.is_finite() && !r.is_finite() {
        return Err(Error::new(
            format!("{name} result is not a real number or is out of range (#NUM!)"),
            None,
        ));
    }
    Ok(Value::Number(r))
}

/// `MOD(number, divisor) = number - divisor * INT(number / divisor)`: the result has the sign of
/// the divisor, as in Excel. A divisor of 0 → `#DIV/0!`.
fn modulo(args: &[Value]) -> Result<Value, Error> {
    arity("MOD", args, 2, 2, "number, divisor")?;
    let a = number_arg("MOD", args, 0, "number")?;
    let b = number_arg("MOD", args, 1, "divisor")?;
    if b == 0.0 {
        return Err(Error::new("MOD divisor is 0 (#DIV/0!)", None));
    }
    let r = a - b * (a / b).floor();
    // Rounding can land exactly on the divisor (MOD(-1e-20, 3)); the result must stay below it.
    Ok(Value::Number(if r == b { 0.0 } else { r }))
}

/// An arithmetic operator (`+ - * / % ^`) on two numbers, with Excel's errors (BL-054, D-1):
/// division or `%` by 0 and `0 ^ negative` are `#DIV/0!`, `0 ^ 0` is `#NUM!`, and any result
/// that is not a finite number (overflow to infinity, NaN) is `#NUM!`. `%` keeps the sign of
/// the left operand; `MOD` is the Excel remainder.
pub(crate) fn checked_binary(op: BinaryOp, a: f64, b: f64) -> Result<f64, Error> {
    let div_zero = || Error::new("Division by zero (#DIV/0!)", None);
    let r = match op {
        BinaryOp::Add => a + b,
        BinaryOp::Sub => a - b,
        BinaryOp::Mul => a * b,
        BinaryOp::Div | BinaryOp::Mod if b == 0.0 => return Err(div_zero()),
        BinaryOp::Div => a / b,
        BinaryOp::Mod => a % b,
        BinaryOp::Pow if a == 0.0 && b < 0.0 => return Err(div_zero()),
        BinaryOp::Pow if a == 0.0 && b == 0.0 => {
            return Err(Error::new("0 ^ 0 is undefined (#NUM!)", None))
        }
        BinaryOp::Pow => a.powf(b),
        _ => return Err(Error::new("Not an arithmetic operator", None)),
    };
    if r.is_finite() {
        Ok(r)
    } else {
        Err(Error::new("Result is not a finite number (#NUM!)", None))
    }
}
