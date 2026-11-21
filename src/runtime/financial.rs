use crate::error::Error;
use crate::runtime::utils::optional_number_arg;
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

            let rate = args[0]
                .as_number()
                .ok_or_else(|| Error::new("FV rate must be a number", None))?;
            let nper = args[1]
                .as_number()
                .ok_or_else(|| Error::new("FV nper must be a number", None))?;
            let pmt = args[2]
                .as_number()
                .ok_or_else(|| Error::new("FV pmt must be a number", None))?;
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
            let fv = optional_number_arg("IPMT", args, 4, "fv", 0.0)?;
            let payment_type = optional_number_arg("IPMT", args, 5, "type", 0.0)?;

            if per < 1.0 || per > nper || nper <= 0.0 {
                return Err(Error::new("IPMT period must be between 1 and nper", None));
            }

            let at_start = payment_type != 0.0;
            Ok(Value::Number(ipmt(rate, per, nper, pv, fv, at_start)))
        }
        _ => Err(Error::new(
            format!("Unknown financial function: {}", name),
            None,
        )),
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
