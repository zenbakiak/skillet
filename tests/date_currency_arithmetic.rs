//! BL-070, ADR-0027 (maintainer decision 2026-11-21):
//! 1. In `DateTime ± x` and `x + DateTime`, `x` converts like any operator operand (ADR-0023):
//!    NULL is 0, numeric text and booleans convert, other text is `#VALUE!`.
//! 2. Currency works in the arithmetic operators. `+ - * /` and unary minus give Currency, except
//!    `Currency / Currency` (a Number ratio); `^` and `%` give Number.
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

fn cur(n: f64) -> Value {
    Value::Currency(n)
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
    // Currency is money, not a number of days.
    assert_eq!(err("=DATE(2026, 1, 1) + 1::Currency"), e);
    assert_eq!(err("=1::Currency + DATE(2026, 1, 1)"), e);
    assert_eq!(
        err("=DATE(2026, 1, 1) + \"1e300\""),
        "Date arithmetic out of range"
    );
}

// ---- 2. Currency arithmetic ----------------------------------------------------------------

#[test]
fn currency_with_numbers_is_currency() {
    assert_eq!(ok("=10::Currency + 2"), cur(12.0));
    assert_eq!(ok("=2 + 10::Currency"), cur(12.0));
    assert_eq!(ok("=10::Currency - 2"), cur(8.0));
    assert_eq!(ok("=2 - 10::Currency"), cur(-8.0));
    assert_eq!(ok("=10::Currency * 3"), cur(30.0));
    assert_eq!(ok("=3 * 10::Currency"), cur(30.0));
    assert_eq!(ok("=10::Currency / 4"), cur(2.5));
    assert_eq!(ok("=10 / 4::Currency"), cur(2.5));
}

#[test]
fn currency_with_currency() {
    assert_eq!(ok("=10::Currency + 2::Currency"), cur(12.0));
    assert_eq!(ok("=10::Currency - 2::Currency"), cur(8.0));
    assert_eq!(ok("=10::Currency * 2::Currency"), cur(20.0));
    // A ratio of two amounts is a plain number.
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
fn unary_plus_keeps_currency() {
    assert_eq!(ok("=+(1::Currency)"), cur(1.0));
    assert_eq!(ok("=+(-2.5::Currency)"), cur(-2.5));
    assert_eq!(ok("=+(+(3::Currency))"), cur(3.0));
    assert_eq!(ok("=+(1::Currency) + 1"), cur(2.0));
}

#[test]
fn unary_minus_keeps_currency() {
    assert_eq!(ok("=-(5::Currency)"), cur(-5.0));
    assert_eq!(ok("=-(-5::Currency)"), cur(5.0));
    // `0 - x`, as for numbers: negating zero is 0, not -0.
    match ok("=-(0::Currency)") {
        Value::Currency(n) => assert!(n == 0.0 && n.is_sign_positive()),
        v => panic!("expected currency, got {v:?}"),
    }
}

#[test]
fn currency_converts_the_other_operand_like_a_number() {
    assert_eq!(ok("=10::Currency + \"2\""), cur(12.0));
    assert_eq!(ok("=\"2\" * 10::Currency"), cur(20.0));
    assert_eq!(ok("=10::Currency + TRUE"), cur(11.0));
    assert_eq!(ok("=10::Currency + NULL"), cur(10.0));
    assert_eq!(ok("=NULL - 10::Currency"), cur(-10.0));
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
fn currency_is_not_rounded() {
    // Currency is a tagged f64 (ADR-0003): arithmetic does not round to cents.
    assert_eq!(ok("=1::Currency / 3"), cur(1.0 / 3.0));
    assert_eq!(ok("=0.1::Currency + 0.2"), cur(0.1 + 0.2));
    assert_eq!(ok("=ROUND(1::Currency / 3, 2)"), cur(0.33));
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
fn currency_from_variables() {
    let mut vars = HashMap::new();
    vars.insert("price".to_string(), Value::Currency(19.5));
    vars.insert("qty".to_string(), Value::Number(2.0));
    assert_eq!(evaluate_with("=:price * :qty", &vars).unwrap(), cur(39.0));
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
