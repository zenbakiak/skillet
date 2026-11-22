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
    round_to("ROUND", n, args.get(1))
}

/// `ROUND(n, digits)`, shared by `ROUND` and `.round([digits])` (BL-088): `digits` missing or
/// NULL is 0, and `name` is the form named in errors.
pub(crate) fn round_to(name: &str, n: f64, digits: Option<&Value>) -> Result<Value, Error> {
    let digits = match digits {
        Some(Value::Number(d)) => *d,
        Some(d) => optional_number_arg(name, std::slice::from_ref(d), 0, "num_digits", 0.0)?,
        None => 0.0,
    };
    if digits.is_nan() {
        return Err(Error::new(
            format!("{name} num_digits is not a number (#NUM!)"),
            None,
        ));
    }
    let r = round_half_away(n, digits.trunc().clamp(-400.0, 400.0) as i32);
    if n.is_finite() && !r.is_finite() {
        return Err(Error::new(
            format!("{name} result is out of range (#NUM!)"),
            None,
        ));
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

/// Exact powers of ten, `10^0` to `10^22` (the largest one an `f64` holds exactly).
const POW10: [f64; 23] = [
    1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15, 1e16,
    1e17, 1e18, 1e19, 1e20, 1e21, 1e22,
];

/// `n` rounded half away from zero to `digits` decimals (negative: to the left of the point),
/// on its 15-significant-digit decimal form, as Excel's `ROUND` does. Bit for bit the result of
/// [`round_half_away_text`], without its text formatting when that cannot change the result
/// (PERF-1, BL-088):
///
/// The decimal form `t` of `|n|` is within `5e-15 * |n|` of it, and `x = |n| * 10^digits` (one
/// correctly rounded operation by an exact power of ten) within half an ulp of the exact
/// product. So when the fraction of `x` is further than `1e-14 * x` (plus a margin) from one
/// half, `x` and `t * 10^digits` round to the same integer `m`, and both return `m / 10^digits`
/// (`m * 10^-digits`). `x < 9e13` keeps `t * 10^digits` below `10^14`, where the decimal form
/// still has a digit to round; `x >= 0.2` keeps it at `0.1` or more, where the text method
/// rounds instead of returning `+0`. Near a half, [`near_half`] decides without text when it
/// can; anything else takes the text method.
#[inline]
pub(crate) fn round_half_away(n: f64, digits: i32) -> f64 {
    if (-22..=22).contains(&digits) {
        let p = POW10[digits.unsigned_abs() as usize];
        let a = n.abs();
        let x = if digits >= 0 { a * p } else { a / p };
        if (0.2..9e13).contains(&x) {
            let frac = x - x.floor();
            let m = if (frac - 0.5).abs() > x * 1e-14 + 1e-9 {
                Some(x.round())
            } else {
                near_half(x)
            };
            if let Some(m) = m {
                return n.signum() * if digits >= 0 { m / p } else { m * p };
            }
        }
    }
    round_half_away_text(n, digits)
}

/// The integer that `t`, the 15-significant-digit decimal form of the exact `y = |n| * 10^digits`,
/// rounds to, for `x = fl(y)` in `[0.2, 9e13)` within `1e-14 * x + 1e-9` of the half `g = k + 0.5`;
/// `None` when it is too close to call. `g` is on the 15-digit grid (its step `u` is at most
/// 0.1 below `10^14`), so `t == g` and `m = k + 1` when `|y - g| < u / 2`, and `t` is on `y`'s
/// side of `g` (`m = x.round()`) when `|y - g| > u / 2`. `|x - g|` is exact and `|x - y|` is at
/// most half an ulp, so only a band of about one ulp each side of `u / 2` is left to the text
/// method. Decimal inputs at a half (`ROUND(0.685, 2)`) are within an ulp or two of `g`.
#[inline(never)]
fn near_half(x: f64) -> Option<f64> {
    // `e`: the decimal exponent of `x` (and of `t`: no power of ten is near a half).
    let e = if x < 1.0 {
        -1
    } else {
        POW10[1..=13].iter().take_while(|&&p| p <= x).count() as i32
    };
    let half_step = 0.5 / POW10[(14 - e) as usize];
    let g = x.floor() + 0.5;
    let dist = (x - g).abs();
    let err = x * 2.3e-16;
    if dist + err < half_step * (1.0 - 1e-9) {
        Some(x.floor() + 1.0)
    } else if dist - err > half_step * (1.0 + 1e-9) {
        Some(x.round())
    } else {
        None
    }
}

/// The reference `ROUND`: format the 15 significant digits Excel works with, then round the
/// digit after `digits` decimals half away from zero. The text goes to a stack buffer.
#[cold]
#[inline(never)]
fn round_half_away_text(n: f64, digits: i32) -> f64 {
    use std::fmt::Write;
    /// "d.dddddddddddddde-308" is 21 bytes.
    struct Buf {
        bytes: [u8; 32],
        len: usize,
    }
    impl Write for Buf {
        fn write_str(&mut self, s: &str) -> std::fmt::Result {
            let end = self.len + s.len();
            let dst = self.bytes.get_mut(self.len..end).ok_or(std::fmt::Error)?;
            dst.copy_from_slice(s.as_bytes());
            self.len = end;
            Ok(())
        }
    }
    if !n.is_finite() || n == 0.0 {
        return n;
    }
    // "d.dddddddddddddde<exp>": the 15 significant digits Excel works with.
    let mut buf = Buf {
        bytes: [0; 32],
        len: 0,
    };
    if write!(buf, "{:.14e}", n.abs()).is_err() {
        return n;
    }
    let Ok(s) = std::str::from_utf8(&buf.bytes[..buf.len]) else {
        return n;
    };
    let Some((mantissa, exp)) = s.split_once('e') else {
        return n;
    };
    let Ok(exp) = exp.parse::<i64>() else {
        return n;
    };
    // Digits of "d.dddddddddddddd" without the point: at most 15.
    let mut ds = [0u8; 16];
    let mut len = 0;
    for d in mantissa.bytes().filter(u8::is_ascii_digit) {
        if len == ds.len() {
            return n;
        }
        ds[len] = d;
        len += 1;
    }
    let ds = &ds[..len];
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
mod rounding_tests {
    use super::{round_currency, round_half_away, round_half_away_text, POW10};

    /// `ROUND` before BL-088 (heap `String` and `Vec`), the oracle for both paths.
    fn reference_round(n: f64, digits: i32) -> f64 {
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

    fn reference(n: f64) -> f64 {
        let r = reference_round(n, 4);
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

    /// `round_currency` gives exactly the text method's `ROUND(n, 4)`: halves at every scale, their
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

    fn check_digits(n: f64, d: i32) {
        let want = reference_round(n, d);
        for (path, got) in [
            ("fast", round_half_away(n, d)),
            ("text", round_half_away_text(n, d)),
        ] {
            assert_eq!(
                got.to_bits(),
                want.to_bits(),
                "ROUND({n:?}, {d}) {path}: {got:?} != {want:?}"
            );
        }
    }

    /// The fast path uses the same powers of ten as the text method's `10f64.powi`.
    #[test]
    fn pow10_table_is_powi() {
        for (d, p) in POW10.iter().enumerate() {
            assert_eq!(p.to_bits(), 10f64.powi(d as i32).to_bits(), "10^{d}");
        }
    }

    /// The fast path of `round_half_away` gives exactly the text method at every digit count it
    /// takes (and around it): halves of each digit count, their neighbouring floats, values near
    /// the `0.2` and `9e13` bounds, and a pseudo-random spread of magnitudes.
    #[test]
    fn fast_path_matches_text_method_for_any_digits() {
        let probe = |x: f64, d: i32| {
            for y in [
                x,
                f64::from_bits(x.to_bits() + 1),
                f64::from_bits(x.to_bits().saturating_sub(1)),
                f64::from_bits(x.to_bits() + 2),
                f64::from_bits(x.to_bits().saturating_sub(2)),
            ] {
                check_digits(y, d);
                check_digits(-y, d);
            }
        };
        for d in -24..=24 {
            let p = 10f64.powi(d);
            for k in 0..1_500u64 {
                // Halves at this digit count, spread over several magnitudes.
                for scale in [1.0, 1e3, 1e7, 1e11] {
                    probe((k as f64 * 13.0 + 0.5) * scale / p, d);
                }
                // Short decimals (one more digit than `d`), the common input.
                probe((k as f64 * 37.0 + 5.0) / (10.0 * p), d);
            }
            for x in [0.1, 0.2, 0.5, 1.5, 9e13, 1e14, 8.99999999999995e13] {
                probe(x / p, d);
            }
            // Half a 15-digit step from a half, where `near_half` hands over to the text method.
            for g in [
                0.5,
                2.5,
                9.5,
                10.5,
                99.5,
                123.5,
                999_999.5,
                12_345_678_901.5,
                8.9e13 + 0.5,
            ] {
                let e = f64::log10(g).floor() as i32;
                let h = 0.5 * 10f64.powi(e - 14);
                for x in [g - h, g + h] {
                    let mut y = x;
                    for _ in 0..4 {
                        y = f64::from_bits(y.to_bits() - 1);
                    }
                    for _ in 0..9 {
                        probe(y / p, d);
                        y = f64::from_bits(y.to_bits() + 1);
                    }
                }
            }
        }
        let mut state = 0x9E37_79B9_7F4A_7C15u64;
        for _ in 0..400_000 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let mantissa = (state >> 11) as f64 / (1u64 << 53) as f64;
            let exp = (state % 40) as i32 - 20;
            let d = ((state >> 32) % 31) as i32 - 15;
            let n = mantissa * 10f64.powi(exp);
            check_digits(n, d);
            check_digits(-n, d);
        }
        for (n, d) in [
            (1.005, 2),
            (2.675, 2),
            (0.285, 2),
            (1.00005, 4),
            (0.1 + 0.2, 16),
            (0.1 + 0.2, 1),
            (1234.567, -2),
            (-0.04, 0),
            (-0.4, 0),
            (0.49999999999999994, 0),
            (4503599627370495.5, 0),
            (1e300, -290),
            (5e-324, 22),
            (f64::MAX, 0),
            (-f64::MIN_POSITIVE, 320),
            (1.5e-300, 300),
        ] {
            check_digits(n, d);
        }
    }

    /// Decimal inputs at a half are decided without text: `near_half` returns the rounded-up
    /// integer for them, so the fast path covers them.
    #[test]
    fn near_half_decides_decimal_halves() {
        use super::near_half;
        for (n, d, m) in [
            (0.685, 2, 69.0),
            (1.005, 2, 101.0),
            (2.675, 2, 268.0),
            (0.5, 0, 1.0),
            (1234.5, 0, 1235.0),
            (8.345, 2, 835.0),
            (1.00005, 4, 10001.0),
        ] {
            let x = n * 10f64.powi(d);
            assert_eq!(near_half(x), Some(m), "{n} at {d} digits");
        }
        let mut decided = 0;
        for k in 0..100_000u32 {
            let n: f64 = format!("{k}.5").parse().unwrap();
            let x = n / 10.0 * 10.0;
            if near_half(x) == Some(f64::from(k) + 1.0) {
                decided += 1;
            }
        }
        assert_eq!(decided, 100_000);
    }
}
