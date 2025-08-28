use crate::types::Value;
use crate::error::Error;

pub fn exec_financial(name: &str, args: &[Value]) -> Result<Value, Error> {
    match name {
        "PMT" => {
            if args.len() < 3 || args.len() > 5 {
                return Err(Error::new("PMT expects 3-5 arguments: rate, nper, pv, [fv], [type]", None));
            }
            
            let rate = args[0].as_number().ok_or_else(|| Error::new("PMT rate must be a number", None))?;
            let nper = args[1].as_number().ok_or_else(|| Error::new("PMT nper must be a number", None))?;
            let pv = args[2].as_number().ok_or_else(|| Error::new("PMT pv must be a number", None))?;
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
                let payment = -(pv * pvif + fv) / (((pvif - 1.0) / rate) * if payment_at_beginning { 1.0 + rate } else { 1.0 });
                payment
            };
            
            Ok(Value::Number(pmt))
        }
        "DB" => {
            if args.len() < 4 || args.len() > 5 {
                return Err(Error::new("DB expects 4-5 arguments: cost, salvage, life, period, [month]", None));
            }
            
            let cost = args[0].as_number().ok_or_else(|| Error::new("DB cost must be a number", None))?;
            let salvage = args[1].as_number().ok_or_else(|| Error::new("DB salvage must be a number", None))?;
            let life = args[2].as_number().ok_or_else(|| Error::new("DB life must be a number", None))?;
            let period = args[3].as_number().ok_or_else(|| Error::new("DB period must be a number", None))?;
            let month = args.get(4).and_then(|v| v.as_number()).unwrap_or(12.0);
            
            if cost < 0.0 || salvage < 0.0 || life <= 0.0 || period < 0.0 {
                return Err(Error::new("DB arguments must be non-negative (life must be positive)", None));
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
                return Err(Error::new("FV expects 3-5 arguments: rate, nper, pmt, [pv], [type]", None));
            }
            
            let rate = args[0].as_number().ok_or_else(|| Error::new("FV rate must be a number", None))?;
            let nper = args[1].as_number().ok_or_else(|| Error::new("FV nper must be a number", None))?;
            let pmt = args[2].as_number().ok_or_else(|| Error::new("FV pmt must be a number", None))?;
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
                let annuity_factor = ((compound_factor - 1.0) / rate) * if payment_at_beginning { 1.0 + rate } else { 1.0 };
                let fv = -pv * compound_factor - pmt * annuity_factor;
                Ok(Value::Number(fv))
            }
        }
        "IPMT" => {
            if args.len() < 4 || args.len() > 6 {
                return Err(Error::new("IPMT expects 4-6 arguments: rate, per, nper, pv, [fv], [type]", None));
            }
            
            let rate = args[0].as_number().ok_or_else(|| Error::new("IPMT rate must be a number", None))?;
            let per = args[1].as_number().ok_or_else(|| Error::new("IPMT per must be a number", None))?;
            let nper = args[2].as_number().ok_or_else(|| Error::new("IPMT nper must be a number", None))?;
            let pv = args[3].as_number().ok_or_else(|| Error::new("IPMT pv must be a number", None))?;
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
                let pmt = -(pv * pvif + fv) / (((pvif - 1.0) / rate) * if payment_at_beginning { 1.0 + rate } else { 1.0 });
                
                let mut balance = pv;
                for _p in 1..(per as i32) {
                    let interest = balance * rate;
                    let principal = if payment_at_beginning { pmt - interest / (1.0 + rate) } else { pmt - interest };
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
        _ => Err(Error::new(format!("Unknown financial function: {}", name), None)),
    }
}