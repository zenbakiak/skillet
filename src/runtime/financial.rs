use crate::error::Error;
use crate::runtime::utils::{number_arg, optional_number_arg};
use crate::types::Value;

pub fn exec_financial(name: &str, args: &[Value]) -> Result<Value, Error> {
    match name {
        "PMT" => {
            if args.len() < 3 || args.len() > 5 {
                return Err(Error::new(
                    "PMT expects 3-5 arguments: rate, nper, pv, [fv], [type]",
                    None,
                ));
            }

            let rate = required(args, 0, "PMT", "rate")?;
            let nper = required(args, 1, "PMT", "nper")?;
            let pv = required(args, 2, "PMT", "pv")?;
            let fv = optional_number_arg("PMT", args, 3, "fv", 0.0)?;
            let payment_type = optional_number_arg("PMT", args, 4, "type", 0.0)?;

            if nper <= 0.0 {
                return Err(Error::new("PMT nper must be positive", None));
            }

            Ok(Value::Number(pmt(rate, nper, pv, fv, payment_type != 0.0)))
        }
        "DB" => {
            if args.len() < 4 || args.len() > 5 {
                return Err(Error::new(
                    "DB expects 4-5 arguments: cost, salvage, life, period, [month]",
                    None,
                ));
            }

            let cost = required(args, 0, "DB", "cost")?;
            let salvage = required(args, 1, "DB", "salvage")?;
            let life = required(args, 2, "DB", "life")?;
            let period = required(args, 3, "DB", "period")?;
            let month = optional_number_arg("DB", args, 4, "month", 12.0)?;

            if cost < 0.0 || salvage < 0.0 || life <= 0.0 || period < 0.0 {
                return Err(Error::new(
                    "DB arguments must be non-negative (life must be positive)",
                    None,
                ));
            }
            if period > life {
                return Ok(Value::Number(0.0));
            }

            let rate = 1.0 - (salvage / cost).powf(1.0 / life);

            if period == 1.0 {
                let depreciation = cost * rate * month / 12.0;
                Ok(Value::Number(depreciation))
            } else {
                let mut book_value = cost;
                book_value -= cost * rate * month / 12.0;
                for _p in 2..=(period as i32 - 1) {
                    book_value -= book_value * rate;
                }

                let current_depreciation = if period as i32 == life as i32 && month < 12.0 {
                    book_value * rate * (12.0 - month) / 12.0
                } else {
                    book_value * rate
                };

                Ok(Value::Number(current_depreciation.max(0.0)))
            }
        }
        "FV" => {
            if args.len() < 3 || args.len() > 5 {
                return Err(Error::new(
                    "FV expects 3-5 arguments: rate, nper, pmt, [pv], [type]",
                    None,
                ));
            }

            let rate = required(args, 0, "FV", "rate")?;
            let nper = required(args, 1, "FV", "nper")?;
            let pmt = required(args, 2, "FV", "pmt")?;
            let pv = optional_number_arg("FV", args, 3, "pv", 0.0)?;
            let payment_type = optional_number_arg("FV", args, 4, "type", 0.0)?;

            if nper < 0.0 {
                return Err(Error::new("FV nper must be non-negative", None));
            }

            Ok(Value::Number(fv(rate, nper, pmt, pv, payment_type != 0.0)))
        }
        "IPMT" => {
            if args.len() < 4 || args.len() > 6 {
                return Err(Error::new(
                    "IPMT expects 4-6 arguments: rate, per, nper, pv, [fv], [type]",
                    None,
                ));
            }

            let rate = required(args, 0, "IPMT", "rate")?;
            let per = required(args, 1, "IPMT", "per")?;
            let nper = required(args, 2, "IPMT", "nper")?;
            let pv = required(args, 3, "IPMT", "pv")?;
            let fv = optional_number_arg("IPMT", args, 4, "fv", 0.0)?;
            let payment_type = optional_number_arg("IPMT", args, 5, "type", 0.0)?;

            if per < 1.0 || per > nper || nper <= 0.0 {
                return Err(Error::new("IPMT period must be between 1 and nper", None));
            }

            let at_start = payment_type != 0.0;
            Ok(Value::Number(ipmt(rate, per, nper, pv, fv, at_start)))
        }
        "PPMT" => exec_ppmt(args),
        "PV" => exec_pv(args),
        "NPV" => exec_npv(args),
        _ => Err(Error::new(
            format!("Unknown financial function: {}", name),
            None,
        )),
    }
}

/// A required numeric argument: a number, currency, or a value Excel converts to a number
/// (`utils::number_arg`, BL-066).
fn required(args: &[Value], i: usize, func: &str, what: &str) -> Result<f64, Error> {
    number_arg(func, args, i, what)
}

/// An optional numeric argument: missing or NULL gives `default`; otherwise as `required`.
fn optional(args: &[Value], i: usize, default: f64, func: &str, what: &str) -> Result<f64, Error> {
    optional_number_arg(func, args, i, what, default)
}

/// A finite result, or Excel's `#NUM!` as an error.
fn finite(func: &str, v: f64) -> Result<Value, Error> {
    if v.is_finite() {
        Ok(Value::Number(v))
    } else {
        Err(Error::new(
            format!("{func} result is not a finite number"),
            None,
        ))
    }
}

/// Payment per period (Excel `PMT`).
fn pmt(rate: f64, nper: f64, pv: f64, fv: f64, at_start: bool) -> f64 {
    if rate == 0.0 {
        return -(pv + fv) / nper;
    }
    let pvif = (1.0 + rate).powf(nper);
    let due = if at_start { 1.0 + rate } else { 1.0 };
    -(pv * pvif + fv) / (((pvif - 1.0) / rate) * due)
}

/// Future value (Excel `FV`): the balance after `nper` periods, with Excel's sign convention.
fn fv(rate: f64, nper: f64, pmt: f64, pv: f64, at_start: bool) -> f64 {
    if rate == 0.0 {
        return -pv - pmt * nper;
    }
    let factor = (1.0 + rate).powf(nper);
    let due = if at_start { 1.0 + rate } else { 1.0 };
    -pv * factor - pmt * ((factor - 1.0) / rate) * due
}

/// Interest part of payment `per` (Excel `IPMT`): the interest on the balance at the start of
/// the period, `FV(rate, per - 1, pmt, pv, type) * rate`, so it is negative for a positive `pv`.
/// With payments at the start of the period the first period has no interest and the others are
/// discounted by one period.
fn ipmt(rate: f64, per: f64, nper: f64, pv: f64, fv_: f64, at_start: bool) -> f64 {
    if rate == 0.0 || (at_start && per == 1.0) {
        return 0.0;
    }
    let payment = pmt(rate, nper, pv, fv_, at_start);
    let interest = fv(rate, per - 1.0, payment, pv, at_start) * rate;
    if at_start {
        interest / (1.0 + rate)
    } else {
        interest
    }
}

/// Excel `PPMT(rate, per, nper, pv, [fv], [type])`: the principal part of payment `per`,
/// `PMT - IPMT`, with Excel's signs (negative for a positive `pv`).
fn exec_ppmt(args: &[Value]) -> Result<Value, Error> {
    if args.len() < 4 || args.len() > 6 {
        return Err(Error::new(
            "PPMT expects 4-6 arguments: rate, per, nper, pv, [fv], [type]",
            None,
        ));
    }
    let rate = required(args, 0, "PPMT", "rate")?;
    let per = required(args, 1, "PPMT", "per")?;
    let nper = required(args, 2, "PPMT", "nper")?;
    let pv = required(args, 3, "PPMT", "pv")?;
    let fv_ = optional(args, 4, 0.0, "PPMT", "fv")?;
    let begin = optional(args, 5, 0.0, "PPMT", "type")? != 0.0;
    // NaN fails both comparisons, so it is rejected too.
    if !(per >= 1.0 && per <= nper) {
        return Err(Error::new("PPMT per must be between 1 and nper", None));
    }
    let payment = pmt(rate, nper, pv, fv_, begin);
    finite("PPMT", payment - ipmt(rate, per, nper, pv, fv_, begin))
}

/// Excel `PV(rate, nper, pmt, [fv], [type])`: the present value of a series of payments.
fn exec_pv(args: &[Value]) -> Result<Value, Error> {
    if args.len() < 3 || args.len() > 5 {
        return Err(Error::new(
            "PV expects 3-5 arguments: rate, nper, pmt, [fv], [type]",
            None,
        ));
    }
    let rate = required(args, 0, "PV", "rate")?;
    let nper = required(args, 1, "PV", "nper")?;
    let payment = required(args, 2, "PV", "pmt")?;
    let fv_ = optional(args, 3, 0.0, "PV", "fv")?;
    let begin = optional(args, 4, 0.0, "PV", "type")? != 0.0;
    let pv = if rate == 0.0 {
        -fv_ - payment * nper
    } else {
        let pvif = (1.0 + rate).powf(nper);
        let due = if begin { 1.0 + rate } else { 1.0 };
        -(fv_ + payment * due * (pvif - 1.0) / rate) / pvif
    };
    finite("PV", pv)
}

/// Excel `NPV(rate, value1, ...)`: `Σ value_i / (1 + rate)^i` for i = 1.., in argument order.
/// Arrays are flattened; as in `SUM`, only numbers and currency count (Excel audit X-3).
fn exec_npv(args: &[Value]) -> Result<Value, Error> {
    fn collect(v: &Value, out: &mut Vec<f64>) {
        match v {
            Value::Number(n) | Value::Currency(n) => out.push(*n),
            Value::Array(items) => items.iter().for_each(|it| collect(it, out)),
            _ => {}
        }
    }
    if args.len() < 2 {
        return Err(Error::new("NPV expects rate, value1, [value2], ...", None));
    }
    let rate = required(args, 0, "NPV", "rate")?;
    let mut values = Vec::new();
    args[1..].iter().for_each(|v| collect(v, &mut values));
    if values.is_empty() {
        return Err(Error::new("NPV expects at least one number", None));
    }
    let npv: f64 = (1..)
        .zip(values)
        .map(|(i, v)| v / (1.0 + rate).powf(f64::from(i)))
        .sum();
    finite("NPV", npv)
}
