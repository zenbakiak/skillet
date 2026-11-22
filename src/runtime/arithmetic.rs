//! Arithmetic builtins. Excel's documented behaviour is the spec (BL-054): where Excel returns
//! `#VALUE!`, `#NUM!` or `#DIV/0!` these return an `Error`, and where Excel returns a value
//! (for example `MAX` of no numbers is 0) they return the same value.

use crate::ast::BinaryOp;
use crate::error::Error;
use crate::runtime::utils::{number_arg, optional_number_arg};
use crate::types::Value;

pub fn exec_arithmetic(name: &str, args: &[Value]) -> Result<Value, Error> {
    match name {
        "SUM" => fold_numbers("SUM", args, 0.0, |acc, n| acc + n),
        "PRODUCT" | "MULTIPLY" => fold_numbers("PRODUCT", args, 1.0, |acc, n| acc * n),
        "AVG" | "AVERAGE" => average(args),
        "MIN" => extreme("MIN", args, f64::min, i64::min),
        "MAX" => extreme("MAX", args, f64::max, i64::max),
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
/// number (ADR-0033). A DateTime is an error: `name` does arithmetic on numbers, and Skillet has
/// no date serial numbers (ADR-0031 amendment 2, B2).
#[allow(deprecated)]
fn for_each_number(name: &str, args: &[Value], f: &mut impl FnMut(f64)) -> Result<(), Error> {
    for a in args {
        match a {
            Value::Number(n) | Value::Currency(n) => f(*n),
            Value::Array(items) => for_each_number(name, items, f)?,
            Value::DateTime(_) => return Err(dates_not_accepted(name)),
            _ => {}
        }
    }
    Ok(())
}

fn fold_numbers(
    name: &str,
    args: &[Value],
    init: f64,
    op: impl Fn(f64, f64) -> f64,
) -> Result<Value, Error> {
    let mut acc = init;
    for_each_number(name, args, &mut |n| acc = op(acc, n))?;
    Ok(Value::Number(acc))
}

fn average(args: &[Value]) -> Result<Value, Error> {
    let (mut sum, mut count) = (0.0, 0usize);
    for_each_number("AVERAGE", args, &mut |n| {
        sum += n;
        count += 1;
    })?;
    if count == 0 {
        return Err(Error::new("AVERAGE of no numbers (#DIV/0!)", None));
    }
    Ok(Value::Number(sum / count as f64))
}

/// `MIN`/`MAX` over the Numbers and DateTimes in the arguments, arrays flattened; other values
/// are ignored. Only DateTimes → the earliest/latest DateTime; both kinds → `#VALUE!`; none →
/// 0, as in Excel (ADR-0031 amendment 2, B1).
#[allow(deprecated)] // `Value::Currency` is read as a Number (ADR-0033)
fn extreme(
    name: &str,
    args: &[Value],
    pick: fn(f64, f64) -> f64,
    pick_date: fn(i64, i64) -> i64,
) -> Result<Value, Error> {
    fn visit(
        args: &[Value],
        num: &mut Option<f64>,
        date: &mut Option<i64>,
        pick: fn(f64, f64) -> f64,
        pick_date: fn(i64, i64) -> i64,
    ) {
        for a in args {
            match a {
                Value::Number(n) | Value::Currency(n) => {
                    *num = Some(num.map_or(*n, |c| pick(c, *n)))
                }
                Value::DateTime(t) => *date = Some(date.map_or(*t, |c| pick_date(c, *t))),
                Value::Array(items) => visit(items, num, date, pick, pick_date),
                _ => {}
            }
        }
    }
    let (mut num, mut date) = (None, None);
    visit(args, &mut num, &mut date, pick, pick_date);
    match (num, date) {
        (Some(_), Some(_)) => Err(Error::new(
            format!("{name} cannot compare dates with numbers (#VALUE!)"),
            None,
        )),
        (None, Some(t)) => Ok(Value::DateTime(t)),
        (n, None) => Ok(Value::Number(n.unwrap_or(0.0))),
    }
}

/// The error of an aggregate that does arithmetic when a DateTime is among its data
/// (ADR-0031 amendment 2, B2/B3). `name` is the canonical name, whichever alias was called.
pub(crate) fn dates_not_accepted(name: &str) -> Error {
    Error::new(format!("{name} does not accept dates (#VALUE!)"), None)
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
    let r = round4(n);
    if r == 0.0 {
        0.0
    } else {
        r
    }
}

/// `round_half_away(n, 4)` without its text formatting when that cannot change the result.
/// `round_half_away` rounds the 15-significant-digit decimal form of `n`, which is within
/// `5e-15 * |n|` of `n`; `|n| * 10^4` is within half an ulp of the exact product. So when the
/// fraction of `|n| * 10^4` is further than `1e-14` of itself (plus a margin) from one half,
/// both round to the same integer `m`, and both return `m / 10^4`. Below `1e9` the decimal form
/// keeps more than 4 decimals, so `round_half_away` does round there.
fn round4(n: f64) -> f64 {
    let a = n.abs();
    if a < 1e9 {
        let x = a * 1e4;
        let frac = x - x.floor();
        if (frac - 0.5).abs() > x * 1e-14 + 1e-9 {
            return n.signum() * (x.round() / 1e4);
        }
    }
    round_half_away(n, 4)
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

#[cfg(test)]
mod currency_rounding_tests {
    use super::{round_currency, round_half_away};

    fn reference(n: f64) -> f64 {
        let r = round_half_away(n, 4);
        if r == 0.0 {
            0.0
        } else {
            r
        }
    }

    fn check(n: f64) {
        let got = round_currency(n);
        let want = reference(n);
        assert_eq!(got.to_bits(), want.to_bits(), "{n:?}: {got:?} != {want:?}");
    }

    /// The fast path of `round4` gives exactly `ROUND(n, 4)`: halves at every scale, their
    /// neighbouring floats, and a pseudo-random spread of magnitudes.
    #[test]
    fn fast_path_matches_round_half_away() {
        for k in 0..40_000u64 {
            for scale in [1.0, 1e3, 1e6, 1e8] {
                let half = (k as f64 * 7.0 + 0.5) / 1e4 * scale;
                for x in [
                    half,
                    f64::from_bits(half.to_bits() + 1),
                    f64::from_bits(half.to_bits().saturating_sub(1)),
                ] {
                    check(x);
                    check(-x);
                }
            }
        }
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        for _ in 0..400_000 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let mantissa = (state >> 11) as f64 / (1u64 << 53) as f64;
            let exp = (state % 32) as i32 - 12;
            check(mantissa * 10f64.powi(exp));
            check(-mantissa * 10f64.powi(exp));
        }
        for n in [
            1.00005,
            -1.00005,
            0.00005,
            2.675,
            1.005,
            0.1 + 0.2,
            1e9 - 0.00005,
            9.999e8,
        ] {
            check(n);
        }
    }
}
