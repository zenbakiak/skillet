use crate::error::Error;
use crate::types::Value;

pub fn is_blank(v: &Value) -> bool {
    match v {
        Value::Null => true,
        Value::String(s) => s.trim().is_empty(),
        Value::Array(items) => items.is_empty(),
        _ => false,
    }
}

/// The type name used in error messages.
pub(crate) fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Number(_) => "number",
        Value::Array(_) => "array",
        Value::Boolean(_) => "boolean",
        Value::String(_) => "string",
        Value::Null => "null",
        Value::Currency(_) => "currency",
        Value::DateTime(_) => "datetime",
        Value::Json(_) => "json",
    }
}

/// Text that reads as a number, Excel style (BL-066): decimal or scientific notation after
/// trimming (`"-3"`, `" 4 "`, `"2.5"`, `"1e3"`). Empty text, other characters (`"inf"`,
/// `"1,000"`) and values out of range are not numbers.
pub(crate) fn numeric_text(s: &str) -> Option<f64> {
    let t = s.trim();
    let allowed = |b: u8| b.is_ascii_digit() || matches!(b, b'.' | b'e' | b'E' | b'+' | b'-');
    if t.is_empty() || !t.bytes().all(allowed) {
        return None;
    }
    t.parse::<f64>().ok().filter(|n| n.is_finite())
}

/// Excel's coercion of text and booleans to a number (BL-066): numeric text, `TRUE` = 1 and
/// `FALSE` = 0. `Ok(None)` for every other type (the caller decides: NULL is 0 in the operators
/// and stays an error in builtins). Text that is not a number is `#VALUE!`; `context` prefixes
/// that message (`"ABS: "`).
pub(crate) fn coerce_to_number(v: &Value, context: &str) -> Result<Option<f64>, Error> {
    match v {
        Value::String(s) => match numeric_text(s) {
            Some(n) => Ok(Some(n)),
            None => Err(not_a_number_text(s, context)),
        },
        Value::Boolean(b) => Ok(Some(if *b { 1.0 } else { 0.0 })),
        _ => Ok(None),
    }
}

/// `Cannot convert "abc" to a number (#VALUE!)`, with long text cut to 32 characters.
#[cold]
fn not_a_number_text(s: &str, context: &str) -> Error {
    const SHOW: usize = 32;
    let shown: String = s.chars().take(SHOW).collect();
    let more = if s.chars().nth(SHOW).is_some() {
        "…"
    } else {
        ""
    };
    let verb = if context.is_empty() {
        "Cannot"
    } else {
        "cannot"
    };
    Error::new(
        format!("{context}{verb} convert {shown:?}{more} to a number (#VALUE!)"),
        None,
    )
}

/// A numeric argument of a builtin: a number or currency, or text or a boolean that Excel
/// converts to one (see `coerce_to_number`). Anything else, or a missing
/// argument, is an error (Excel `#VALUE!`), never a silent default.
pub(crate) fn number_arg(name: &str, args: &[Value], i: usize, what: &str) -> Result<f64, Error> {
    match args.get(i) {
        Some(Value::Number(n) | Value::Currency(n)) => Ok(*n),
        Some(other) => {
            let context = if what == "number" {
                format!("{name}: ")
            } else {
                format!("{name} {what}: ")
            };
            if let Some(n) = coerce_to_number(other, &context)? {
                return Ok(n);
            }
            let got = type_name(other);
            let message = if what == "number" {
                format!("{name} expects number, got {got}")
            } else {
                format!("{name} {what} must be a number, got {got}")
            };
            Err(Error::new(message, None))
        }
        None => Err(Error::new(format!("{name} expects {what}"), None)),
    }
}

/// An optional numeric argument: `default` when missing or NULL, an error for other types.
pub(crate) fn optional_number_arg(
    name: &str,
    args: &[Value],
    i: usize,
    what: &str,
    default: f64,
) -> Result<f64, Error> {
    match args.get(i) {
        None | Some(Value::Null) => Ok(default),
        Some(_) => number_arg(name, args, i, what),
    }
}

pub fn clamp_index(len: usize, idx: isize) -> Option<usize> {
    if idx >= 0 {
        let i = idx as usize;
        if i < len {
            Some(i)
        } else {
            None
        }
    } else {
        let neg = idx.unsigned_abs(); // `-idx` overflows for isize::MIN (`[1][-1e300]`)
        if neg <= len {
            Some(len - neg)
        } else {
            None
        }
    }
}

pub fn index_array(items: Vec<Value>, idx: isize) -> Result<Value, Error> {
    match clamp_index(items.len(), idx) {
        Some(i) => Ok(items[i].clone()),
        None => Err(Error::new("Index out of bounds", None)),
    }
}

pub fn slice_array(
    items: Vec<Value>,
    start: Option<Value>,
    end: Option<Value>,
) -> Result<Value, Error> {
    slice_items(&items, start, end)
}

/// `slice_array` over borrowed items (the evaluator slices variables without copying them).
pub(crate) fn slice_items(
    items: &[Value],
    start: Option<Value>,
    end: Option<Value>,
) -> Result<Value, Error> {
    let len = items.len() as isize;
    let s = match start {
        Some(Value::Number(n)) => n as isize,
        None => 0,
        Some(_) => return Err(Error::new("Slice bounds must be numbers", None)),
    };
    let e = match end {
        Some(Value::Number(n)) => n as isize,
        None => len,
        Some(_) => return Err(Error::new("Slice bounds must be numbers", None)),
    };
    let s_norm = if s < 0 { len + s } else { s };
    let e_norm = if e < 0 { len + e } else { e };
    let s_idx = s_norm.max(0).min(len) as usize;
    let e_idx = e_norm.max(0).min(len) as usize;
    if s_idx > e_idx {
        return Ok(Value::Array(Vec::new()));
    }
    Ok(Value::Array(items[s_idx..e_idx].to_vec()))
}

pub fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x == y,
        (Value::Currency(x), Value::Currency(y)) => x == y,
        (Value::Boolean(x), Value::Boolean(y)) => x == y,
        (Value::String(x), Value::String(y)) => x == y,
        (Value::DateTime(x), Value::DateTime(y)) => x == y,
        (Value::Json(x), Value::Json(y)) => x == y,
        (Value::Null, Value::Null) => true,
        // Arrays: shallow equality by elements
        (Value::Array(ax), Value::Array(ay)) => {
            ax.len() == ay.len() && ax.iter().zip(ay.iter()).all(|(u, v)| values_equal(u, v))
        }
        _ => false,
    }
}

/// Sorts numbers ascending (`descending` flips the comparator), or returns an error if any is
/// NaN. NaN has no order, so sorting it either panics or gives an arbitrary result. For NaN-free
/// data the comparator is `partial_cmp` as before, so equal values (`-0.0`, `0.0`) keep their order.
pub(crate) fn sort_numbers(nums: &mut [f64], descending: bool) -> Result<(), Error> {
    if nums.iter().any(|n| n.is_nan()) {
        return Err(Error::new("Cannot sort NaN values", None));
    }
    let cmp = |a: &f64, b: &f64| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal);
    if descending {
        nums.sort_by(|a, b| cmp(b, a));
    } else {
        nums.sort_by(cmp);
    }
    Ok(())
}
