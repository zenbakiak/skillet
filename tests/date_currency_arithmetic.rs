//! BL-070, ADR-0027 (maintainer decision 2026-11-21):
//! 1. In `DateTime ± x` and `x + DateTime`, `x` converts like any operator operand (ADR-0023):
//!    NULL is 0, numeric text and booleans convert, other text is `#VALUE!`.
//! 2. ADR-0033 (supersedes ADR-0027's Currency rules): `x::Currency` is a Number rounded to 4
//!    decimals, so every operator on it follows the Number rules and returns a Number.
//! 3. Lowercase methods are unchanged (`[1, "2"].sum()` errors).

use skillet::{evaluate, evaluate_with, Value};
use std::collections::HashMap;

fn ok(src: &str) -> Value {
    evaluate(src).unwrap_or_else(|e| panic!("{src}: {e:?}"))
}

fn err(src: &str) -> String {
    match evaluate(src) {
        Err(e) => e.message,
        Ok(v) => panic!("{src}: expected an error, got {v:?}"),
    }
}

/// What an expression on `::Currency` values returns since ADR-0033: a Number.
fn num(n: f64) -> Value {
    Value::Number(n)
}

// ---- 1. Date arithmetic --------------------------------------------------------------------

#[test]
fn date_plus_null_is_the_date() {
    assert_eq!(ok("=DATE(2026, 1, 1) + NULL"), ok("=DATE(2026, 1, 1)"));
    assert_eq!(ok("=NULL + DATE(2026, 1, 1)"), ok("=DATE(2026, 1, 1)"));
    assert_eq!(ok("=DATE(2026, 1, 1) - NULL"), ok("=DATE(2026, 1, 1)"));
}

#[test]
fn numeric_text_and_booleans_shift_a_date() {
    let jan2 = ok("=DATE(2026, 1, 2)");
    assert_eq!(ok("=DATE(2026, 1, 1) + \"1\""), jan2);
    assert_eq!(ok("=\"1\" + DATE(2026, 1, 1)"), jan2);
    assert_eq!(ok("=DATE(2026, 1, 1) + \" 1 \""), jan2);
    assert_eq!(ok("=DATE(2026, 1, 1) + TRUE"), jan2);
    assert_eq!(ok("=TRUE + DATE(2026, 1, 1)"), jan2);
    assert_eq!(ok("=DATE(2026, 1, 3) - \"2\""), ok("=DATE(2026, 1, 1)"));
    assert_eq!(ok("=DATE(2026, 1, 2) - TRUE"), ok("=DATE(2026, 1, 1)"));
    assert_eq!(ok("=DATE(2026, 1, 1) + FALSE"), ok("=DATE(2026, 1, 1)"));
    // Fractional text is a fraction of a day.
    assert_eq!(
        ok("=DATE(2026, 1, 1) + \"0.5\""),
        ok("=DATE(2026, 1, 1) + 0.5")
    );
}

#[test]
fn date_operands_come_from_variables_too() {
    let mut vars = HashMap::new();
    vars.insert("d".to_string(), ok("=DATE(2026, 1, 1)"));
    vars.insert("n".to_string(), Value::Null);
    vars.insert("s".to_string(), Value::String("7".into()));
    assert_eq!(
        evaluate_with("=:d + :n", &vars).unwrap(),
        ok("=DATE(2026, 1, 1)")
    );
    assert_eq!(
        evaluate_with("=:d + :s", &vars).unwrap(),
        ok("=DATE(2026, 1, 8)")
    );
}

#[test]
fn non_numeric_text_with_a_date_is_value_error() {
    let e = "Cannot convert \"abc\" to a number (#VALUE!)";
    assert_eq!(err("=DATE(2026, 1, 1) + \"abc\""), e);
    assert_eq!(err("=\"abc\" + DATE(2026, 1, 1)"), e);
    assert_eq!(err("=DATE(2026, 1, 1) - \"abc\""), e);
    assert_eq!(
        err("=DATE(2026, 1, 1) + \"\""),
        "Cannot convert \"\" to a number (#VALUE!)"
    );
}

#[test]
fn other_date_operations_are_unchanged() {
    let e = "Arithmetic op on non-number";
    assert_eq!(
        ok("=DATE(2026, 1, 2) - DATE(2026, 1, 1)"),
        Value::Number(1.0)
    );
    assert_eq!(err("=DATE(2026, 1, 1) + DATE(2026, 1, 1)"), e);
    assert_eq!(err("=\"1\" - DATE(2026, 1, 1)"), e);
    assert_eq!(err("=NULL - DATE(2026, 1, 1)"), e);
    assert_eq!(err("=DATE(2026, 1, 1) * \"2\""), e);
    assert_eq!(err("=DATE(2026, 1, 1) / TRUE"), e);
    assert_eq!(err("=DATE(2026, 1, 1) ^ NULL"), e);
    assert_eq!(err("=DATE(2026, 1, 1) + [1]"), e);
    // ADR-0033: `1::Currency` is the Number 1, a number of days like any other.
    assert_eq!(
        ok("=DATE(2026, 1, 1) + 1::Currency"),
        ok("=DATE(2026, 1, 2)")
    );
    assert_eq!(
        ok("=1::Currency + DATE(2026, 1, 1)"),
        ok("=DATE(2026, 1, 2)")
    );
    assert_eq!(
        err("=DATE(2026, 1, 1) + \"1e300\""),
        "Date arithmetic out of range"
    );
}

// ---- 2. Currency arithmetic ----------------------------------------------------------------

#[test]
fn currency_with_numbers_is_a_number() {
    assert_eq!(ok("=10::Currency + 2"), num(12.0));
    assert_eq!(ok("=2 + 10::Currency"), num(12.0));
    assert_eq!(ok("=10::Currency - 2"), num(8.0));
    assert_eq!(ok("=2 - 10::Currency"), num(-8.0));
    assert_eq!(ok("=10::Currency * 3"), num(30.0));
    assert_eq!(ok("=3 * 10::Currency"), num(30.0));
    assert_eq!(ok("=10::Currency / 4"), num(2.5));
    assert_eq!(ok("=10 / 4::Currency"), num(2.5));
}

#[test]
fn currency_with_currency_is_a_number() {
    assert_eq!(ok("=10::Currency + 2::Currency"), num(12.0));
    assert_eq!(ok("=10::Currency - 2::Currency"), num(8.0));
    assert_eq!(ok("=10::Currency * 2::Currency"), num(20.0));
    assert_eq!(ok("=10::Currency / 4::Currency"), Value::Number(2.5));
}

#[test]
fn power_and_modulo_with_currency_are_numbers() {
    assert_eq!(ok("=2::Currency ^ 3"), Value::Number(8.0));
    assert_eq!(ok("=2 ^ 3::Currency"), Value::Number(8.0));
    assert_eq!(ok("=2::Currency ^ 3::Currency"), Value::Number(8.0));
    assert_eq!(ok("=7::Currency % 4"), Value::Number(3.0));
    assert_eq!(ok("=7 % 4::Currency"), Value::Number(3.0));
    assert_eq!(ok("=7::Currency % 4::Currency"), Value::Number(3.0));
}

#[test]
fn unary_plus_on_currency_is_a_number() {
    assert_eq!(ok("=+(1::Currency)"), num(1.0));
    assert_eq!(ok("=+(-2.5::Currency)"), num(-2.5));
    assert_eq!(ok("=+(+(3::Currency))"), num(3.0));
    assert_eq!(ok("=+(1::Currency) + 1"), num(2.0));
}

#[test]
fn unary_minus_on_currency_is_a_number() {
    assert_eq!(ok("=-(5::Currency)"), num(-5.0));
    assert_eq!(ok("=-(-5::Currency)"), num(5.0));
    // ADR-0033 supersedes ADR-0027 decision 4: negating follows the Number rule, as `-(0)`
    // does (the cast itself gives 0, never -0).
    match (ok("=-(0::Currency)"), ok("=-(0)")) {
        (Value::Number(c), Value::Number(n)) => {
            assert_eq!(c.to_bits(), n.to_bits());
        }
        v => panic!("expected numbers, got {v:?}"),
    }
    match ok("=0::Currency") {
        Value::Number(n) => assert!(n == 0.0 && n.is_sign_positive()),
        v => panic!("expected a number, got {v:?}"),
    }
}

#[test]
fn currency_converts_the_other_operand_like_a_number() {
    assert_eq!(ok("=10::Currency + \"2\""), num(12.0));
    assert_eq!(ok("=\"2\" * 10::Currency"), num(20.0));
    assert_eq!(ok("=10::Currency + TRUE"), num(11.0));
    assert_eq!(ok("=10::Currency + NULL"), num(10.0));
    assert_eq!(ok("=NULL - 10::Currency"), num(-10.0));
    assert_eq!(ok("=10::Currency ^ \"2\""), Value::Number(100.0));
    assert_eq!(
        err("=10::Currency + \"abc\""),
        "Cannot convert \"abc\" to a number (#VALUE!)"
    );
    assert_eq!(err("=10::Currency + [1]"), "Arithmetic op on non-number");
    assert_eq!(
        err("=10::Currency + {\"a\": 1}"),
        "Arithmetic op on non-number"
    );
}

#[test]
fn arithmetic_on_currency_is_not_rounded_again() {
    // ADR-0033 §3: the cast rounds once; arithmetic on the result is plain f64 arithmetic.
    assert_eq!(ok("=1::Currency / 3"), num(1.0 / 3.0));
    assert_eq!(ok("=0.1::Currency + 0.2"), num(0.1 + 0.2));
    assert_eq!(ok("=ROUND(1::Currency / 3, 2)"), num(0.33));
}

#[test]
fn currency_keeps_the_arithmetic_errors() {
    let div0 = "Division by zero (#DIV/0!)";
    assert_eq!(err("=1::Currency / 0"), div0);
    assert_eq!(err("=1 / 0::Currency"), div0);
    assert_eq!(err("=1::Currency / 0::Currency"), div0);
    assert_eq!(err("=1::Currency / NULL"), div0);
    assert_eq!(err("=1::Currency % 0"), div0);
    assert_eq!(err("=0::Currency ^ -1"), div0);
    assert_eq!(err("=0::Currency ^ 0"), "0 ^ 0 is undefined (#NUM!)");
    assert_eq!(
        err("=1e308::Currency * 10"),
        "Result is not a finite number (#NUM!)"
    );
    assert_eq!(
        err("=(-1)::Currency ^ 0.5"),
        "Result is not a finite number (#NUM!)"
    );
}

#[test]
#[allow(deprecated)] // a host `Value::Currency` is read as a Number (ADR-0033 §5)
fn currency_from_variables() {
    let mut vars = HashMap::new();
    vars.insert("price".to_string(), Value::Currency(19.5));
    vars.insert("qty".to_string(), Value::Number(2.0));
    assert_eq!(evaluate_with("=:price * :qty", &vars).unwrap(), num(39.0));
    assert_eq!(
        evaluate_with("=:price * :qty / :price", &vars).unwrap(),
        Value::Number(2.0)
    );
}

#[test]
fn number_arithmetic_is_unchanged() {
    assert_eq!(ok("=2 + 3 * 4 - (10 / 5) ^ 2 % 3"), Value::Number(13.0));
    assert_eq!(ok("=-(5)"), Value::Number(-5.0));
}

// ---- 3. Lowercase methods are unchanged ----------------------------------------------------

#[test]
fn lowercase_methods_stay_strict() {
    assert_eq!(err("=[1, \"2\"].sum()"), "sum method expects numeric array");
}
