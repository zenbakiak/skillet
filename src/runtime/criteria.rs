//! Excel criteria for `COUNTIF`, `SUMIF` and `AVERAGEIF` (BL-054).
//!
//! One matcher for the three functions, following Microsoft's documented rules:
//! - An optional operator prefix: `=`, `<>`, `<`, `<=`, `>`, `>=` (none means `=`).
//! - A numeric operand compares with numbers (and currency). For `=` and `<>`, text that reads
//!   as a number also compares as that number, as Excel's `COUNTIF` does.
//! - `TRUE` / `FALSE` compare with booleans.
//! - An empty operand (`""`, `"="`) matches blank values (`NULL`, `""`); `"<>"` matches the rest.
//! - Any other operand is text: case-insensitive, with the wildcards `*` (any run), `?` (one
//!   character) and `~` (escapes the next `*`, `?` or `~`) for `=` and `<>`. `<`, `>` compare
//!   text case-insensitively, without wildcards.

use crate::error::Error;
use crate::types::Value;
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Op {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl Op {
    fn accepts(self, ord: Ordering) -> bool {
        match self {
            Op::Eq => ord == Ordering::Equal,
            Op::Ne => ord != Ordering::Equal,
            Op::Lt => ord == Ordering::Less,
            Op::Le => ord != Ordering::Greater,
            Op::Gt => ord == Ordering::Greater,
            Op::Ge => ord != Ordering::Less,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Operand {
    Number(f64),
    Bool(bool),
    Blank,
    /// Lower-cased text.
    Text(String),
}

/// A parsed Excel criterion.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Criterion {
    op: Op,
    operand: Operand,
}

impl Criterion {
    /// The criterion for a static value, or `None` when the value is not an Excel criterion
    /// (the caller then treats the argument as a lambda over `:x`).
    pub(crate) fn from_value(v: &Value) -> Option<Criterion> {
        match v {
            Value::Number(n) | Value::Currency(n) => Some(Criterion {
                op: Op::Eq,
                operand: Operand::Number(*n),
            }),
            Value::String(s) => Some(Self::parse(s)),
            _ => None,
        }
    }

    fn parse(s: &str) -> Criterion {
        const OPS: [(&str, Op); 6] = [
            (">=", Op::Ge),
            ("<=", Op::Le),
            ("<>", Op::Ne),
            (">", Op::Gt),
            ("<", Op::Lt),
            ("=", Op::Eq),
        ];
        let (op, rest) = OPS
            .iter()
            .find_map(|(p, op)| s.strip_prefix(p).map(|rest| (*op, rest)))
            .unwrap_or((Op::Eq, s));
        Criterion {
            op,
            operand: parse_operand(rest),
        }
    }

    /// Whether `item` meets the criterion.
    pub(crate) fn matches(&self, item: &Value) -> bool {
        let op = self.op;
        match &self.operand {
            Operand::Number(t) => match number_of(item, op) {
                Some(n) => n.partial_cmp(t).is_some_and(|o| op.accepts(o)),
                None => op == Op::Ne,
            },
            Operand::Bool(b) => match item {
                Value::Boolean(v) => op.accepts(v.cmp(b)),
                _ => op == Op::Ne,
            },
            Operand::Blank => {
                let blank =
                    matches!(item, Value::Null) || matches!(item, Value::String(s) if s.is_empty());
                match op {
                    Op::Eq => blank,
                    Op::Ne => !blank,
                    _ => false,
                }
            }
            Operand::Text(pat) => match (op, item) {
                (Op::Eq, Value::String(s)) => wildcard_match(pat, &s.to_lowercase()),
                (Op::Eq, _) => false,
                (Op::Ne, Value::String(s)) => !wildcard_match(pat, &s.to_lowercase()),
                (Op::Ne, _) => true,
                (_, Value::String(s)) => op.accepts(s.to_lowercase().as_str().cmp(pat.as_str())),
                _ => false,
            },
        }
    }
}

fn parse_operand(rest: &str) -> Operand {
    if rest.is_empty() {
        return Operand::Blank;
    }
    if let Some(n) = parse_number(rest) {
        return Operand::Number(n);
    }
    if rest.eq_ignore_ascii_case("TRUE") {
        return Operand::Bool(true);
    }
    if rest.eq_ignore_ascii_case("FALSE") {
        return Operand::Bool(false);
    }
    Operand::Text(rest.to_lowercase())
}

/// A finite decimal number. Rust also parses `inf` and `NaN`, which Excel reads as text.
fn parse_number(s: &str) -> Option<f64> {
    let t = s.trim();
    if !t.bytes().any(|b| b.is_ascii_digit()) {
        return None;
    }
    t.parse::<f64>().ok().filter(|n| n.is_finite())
}

/// The number an item compares as: numbers and currency always; numeric text for `=`/`<>`.
fn number_of(item: &Value, op: Op) -> Option<f64> {
    match item {
        Value::Number(n) | Value::Currency(n) => Some(*n),
        Value::String(s) if matches!(op, Op::Eq | Op::Ne) => parse_number(s),
        _ => None,
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Tok {
    Any,
    One,
    Lit(char),
}

fn tokens(pattern: &str) -> Vec<Tok> {
    let mut out = Vec::new();
    let mut chars = pattern.chars();
    while let Some(c) = chars.next() {
        out.push(match c {
            '*' => Tok::Any,
            '?' => Tok::One,
            '~' => Tok::Lit(chars.next().unwrap_or('~')),
            c => Tok::Lit(c),
        });
    }
    out
}

/// Excel wildcard match over the whole text (both sides already lower-cased). Greedy with one
/// backtrack point, so it is linear for patterns with a single `*` and O(n·m) at worst.
fn wildcard_match(pattern: &str, text: &str) -> bool {
    let pat = tokens(pattern);
    let txt: Vec<char> = text.chars().collect();
    let (mut p, mut t) = (0usize, 0usize);
    let mut star: Option<(usize, usize)> = None;
    while t < txt.len() {
        match pat.get(p) {
            Some(Tok::One) => {
                p += 1;
                t += 1;
            }
            Some(Tok::Lit(c)) if *c == txt[t] => {
                p += 1;
                t += 1;
            }
            Some(Tok::Any) => {
                star = Some((p, t));
                p += 1;
            }
            _ => match star {
                Some((sp, st)) => {
                    p = sp + 1;
                    t = st + 1;
                    star = Some((sp, st + 1));
                }
                None => return false,
            },
        }
    }
    pat[p..].iter().all(|tok| *tok == Tok::Any)
}

/// Which conditional aggregate to compute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Aggregate {
    Count,
    Sum,
    Average,
}

impl Aggregate {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Aggregate::Count => "COUNTIF",
            Aggregate::Sum => "SUMIF",
            Aggregate::Average => "AVERAGEIF",
        }
    }
}

/// The items of a range argument: an array's items, or the value itself (Excel accepts a single
/// cell as a range).
pub(crate) fn range_items(v: Value) -> Vec<Value> {
    match v {
        Value::Array(items) => items,
        other => vec![other],
    }
}

/// `COUNTIF`/`SUMIF`/`AVERAGEIF` over `range`. `values` is the optional sum or average range,
/// paired with `range` by position. Only numbers and currency are summed or averaged; the
/// result is Currency when any summed value is (ADR-0031 A5), else a Number. `COUNTIF` is a Number.
pub(crate) fn aggregate(
    kind: Aggregate,
    range: &[Value],
    values: Option<&[Value]>,
    mut meets: impl FnMut(&Value) -> Result<bool, Error>,
) -> Result<Value, Error> {
    let (mut count, mut sum, mut numbers, mut currency) = (0usize, 0.0, 0usize, false);
    for (i, item) in range.iter().enumerate() {
        if !meets(item)? {
            continue;
        }
        count += 1;
        let target = match values {
            Some(vals) => vals.get(i),
            None => Some(item),
        };
        match target {
            Some(Value::Number(n)) => sum += n,
            Some(Value::Currency(n)) => {
                sum += n;
                currency = true;
            }
            _ => continue,
        }
        numbers += 1;
    }
    let num = |n| {
        if currency {
            Value::Currency(n)
        } else {
            Value::Number(n)
        }
    };
    match kind {
        Aggregate::Count => Ok(Value::Number(count as f64)),
        Aggregate::Sum => Ok(num(sum)),
        Aggregate::Average if numbers == 0 => {
            Err(Error::new("AVERAGEIF: no matching numbers (#DIV/0!)", None))
        }
        Aggregate::Average => Ok(num(sum / numbers as f64)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn m(crit: &str, v: Value) -> bool {
        Criterion::parse(crit).matches(&v)
    }

    #[test]
    fn numeric_operators() {
        assert!(m(">2", Value::Number(3.0)));
        assert!(!m(">2", Value::Number(2.0)));
        assert!(m(">=2", Value::Number(2.0)));
        assert!(m("<2", Value::Number(1.0)));
        assert!(m("<=2", Value::Number(2.0)));
        assert!(m("<>2", Value::Number(1.0)));
        assert!(m("<>2", Value::String("a".into())));
        assert!(m("=2", Value::Number(2.0)));
        assert!(m("2", Value::Currency(2.0)));
        assert!(m("2", Value::String("2".into())));
        assert!(!m(">1", Value::String("2".into())));
        assert!(!m(">1", Value::Boolean(true)));
    }

    #[test]
    fn text_is_case_insensitive_with_wildcards() {
        assert!(m("a", Value::String("A".into())));
        assert!(m("=apple", Value::String("APPLE".into())));
        assert!(m("a*", Value::String("avocado".into())));
        assert!(m("*o", Value::String("avocado".into())));
        assert!(m("a?c", Value::String("abc".into())));
        assert!(!m("a?c", Value::String("abbc".into())));
        assert!(m("*", Value::String(String::new())));
        assert!(m("~*", Value::String("*".into())));
        assert!(!m("~*", Value::String("x".into())));
        assert!(m("a~?", Value::String("a?".into())));
        assert!(m("<>a*", Value::String("banana".into())));
        assert!(!m("<>a*", Value::String("apple".into())));
        assert!(m("<>a", Value::Number(1.0)));
        assert!(m(">b", Value::String("C".into())));
        assert!(!m("a", Value::Number(1.0)));
        assert!(m("inf", Value::String("INF".into())));
    }

    #[test]
    fn blanks_and_booleans() {
        assert!(m("", Value::Null));
        assert!(m("=", Value::String(String::new())));
        assert!(!m("", Value::Number(0.0)));
        assert!(m("<>", Value::Number(0.0)));
        assert!(!m("<>", Value::Null));
        assert!(m("TRUE", Value::Boolean(true)));
        assert!(m("<>true", Value::Boolean(false)));
        assert!(!m("TRUE", Value::String("TRUE".into())));
    }

    #[test]
    fn wildcard_backtracks() {
        assert!(wildcard_match("*a*b", "xxaxxab"));
        assert!(!wildcard_match("*a*b", "xxaxxa"));
        assert!(wildcard_match("**", "abc"));
        assert!(!wildcard_match("abc", "ab"));
    }
}
