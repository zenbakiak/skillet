//! ADR-0033 (BL-097): Skillet has one number type. `x::Currency` rounds to 4 decimals (half away
//! from zero, as `ROUND(x, 4)`) and returns a Number; nothing the language evaluates produces a
//! `Value::Currency`. A `Value::Currency` from Rust (a variable, a custom function result) is read
//! as the Number of the same amount, and results never contain one.
//!
//! One test per section of the ADR's before -> after table, every row included, plus the cast's
//! range rule (it never raises `ROUND`'s `#NUM!`) and the CLI/server `"type"` field.

mod common;

use common::ServerProcess;
use skillet::{
    evaluate, evaluate_with, evaluate_with_assignments_and_context, evaluate_with_custom, Error,
    Value,
};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};

fn v(expr: &str) -> Value {
    evaluate(expr).unwrap_or_else(|e| panic!("{expr}: unexpected error {e:?}"))
}

fn err(expr: &str) -> String {
    match evaluate(expr) {
        Err(Error { message, .. }) => message,
        Ok(r) => panic!("{expr}: expected an error, got {r:?}"),
    }
}

fn n(x: f64) -> Value {
    Value::Number(x)
}

fn s(text: &str) -> Value {
    Value::String(text.to_string())
}

fn nums(xs: &[f64]) -> Value {
    Value::Array(xs.iter().copied().map(Value::Number).collect())
}

/// Each expression evaluates to the value of the paired one.
fn same(cases: &[(&str, &str)]) {
    for (expr, want) in cases {
        assert_eq!(v(expr), v(want), "{expr}");
    }
}

/// Each expression evaluates to the given value.
fn all(cases: &[(&str, Value)]) {
    for (expr, want) in cases {
        assert_eq!(&v(expr), want, "{expr}");
    }
}

/// Exactly `0.0` with a positive sign (not `-0`).
fn assert_positive_zero(expr: &str) {
    match v(expr) {
        Value::Number(x) => assert!(x == 0.0 && x.is_sign_positive(), "{expr}: {x:?}"),
        other => panic!("{expr}: expected a number, got {other:?}"),
    }
}

// ---- Casts ---------------------------------------------------------------------------------

#[test]
fn currency_cast_rounds_to_four_decimals_and_returns_a_number() {
    all(&[
        ("1.23456789::Currency", n(1.2346)),
        ("12.5::Currency", n(12.5)),
        ("\"12.5\"::Currency", n(12.5)),
        ("1.00005::Currency", n(1.0001)),
        ("-1.00005::Currency", n(-1.0001)),
        ("0.00005::Currency", n(0.0001)),
        ("(0.1 + 0.2)::Currency", n(0.3)),
        ("(1/3)::Currency", n(0.3333)),
        ("TRUE::Currency", n(1.0)),
        ("NULL::Currency", n(0.0)),
        ("\"1e3\"::Currency", n(1000.0)),
        ("\"+5\"::Currency", n(5.0)),
    ]);
    // Exactly `ROUND(x, 4)`.
    for x in [
        "1.23456789",
        "1.00005",
        "-1.00005",
        "0.00005",
        "2.675",
        "1234.56785",
    ] {
        assert_eq!(
            v(&format!("({x})::Currency")),
            v(&format!("ROUND({x}, 4)")),
            "{x}"
        );
    }
}

#[test]
fn currency_cast_turns_negative_zero_into_zero() {
    assert_positive_zero("(0 - 0.00004)::Currency");
    assert_positive_zero("(0 - 0)::Currency");
    // A prefix `-` takes the whole cast chain, so the -0 goes in parentheses.
    assert_positive_zero("(-(0))::Currency");
    // Only the cast normalises; ROUND keeps -0.
    match v("ROUND(0 - 0.00004, 4)") {
        Value::Number(x) => assert!(x == 0.0 && x.is_sign_negative()),
        other => panic!("expected a number, got {other:?}"),
    }
}

#[test]
fn currency_cast_input_errors_are_unchanged() {
    for text in ["\" 12.5 \"", "\"$5\"", "\"abc\"", "\"1,5\"", "\"0x10\""] {
        assert_eq!(
            err(&format!("{text}::Currency")),
            "Cannot cast String to Currency",
            "{text}"
        );
    }
    assert_eq!(err("DATE(2026,1,1)::Currency"), "Cannot cast to Currency");
    assert_eq!(err("[1]::Currency"), "Cannot cast to Currency");
}

#[test]
fn currency_cast_never_raises_num_for_large_or_non_finite_values() {
    // Where 4 decimals cannot change an f64 (|n| >= 2^53 / 10^4), the value is kept as is.
    all(&[
        ("1e300::Currency", n(1e300)),
        ("(0 - 1e300)::Currency", n(-1e300)),
        ("1e15::Currency", n(1e15)),
        ("\"1.7976931348623157e308\"::Currency", n(f64::MAX)),
        ("\"inf\"::Currency", n(f64::INFINITY)),
        ("\"-inf\"::Currency", n(f64::NEG_INFINITY)),
    ]);
    match v("\"NaN\"::Currency") {
        Value::Number(x) => assert!(x.is_nan()),
        other => panic!("expected NaN, got {other:?}"),
    }
    // Just below the threshold, rounding still applies.
    assert_eq!(v("123456789.123456::Currency"), n(123456789.1235));
}

#[test]
fn integer_and_float_casts_are_unchanged() {
    all(&[
        ("(1.23456::Currency)::Integer", n(1.0)),
        ("(2.5::Currency)::Float", n(2.5)),
        ("-7.9::Integer", n(-7.0)),
        ("(0-7.9)::Integer", n(-7.0)),
        ("\"abc\"::Integer", n(0.0)),
        ("\"-7.9\"::Integer", n(-7.0)),
        ("1.23456789::Float", n(1.23456789)),
    ]);
}

// ---- Arithmetic ----------------------------------------------------------------------------

#[test]
fn arithmetic_on_currency_is_number_arithmetic() {
    all(&[
        ("0.1::Currency + 0.2::Currency", n(0.1 + 0.2)),
        ("1::Currency + 1", n(2.0)),
        ("2 * 1::Currency", n(2.0)),
        ("1::Currency / 3", n(1.0 / 3.0)),
        ("10::Currency / 4::Currency", n(2.5)),
        ("2::Currency ^ 2", n(4.0)),
        ("7::Currency % 4", n(3.0)),
        ("-(5::Currency)", n(-5.0)),
        ("+(5::Currency)", n(5.0)),
        ("(10::Currency)%", n(0.1)),
        ("1.23456::Currency * 2", n(2.4692)),
    ]);
    same(&[("DATE(2026,1,1) + 1::Currency", "DATE(2026,1,2)")]);
    assert_eq!(err("1::Currency / 0"), "Division by zero (#DIV/0!)");
    assert_eq!(
        err("1e300::Currency * 1e10"),
        "Result is not a finite number (#NUM!)"
    );
}

// ---- Aggregates ----------------------------------------------------------------------------

#[test]
fn aggregates_with_currency_return_numbers() {
    all(&[
        ("SUM(10::Currency, 5)", n(15.0)),
        ("PRODUCT(2::Currency, 3)", n(6.0)),
        ("AVERAGE(10::Currency, 20)", n(15.0)),
        ("MIN(1, 3::Currency)", n(1.0)),
        ("MAX(3::Currency, 9::Currency)", n(9.0)),
        ("SUMIF([1,2::Currency,3], \">1\")", n(5.0)),
        ("AVERAGEIF([1,2::Currency,3], \">1\")", n(2.5)),
        ("ROUND(10::Currency, 1)", n(10.0)),
        ("ROUND(1.23456::Currency, 2)", n(1.23)),
        ("[1::Currency, 2].sum()", n(3.0)),
        ("MEDIAN(1::Currency, 2)", n(1.5)),
        ("COUNT(1::Currency)", n(1.0)),
    ]);
}

// ---- Comparisons and membership ------------------------------------------------------------

#[test]
fn currency_compares_and_matches_as_a_number() {
    let t = Value::Boolean(true);
    let f = Value::Boolean(false);
    all(&[
        ("1 == 1::Currency", t.clone()),
        ("SUM(10::Currency, 5) == 15", t.clone()),
        ("1::Currency < 2", t.clone()),
        ("SUM(10::Currency, 5) > 10", t.clone()),
        ("[1::Currency, 2] == [1, 2]", t.clone()),
        ("FILTER([1,2], :x > 1::Currency)", nums(&[2.0])),
        ("CONTAINS([1::Currency], 1)", t.clone()),
        ("[1::Currency].includes(1)", t.clone()),
        ("UNIQUE([1, 1::Currency])", nums(&[1.0])),
        ("15::Currency == \"15\"", f.clone()),
        ("1::Currency == TRUE", f),
    ]);
}

// ---- Text ----------------------------------------------------------------------------------

#[test]
fn currency_has_the_number_text() {
    all(&[
        ("(2.5::Currency)::String", s("2.5")),
        ("(5::Currency)::String", s("5")),
        ("(1.23456::Currency)::String", s("1.2346")),
        ("JOIN([2.5::Currency])", s("2.5")),
        ("JOIN([1, 2.5::Currency, NULL], \",\")", s("1,2.5,")),
        ("[2.5::Currency].join(\"-\")", s("2.5")),
        ("(2.5::Currency).to_s()", s("2.5")),
        ("[2.5::Currency].to_s()", s("[2.5]")),
        ("CONCAT(2.5::Currency)", s("2.5")),
        ("2.5::Currency & \"\"", s("2.5")),
        ("TEXTJOIN(\",\", TRUE, 2.5::Currency)", s("2.5")),
        ("LEN(2.5::Currency)", n(3.0)),
        ("LEN(1.23456::Currency)", n(6.0)),
    ]);
}

// ---- JSON ----------------------------------------------------------------------------------

#[test]
fn currency_json_is_unchanged() {
    all(&[
        ("(2.5::Currency).to_json()", Value::Json("2.5".into())),
        ("(2.5::Currency)::Json", Value::Json("2.5".into())),
        ("{\"a\": 2.5::Currency}", Value::Json("{\"a\":2.5}".into())),
    ]);
}

#[test]
fn cli_output_json_type_is_number() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_sk"))
        .args(["=2.5::Currency", "--output-json"])
        .env("SKILLET_HOOKS_DIR", hooks_dir())
        .output()
        .expect("run sk");
    assert!(out.status.success(), "{out:?}");
    let text = String::from_utf8_lossy(&out.stdout);
    let start = text.find('{').unwrap_or_else(|| panic!("no JSON: {text}"));
    let json: serde_json::Value =
        serde_json::from_str(&text[start..]).unwrap_or_else(|e| panic!("{e}: {text}"));
    assert_eq!(json["result"].as_f64(), Some(2.5), "{json}");
    assert_eq!(json["type"], "Number", "{json}");
}

fn hooks_dir() -> std::path::PathBuf {
    std::env::temp_dir().join("skillet-one-numeric-type-no-hooks")
}

fn spawn(bin: &str, args: impl Fn(u16) -> Vec<String>) -> ServerProcess {
    ServerProcess::start(|port| {
        let mut cmd = std::process::Command::new(bin);
        cmd.args(args(port))
            .env("SKILLET_HOOKS_DIR", hooks_dir())
            .env("SKILLET_BIND_HOST", "127.0.0.1")
            .env_remove("SKILLET_AUTH_TOKEN")
            .env_remove("SKILLET_ADMIN_TOKEN")
            .stdout(std::process::Stdio::null());
        cmd
    })
}

/// `POST /eval` with `output_json`; returns the structured `result` object.
fn http_structured(server: &ServerProcess, expression: &str) -> serde_json::Value {
    let body = serde_json::json!({ "expression": expression, "output_json": true }).to_string();
    let mut sock = server.connect();
    let req = format!(
        "POST /eval HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    sock.write_all(req.as_bytes()).unwrap();
    let mut resp = String::new();
    let _ = sock.read_to_string(&mut resp);
    let text = resp.split_once("\r\n\r\n").map_or(&*resp, |(_, b)| b);
    let json: serde_json::Value =
        serde_json::from_str(text).unwrap_or_else(|e| panic!("{e}: {resp}"));
    assert_eq!(json["success"], true, "{json}");
    json["result"].clone()
}

/// One `sk_server` request with `output_json`; returns the structured `result` object.
fn sk_server_structured(server: &ServerProcess, expression: &str) -> serde_json::Value {
    let mut sock = server.connect();
    let body = serde_json::json!({ "expression": expression, "output_json": true });
    sock.write_all(format!("{body}\n").as_bytes()).unwrap();
    let mut line = String::new();
    let _ = BufReader::new(sock).read_line(&mut line);
    let json: serde_json::Value =
        serde_json::from_str(&line).unwrap_or_else(|e| panic!("{e}: {line}"));
    assert_eq!(json["success"], true, "{json}");
    json["result"].clone()
}

#[test]
fn http_server_structured_type_is_number() {
    let server = spawn(env!("CARGO_BIN_EXE_sk_http_server"), |port| {
        vec![port.to_string(), "--threads".into(), "1".into()]
    });
    let r = http_structured(&server, "=2.5::Currency");
    assert_eq!(r["result"].as_f64(), Some(2.5), "{r}");
    assert_eq!(r["type"], "Number", "{r}");
    let r = http_structured(&server, "=[1::Currency]");
    assert_eq!(r["result"], serde_json::json!([1.0]), "{r}");
    assert_eq!(r["type"], "Array", "{r}");
}

#[test]
fn sk_server_structured_type_is_number() {
    let server = spawn(env!("CARGO_BIN_EXE_sk_server"), |port| {
        vec![port.to_string(), "1".into()]
    });
    let r = sk_server_structured(&server, "=2.5::Currency");
    assert_eq!(r["result"].as_f64(), Some(2.5), "{r}");
    assert_eq!(r["type"], "Number", "{r}");
    let r = sk_server_structured(&server, "=[1::Currency]");
    assert_eq!(r["result"], serde_json::json!([1.0]), "{r}");
    assert_eq!(r["type"], "Array", "{r}");
}

// ---- Methods and builtins that rejected Currency -------------------------------------------

#[test]
fn number_methods_and_builtins_accept_currency() {
    all(&[
        ("[1::Currency, 2].sort()", nums(&[1.0, 2.0])),
        ("(2.5::Currency).abs()", n(2.5)),
        ("(2.567::Currency).round(2)", n(2.57)),
        ("(2.5::Currency).between(1, 3)", Value::Boolean(true)),
        ("(2.5::Currency).positive?", Value::Boolean(true)),
        ("SORT([3,1,2], -1::Currency)", nums(&[3.0, 2.0, 1.0])),
        ("SORT([3, 1::Currency, 2])", nums(&[1.0, 2.0, 3.0])),
        ("ISNUMBER(1::Currency)", Value::Boolean(true)),
        ("IF(1::Currency, \"y\", \"n\")", s("y")),
        ("NOT 0::Currency", Value::Boolean(true)),
    ]);
    same(&[(
        "DATEADD(DATE(2026,1,1), 1::Currency, \"days\")",
        "DATE(2026,1,2)",
    )]);
}

// ---- Host values (Rust) --------------------------------------------------------------------

#[allow(deprecated)] // the host still builds `Value::Currency` until 0.8
fn host_vars() -> HashMap<String, Value> {
    let mut vars = HashMap::new();
    vars.insert("p".to_string(), Value::Currency(2.5));
    vars.insert(
        "items".to_string(),
        Value::Array(vec![
            Value::Currency(1.0),
            Value::Array(vec![Value::Currency(2.0)]),
            Value::Number(3.0),
        ]),
    );
    vars
}

#[test]
fn host_currency_variables_are_numbers() {
    let vars = host_vars();
    let eval = |expr: &str| evaluate_with(expr, &vars).unwrap_or_else(|e| panic!("{expr}: {e:?}"));
    assert_eq!(eval("=:p"), n(2.5));
    assert_eq!(eval("=[:p]"), nums(&[2.5]));
    assert_eq!(eval("=:p * 2"), n(5.0));
    assert_eq!(eval("=:p == 2.5"), Value::Boolean(true));
    assert_eq!(eval("=[:p].sort()"), nums(&[2.5]));
    // Items of arrays, at any depth.
    assert_eq!(
        eval("=:items"),
        Value::Array(vec![n(1.0), nums(&[2.0]), n(3.0)])
    );
    assert_eq!(eval("=:items.flatten().sum()"), n(6.0));
    assert_eq!(evaluate_with_custom("=:p + 1", &vars).unwrap(), n(3.5));
    let expr = skillet::parse("=:p").unwrap();
    assert_eq!(skillet::evaluate_parsed_with(&expr, &vars).unwrap(), n(2.5));
}

#[test]
fn assignment_outputs_hold_numbers() {
    let vars = host_vars();
    let (result, context) = evaluate_with_assignments_and_context(":q := :p; :q", &vars).unwrap();
    assert_eq!(result, n(2.5));
    assert_eq!(context.get("q"), Some(&n(2.5)));
    assert_eq!(context.get("p"), Some(&n(2.5)));
}

struct HostCurrency;

impl skillet::CustomFunction for HostCurrency {
    fn name(&self) -> &str {
        "ADR33_HOST_CURRENCY"
    }
    fn min_args(&self) -> usize {
        0
    }
    fn max_args(&self) -> Option<usize> {
        Some(0)
    }
    #[allow(deprecated)] // a host function that still returns the deprecated variant
    fn execute(&self, _args: Vec<Value>) -> Result<Value, Error> {
        Ok(Value::Currency(3.0))
    }
}

#[test]
fn custom_function_currency_results_are_numbers() {
    skillet::register_function(Box::new(HostCurrency)).unwrap();
    let vars = HashMap::new();
    let eval = |expr: &str| evaluate_with_custom(expr, &vars).unwrap();
    assert_eq!(eval("=ADR33_HOST_CURRENCY()"), n(3.0));
    assert_eq!(eval("=ADR33_HOST_CURRENCY() + 1"), n(4.0));
    assert_eq!(eval("=[ADR33_HOST_CURRENCY()].sort()"), nums(&[3.0]));
}

#[test]
#[allow(deprecated)]
fn the_deprecated_variant_still_works_from_rust() {
    assert_eq!(Value::Currency(2.5).as_number(), Some(2.5));
    // The derived `PartialEq` is unchanged (ADR-0033 §5.3).
    assert_ne!(Value::Currency(1.0), Value::Number(1.0));
}
