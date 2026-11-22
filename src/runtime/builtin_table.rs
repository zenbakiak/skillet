//! The single table of `UPPERCASE()` builtins (BL-010).
//!
//! One row per function: name, aliases, category, arity, signature, one-line summary and an
//! example. The table drives dispatch (`function_dispatch`) and generates
//! `docs/reference/functions.md` (see the tests at the bottom; `UPDATE_DOCS=1` rewrites it).
//!
//! For [`Dispatch::Module`] rows the category picks the module that runs the function
//! (`arithmetic.rs`, `string.rs`, ...). [`Dispatch::Evaluator`] rows document functions the
//! evaluator handles itself because they take lambdas; they are not dispatched from here.
//!
//! Arity is documentation: the number of arguments the function reads, with `max = None` for
//! variadic. Each function still does its own argument checks, so errors and defaults for
//! missing or extra arguments are unchanged by this table.

use crate::ast::Expr;

/// Which builtin module runs a function, and the section it is documented under.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Category {
    Arithmetic,
    Logical,
    String,
    Array,
    DateTime,
    Financial,
    Statistical,
    Json,
}

/// Where a call to the function is executed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Dispatch {
    /// Evaluated arguments are passed to the category's module by `function_dispatch`.
    Module,
    /// Handled by `runtime::evaluator` before dispatch (lambda arguments, lazy evaluation).
    Evaluator,
}

/// One builtin function. Only `name`, `aliases`, `category` and `dispatch` drive dispatch;
/// the other fields feed the generated reference (read by the tests).
#[allow(dead_code)]
#[derive(Clone, Copy, Debug)]
pub(crate) struct Builtin {
    pub name: &'static str,
    pub aliases: &'static [&'static str],
    pub category: Category,
    pub dispatch: Dispatch,
    pub min_args: usize,
    /// `None` = variadic.
    pub max_args: Option<usize>,
    pub args: &'static str,
    pub summary: &'static str,
    pub example: &'static str,
    /// Result depends on the clock, so the docs do not show a result and the server's result
    /// cache does not keep it (BL-085).
    pub volatile: bool,
    /// Reads the clock when called with no arguments (`DATE()`, `TIME()`); the docs example
    /// passes arguments, so it still shows a result.
    pub volatile_without_args: bool,
}

impl Builtin {
    const fn aliases(self, aliases: &'static [&'static str]) -> Self {
        Builtin { aliases, ..self }
    }
    const fn evaluator(self) -> Self {
        Builtin {
            dispatch: Dispatch::Evaluator,
            ..self
        }
    }
    const fn volatile(self) -> Self {
        Builtin {
            volatile: true,
            ..self
        }
    }
    const fn volatile_without_args(self) -> Self {
        Builtin {
            volatile_without_args: true,
            ..self
        }
    }

    /// The name and every alias.
    pub(crate) fn names(&self) -> impl Iterator<Item = &'static str> {
        std::iter::once(self.name).chain(self.aliases.iter().copied())
    }
}

const fn f(
    name: &'static str,
    category: Category,
    min_args: usize,
    max_args: Option<usize>,
    args: &'static str,
    summary: &'static str,
    example: &'static str,
) -> Builtin {
    Builtin {
        name,
        aliases: &[],
        category,
        dispatch: Dispatch::Module,
        min_args,
        max_args,
        args,
        summary,
        example,
        volatile: false,
        volatile_without_args: false,
    }
}

// `Category::String` stays qualified so `String` keeps meaning `std::string::String`.
use Category::{Arithmetic, Array, DateTime, Financial, Json, Logical, Statistical};

const ANY: Option<usize> = None;
const fn n(max: usize) -> Option<usize> {
    Some(max)
}

/// Every builtin. Names and aliases are unique across the table (tested).
#[rustfmt::skip]
pub(crate) static BUILTINS: &[Builtin] = &[
    // Arithmetic
    f("SUM", Arithmetic, 0, ANY, "values…", "Sum of numbers; arrays are flattened, other values ignored; a date is an error (`#VALUE!`).", "SUM(1, 2, [3, 4])"),
    f("AVG", Arithmetic, 0, ANY, "values…", "Mean of numbers (arrays flattened); error when there are none (Excel `#DIV/0!`) or for a date (`#VALUE!`).", "AVG(2, 4, 6)").aliases(&["AVERAGE"]),
    f("MIN", Arithmetic, 0, ANY, "values…", "Smallest number, or earliest date when there are only dates (arrays flattened); 0 when there are none; numbers and dates together are an error (`#VALUE!`).", "MIN(3, 1, 2)"),
    f("MAX", Arithmetic, 0, ANY, "values…", "Largest number, or latest date when there are only dates (arrays flattened); 0 when there are none; numbers and dates together are an error (`#VALUE!`).", "MAX([3, 9, 2])"),
    f("ROUND", Arithmetic, 1, n(2), "number, [digits]", "Round half away from zero to `digits` decimals (default 0); negative `digits` round left of the point.", "ROUND(1234.567, -2)"),
    f("CEILING", Arithmetic, 1, n(2), "number, [significance]", "Round up to a multiple of `significance` (default 1), Excel rules for negatives.", "CEILING(2.1, 0.5)").aliases(&["CEIL"]),
    f("FLOOR", Arithmetic, 1, n(2), "number, [significance]", "Round down to a multiple of `significance` (default 1), Excel rules for negatives.", "FLOOR(7, 5)"),
    f("ABS", Arithmetic, 1, n(1), "number", "Absolute value.", "ABS(-7)"),
    f("SQRT", Arithmetic, 1, n(1), "number", "Square root; error for a negative number.", "SQRT(16)"),
    f("POW", Arithmetic, 2, n(2), "base, exponent", "`base` raised to `exponent`; error for `0^0`, `0` to a negative power, or a non-real result.", "POW(2, 10)").aliases(&["POWER"]),
    f("MOD", Arithmetic, 2, n(2), "number, divisor", "Remainder of `number / divisor`; the sign follows the divisor (Excel). Error for 0.", "MOD(-10, 3)"),
    f("INT", Arithmetic, 1, n(1), "number", "Round down to an integer: `INT(-3.9)` is -4.", "INT(5.9)"),
    f("PRODUCT", Arithmetic, 0, ANY, "values…", "Product of numbers (arrays flattened); 1 when there are none; a date is an error (`#VALUE!`).", "PRODUCT(2, 3, 4)").aliases(&["MULTIPLY"]),
    f("SUMIF", Arithmetic, 2, n(3), "range, criteria, [sum_range]", "Sum of items meeting an Excel criteria (`\">2\"`, `5`, `\"a*\"`, a date or ISO date text) or a lambda over `:x`; a matched date to sum is an error (`#VALUE!`).", "SUMIF([1, -2, 3], \">0\")").evaluator(),
    // Logical
    f("AND", Logical, 1, ANY, "logical…", "Excel `AND`: TRUE when every logical value is TRUE (numbers: non-zero). Text and NULL inside arrays are ignored; direct text, or no logical value at all, is an error. Every argument is evaluated.", "AND(TRUE, 1)"),
    f("OR", Logical, 1, ANY, "logical…", "Excel `OR`: TRUE when any logical value is TRUE. Same argument rules as `AND`; every argument is evaluated.", "OR(FALSE, 1)"),
    f("NOT", Logical, 1, n(1), "logical", "Excel `NOT`: FALSE for TRUE or a non-zero number, TRUE for FALSE, 0 or NULL; text is an error. Also a prefix keyword that binds looser than comparisons: `NOT :x > 1`.", "NOT(0)"),
    f("XOR", Logical, 1, ANY, "logical…", "Excel `XOR`: TRUE when an odd number of the logical values are TRUE. Same argument rules as `AND`.", "XOR(TRUE, TRUE, TRUE)"),
    f("IF", Logical, 2, n(3), "condition, then, [else]", "`then` when `condition` is TRUE or a non-zero number, otherwise `else` (default FALSE). NULL is FALSE; a text condition is an error. Only the chosen branch is evaluated.", "IF(1 > 0, 'yes', 'no')"),
    f("IFS", Logical, 2, ANY, "condition, value, …", "Value of the first TRUE condition (conditions as in `IF`); an error (`#N/A`) when none is TRUE. Conditions are evaluated in order up to the first TRUE one, and only its value is evaluated.", "IFS(FALSE, 'a', TRUE, 'b')"),
    f("BETWEEN", Logical, 3, n(3), "min, max, value", "TRUE when `min <= value <= max` (both bounds inclusive). All three arguments must be numbers, or all three dates (compared by instant).", "BETWEEN(10, 20, 15)"),
    // String
    f("LENGTH", Category::String, 1, n(1), "text_or_array", "Characters in a string or items in an array; 0 for NULL.", "LENGTH('héllo')"),
    f("LEN", Category::String, 1, n(1), "text", "Excel `LEN`: characters in the text; numbers and booleans count as their text, NULL as 0.", "LEN('héllo')"),
    f("TEXTJOIN", Category::String, 3, ANY, "delimiter, ignore_empty, text…", "Excel `TEXTJOIN`: join the text of each value (arrays flattened) with `delimiter`; skip empty ones when `ignore_empty` is TRUE.", "TEXTJOIN(', ', TRUE, 'a', '', ['b', 'c'])"),
    f("CONCAT", Category::String, 0, ANY, "values…", "Join the text of each value: numbers to 15 significant digits, booleans as TRUE/FALSE, a DateTime as ISO 8601 UTC, a JSON object as its JSON text, arrays flattened; NULL adds nothing.", "CONCAT('a', 1, TRUE)"),
    f("UPPER", Category::String, 1, n(1), "text", "Upper-case.", "UPPER('abc')"),
    f("LOWER", Category::String, 1, n(1), "text", "Lower-case.", "LOWER('ABC')"),
    f("TRIM", Category::String, 1, n(1), "text", "Remove leading and trailing spaces and collapse inner runs of spaces (Excel). `.trim()` keeps inner spaces.", "TRIM('  a   b  ')"),
    f("SUBSTRING", Category::String, 2, n(3), "text, start, [length]", "Characters from 0-based `start`, to the end or `length` characters.", "SUBSTRING('hello', 1, 3)"),
    f("SPLIT", Category::String, 1, n(2), "text, [separator]", "Split into an array; without a separator, split on commas and trim each part.", "SPLIT('a, b,c')"),
    f("REPLACE", Category::String, 4, n(4), "text, start, count, new_text", "Excel `REPLACE`: replace `count` characters from 1-based `start` with `new_text`; `start` < 1 or a negative `count` is an error, a `start` past the end appends.", "REPLACE('abcdef', 2, 3, 'X')"),
    f("SUBSTITUTE", Category::String, 3, n(4), "text, find, replacement, [instance]", "Replace every occurrence of `find`, or only the `instance`-th.", "SUBSTITUTE('a-b-c', '-', '+', 2)"),
    f("SUBSTITUTEM", Category::String, 3, n(3), "text, find, replacement", "Deprecated since 0.7.0: use `SUBSTITUTE`, which also takes `instance`; removed in a later minor release. Replaces every occurrence of `find`.", "SUBSTITUTEM('a-b-c', '-', '')"),
    f("ISBLANK", Category::String, 1, n(1), "value", "TRUE for NULL, a whitespace-only string or an empty array.", "ISBLANK('  ')"),
    f("ISNUMBER", Category::String, 1, n(1), "value", "TRUE for a number.", "ISNUMBER(5)"),
    f("ISTEXT", Category::String, 1, n(1), "value", "TRUE for a string.", "ISTEXT('x')"),
    f("INCLUDES", Category::String, 2, n(2), "text, substring", "TRUE when `text` contains `substring`.", "INCLUDES('hello', 'ell')"),
    f("LEFT", Category::String, 1, n(2), "text, [count]", "First `count` characters (default 1); a negative `count` is an error (Excel).", "LEFT('hello', 2)"),
    f("RIGHT", Category::String, 1, n(2), "text, [count]", "Last `count` characters (default 1); a negative `count` is an error (Excel).", "RIGHT('hello', 2)"),
    f("MID", Category::String, 2, n(3), "text, start, [count]", "Characters from 1-based `start`, to the end or `count` characters; `start` < 1 or a negative `count` is an error, a `start` past the end gives '' (Excel).", "MID('hello', 2, 3)"),
    // Array
    f("ARRAY", Array, 0, ANY, "values…", "Array of the arguments.", "ARRAY(1, 'a', TRUE)"),
    f("FLATTEN", Array, 0, ANY, "values…", "All arguments flattened, recursively, into one array.", "FLATTEN([1, [2, [3]]])"),
    f("FIRST", Array, 1, n(1), "array", "First item; error on an empty array.", "FIRST([9, 8, 7])"),
    f("LAST", Array, 1, n(1), "array", "Last item; error on an empty array.", "LAST([9, 8, 7])"),
    f("CONTAINS", Array, 2, n(2), "array, value", "TRUE when the array has an item equal to `value`.", "CONTAINS([1, 2, 3], 2)"),
    f("IN", Array, 2, n(2), "array, value", "Same as `CONTAINS`, with an exact argument count.", "IN([1, 2, 3], 4)"),
    f("COUNT", Array, 1, ANY, "values…", "How many numbers (and datetimes) there are; arrays flattened, other values ignored (Excel).", "COUNT([1, 'a', TRUE], 2)"),
    f("COUNTA", Array, 1, ANY, "values…", "How many values are not NULL; arrays flattened (Excel).", "COUNTA([1, 'a', NULL], '')"),
    f("UNIQUE", Array, 1, n(1), "array", "Distinct values in first-seen order (`==` equality: same type and value, text case-sensitive).", "UNIQUE([1, 'a', 1, 'a'])"),
    f("SORT", Array, 1, n(2), "array, [order]", "Sorted ascending (Excel: numbers or dates < text < booleans, dates by instant, text ignores case), or descending with order `-1` or `'DESC'`; numbers with dates, and other values, are an error.", "SORT(['b', 2, 'A', 1])"),
    f("REVERSE", Array, 1, n(1), "array or text", "Items of an array, or characters of a string, in reverse order.", "REVERSE([1, 2, 3])"),
    f("JOIN", Array, 1, n(2), "array, [separator]", "The text of each item joined with `separator` (default `,`); nested arrays are an error.", "JOIN([1, 'a', TRUE], '-')"),
    f("MERGE", Array, 0, ANY, "values…", "Arrays concatenated; scalar arguments are appended as items.", "MERGE([1, 2], 3, [4])"),
    f("FILTER", Array, 2, n(3), "array, expr, [param]", "Items for which `expr` is TRUE; the item is `:x` (or `:param`).", "FILTER([1, 2, 3, 4], :x % 2 == 0)").evaluator(),
    f("FIND", Array, 2, n(3), "array, expr, [param]", "First item for which `expr` is TRUE, else NULL. With text first, Excel `FIND(find_text, within_text, [start_num])`: 1-based, case-sensitive position; error when not found.", "FIND([1, 5, 10], :x > 3)").evaluator(),
    f("MAP", Array, 2, n(3), "array, expr, [param]", "`expr` applied to each item `:x`.", "MAP([1, 2, 3], :x * 10)").evaluator(),
    f("REDUCE", Array, 3, n(5), "array, expr, initial, [param], [acc_param]", "Fold: `expr` over `:acc` and `:x`, starting from `initial`.", "REDUCE([1, 2, 3], :acc + :x, 0)").evaluator(),
    // Date and time
    f("NOW", DateTime, 0, n(0), "", "Current UTC time as a DateTime (seconds).", "NOW()").volatile(),
    f("DATE", DateTime, 0, n(3), "[year, month, day]", "Midnight UTC of the given date, Excel rules: months and days roll over, years 0-1899 get 1900 added. Today's local date with no arguments.", "DATE(2024, 1, 15)").volatile_without_args(),
    f("TODAY", DateTime, 0, n(0), "", "Excel `TODAY`: today's local date at midnight, as a UTC DateTime (same as `DATE()`).", "TODAY()").volatile(),
    f("TIME", DateTime, 0, n(3), "[hour, minute, second]", "Time as a fraction of a day (Excel): parts truncated, wrapped at 24 hours; the current local time with no arguments.", "TIME(14, 30, 0)").volatile_without_args(),
    f("YEAR", DateTime, 1, n(1), "datetime", "Year (UTC).", "YEAR(DATE(2024, 3, 9))"),
    f("MONTH", DateTime, 1, n(1), "datetime", "Month 1-12 (UTC).", "MONTH(DATE(2024, 3, 9))"),
    f("DAY", DateTime, 1, n(1), "datetime", "Day of the month (UTC).", "DAY(DATE(2024, 3, 9))"),
    f("DATEADD", DateTime, 3, n(3), "datetime, amount, unit", "Add `amount` units. Units are VBA `DateAdd` codes, any case: yyyy, q, m (months), y or d (days), w (days), ww (weeks), h, n (minutes), s; or years, months, weeks, days, hours, minutes, seconds. Months clamp to the month end.", "DATEADD(DATE(2024, 1, 31), 1, 'm')"),
    f("DATEDIFF", DateTime, 3, n(3), "date1, date2, unit", "`date2 - date1` in VBA `DateDiff` units (the `DATEADD` units): yyyy, q, m and ww count calendar boundaries, the others whole elapsed units.", "DATEDIFF(DATE(2024, 1, 1), DATE(2024, 3, 1), 'days')"),
    f("DATEDIF", DateTime, 3, n(3), "start_date, end_date, unit", "Excel `DATEDIF`: complete units between the dates, unit y, m, d, md, ym or yd (any case); `#NUM!` when start_date > end_date.", "DATEDIF(DATE(2001, 6, 1), DATE(2002, 8, 15), 'yd')"),
    // Financial
    f("PMT", Financial, 3, n(5), "rate, nper, pv, [fv], [type]", "Payment per period of a loan (Excel `PMT`).", "PMT(0.05 / 12, 360, 200000)"),
    f("DB", Financial, 4, n(5), "cost, salvage, life, period, [month]", "Fixed-declining-balance depreciation for a period (Excel `DB`: rate rounded to 3 decimals; period `life + 1` when `month` < 12).", "DB(1000000, 100000, 6, 7, 7)"),
    f("FV", Financial, 3, n(5), "rate, nper, pmt, [pv], [type]", "Future value of an investment (Excel `FV`).", "FV(0.05, 10, -100)"),
    f("IPMT", Financial, 4, n(6), "rate, per, nper, pv, [fv], [type]", "Interest part of payment `per` (Excel `IPMT`; negative for a positive `pv`).", "IPMT(0.1 / 12, 1, 36, 8000)"),
    f("PPMT", Financial, 4, n(6), "rate, per, nper, pv, [fv], [type]", "Principal part of payment `per` (Excel `PPMT`).", "PPMT(0.1 / 12, 1, 24, 2000)"),
    f("PV", Financial, 3, n(5), "rate, nper, pmt, [fv], [type]", "Present value of a series of payments (Excel `PV`).", "PV(0.08 / 12, 240, 500)"),
    f("NPV", Financial, 2, ANY, "rate, values…", "Net present value of periodic cash flows, the first one period from now (Excel `NPV`).", "NPV(0.1, -10000, 3000, 4200, 6800)"),
    // Statistical
    f("MEDIAN", Statistical, 0, ANY, "values…", "Median of numbers (arrays flattened); error when there are none or for a date.", "MEDIAN(1, 3, 2, 4)"),
    f("MODE_SNGL", Statistical, 0, ANY, "values…", "Most frequent number (first seen wins a tie); error when no number repeats or for a date.", "MODE_SNGL(1, 2, 2, 3)").aliases(&["MODESNGL", "MODE.SNGL", "MODE"]),
    f("STDEV_P", Statistical, 0, ANY, "values…", "Population standard deviation; error when there are no numbers or for a date.", "STDEV_P(2, 4, 4, 4, 5, 5, 7, 9)").aliases(&["STDEVP", "STDEV.P"]),
    f("VAR_P", Statistical, 0, ANY, "values…", "Population variance; error when there are no numbers or for a date.", "VAR_P(1, 2, 3, 4)").aliases(&["VARP", "VAR.P"]),
    f("STDEV_S", Statistical, 1, ANY, "values…", "Sample standard deviation (divides by n - 1); error with fewer than two numbers or for a date.", "STDEV_S(2, 4, 4, 4, 5, 5, 7, 9)").aliases(&["STDEV.S", "STDEV"]),
    f("VAR_S", Statistical, 1, ANY, "values…", "Sample variance (divides by n - 1); error with fewer than two numbers or for a date.", "VAR_S(1, 2, 3, 4)").aliases(&["VAR.S", "VAR"]),
    f("PERCENTILE_INC", Statistical, 2, ANY, "values…, k", "Inclusive percentile `k` (0-1) of the values, interpolated; a date is an error.", "PERCENTILE_INC([1, 2, 3, 4], 0.5)").aliases(&["PERCENTILEINC", "PERCENTILE.INC", "PERCENTILE"]),
    f("QUARTILE_INC", Statistical, 2, ANY, "values…, quart", "Inclusive quartile `quart` (0-4) of the values; a date is an error.", "QUARTILE_INC([1, 2, 3, 4, 5], 1)").aliases(&["QUARTILEINC", "QUARTILE.INC", "QUARTILE"]),
    f("AVERAGEIF", Statistical, 2, n(3), "range, criteria, [average_range]", "Mean of items meeting an Excel criteria (as `SUMIF`) or a lambda over `:x`; error when none match or a matched date is to be averaged.", "AVERAGEIF([1, 3, 5, -1], \">0\")").aliases(&["AVGIF"]).evaluator(),
    f("COUNTIF", Statistical, 2, n(2), "range, criteria", "Number of items meeting an Excel criteria (`\">2\"`, `\"<>a\"`, `\"a*\"`, `\">\" & DATE(2026, 1, 1)`) or a lambda over `:x`; dates compare by instant.", "COUNTIF([1, 2, 3, 4], \">2\")").evaluator(),
    // JSON
    f("DIG", Json, 2, n(3), "json, path, [default]", "Value at `path` (array of keys and indexes) in a JSON object, else `default` or NULL.", "DIG(:obj, ['user', 'tags', 1])"),
    f("JQ", Json, 2, n(2), "json, jsonpath", "JSONPath query (`$…`) over a JSON value or array.", "JQ(:obj, '$.user.name')").evaluator(),
];

/// Call names the parser produces for operators and constants; the evaluator runs them before
/// any custom function lookup.
const INTERNAL_NAMES: [&str; 3] = ["__TERNARY__", "__CONST_TRUE__", "__CONST_FALSE__"];

/// True when `name` (any case) is a builtin name or alias, including the ones the evaluator
/// runs itself (`IF`, `FILTER`, `JQ`, ...), or an internal operator name. JS hooks may not use
/// these names (ADR-0024); Rust `CustomFunction`s registered by the embedder still may.
#[cfg_attr(not(feature = "plugins"), allow(dead_code))]
pub(crate) fn is_reserved_function_name(name: &str) -> bool {
    // `to_uppercase` like the custom registry, so every spelling that reaches a builtin counts.
    let upper = name.trim().to_uppercase();
    INTERNAL_NAMES.contains(&upper.as_str())
        || BUILTINS.iter().flat_map(Builtin::names).any(|n| n == upper)
}

/// True when a call to `name` can read the clock, so its result differs from one evaluation to
/// the next. `may_have_no_args` is true when the call can receive no arguments (none written,
/// or only spreads of possibly empty arrays).
fn is_volatile_call(name: &str, may_have_no_args: bool) -> bool {
    BUILTINS
        .iter()
        .find(|b| b.names().any(|n| n.eq_ignore_ascii_case(name)))
        .is_some_and(|b| b.volatile || (may_have_no_args && b.volatile_without_args))
}

/// True when `expr` calls a volatile builtin anywhere: nested in arguments, operands, method
/// receivers or lambdas. Custom functions are not volatile. Recursion is bounded by the parser's
/// depth limit, like evaluation.
pub(crate) fn expr_is_volatile(expr: &Expr) -> bool {
    let any = |items: &[Expr]| items.iter().any(expr_is_volatile);
    match expr {
        Expr::Number(_) | Expr::StringLit(_) | Expr::Null | Expr::Variable(_) => false,
        Expr::Unary(_, e)
        | Expr::Spread(e)
        | Expr::TypeCast { expr: e, .. }
        | Expr::Assignment { value: e, .. }
        | Expr::PropertyAccess { target: e, .. }
        | Expr::SafePropertyAccess { target: e, .. } => expr_is_volatile(e),
        Expr::Binary(a, _, b)
        | Expr::Index {
            target: a,
            index: b,
        } => expr_is_volatile(a) || expr_is_volatile(b),
        Expr::FunctionCall { name, args } => {
            let may_have_no_args = args.iter().all(|a| matches!(a, Expr::Spread(_)));
            is_volatile_call(name, may_have_no_args) || any(args)
        }
        Expr::MethodCall { target, args, .. } | Expr::SafeMethodCall { target, args, .. } => {
            expr_is_volatile(target) || any(args)
        }
        Expr::Array(items) | Expr::Sequence(items) => any(items),
        Expr::ObjectLiteral(pairs) => pairs.iter().any(|(_, v)| expr_is_volatile(v)),
        Expr::Slice { target, start, end } => {
            expr_is_volatile(target)
                || [start, end]
                    .into_iter()
                    .flatten()
                    .any(|e| expr_is_volatile(e))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::function_dispatch::{builtin_function_count, has_builtin_function};
    use crate::types::Value;
    use std::collections::HashSet;
    use std::fmt::Write as _;

    /// Documentation order and section titles.
    const SECTIONS: [(Category, &str); 8] = [
        (Category::Arithmetic, "Arithmetic"),
        (Category::Logical, "Logical"),
        (Category::String, "String"),
        (Category::Array, "Array"),
        (Category::DateTime, "Date and time"),
        (Category::Financial, "Financial"),
        (Category::Statistical, "Statistical"),
        (Category::Json, "JSON"),
    ];

    /// Variables every example is evaluated with.
    const EXAMPLE_VARS: &str = r#"{"obj": {"user": {"name": "alice", "tags": ["a", "b"]}}}"#;
    const DOC_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/docs/reference/functions.md");

    #[test]
    fn names_are_unique_and_uppercase() {
        let mut seen = HashSet::new();
        for b in BUILTINS {
            for name in b.names() {
                assert!(seen.insert(name), "duplicate builtin name {name}");
                assert_eq!(name, name.to_uppercase(), "{name} must be upper-case");
            }
            assert!(b.max_args.unwrap_or(usize::MAX) >= b.min_args, "{}", b.name);
        }
    }

    #[test]
    fn every_table_name_is_reserved_in_any_case() {
        for name in BUILTINS.iter().flat_map(Builtin::names) {
            assert!(is_reserved_function_name(name), "{name}");
            assert!(is_reserved_function_name(&name.to_lowercase()), "{name}");
        }
        // The names `eval_function_call` matches itself, before or after custom lookup.
        for name in [
            "IF",
            "IFS",
            "FILTER",
            "FIND",
            "MAP",
            "REDUCE",
            "SUMIF",
            "AVERAGEIF",
            "AVGIF",
            "COUNTIF",
            "JQ",
            "MERGE",
            "CONCAT",
            "TEXTJOIN",
        ] {
            assert!(is_reserved_function_name(name), "{name}");
        }
        for name in INTERNAL_NAMES {
            assert!(is_reserved_function_name(name), "{name}");
        }
        assert!(is_reserved_function_name("Stdev.p"));
        for name in ["MYFN", "SUMX", "SUM_", "", "STDEV P"] {
            assert!(!is_reserved_function_name(name), "{name}");
        }
    }

    #[test]
    fn module_rows_are_exactly_the_dispatched_names() {
        let module_names: Vec<_> = BUILTINS
            .iter()
            .filter(|b| b.dispatch == Dispatch::Module)
            .flat_map(Builtin::names)
            .collect();
        assert_eq!(module_names.len(), builtin_function_count());
        for b in BUILTINS {
            for name in b.names() {
                assert_eq!(
                    has_builtin_function(name),
                    b.dispatch == Dispatch::Module,
                    "{name}"
                );
            }
        }
    }

    #[test]
    fn examples_call_their_function_and_evaluate() {
        for b in BUILTINS {
            assert!(
                b.example.starts_with(&format!("{}(", b.name)),
                "{}: example must call it",
                b.name
            );
            let r = crate::evaluate_with_json(&format!("={}", b.example), EXAMPLE_VARS);
            assert!(r.is_ok(), "{}: {:?}", b.example, r);
        }
    }

    #[allow(deprecated)] // `Value::Currency` is read as a Number (ADR-0033)
    fn render_value(v: &Value) -> String {
        match v {
            Value::Number(n) | Value::Currency(n) => format!("{n}"),
            Value::String(s) => format!("{s:?}"),
            Value::Boolean(b) => (if *b { "TRUE" } else { "FALSE" }).to_string(),
            Value::Null => "NULL".to_string(),
            Value::Array(items) => {
                let parts: Vec<_> = items.iter().map(render_value).collect();
                format!("[{}]", parts.join(", "))
            }
            Value::DateTime(ts) => chrono::DateTime::from_timestamp(*ts, 0)
                .map(|d| d.format("%Y-%m-%dT%H:%M:%SZ").to_string())
                .unwrap_or_else(|| ts.to_string()),
            Value::Json(s) => s.clone(),
        }
    }

    fn arity(b: &Builtin) -> String {
        match b.max_args {
            None => format!("{}+", b.min_args),
            Some(max) if max == b.min_args => max.to_string(),
            Some(max) => format!("{}-{}", b.min_args, max),
        }
    }

    fn cell(s: &str) -> String {
        s.replace('|', "\\|")
    }

    fn render_row(out: &mut String, b: &Builtin) {
        let mut name = format!("`{}`", b.name);
        if b.dispatch == Dispatch::Evaluator {
            name.push_str(" †");
        }
        if !b.aliases.is_empty() {
            let aliases: Vec<_> = b.aliases.iter().map(|a| format!("`{a}`")).collect();
            let _ = write!(name, "<br>alias {}", aliases.join(", "));
        }
        let result = if b.volatile {
            "(depends on the clock)".to_string()
        } else {
            let r = crate::evaluate_with_json(&format!("={}", b.example), EXAMPLE_VARS);
            format!(
                "`{}`",
                r.map(|v| render_value(&v)).unwrap_or_else(|e| e.message)
            )
        };
        let _ = writeln!(
            out,
            "| {} | `{}({})` | {} | {} | `{}` | {} |",
            name,
            b.name,
            cell(b.args),
            arity(b),
            cell(b.summary),
            cell(b.example),
            cell(&result)
        );
    }

    fn render_markdown() -> String {
        let mut out = String::new();
        out.push_str(
            "# Builtin function reference\n\n\
             <!-- Generated from src/runtime/builtin_table.rs. Do not edit by hand: change the \
             table, then run `UPDATE_DOCS=1 cargo test --lib builtin_table`. -->\n\n\
             Every `UPPERCASE()` builtin. Names are case-insensitive in expressions (the parser \
             upper-cases them, so `sum(1, 2)` calls `SUM`). Custom functions registered with \
             `register_function` are checked first and can shadow a builtin.\n\n\
             - **Arity** is the number of arguments the function reads (`2+` = two or more). \
             Functions check their own arguments: some return an error for a wrong count, others \
             use a default for a missing argument and ignore extra ones.\n\
             - **Result** is what the example evaluates to, produced by the test that writes this \
             file. Examples run with `:obj` = `{\"user\": {\"name\": \"alice\", \"tags\": [\"a\", \"b\"]}}`.\n\
             - **†** marks functions the evaluator runs itself because they take a lambda \
             (`:x` is the item, `:acc` the accumulator) or need unevaluated arguments.\n\
             - Dotted Excel spellings (`STDEV.P(…)`, `MODE.SNGL(…)`, ...) work as written; the \
             underscore forms are aliases. No spaces around the dot.\n\n\
             Methods (`:arr.sum()`), operators and the rest of the language: [api.md](api.md).\n",
        );
        for (category, title) in SECTIONS {
            let _ = write!(
                out,
                "\n## {title}\n\n\
                 | Function | Signature | Arity | Summary | Example | Result |\n\
                 |---|---|---|---|---|---|\n"
            );
            for b in BUILTINS.iter().filter(|b| b.category == category) {
                render_row(&mut out, b);
            }
        }
        out
    }

    /// Fails when `docs/reference/functions.md` does not match the table.
    /// `UPDATE_DOCS=1 cargo test --lib builtin_table` rewrites it.
    #[test]
    fn functions_reference_is_up_to_date() {
        let want = render_markdown();
        if std::env::var_os("UPDATE_DOCS").is_some() {
            std::fs::write(DOC_PATH, &want).expect("write docs/reference/functions.md");
            return;
        }
        let have = std::fs::read_to_string(DOC_PATH).unwrap_or_default();
        assert!(
            have == want,
            "docs/reference/functions.md is out of date; run \
             `UPDATE_DOCS=1 cargo test --lib builtin_table` and commit the result"
        );
    }

    fn volatile(src: &str) -> bool {
        expr_is_volatile(&crate::parse(src).expect("parses"))
    }

    #[test]
    fn volatility_walk_finds_clock_calls_anywhere() {
        for src in [
            "NOW()",
            "TODAY()",
            "now() + 1",
            "IF(TRUE, NOW(), 1)",
            "[1, 2].map(:x + DAY(TODAY()))",
            "{a: {b: NOW()}}",
            ":a := 1; :a + NOW()",
            "DATE()",
            "TIME()",
            "DATE(...:parts)",
            "[1, 2][0:DAY(TODAY())]",
            "(TODAY())::String",
            ":u&.f(NOW())",
            "TRUE ? 1 : NOW()",
        ] {
            assert!(volatile(src), "{src} should be volatile");
        }
        for src in [
            "1 + 2",
            "DATE(2024, 1, 15)",
            "TIME(14, 30, 0)",
            "[1, 2].map(:x * 2).sum()",
            "MYHOOK(1)",
            ":x.now",
            "'NOW()'",
        ] {
            assert!(!volatile(src), "{src} should not be volatile");
        }
    }

    #[test]
    fn every_clock_reader_is_flagged() {
        let flagged: Vec<_> = BUILTINS
            .iter()
            .filter(|b| b.volatile || b.volatile_without_args)
            .map(|b| b.name)
            .collect();
        assert_eq!(flagged, ["NOW", "DATE", "TODAY", "TIME"]);
    }
}
