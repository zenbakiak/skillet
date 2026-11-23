//! BL-066 (maintainer decision 2026-11-21, "like Excel"), with BL-058:
//! 1. Numeric text and booleans convert to numbers in the arithmetic operators and in the scalar
//!    math builtins; other text is `#VALUE!`; NULL is 0 in the operators (a blank cell).
//! 2. `%` is the percent suffix unless an operand starts right after it (then it is modulo).
//! 3. Scientific notation is a number literal (`1e3`), and `1e308*10` overflows to `#NUM!`.

use skillet::{evaluate, evaluate_with, Value};
use std::collections::HashMap;

fn num(src: &str) -> f64 {
    match evaluate(src) {
        Ok(Value::Number(n)) => n,
        other => panic!("{src}: expected a number, got {other:?}"),
    }
}

fn close(src: &str, want: f64) {
    let got = num(src);
    assert!((got - want).abs() < 1e-12, "{src}: {got} != {want}");
}

fn err(src: &str) -> String {
    match evaluate(src) {
        Err(e) => e.message,
        Ok(v) => panic!("{src}: expected an error, got {v:?}"),
    }
}

// ---- 1. Coercion in the operators ----------------------------------------------------------

#[test]
fn numeric_text_converts_in_operators() {
    close("=\"3\" + 1", 4.0);
    close("=1 + \"3\"", 4.0);
    close("=\"-3\" * 2", -6.0);
    close("=\"2.5\" * 2", 5.0);
    close("=\"1e3\" / 10", 100.0);
    close("=\" 4 \" - 1", 3.0);
    close("=\".5\" + 0", 0.5);
    close("=\"2\" ^ \"3\"", 8.0);
    close("=\"10\" % \"3\"", 1.0);
    close("=-\"3\"", -3.0);
    close("=+\"3\"", 3.0);
    close("=\"50\"%", 0.5);
}

#[test]
fn numeric_text_from_variables_converts() {
    let vars = HashMap::from([
        ("price".to_string(), Value::String("19.99".to_string())),
        ("qty".to_string(), Value::String(" 3 ".to_string())),
    ]);
    let got = evaluate_with(":price * :qty", &vars).unwrap();
    match got {
        Value::Number(n) => assert!((n - 59.97).abs() < 1e-9, "{n}"),
        other => panic!("expected a number, got {other:?}"),
    }
}

#[test]
fn booleans_are_one_and_zero_in_operators() {
    close("=TRUE + 1", 2.0);
    close("=FALSE + 1", 1.0);
    close("=TRUE * 5", 5.0);
    close("=TRUE + TRUE", 2.0);
    close("=-TRUE", -1.0);
    close("=(3 > 2) * 10", 10.0);
    assert_eq!(err("=1 / FALSE"), "Division by zero (#DIV/0!)");
}

#[test]
fn null_is_zero_in_operators() {
    close("=1 + NULL", 1.0);
    close("=NULL * 2", 0.0);
    close("=NULL - 5", -5.0);
    close("=NULL ^ 2", 0.0);
    // 0, not -0: Excel has no negative zero.
    assert_eq!(evaluate("=-NULL").unwrap(), Value::Number(0.0));
    assert!(num("=-NULL").is_sign_positive());
    assert_eq!(err("=1 / NULL"), "Division by zero (#DIV/0!)");
}

#[test]
fn text_that_is_not_a_number_is_value_error() {
    assert_eq!(
        err("=\"abc\" + 1"),
        "Cannot convert \"abc\" to a number (#VALUE!)"
    );
    assert_eq!(
        err("=1 * \"abc\""),
        "Cannot convert \"abc\" to a number (#VALUE!)"
    );
    assert_eq!(
        err("=-\"abc\""),
        "Cannot convert \"abc\" to a number (#VALUE!)"
    );
    assert_eq!(
        err("=\"\" + 1"),
        "Cannot convert \"\" to a number (#VALUE!)"
    );
    assert_eq!(
        err("=\"  \" + 1"),
        "Cannot convert \"  \" to a number (#VALUE!)"
    );
    for text in [
        "1,000", "$5", "inf", "NaN", "1e", "e3", "0x10", "1 2", "1e999",
    ] {
        let msg = err(&format!("=\"{text}\" + 1"));
        assert!(msg.ends_with("to a number (#VALUE!)"), "{text}: {msg}");
    }
    // Long text is cut in the message.
    let long = "x".repeat(100);
    let msg = err(&format!("=\"{long}\" + 1"));
    assert_eq!(
        msg,
        format!(
            "Cannot convert \"{}\"… to a number (#VALUE!)",
            "x".repeat(32)
        )
    );
}

#[test]
fn plus_on_two_strings_is_not_concatenation() {
    // `+` never joined text (it was a type error); `&` is the text operator.
    assert_eq!(
        err("=\"a\" + \"b\""),
        "Cannot convert \"a\" to a number (#VALUE!)"
    );
    assert_eq!(
        evaluate("=\"a\" & \"b\"").unwrap(),
        Value::String("ab".to_string())
    );
    close("=\"1\" + \"2\"", 3.0);
}

#[test]
fn other_types_keep_the_type_error() {
    assert_eq!(err("=[1] + 1"), "Arithmetic op on non-number");
    assert_eq!(err("=1 + {\"a\": 1}"), "Arithmetic op on non-number");
    assert_eq!(err("=-[1]"), "Unary '-' on non-number");
}

#[test]
fn date_arithmetic_converts_the_other_operand() {
    let jan2 = evaluate("=DATE(2026, 1, 1) + 1").unwrap();
    assert_eq!(jan2, evaluate("=DATE(2026, 1, 2)").unwrap());
    close("=DATE(2026, 1, 2) - DATE(2026, 1, 1)", 1.0);
    // ADR-0027: the non-date operand converts like any operator operand.
    assert_eq!(evaluate("=DATE(2026, 1, 1) + \"1\"").unwrap(), jan2);
    assert_eq!(
        evaluate("=DATE(2026, 1, 1) + NULL").unwrap(),
        evaluate("=DATE(2026, 1, 1)").unwrap()
    );
}

// ---- 1. Coercion in the scalar math builtins -----------------------------------------------

#[test]
fn numeric_text_and_booleans_convert_in_math_builtins() {
    close("=ABS(\"-3\")", 3.0);
    close("=ABS(TRUE)", 1.0);
    close("=SQRT(\"16\")", 4.0);
    close("=INT(\" 3.7 \")", 3.0);
    close("=MOD(\"10\", \"3\")", 1.0);
    close("=POWER(\"2\", 3)", 8.0);
    close("=POWER(2, TRUE)", 2.0);
    close("=ROUND(\"2.567\", \"2\")", 2.57);
    close("=CEILING(\"2.1\")", 3.0);
    close("=FLOOR(\"3.7\")", 3.0);
    close("=FLOOR(\"1e1\", FALSE + 3)", 9.0);
}

#[test]
fn numeric_text_converts_in_financial_and_date_part_arguments() {
    close(
        "=PMT(\"0.05\", \"10\", \"1000\")",
        num("=PMT(0.05, 10, 1000)"),
    );
    close(
        "=FV(0.05, 10, -100, \"0\", FALSE)",
        num("=FV(0.05, 10, -100)"),
    );
    close("=NPV(\"0.1\", 110)", 100.0);
    assert_eq!(
        evaluate("=DATE(\"2026\", \"1\", \"15\")").unwrap(),
        evaluate("=DATE(2026, 1, 15)").unwrap()
    );
    close("=TIME(\"12\", 0, FALSE)", 0.5);
}

#[test]
fn builtins_report_value_error_for_text() {
    assert_eq!(
        err("=ABS(\"abc\")"),
        "ABS: cannot convert \"abc\" to a number (#VALUE!)"
    );
    assert_eq!(
        err("=ROUND(1, \"x\")"),
        "ROUND num_digits: cannot convert \"x\" to a number (#VALUE!)"
    );
    assert_eq!(
        err("=PMT(\"x\", 10, 1000)"),
        "PMT rate: cannot convert \"x\" to a number (#VALUE!)"
    );
    assert_eq!(
        err("=TIME(\"x\", 0, 0)"),
        "TIME hour: cannot convert \"x\" to a number (#VALUE!)"
    );
    // Types that never convert keep their messages.
    assert_eq!(err("=ABS([1])"), "ABS expects number, got array");
    // NULL in a required builtin argument stays an error (only the operators read it as 0).
    assert_eq!(err("=ABS(NULL)"), "ABS expects number, got null");
    assert_eq!(err("=DATE(2026, 1, NULL)"), "DATE expects day as number");
}

#[test]
fn aggregates_still_ignore_text_in_arrays() {
    // Excel audit X-3: SUM/AVERAGE skip text and booleans inside a range.
    close("=SUM([1, \"2\", TRUE, NULL])", 1.0);
    close("=AVERAGE([2, \"4\", 4])", 3.0);
    close("=MAX([1, \"9\"])", 1.0);
}

// ---- 2. Percent vs modulo ------------------------------------------------------------------

#[test]
fn percent_before_an_operator_or_the_end() {
    close("=10%", 0.1);
    close("=10%-3", -2.9);
    close("=10% - 3", -2.9);
    close("=10 % -3", -2.9);
    close("=10% + 1", 1.1);
    close("=10%+1", 1.1);
    close("=50%*2", 1.0);
    close("=50% / 2", 0.25);
    close("=50%^2", 0.25);
    close("=(10%)", 0.1);
    close("=SUM(10%, 20%)", 0.3);
    close("=[10%][0]", 0.1);
    close("=10%; 20%", 0.2);
    assert_eq!(
        evaluate("=10% & \"\"").unwrap(),
        Value::String("0.1".to_string())
    );
    assert_eq!(evaluate("=10% > 0.05").unwrap(), Value::Boolean(true));
    assert_eq!(evaluate("=10% == 0.1").unwrap(), Value::Boolean(true));
    close("=TRUE ? 10% : 2", 0.1);
}

#[test]
fn percent_before_an_operand_is_modulo() {
    close("=10 % 3", 1.0);
    close("=10%3", 1.0);
    close("=10 % (3)", 1.0);
    close("=10 % \"3\"", 1.0);
    close("=10 % ABS(-3)", 1.0);
    close("=10 % 3e0", 1.0);
    let vars = HashMap::from([("b".to_string(), Value::Number(4.0))]);
    assert_eq!(evaluate_with("10 % :b", &vars).unwrap(), Value::Number(2.0));
}

// ---- 3. Scientific notation (BL-058) -------------------------------------------------------

#[test]
fn scientific_notation_is_a_number() {
    close("=1e3", 1000.0);
    close("=1E3", 1000.0);
    close("=2.5e-4", 0.00025);
    close("=1e+2", 100.0);
    close("=1.5E-2", 0.015);
    close("=.5e1", 5.0);
    close("=1e3 + 1", 1001.0);
    close("=-1e3", -1000.0);
    close("=SUM(1e3, 2e3)", 3000.0);
    close("=1e3%", 10.0);
    assert_eq!(num("=1e308"), 1e308);
}

#[test]
fn scientific_overflow_is_num_error() {
    assert_eq!(err("=1e308*10"), "Result is not a finite number (#NUM!)");
    assert_eq!(
        err("=1e308 + 1e308"),
        "Result is not a finite number (#NUM!)"
    );
}

#[test]
fn a_number_running_into_a_letter_is_an_error() {
    // Before BL-058 these silently evaluated to the leading number (`1e3` was 1).
    for src in [
        "=1e", "=1x", "=2ex", "=1e3x", "=1e+", "=1e-x", "=1_000", "=1 + 2E",
    ] {
        assert_eq!(err(src), "Invalid number literal", "{src}");
    }
    // A method call on a number still works; the dot is not part of the exponent.
    close("=1.5.round()", 2.0);
    close("=(1e1).abs()", 10.0);
}
