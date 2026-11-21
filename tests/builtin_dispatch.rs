//! Characterization tests for builtin `UPPERCASE()` dispatch (BL-010).
//!
//! Pins, before the dispatch refactor, exactly which names `function_dispatch` resolves, the
//! value or error message of a representative call for each, aliases, case handling, and the
//! names that are deliberately *not* dispatched there (handled by the evaluator, or unreachable).
//! The name list was copied from the per-category `HashSet`s in `function_dispatch.rs` at d3e6a5d.

use skillet::runtime::function_dispatch::{
    builtin_function_count, exec_builtin_fast, has_builtin_function,
};
use skillet::Value;

/// Every name in the dispatch sets at d3e6a5d (81 names), grouped by the set it was in.
const DISPATCHED: &[&str] = &[
    // arithmetic (17)
    "SUM",
    "AVG",
    "AVERAGE",
    "MIN",
    "MAX",
    "ROUND",
    "CEIL",
    "CEILING",
    "FLOOR",
    "ABS",
    "SQRT",
    "POW",
    "POWER",
    "MOD",
    "INT",
    "PRODUCT",
    "MULTIPLY",
    // logical (6)
    "AND",
    "OR",
    "NOT",
    "XOR",
    "IF",
    "IFS",
    // string (17)
    "LENGTH",
    "CONCAT",
    "UPPER",
    "LOWER",
    "TRIM",
    "SUBSTRING",
    "SPLIT",
    "REPLACE",
    "SUBSTITUTE",
    "SUBSTITUTEM",
    "ISBLANK",
    "ISNUMBER",
    "ISTEXT",
    "INCLUDES",
    "LEFT",
    "RIGHT",
    "MID",
    // array (12)
    "ARRAY",
    "FLATTEN",
    "FIRST",
    "LAST",
    "CONTAINS",
    "IN",
    "COUNT",
    "UNIQUE",
    "SORT",
    "REVERSE",
    "JOIN",
    "MERGE",
    // datetime (8)
    "NOW",
    "DATE",
    "TIME",
    "YEAR",
    "MONTH",
    "DAY",
    "DATEADD",
    "DATEDIFF",
    // financial (4)
    "PMT",
    "DB",
    "FV",
    "IPMT",
    // statistical (16)
    "MEDIAN",
    "MODE.SNGL",
    "MODESNGL",
    "MODE_SNGL",
    "STDEV.P",
    "STDEVP",
    "STDEV_P",
    "VAR.P",
    "VARP",
    "VAR_P",
    "PERCENTILE.INC",
    "PERCENTILEINC",
    "PERCENTILE_INC",
    "QUARTILE.INC",
    "QUARTILEINC",
    "QUARTILE_INC",
    // json (1)
    "DIG",
];

/// Names that `exec_builtin_fast` does NOT resolve today. The evaluator handles the
/// higher-order ones and the internal `__…__` names before dispatch; `BETWEEN` has an
/// implementation in `logical.rs` but is in no set, so it is unreachable.
const NOT_DISPATCHED: &[&str] = &[
    "FILTER",
    "FIND",
    "MAP",
    "REDUCE",
    "SUMIF",
    "AVGIF",
    "COUNTIF",
    "JQ",
    "__TERNARY__",
    "__CONST_TRUE__",
    "__CONST_FALSE__",
    "BETWEEN",
    "NOPE",
    "sum",
    "Sum",
    "",
];

#[test]
fn every_dispatched_name_resolves() {
    assert_eq!(DISPATCHED.len(), 81);
    assert_eq!(builtin_function_count(), 81);
    for name in DISPATCHED {
        assert!(has_builtin_function(name), "{name} should be a builtin");
        // Zero-arg call must reach the right module: no "Unknown … function" from any layer.
        if let Err(e) = exec_builtin_fast(name, &[]) {
            assert!(
                !e.message.starts_with("Unknown"),
                "{name} mis-routed: {}",
                e.message
            );
        }
    }
}

#[test]
fn non_dispatched_names_stay_unknown() {
    for name in NOT_DISPATCHED {
        assert!(!has_builtin_function(name), "{name} must not be a builtin");
        let err = exec_builtin_fast(name, &[Value::Number(1.0)]).unwrap_err();
        assert_eq!(err.message, format!("Unknown function: {name}"));
        assert_eq!(err.position, None);
    }
}

/// Dotted Excel names are in the table but the lexer cannot produce them, so they are only
/// reachable through `exec_builtin_fast`; they must stay equal to their underscore aliases.
#[test]
fn dotted_aliases_match_underscore_forms() {
    let nums = |v: &[f64]| v.iter().map(|n| Value::Number(*n)).collect::<Vec<_>>();
    let groups: &[(&[&str], Vec<Value>, f64)] = &[
        (
            &["MODE.SNGL", "MODESNGL", "MODE_SNGL"],
            nums(&[1.0, 2.0, 2.0, 3.0]),
            2.0,
        ),
        (
            &["STDEV.P", "STDEVP", "STDEV_P"],
            nums(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]),
            2.0,
        ),
        (
            &["VAR.P", "VARP", "VAR_P"],
            nums(&[1.0, 2.0, 3.0, 4.0]),
            1.25,
        ),
        (
            &["PERCENTILE.INC", "PERCENTILEINC", "PERCENTILE_INC"],
            vec![
                Value::Array(nums(&[1.0, 2.0, 3.0, 4.0])),
                Value::Number(0.5),
            ],
            2.5,
        ),
        (
            &["QUARTILE.INC", "QUARTILEINC", "QUARTILE_INC"],
            vec![
                Value::Array(nums(&[1.0, 2.0, 3.0, 4.0, 5.0])),
                Value::Number(1.0),
            ],
            2.0,
        ),
    ];
    for (names, args, want) in groups {
        for name in *names {
            assert_eq!(
                exec_builtin_fast(name, args).unwrap(),
                Value::Number(*want),
                "{name}"
            );
        }
    }
    // Through the parser the dotted spelling is a syntax error, not a call.
    let err = skillet::evaluate("=STDEV.P(1, 3)").unwrap_err();
    assert_eq!(
        err.message,
        "Unexpected identifier (expected function call)"
    );
}

#[test]
fn clock_functions_return_their_types() {
    assert!(matches!(
        skillet::evaluate("=NOW()"),
        Ok(Value::DateTime(_))
    ));
    assert!(matches!(
        skillet::evaluate("=DATE()"),
        Ok(Value::DateTime(_))
    ));
    assert!(matches!(skillet::evaluate("=TIME()"), Ok(Value::Number(_))));
}

#[test]
fn dig_on_json_variable() {
    let r = skillet::evaluate_with_json(
        "=DIG(:obj, ['user', 'tags', 1])",
        r#"{"obj": {"user": {"tags": ["a", "b"]}}}"#,
    );
    assert_eq!(r.unwrap(), Value::String("b".into()));
}

/// `(expression, Debug of the evaluate() result)` captured at d3e6a5d. Covers one call per
/// dispatched name, aliases, mixed-case spellings (the parser uppercases call names),
/// representative error messages, and the evaluator-handled higher-order names.
const CASES: &[(&str, &str)] = &[
    ("SUM(1, 2, [3, 4])", "Ok(Number(10.0))"),
    ("AVG(2, 4, 6)", "Ok(Number(4.0))"),
    ("AVERAGE([1, 2, 3, 4])", "Ok(Number(2.5))"),
    ("MIN(3, 1, 2)", "Ok(Number(1.0))"),
    ("MAX([3, 9, 2])", "Ok(Number(9.0))"),
    ("ROUND(3.14159, 2)", "Ok(Number(3.14))"),
    ("CEIL(2.1)", "Ok(Number(3.0))"),
    ("CEILING(2.1, 5)", "Ok(Number(3.0))"),
    ("FLOOR(-2.1)", "Ok(Number(-3.0))"),
    ("ABS(-7)", "Ok(Number(7.0))"),
    ("SQRT(16)", "Ok(Number(4.0))"),
    ("POW(2, 10)", "Ok(Number(1024.0))"),
    ("POWER(3, 2)", "Ok(Number(9.0))"),
    ("MOD(10, 3)", "Ok(Number(1.0))"),
    ("INT(5.9)", "Ok(Number(5.0))"),
    ("PRODUCT(2, 3, 4)", "Ok(Number(24.0))"),
    ("MULTIPLY(2, [3, 4])", "Ok(Number(24.0))"),
    ("AND(TRUE, FALSE)", "Ok(Boolean(false))"),
    ("OR(FALSE, 1)", "Ok(Boolean(true))"),
    ("NOT(0)", "Ok(Boolean(true))"),
    ("XOR(TRUE, FALSE)", "Ok(Boolean(true))"),
    ("IF(1 > 0, 'yes', 'no')", "Ok(String(\"yes\"))"),
    ("IFS(FALSE, 'a', TRUE, 'b')", "Ok(String(\"b\"))"),
    ("LENGTH('héllo')", "Ok(Number(5.0))"),
    ("CONCAT('a', 1, TRUE, [2, 'b'])", "Ok(String(\"a1TRUE2b\"))"),
    ("UPPER('abc')", "Ok(String(\"ABC\"))"),
    ("LOWER('ABC')", "Ok(String(\"abc\"))"),
    ("TRIM('  x  ')", "Ok(String(\"x\"))"),
    ("SUBSTRING('hello', 1, 3)", "Ok(String(\"ell\"))"),
    ("SPLIT('a, b,c')", "Ok(Array([String(\"a\"), String(\"b\"), String(\"c\")]))"),
    ("REPLACE('abcdef', 2, 3, 'X')", "Ok(String(\"aXef\"))"),
    ("SUBSTITUTE('a-b-c', '-', '+')", "Ok(String(\"a+b+c\"))"),
    ("SUBSTITUTEM('a-b-c', '-', '')", "Ok(String(\"abc\"))"),
    ("ISBLANK('')", "Ok(Boolean(true))"),
    ("ISNUMBER(5)", "Ok(Boolean(true))"),
    ("ISTEXT('x')", "Ok(Boolean(true))"),
    ("INCLUDES('hello', 'ell')", "Ok(Boolean(true))"),
    ("LEFT('hello', 2)", "Ok(String(\"he\"))"),
    ("RIGHT('hello')", "Ok(String(\"o\"))"),
    ("MID('hello', 2, 3)", "Ok(String(\"ell\"))"),
    ("ARRAY(1, 'a', TRUE)", "Ok(Array([Number(1.0), String(\"a\"), Boolean(true)]))"),
    ("FLATTEN([1, [2, [3]]])", "Ok(Array([Number(1.0), Number(2.0), Number(3.0)]))"),
    ("FIRST([9, 8, 7])", "Ok(Number(9.0))"),
    ("LAST([9, 8, 7])", "Ok(Number(7.0))"),
    ("CONTAINS([1, 2, 3], 2)", "Ok(Boolean(true))"),
    ("IN([1, 2, 3], 4)", "Ok(Boolean(false))"),
    ("COUNT([1, 2, 3])", "Ok(Number(3.0))"),
    ("UNIQUE([1, 2, 2, 3])", "Ok(Array([Number(1.0), Number(2.0), Number(3.0)]))"),
    ("SORT([3, 1, 2], 'DESC')", "Ok(Array([Number(3.0), Number(2.0), Number(1.0)]))"),
    ("REVERSE([1, 2, 3])", "Ok(Array([Number(3.0), Number(2.0), Number(1.0)]))"),
    ("JOIN([1, 'a', TRUE], '-')", "Ok(String(\"1-a-TRUE\"))"),
    ("MERGE([1, 2], 3, [4])", "Ok(Array([Number(1.0), Number(2.0), Number(3.0), Number(4.0)]))"),
    ("DATE(2024, 1, 15)", "Ok(DateTime(1705276800))"),
    ("YEAR(DATE(2024, 3, 9))", "Ok(Number(2024.0))"),
    ("MONTH(DATE(2024, 3, 9))", "Ok(Number(3.0))"),
    ("DAY(DATE(2024, 3, 9))", "Ok(Number(9.0))"),
    ("DATEADD(DATE(2024, 1, 31), 1, 'months')", "Ok(DateTime(1709078400))"),
    ("DATEDIFF(DATE(2024, 1, 1), DATE(2024, 3, 1), 'days')", "Ok(Number(60.0))"),
    ("PMT(0.05 / 12, 360, 200000)", "Ok(Number(-1073.6432460242797))"),
    ("DB(10000, 1000, 5, 2)", "Ok(Number(2328.5017392669597))"),
    ("FV(0.05, 10, -100)", "Ok(Number(1257.789253554884))"),
    ("IPMT(0.1 / 12, 1, 36, 8000)", "Ok(Number(66.66666666666667))"),
    ("MEDIAN(1, 3, 2, 4)", "Ok(Number(2.5))"),
    ("MODESNGL(1, 2, 2, 3)", "Ok(Number(2.0))"),
    ("MODE_SNGL(1, 2, 2, 3)", "Ok(Number(2.0))"),
    ("STDEVP(2, 4, 4, 4, 5, 5, 7, 9)", "Ok(Number(2.0))"),
    ("STDEV_P(2, 4, 4, 4, 5, 5, 7, 9)", "Ok(Number(2.0))"),
    ("VARP(1, 2, 3, 4)", "Ok(Number(1.25))"),
    ("VAR_P(1, 2, 3, 4)", "Ok(Number(1.25))"),
    ("PERCENTILEINC([1, 2, 3, 4], 0.25)", "Ok(Number(1.75))"),
    ("PERCENTILE_INC([1, 2, 3, 4], 0.75)", "Ok(Number(3.25))"),
    ("QUARTILEINC([1, 2, 3, 4, 5], 2)", "Ok(Number(3.0))"),
    ("QUARTILE_INC([1, 2, 3, 4, 5], 3)", "Ok(Number(4.0))"),
    ("sum(1, 2)", "Ok(Number(3.0))"),
    ("Avg(2, 4)", "Ok(Number(3.0))"),
    ("average(2, 4)", "Ok(Number(3.0))"),
    ("upper('x')", "Ok(String(\"X\"))"),
    ("mode_sngl(1, 1, 2)", "Ok(Number(1.0))"),
    ("IsBlank(NULL)", "Ok(Boolean(true))"),
    ("dateDiff(DATE(2024,1,1), DATE(2024,1,2), 'days')", "Ok(Number(1.0))"),
    ("BETWEEN(1, 10, 5)", "Err(Error { message: \"Unknown function: BETWEEN\", position: None })"),
    ("NOPE(1)", "Err(Error { message: \"Unknown function: NOPE\", position: None })"),
    ("REVERSE('abc')", "Err(Error { message: \"REVERSE expects array\", position: None })"),
    ("COUNT()", "Err(Error { message: \"COUNT expects 1 argument: array\", position: None })"),
    ("IN([1])", "Err(Error { message: \"IN expects 2 arguments: array, value\", position: None })"),
    ("XOR(TRUE)", "Err(Error { message: \"XOR expects 2 arguments\", position: None })"),
    ("IF(TRUE)", "Err(Error { message: \"IF expects at least 2 arguments\", position: None })"),
    ("IFS(TRUE)", "Err(Error { message: \"IFS expects pairs of condition,value arguments\", position: None })"),
    ("UPPER(1)", "Err(Error { message: \"UPPER expects string\", position: None })"),
    ("FIRST([])", "Err(Error { message: \"FIRST on empty array\", position: None })"),
    ("SORT('x')", "Err(Error { message: \"SORT expects array\", position: None })"),
    ("PMT(1, 2)", "Err(Error { message: \"PMT expects 3-5 arguments: rate, nper, pv, [fv], [type]\", position: None })"),
    ("DB(1, 2, 3)", "Err(Error { message: \"DB expects 4-5 arguments: cost, salvage, life, period, [month]\", position: None })"),
    ("FV(1)", "Err(Error { message: \"FV expects 3-5 arguments: rate, nper, pmt, [pv], [type]\", position: None })"),
    ("IPMT(1)", "Err(Error { message: \"IPMT expects 4-6 arguments: rate, per, nper, pv, [fv], [type]\", position: None })"),
    ("DATE(1, 2)", "Err(Error { message: \"DATE expects either no arguments or three arguments (year, month, day)\", position: None })"),
    ("YEAR(1)", "Err(Error { message: \"YEAR expects datetime\", position: None })"),
    ("DATEADD(1, 2)", "Err(Error { message: \"DATEADD expects date, interval, unit\", position: None })"),
    ("DATEADD(DATE(2024,1,1), 1, 'eons')", "Err(Error { message: \"DATEADD unit must be one of: days, hours, minutes, seconds, weeks, months, years\", position: None })"),
    ("SUBSTRING('x')", "Err(Error { message: \"SUBSTRING expects string, start, [length]\", position: None })"),
    ("MID('x')", "Err(Error { message: \"MID expects string, start, [num_chars]\", position: None })"),
    ("LEFT()", "Err(Error { message: \"LEFT expects string, [num_chars]\", position: None })"),
    ("REPLACE('a')", "Err(Error { message: \"REPLACE expects (old_text: string, start_num: number, num_chars: number, new_text: string)\", position: None })"),
    ("LENGTH(1)", "Err(Error { message: \"LENGTH expects array or string\", position: None })"),
    ("SPLIT(1)", "Err(Error { message: \"SPLIT expects string, [separator]\", position: None })"),
    ("SUBSTITUTE('a')", "Err(Error { message: \"SUBSTITUTE expects (text: string, substr: string, replacement: string)\", position: None })"),
    ("INCLUDES('a')", "Err(Error { message: \"INCLUDES expects string, substring\", position: None })"),
    ("TRIM(1)", "Err(Error { message: \"TRIM expects string\", position: None })"),
    ("LOWER(1)", "Err(Error { message: \"LOWER expects string\", position: None })"),
    ("ROUND('a')", "Err(Error { message: \"ROUND expects number\", position: None })"),
    ("ROUND()", "Ok(Number(0.0))"),
    ("ABS()", "Ok(Number(0.0))"),
    ("MOD(7)", "Ok(Number(0.0))"),
    ("MIN()", "Ok(Number(0.0))"),
    ("AVG()", "Ok(Number(0.0))"),
    ("MEDIAN()", "Ok(Number(0.0))"),
    ("JOIN([[1]])", "Err(Error { message: \"JOIN does not flatten nested arrays\", position: None })"),
    ("CONTAINS(1, 1)", "Err(Error { message: \"CONTAINS expects array, value\", position: None })"),
    ("UNIQUE(1)", "Err(Error { message: \"UNIQUE expects array\", position: None })"),
    ("LAST(1)", "Err(Error { message: \"LAST expects array\", position: None })"),
    ("DIG(1, 2)", "Err(Error { message: \"DIG first argument must be JSON object\", position: None })"),
    ("DIG(1)", "Err(Error { message: \"DIG expects (json_obj, path_array, [default_value])\", position: None })"),
    ("FILTER([1, 2, 3, 4], :x % 2 == 0)", "Ok(Array([Number(2.0), Number(4.0)]))"),
    ("MAP([1, 2, 3], :x * 10)", "Ok(Array([Number(10.0), Number(20.0), Number(30.0)]))"),
    ("REDUCE([1, 2, 3], :acc + :x, 0)", "Ok(Number(6.0))"),
    ("FIND([1, 5, 10], :x > 3)", "Ok(Number(5.0))"),
    ("SUMIF([1, -2, 3], :x > 0)", "Ok(Number(4.0))"),
    ("AVGIF([1, 3, 5, -1], :x > 0)", "Ok(Number(3.0))"),
    ("COUNTIF([1, 2, 3, 4], :x % 2 == 0)", "Ok(Number(2.0))"),
    ("JQ([1, 2], '$[0]')", "Ok(Number(1.0))"),
    ("filter([1, 2], :x > 1)", "Ok(Array([Number(2.0)]))"),
    ("SUM(...[1, 2, 3])", "Ok(Number(6.0))"),
];

#[test]
fn representative_calls_keep_their_results() {
    let mut failures = Vec::new();
    for (expr, want) in CASES {
        let got = format!("{:?}", skillet::evaluate(&format!("={expr}")));
        if got != *want {
            failures.push(format!("{expr}\n   want {want}\n    got {got}"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
