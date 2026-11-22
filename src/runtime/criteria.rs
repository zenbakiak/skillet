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

use crate::ast::Expr;
use crate::error::Error;
use crate::limits;
use crate::types::Value;
use std::cmp::Ordering;
use std::collections::HashMap;

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
    /// Lower-cased text, compared with `<`, `<=`, `>`, `>=`.
    Text(String),
    /// Lower-cased wildcard pattern, for `=` and `<>`.
    Pattern(Pattern),
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
        let operand = match parse_operand(rest) {
            Operand::Text(t) if matches!(op, Op::Eq | Op::Ne) => {
                Operand::Pattern(Pattern::compile(&t))
            }
            other => other,
        };
        Criterion { op, operand }
    }

    /// Whether `item` meets the criterion. Errors only when the evaluation deadline passes
    /// during a long wildcard match.
    pub(crate) fn matches(&self, item: &Value) -> Result<bool, Error> {
        let op = self.op;
        Ok(match &self.operand {
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
            Operand::Pattern(pat) => match item {
                Value::String(s) => pat.matches(&s.to_lowercase())? == (op == Op::Eq),
                _ => op == Op::Ne,
            },
            Operand::Text(t) => match item {
                Value::String(s) => op.accepts(s.to_lowercase().as_str().cmp(t.as_str())),
                _ => false,
            },
        })
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

/// One token of a wildcard pattern: `*`, `?` or a literal character (`~` already applied).
#[derive(Clone, Copy, Debug, PartialEq)]
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

/// Whether `seg` (no `*`) matches `text` exactly; `text` has the same length as `seg`.
fn fits(seg: &[Tok], text: &[char]) -> bool {
    seg.iter().zip(text).all(|(t, c)| match t {
        Tok::Lit(l) => l == c,
        _ => true,
    })
}

/// A compiled wildcard pattern for `=`/`<>` text criteria (lower-cased, whole-text match).
///
/// The pattern is split on `*` into segments: the first must match the start of the text, the
/// last its end, and the ones in between are found left to right, each at its earliest place
/// after the previous one. Taking the earliest place is always safe, because a later one only
/// leaves less room for the rest. So every text character is looked at a bounded number of
/// times per segment search, and the deadline is checked while scanning (BL-080).
#[derive(Clone, Debug, PartialEq)]
struct Pattern {
    first: Vec<Tok>,
    /// `None` when the pattern has no `*`: the text must then match `first` exactly.
    last: Option<Vec<Tok>>,
    middle: Vec<Finder>,
}

impl Pattern {
    fn compile(pattern: &str) -> Pattern {
        let toks = tokens(pattern);
        let mut segs: Vec<Vec<Tok>> = toks
            .split(|t| *t == Tok::Any)
            .map(<[Tok]>::to_vec)
            .collect();
        let first = segs.remove(0); // `split` yields at least one (possibly empty) segment
        let last = segs.pop();
        let middle = segs
            .into_iter()
            .filter(|s| !s.is_empty())
            .map(Finder::new)
            .collect();
        Pattern {
            first,
            last,
            middle,
        }
    }

    fn matches(&self, text: &str) -> Result<bool, Error> {
        let txt: Vec<char> = text.chars().collect();
        let Some(last) = &self.last else {
            return Ok(self.first.len() == txt.len() && fits(&self.first, &txt));
        };
        let (head, tail) = (self.first.len(), last.len());
        if head + tail > txt.len()
            || !fits(&self.first, &txt[..head])
            || !fits(last, &txt[txt.len() - tail..])
        {
            return Ok(false);
        }
        let mut rest = &txt[head..txt.len() - tail];
        for finder in &self.middle {
            match finder.find(rest)? {
                Some(end) => rest = &rest[end..],
                None => return Ok(false),
            }
        }
        Ok(true)
    }
}

/// Finds the earliest occurrence of one non-empty segment between two `*`.
#[derive(Clone, Debug, PartialEq)]
enum Finder {
    /// Literal characters only: Knuth-Morris-Pratt, linear in the text.
    Literal { needle: Vec<char>, fail: Vec<usize> },
    /// With `?`: bit-parallel (shift-and) over 64-position words; one word per text character
    /// for segments of up to 64 characters.
    Masked {
        len: usize,
        any: Vec<u64>,
        by_char: HashMap<char, Vec<u64>>,
    },
}

impl Finder {
    fn new(seg: Vec<Tok>) -> Finder {
        if seg.iter().all(|t| matches!(t, Tok::Lit(_))) {
            let needle: Vec<char> = seg
                .iter()
                .filter_map(|t| match t {
                    Tok::Lit(c) => Some(*c),
                    _ => None,
                })
                .collect();
            let fail = kmp_table(&needle);
            return Finder::Literal { needle, fail };
        }
        let words = seg.len().div_ceil(64);
        let mut any = vec![0u64; words];
        let mut by_char: HashMap<char, Vec<u64>> = HashMap::new();
        for (i, t) in seg.iter().enumerate() {
            let (w, bit) = (i / 64, 1u64 << (i % 64));
            match t {
                Tok::Lit(c) => by_char.entry(*c).or_insert_with(|| vec![0; words])[w] |= bit,
                _ => any[w] |= bit,
            }
        }
        Finder::Masked {
            len: seg.len(),
            any,
            by_char,
        }
    }

    /// The end index (exclusive) of the earliest match in `text`, if any.
    fn find(&self, text: &[char]) -> Result<Option<usize>, Error> {
        match self {
            Finder::Literal { needle, fail } => {
                let mut k = 0;
                for (i, c) in text.iter().enumerate() {
                    limits::tick_item(i)?;
                    while k > 0 && needle[k] != *c {
                        k = fail[k - 1];
                    }
                    if needle[k] == *c {
                        k += 1;
                    }
                    if k == needle.len() {
                        return Ok(Some(i + 1));
                    }
                }
                Ok(None)
            }
            Finder::Masked { len, any, by_char } => {
                let (top_word, top_bit) = ((len - 1) / 64, 1u64 << ((len - 1) % 64));
                let mut state = vec![0u64; any.len()];
                for (i, c) in text.iter().enumerate() {
                    limits::tick_item(i)?;
                    let mask = by_char.get(c);
                    let mut carry = 1u64;
                    for (w, s) in state.iter_mut().enumerate() {
                        let next_carry = *s >> 63;
                        *s = ((*s << 1) | carry) & (any[w] | mask.map_or(0, |m| m[w]));
                        carry = next_carry;
                    }
                    if state[top_word] & top_bit != 0 {
                        return Ok(Some(i + 1));
                    }
                }
                Ok(None)
            }
        }
    }
}

/// KMP failure table: `fail[i]` is the length of the longest proper border of `needle[..=i]`.
fn kmp_table(needle: &[char]) -> Vec<usize> {
    let mut fail = vec![0; needle.len()];
    let mut k = 0;
    for i in 1..needle.len() {
        while k > 0 && needle[i] != needle[k] {
            k = fail[k - 1];
        }
        if needle[i] == needle[k] {
            k += 1;
        }
        fail[i] = k;
    }
    fail
}

/// Whether a criteria expression reads the item `:x`, which makes it a per-item lambda (BL-081).
/// This is decided from the expression, never by evaluating it: evaluating it first in the
/// caller's scope would read an outer `:x` and turn the lambda into a fixed criterion.
/// `:x` inside the lambda argument of a nested `filter`/`map`/`find`/`reduce` method or a
/// `FILTER`/`FIND`/`MAP`/`REDUCE`/`COUNTIF`/`SUMIF`/`AVERAGEIF` call is bound there, so it does
/// not count.
pub(crate) fn reads_item(expr: &Expr) -> bool {
    match expr {
        Expr::Variable(name) => name == "x",
        Expr::Number(_) | Expr::StringLit(_) | Expr::Null => false,
        Expr::Unary(_, e) | Expr::Spread(e) => reads_item(e),
        Expr::PropertyAccess { target, .. } | Expr::SafePropertyAccess { target, .. } => {
            reads_item(target)
        }
        Expr::TypeCast { expr, .. } => reads_item(expr),
        Expr::Assignment { value, .. } => reads_item(value),
        Expr::Binary(a, _, b) => reads_item(a) || reads_item(b),
        Expr::Index { target, index } => reads_item(target) || reads_item(index),
        Expr::Slice { target, start, end } => {
            reads_item(target) || start.iter().chain(end).any(|e| reads_item(e))
        }
        Expr::MethodCall {
            target, name, args, ..
        }
        | Expr::SafeMethodCall { target, name, args } => {
            let lambda = matches!(name.as_str(), "filter" | "map" | "find" | "reduce");
            reads_item(target)
                || args
                    .iter()
                    .enumerate()
                    .any(|(i, a)| !(lambda && i == 0) && reads_item(a))
        }
        Expr::FunctionCall { name, args } => {
            let lambda = matches!(
                name.to_ascii_uppercase().as_str(),
                "FILTER" | "FIND" | "MAP" | "REDUCE" | "COUNTIF" | "SUMIF" | "AVERAGEIF" | "AVGIF"
            );
            args.iter()
                .enumerate()
                .any(|(i, a)| !(lambda && i == 1) && reads_item(a))
        }
        Expr::Array(items) | Expr::Sequence(items) => items.iter().any(reads_item),
        Expr::ObjectLiteral(fields) => fields.iter().any(|(_, e)| reads_item(e)),
    }
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
        Criterion::parse(crit).matches(&v).unwrap()
    }

    fn wildcard_match(pattern: &str, text: &str) -> bool {
        Pattern::compile(pattern).matches(text).unwrap()
    }

    /// Reference matcher: dynamic programming over (pattern, text) prefixes, as the matcher
    /// before BL-080 behaved. Same tokens, same whole-text rule.
    fn reference_match(pattern: &str, text: &str) -> bool {
        let pat = tokens(pattern);
        let txt: Vec<char> = text.chars().collect();
        // ok[j] = pattern prefix (so far) matches txt[..j]
        let mut ok = vec![false; txt.len() + 1];
        ok[0] = true;
        for tok in &pat {
            let mut next = vec![false; txt.len() + 1];
            for j in 0..=txt.len() {
                next[j] = match tok {
                    Tok::Any => ok[j] || (j > 0 && next[j - 1]),
                    Tok::One => j > 0 && ok[j - 1],
                    Tok::Lit(c) => j > 0 && ok[j - 1] && txt[j - 1] == *c,
                };
            }
            ok = next;
        }
        ok[txt.len()]
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

    #[test]
    fn wildcard_matches_the_reference_on_a_table() {
        let patterns = [
            "", "*", "**", "***", "?", "??", "a", "ab", "a*", "*a", "*a*", "a*b", "a?b", "*a?b*",
            "a*b*c", "*ab*ab*", "~*", "~?", "~~", "a~*b", "*~**", "~?*?", "a~", "~", "*?", "?*",
            "?*?", "*aa*a", "ab*ba", "aba*aba", "*abab*", "a?a?a", "*a?*?b*", "é*ü", "?é",
            "~*~?~~", "x*", "*x*", "ab?*?ba",
        ];
        let texts = [
            "",
            "a",
            "b",
            "ab",
            "ba",
            "abc",
            "aab",
            "abab",
            "ababab",
            "aaaa",
            "abba",
            "abaaba",
            "*",
            "?",
            "~",
            "a*b",
            "a?",
            "**",
            "x",
            "axb",
            "acb",
            "ab*ba",
            "éxü",
            "ée",
            "~*?~",
            "aabab",
            "abcabcabc",
            "bab",
        ];
        for p in patterns {
            for t in texts {
                assert_eq!(
                    wildcard_match(p, t),
                    reference_match(p, t),
                    "pattern {p:?} text {t:?}"
                );
            }
        }
    }

    #[test]
    fn wildcard_matches_the_reference_on_generated_cases() {
        // Deterministic pseudo-random patterns and texts over a small alphabet, so `*`, `?`,
        // `~` and repeats mix often. Includes `?` segments longer than 64 characters.
        let mut seed = 0x2545_f491_4f6c_dd1du64;
        let mut next = move |n: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n) as usize
        };
        let pat_chars = ['a', 'b', '*', '?', '~'];
        for round in 0..4000 {
            let (plen, tlen) = if round % 100 == 0 {
                (150, 300)
            } else {
                (8, 14)
            };
            let pattern: String = (0..next(plen) + 1)
                .map(|_| pat_chars[next(pat_chars.len() as u64)])
                .collect();
            let text: String = (0..next(tlen)).map(|_| ['a', 'b', '*'][next(3)]).collect();
            assert_eq!(
                wildcard_match(&pattern, &text),
                reference_match(&pattern, &text),
                "pattern {pattern:?} text {text:?}"
            );
        }
        // Long `?` segments (more than one 64-bit word) in the middle of a pattern.
        let long = format!("*{}b*", "?".repeat(70));
        assert!(wildcard_match(&long, &format!("{}b", "a".repeat(80))));
        assert!(!wildcard_match(&long, &format!("{}b", "a".repeat(69))));
        assert_eq!(
            wildcard_match(&long, &format!("x{}bx", "a".repeat(70))),
            reference_match(&long, &format!("x{}bx", "a".repeat(70)))
        );
    }

    #[test]
    fn reads_item_is_decided_from_the_expression() {
        let reads = |src: &str| reads_item(&crate::parse(src).unwrap());
        for src in [
            ":x",
            ":x > 2",
            "NOT :x",
            ":x % 2",
            ":x.length() > 1",
            "SUM(:x, 1) > 2",
            "[1, :x]",
        ] {
            assert!(reads(src), "{src}");
        }
        for src in [
            "\">2\"",
            "5",
            ":c",
            ":y > 2",
            ":lst.filter(:x > 1).length()",
            ":lst.map(:x * 2).sum()",
            "FILTER(:lst, :x > 1).length()",
            "COUNTIF(:lst, :x > 1)",
        ] {
            assert!(!reads(src), "{src}");
        }
        assert!(reads(":lst.filter(:x > 1).length() + :x"));
        assert!(reads("FILTER(:x, :y > 1)"));
    }

    #[test]
    fn case_rules_are_unchanged() {
        // Criteria text and item text are both lower-cased; `<`/`>` compare without wildcards.
        assert!(m("A*Z", Value::String("abcz".into())));
        assert!(m("*É", Value::String("café".into())));
        assert!(m("<>A?C", Value::String("abd".into())));
        assert!(!m("<>A?C", Value::String("ABC".into())));
        assert!(m(">a*", Value::String("b".into())));
        assert!(!m(">a*", Value::String("a".into())));
        assert!(m("<b*", Value::String("b".into())));
    }
}
