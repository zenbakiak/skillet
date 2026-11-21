use crate::error::Error;
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

            let rate = args[0]
                .as_number()
                .ok_or_else(|| Error::new("PMT rate must be a number", None))?;
            let nper = args[1]
                .as_number()
                .ok_or_else(|| Error::new("PMT nper must be a number", None))?;
            let pv = args[2]
                .as_number()
                .ok_or_else(|| Error::new("PMT pv must be a number", None))?;
            let fv = args.get(3).and_then(|v| v.as_number()).unwrap_or(0.0);
            let payment_type = args.get(4).and_then(|v| v.as_number()).unwrap_or(0.0);

            if nper <= 0.0 {
                return Err(Error::new("PMT nper must be positive", None));
            }

            let payment_at_beginning = payment_type != 0.0;

            let pmt = if rate == 0.0 {
                -(pv + fv) / nper
            } else {
                let pvif = (1.0 + rate).powf(nper);

                -(pv * pvif + fv)
                    / (((pvif - 1.0) / rate)
                        * if payment_at_beginning {
                            1.0 + rate
                        } else {
                            1.0
                        })
            };

            Ok(Value::Number(pmt))
        }
        "DB" => {
            if args.len() < 4 || args.len() > 5 {
                return Err(Error::new(
                    "DB expects 4-5 arguments: cost, salvage, life, period, [month]",
                    None,
                ));
            }

            let cost = args[0]
                .as_number()
                .ok_or_else(|| Error::new("DB cost must be a number", None))?;
            let salvage = args[1]
                .as_number()
                .ok_or_else(|| Error::new("DB salvage must be a number", None))?;
            let life = args[2]
                .as_number()
                .ok_or_else(|| Error::new("DB life must be a number", None))?;
            let period = args[3]
                .as_number()
                .ok_or_else(|| Error::new("DB period must be a number", None))?;
            let month = args.get(4).and_then(|v| v.as_number()).unwrap_or(12.0);

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

            let rate = args[0]
                .as_number()
                .ok_or_else(|| Error::new("FV rate must be a number", None))?;
            let nper = args[1]
                .as_number()
                .ok_or_else(|| Error::new("FV nper must be a number", None))?;
            let pmt = args[2]
                .as_number()
                .ok_or_else(|| Error::new("FV pmt must be a number", None))?;
            let pv = args.get(3).and_then(|v| v.as_number()).unwrap_or(0.0);
            let payment_type = args.get(4).and_then(|v| v.as_number()).unwrap_or(0.0);

            if nper < 0.0 {
                return Err(Error::new("FV nper must be non-negative", None));
            }

            let payment_at_beginning = payment_type != 0.0;

            if rate == 0.0 {
                let fv = -pv - pmt * nper;
                Ok(Value::Number(fv))
            } else {
                let compound_factor = (1.0 + rate).powf(nper);
                let annuity_factor = ((compound_factor - 1.0) / rate)
                    * if payment_at_beginning {
                        1.0 + rate
                    } else {
                        1.0
                    };
                let fv = -pv * compound_factor - pmt * annuity_factor;
                Ok(Value::Number(fv))
            }
        }
        "IPMT" => {
            if args.len() < 4 || args.len() > 6 {
                return Err(Error::new(
                    "IPMT expects 4-6 arguments: rate, per, nper, pv, [fv], [type]",
                    None,
                ));
            }

            let rate = args[0]
                .as_number()
                .ok_or_else(|| Error::new("IPMT rate must be a number", None))?;
            let per = args[1]
                .as_number()
                .ok_or_else(|| Error::new("IPMT per must be a number", None))?;
            let nper = args[2]
                .as_number()
                .ok_or_else(|| Error::new("IPMT nper must be a number", None))?;
            let pv = args[3]
                .as_number()
                .ok_or_else(|| Error::new("IPMT pv must be a number", None))?;
            let fv = args.get(4).and_then(|v| v.as_number()).unwrap_or(0.0);
            let payment_type = args.get(5).and_then(|v| v.as_number()).unwrap_or(0.0);

            if per < 1.0 || per > nper || nper <= 0.0 {
                return Err(Error::new("IPMT period must be between 1 and nper", None));
            }

            let payment_at_beginning = payment_type != 0.0;

            if rate == 0.0 {
                Ok(Value::Number(0.0))
            } else {
                let pvif = (1.0 + rate).powf(nper);
                let pmt = -(pv * pvif + fv)
                    / (((pvif - 1.0) / rate)
                        * if payment_at_beginning {
                            1.0 + rate
                        } else {
                            1.0
                        });

                let mut balance = pv;
                for _p in 1..(per as i32) {
                    let interest = balance * rate;
                    let principal = if payment_at_beginning {
                        pmt - interest / (1.0 + rate)
                    } else {
                        pmt - interest
                    };
                    balance += principal;
                }

                let interest_payment = if payment_at_beginning && per == 1.0 {
                    0.0
                } else {
                    balance * rate
                };

                Ok(Value::Number(interest_payment))
            }
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

/// A required numeric argument (number or currency).
fn required(args: &[Value], i: usize, func: &str, what: &str) -> Result<f64, Error> {
    match args.get(i) {
        Some(Value::Number(n)) | Some(Value::Currency(n)) => Ok(*n),
        _ => Err(Error::new(format!("{func} {what} must be a number"), None)),
    }
}

/// An optional numeric argument: missing or NULL gives `default`; any other non-number is an
/// error (Excel `#VALUE!`), never silently ignored.
fn optional(args: &[Value], i: usize, default: f64, func: &str, what: &str) -> Result<f64, Error> {
    match args.get(i) {
        None | Some(Value::Null) => Ok(default),
        Some(_) => required(args, i, func, what),
    }
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

/// Excel `PMT` with `type` as a flag (non-zero = payments at the start of each period).
fn pmt(rate: f64, nper: f64, pv: f64, fv: f64, begin: bool) -> f64 {
    if rate == 0.0 {
        return -(pv + fv) / nper;
    }
    let pvif = (1.0 + rate).powf(nper);
    let due = if begin { 1.0 + rate } else { 1.0 };
    -(pv * pvif + fv) * rate / ((pvif - 1.0) * due)
}

/// Excel `FV` of `pv` and `pmt` after `nper` periods.
fn fv(rate: f64, nper: f64, pmt: f64, pv: f64, begin: bool) -> f64 {
    if rate == 0.0 {
        return -pv - pmt * nper;
    }
    let pvif = (1.0 + rate).powf(nper);
    let due = if begin { 1.0 + rate } else { 1.0 };
    -pv * pvif - pmt * due * (pvif - 1.0) / rate
}

/// Excel `PPMT(rate, per, nper, pv, [fv], [type])`: the principal part of payment `per`,
/// `PMT - IPMT`, with Excel's signs (negative for a positive `pv`). Interest in period `per` is
/// the balance after `per - 1` payments times `rate` (discounted one period, and zero in the
/// first period, when payments are due at the start).
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
    let interest = if rate == 0.0 || (begin && per == 1.0) {
        0.0
    } else {
        let balance = fv(rate, per - 1.0, payment, pv, begin);
        let interest = balance * rate;
        if begin {
            interest / (1.0 + rate)
        } else {
            interest
        }
    };
    finite("PPMT", payment - interest)
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
