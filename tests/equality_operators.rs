//! ADR-0023 (BL-066, BL-059): `==`/`!=` compare text case-insensitively, `===`/`!==` are
//! strict; no comparison converts between types; DateTimes compare and order by timestamp.

use skillet::{evaluate, evaluate_with_json, parse, Value};

fn b(src: &str) -> bool {
    match evaluate(src) {
        Ok(Value::Boolean(b)) => b,
        other => panic!("{src}: expected a boolean, got {other:?}"),
    }
}

fn all(cases: &[(&str, bool)]) {
    for (src, want) in cases {
        assert_eq!(b(src), *want, "{src}");
    }
}

fn is_error(src: &str) {
    assert!(evaluate(src).is_err(), "{src}: expected an error");
}

#[test]
fn text_equality_ignores_case_under_fuzzy_only() {
    all(&[
        (r#""a" == "A""#, true),
        (r#""Hello" == "hELLO""#, true),
        (r#""a" != "A""#, false),
        (r#""a" == "b""#, false),
        (r#""ÉCOLE" == "école""#, true),
        (r#""a" = "A""#, true),
        (r#""a" === "A""#, false),
        (r#""a" !== "A""#, true),
        (r#""a" === "a""#, true),
        (r#""a" !== "a""#, false),
        (r#""ab" == "abc""#, false),
    ]);
}

#[test]
fn no_cross_type_conversion_under_either_operator() {
    for op in ["==", "==="] {
        for (l, r) in [
            ("1", r#""1""#),
            ("TRUE", "1"),
            ("FALSE", "0"),
            ("TRUE", r#""TRUE""#),
            ("0", r#""""#),
            ("1", "1::Currency"),
            ("[1]", "1"),
        ] {
            assert!(!b(&format!("{l} {op} {r}")), "{l} {op} {r}");
            assert!(!b(&format!("{r} {op} {l}")), "{r} {op} {l}");
        }
    }
    all(&[
        (r#"1 != "1""#, true),
        (r#"1 !== "1""#, true),
        ("TRUE != 1", true),
        ("TRUE !== 1", true),
    ]);
}

#[test]
fn numbers_booleans_and_currency_compare_within_their_type() {
    all(&[
        ("1 == 1.0", true),
        ("1 === 1.0", true),
        ("0.1 + 0.2 == 0.3", false),
        ("TRUE == TRUE", true),
        ("TRUE === FALSE", false),
        ("1::Currency == 1::Currency", true),
        ("1::Currency === 1::Currency", true),
        ("1::Currency == 2::Currency", false),
    ]);
}

#[test]
fn null_equals_only_null() {
    all(&[
        ("NULL == NULL", true),
        ("NULL === NULL", true),
        ("NULL != NULL", false),
        ("NULL !== NULL", false),
        ("NULL == 0", false),
        (r#"NULL === """#, false),
        ("NULL !== FALSE", true),
    ]);
}

#[test]
fn arrays_compare_element_wise() {
    all(&[
        ("[1, 2] == [1, 2]", true),
        ("[1, 2] === [1, 2]", true),
        ("[1, 2] == [1, 2, 3]", false),
        (r#"["a", "B"] == ["A", "b"]"#, true),
        (r#"["a", "B"] === ["A", "b"]"#, false),
        (r#"[1, "2"] == [1, 2]"#, false),
        ("[[1, NULL]] == [[1, NULL]]", true),
        ("[] == []", true),
    ]);
}

#[test]
fn json_objects_compare_element_wise() {
    let r = |src: &str| evaluate_with_json(src, r#"{"u": {"name": "Ann", "tags": ["x"]}}"#);
    let t = Value::Boolean(true);
    let f = Value::Boolean(false);
    assert_eq!(r(r#":u == {"name": "ann", "tags": ["X"]}"#).unwrap(), t);
    assert_eq!(r(r#":u === {"name": "Ann", "tags": ["x"]}"#).unwrap(), t);
    assert_eq!(r(r#":u === {"name": "ann", "tags": ["x"]}"#).unwrap(), f);
    assert_eq!(r(r#":u == {"name": "Ann"}"#).unwrap(), f);
    assert_eq!(r(r#":u == {"Name": "Ann", "tags": ["x"]}"#).unwrap(), f);
    all(&[
        (r#"{"a": 1, "b": 2} == {"b": 2, "a": 1}"#, true),
        (r#"{"a": 1} === {"a": 1.0}"#, true),
        (r#"{"a": 1} == {"a": "1"}"#, false),
    ]);
}

#[test]
fn datetime_equality_compares_timestamps() {
    all(&[
        ("DATE(2026, 1, 1) == DATE(2026, 1, 1)", true),
        ("DATE(2026, 1, 1) === DATE(2026, 1, 1)", true),
        ("DATE(2026, 1, 1) != DATE(2026, 1, 2)", true),
        ("DATE(2026, 1, 1) !== DATE(2026, 1, 1)", false),
        ("DATE(2026, 1, 1) + 1 == DATE(2026, 1, 2)", true),
        ("NOW() == NOW()", true),
        // A DateTime equals only a DateTime.
        ("DATE(2026, 1, 1) == 0", false),
        ("DATE(2026, 1, 1) === 0", false),
        (r#"DATE(2026, 1, 1) == "2026-01-01""#, false),
        ("DATE(2026, 1, 1) != NULL", true),
    ]);
}

#[test]
fn datetime_ordering() {
    all(&[
        ("DATE(2026, 1, 1) < DATE(2026, 1, 2)", true),
        ("DATE(2026, 1, 2) > DATE(2026, 1, 1)", true),
        ("DATE(2026, 1, 1) <= DATE(2026, 1, 1)", true),
        ("DATE(2026, 1, 1) >= DATE(2026, 1, 2)", false),
        ("DATE(2025, 12, 31) < DATE(2026, 1, 1)", true),
    ]);
    is_error("DATE(2026, 1, 1) < 1");
    is_error("1 > DATE(2026, 1, 1)");
    is_error(r#"DATE(2026, 1, 1) <= "2026-01-02""#);
    is_error("DATE(2026, 1, 1) >= NULL");
}

#[test]
fn text_ordering_ignores_case() {
    all(&[
        (r#""apple" < "Banana""#, true),
        (r#""B" > "a""#, true),
        (r#""a" < "A""#, false),
        (r#""a" <= "A""#, true),
        (r#""A" >= "a""#, true),
        (r#""abc" < "ABD""#, true),
        (r#""ab" < "ABC""#, true),
        (r#""10" < "9""#, true), // text, not numbers
    ]);
}

#[test]
fn ordering_needs_two_values_of_the_same_type() {
    all(&[
        ("FALSE < TRUE", true),
        ("TRUE > FALSE", true),
        ("TRUE >= TRUE", true),
        ("1::Currency < 2::Currency", true),
    ]);
    for src in [
        r#"1 < "2""#,
        "TRUE > 0",
        "1 < 2::Currency",
        "[1] < [2]",
        r#""a" > NULL"#,
    ] {
        is_error(src);
    }
}

#[test]
fn strict_operators_share_the_precedence_of_equality() {
    all(&[
        ("1 + 1 === 2", true),
        ("1 + 1 !== 2", false),
        ("2 * 3 === 6 AND 1 < 2", true),
        ("1 < 2 === TRUE", true),
        ("1 === 1 == TRUE", true),
        (r#""a" & "b" === "ab""#, true),
    ]);
    assert_eq!(
        evaluate("TRUE ? 1 === 1 : 2").unwrap(),
        Value::Boolean(true)
    );
}

#[test]
fn lexer_keeps_two_char_equality_and_rejects_four() {
    assert!(parse("1 == 1").is_ok());
    assert!(parse("1 = 1").is_ok());
    assert!(parse("1 === 1").is_ok());
    assert!(parse("1 !== 1").is_ok());
    assert!(parse("1 ==== 1").is_err());
    assert!(parse("1 !=== 1").is_err());
    assert!(b("!(1 === 2)"));
    assert!(b("=1===1"));
}

#[test]
fn lambdas_and_filter_use_the_new_equality() {
    let v = |src: &str| evaluate(src).unwrap();
    let s = |x: &str| Value::String(x.to_string());
    assert_eq!(
        v(r#"FILTER(["Apple", "apple", "pear"], :x == "APPLE")"#),
        Value::Array(vec![s("Apple"), s("apple")])
    );
    assert_eq!(
        v(r#"FILTER(["Apple", "apple", "pear"], :x === "apple")"#),
        Value::Array(vec![s("apple")])
    );
    assert_eq!(
        v(r#"[1, "1", TRUE].filter(:x == 1)"#),
        Value::Array(vec![Value::Number(1.0)])
    );
    assert_eq!(
        v("[1, 2, 3, 4].filter(:x % 2 == 0)"),
        Value::Array(vec![Value::Number(2.0), Value::Number(4.0)])
    );
    assert_eq!(
        v(r#"["a", "B"].map(:x == "b")"#),
        Value::Array(vec![Value::Boolean(false), Value::Boolean(true)])
    );
    // Methods keep their own semantics: `.includes` stays case-sensitive.
    assert_eq!(v(r#"["a"].includes("A")"#), Value::Boolean(false));
}
