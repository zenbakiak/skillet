use crate::error::Error;
use crate::types::Value;

pub fn exec_json(name: &str, args: &[Value]) -> Result<Value, Error> {
    match name {
        "DIG" => {
            // DIG(json_obj, path_array, [default_value])
            if args.len() < 2 {
                return Err(Error::new(
                    "DIG expects (json_obj, path_array, [default_value])",
                    None,
                ));
            }
            let json_str = match args.get(0) {
                Some(Value::Json(s)) => s,
                Some(_) => return Err(Error::new("DIG first argument must be JSON object", None)),
                None => return Err(Error::new("DIG missing first argument", None)),
            };
            let path_vals = match args.get(1) {
                Some(Value::Array(v)) => v,
                _ => return Err(Error::new("DIG second argument must be an array path", None)),
            };

            // Traverse JSON by keys and indexes
            let found = match serde_json::from_str::<serde_json::Value>(json_str) {
                Ok(parsed) => {
                    let mut cur = &parsed;
                    let mut ok = true;
                    for seg in path_vals {
                        match seg {
                            Value::String(key) => {
                                if let serde_json::Value::Object(map) = cur {
                                    if let Some(next) = map.get(key) {
                                        cur = next;
                                    } else {
                                        ok = false;
                                        break;
                                    }
                                } else {
                                    ok = false;
                                    break;
                                }
                            }
                            Value::Number(n) => {
                                if let serde_json::Value::Array(arr) = cur {
                                    let idx = if n.is_finite() { n.floor() as isize } else { -1 };
                                    if idx >= 0 && (idx as usize) < arr.len() {
                                        cur = &arr[idx as usize];
                                    } else {
                                        ok = false;
                                        break;
                                    }
                                } else {
                                    ok = false;
                                    break;
                                }
                            }
                            _ => {
                                ok = false;
                                break;
                            }
                        }
                    }
                    if ok { Some(cur.clone()) } else { None }
                }
                Err(e) => return Err(Error::new(format!("Invalid JSON: {}", e), None)),
            };

            if let Some(json_value) = found {
                crate::json_to_value(json_value)
            } else if let Some(default_v) = args.get(2) {
                Ok(default_v.clone())
            } else {
                Ok(Value::Null)
            }
        }
        _ => Err(Error::new(
            format!("Unknown JSON function: {}", name),
            None,
        )),
    }
}

