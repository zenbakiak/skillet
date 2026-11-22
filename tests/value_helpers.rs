//! BL-104: the additive `Value` helpers of ADR-0030 §3 (0.7.0): `from_json_text`,
//! `to_json_text`, `as_str`, `as_array`, the `From` conversions and `Serialize`.

use skillet::{evaluate, evaluate_with_json, Value};

/// `text` parsed, printed back, and serialized: `want` each time, and stable on a second trip.
fn round_trip(text: &str, want: &str) {
    let v = Value::from_json_text(text).unwrap_or_else(|e| panic!("{text}: {e}"));
    let out = v.to_json_text().unwrap();
    assert_eq!(out, want, "to_json_text of {text}");
    assert_eq!(
        serde_json::to_string(&v).unwrap(),
        want,
        "Serialize of {text}"
    );
    let again = Value::from_json_text(&out).unwrap();
    assert_eq!(again, v, "value after a second trip of {text}");
    assert_eq!(again.to_json_text().unwrap(), want, "second trip of {text}");
}

#[test]
fn scalars_round_trip() {
    for (text, want) in [
        ("null", "null"),
        ("true", "true"),
        ("false", "false"),
        ("0", "0"),
        ("5", "5"),
        ("-42", "-42"),
        ("5.0", "5"),
        ("1.5", "1.5"),
        ("0.1", "0.1"),
        ("1e-7", "1e-7"),
        ("1e21", "1e+21"),
        ("-0.0", "-0.0"),
        (r#""""#, r#""""#),
        (r#""héllo €""#, r#""héllo €""#),
        (r#""a\"b\\c\n\t\u0001""#, r#""a\"b\\c\n\t\u0001""#),
        (r#""é""#, r#""é""#),
    ] {
        round_trip(text, want);
    }
}

#[test]
fn nested_arrays_and_objects_round_trip() {
    for (text, want) in [
        ("[]", "[]"),
        ("[1, [2, [3, []]], null]", "[1,[2,[3,[]]],null]"),
        ("{}", "{}"),
        (r#"{"b": 2, "a": 1}"#, r#"{"a":1,"b":2}"#),
        (
            r#"{"z": {"y": [1, {"x": "deep"}]}, "a": [true, null, 1.5]}"#,
            r#"{"a":[true,null,1.5],"z":{"y":[1,{"x":"deep"}]}}"#,
        ),
        (
            r#"[{"b": [1, 2]}, [{"a": {}}], "s"]"#,
            r#"[{"b":[1,2]},[{"a":{}}],"s"]"#,
        ),
    ] {
        round_trip(text, want);
    }
}

#[test]
#[allow(deprecated)] // objects are `Value::Json` until 0.8 (ADR-0030)
fn objects_are_json_and_arrays_are_values() {
    let v = Value::from_json_text(r#"[1, "a", {"k": 2}]"#).unwrap();
    assert_eq!(
        v,
        Value::Array(vec![
            Value::Number(1.0),
            Value::String("a".to_string()),
            Value::Json(r#"{"k":2}"#.to_string()),
        ])
    );
}

/// ADR-0030 decision b arrives in 0.8.0: today a bare number is an f64 (rounded beyond 2^53),
/// while a number inside an object keeps its digits because the object is held as text.
#[test]
fn numbers_beyond_2_pow_53() {
    let big = Value::from_json_text("9007199254740993").unwrap();
    assert_eq!(big, Value::Number(9_007_199_254_740_992.0));
    round_trip("9007199254740993", "9007199254740992.0");
    round_trip("[9007199254740993]", "[9007199254740992.0]");
    round_trip(r#"{"id": 9007199254740993}"#, r#"{"id":9007199254740993}"#);
    round_trip(
        r#"{"id": 18446744073709551615}"#,
        r#"{"id":18446744073709551615}"#,
    );
    // The last integer that prints as digits, and the first that does not.
    round_trip("9007199254740991", "9007199254740991");
    round_trip("9007199254740992", "9007199254740992.0");
}

/// A Currency built in Rust prints as the Number of the same amount (ADR-0033), and reads back
/// as that Number.
#[test]
#[allow(deprecated)] // `Value::Currency` is deprecated (ADR-0033)
fn currency_text() {
    for (n, want) in [(5.0, "5"), (5.25, "5.25"), (-0.1, "-0.1")] {
        let v = Value::Currency(n);
        assert_eq!(v.to_json_text().unwrap(), want);
        assert_eq!(serde_json::to_string(&v).unwrap(), want);
        assert_eq!(Value::from_json_text(want).unwrap(), Value::Number(n));
    }
    let arr = Value::Array(vec![Value::Currency(5.0), Value::Currency(5.25)]);
    assert_eq!(arr.to_json_text().unwrap(), "[5,5.25]");
    assert_eq!(serde_json::to_string(&arr).unwrap(), "[5,5.25]");
}

/// A DateTime is its text in quotes (ADR-0034); it reads back as that String.
#[test]
fn datetime_text() {
    let dt = evaluate("DATE(2024, 1, 15)").unwrap();
    let want = r#""2024-01-15T00:00:00Z""#;
    assert_eq!(dt.to_json_text().unwrap(), want);
    assert_eq!(serde_json::to_string(&dt).unwrap(), want);
    let back = Value::from_json_text(want).unwrap();
    assert_eq!(back, Value::from("2024-01-15T00:00:00Z"));
    assert_eq!(back.to_json_text().unwrap(), want);

    let arr = Value::from(vec![dt, Value::Null]);
    assert_eq!(
        arr.to_json_text().unwrap(),
        r#"["2024-01-15T00:00:00Z",null]"#
    );
    assert_eq!(
        serde_json::to_string(&arr).unwrap(),
        arr.to_json_text().unwrap()
    );
}

#[test]
fn invalid_json_is_an_error() {
    for text in ["", "{", "[1,", "abc", "{'a': 1}", "[1] [2]", "NaN", "01"] {
        let err = Value::from_json_text(text).unwrap_err();
        assert!(
            err.message.starts_with("Invalid JSON: "),
            "{text:?}: {}",
            err.message
        );
    }
}

/// `from_json_text` reads a value exactly as `evaluate_with_json` reads a variable.
#[test]
fn reads_like_the_json_entry_points() {
    for text in [
        "null",
        "1.5",
        r#""s""#,
        "[1, [2, null]]",
        r#"{"b": {"c": [1, 2.5]}, "a": 9007199254740993}"#,
    ] {
        let via_vars = evaluate_with_json(":v", &format!(r#"{{"v": {text}}}"#)).unwrap();
        assert_eq!(Value::from_json_text(text).unwrap(), via_vars, "{text}");
    }
}

/// `to_json_text` is the text `.to_json()` returns (for every value but NULL, whose `.to_json()`
/// is `{}` by documented method behaviour).
#[test]
#[allow(deprecated)] // `.to_json()` returns `Value::Json` until 0.8 (ADR-0030)
fn matches_the_to_json_method() {
    for text in [
        "true",
        "5",
        "1.5",
        "1e21",
        r#""a\"b""#,
        "[1, [2.5, null], \"x\"]",
        r#"{"b": [1, {"c": 2}], "a": "x"}"#,
    ] {
        let vars = format!(r#"{{"v": {text}}}"#);
        let Value::Json(method) = evaluate_with_json(":v.to_json()", &vars).unwrap() else {
            panic!("{text}: .to_json() is not Json");
        };
        let v = Value::from_json_text(text).unwrap();
        assert_eq!(v.to_json_text().unwrap(), method, "{text}");
    }
    assert_eq!(Value::Null.to_json_text().unwrap(), "null");
}

#[test]
#[allow(deprecated)] // building a `Value::Json` with invalid text (ADR-0030)
fn values_with_no_json_text_are_errors() {
    for v in [
        Value::from(f64::NAN),
        Value::from(f64::INFINITY),
        Value::from(vec![Value::from(f64::NEG_INFINITY)]),
        Value::Json("not json".to_string()),
    ] {
        assert!(v.to_json_text().is_err(), "{v:?}");
        assert!(serde_json::to_string(&v).is_err(), "{v:?}");
    }
}

#[test]
fn serialize_writes_into_a_larger_document() {
    let v = Value::from_json_text(r#"[1, {"a": [2]}]"#).unwrap();
    let doc = serde_json::json!({ "result": v, "ok": true });
    assert_eq!(doc.to_string(), r#"{"ok":true,"result":[1,{"a":[2]}]}"#);
}

#[test]
fn accessors() {
    assert_eq!(Value::from("x").as_str(), Some("x"));
    assert_eq!(Value::from(String::from("y")).as_str(), Some("y"));
    assert_eq!(Value::from(1.0).as_str(), None);
    assert_eq!(Value::from_json_text(r#"{"a":1}"#).unwrap().as_str(), None);

    let arr = evaluate("[1, 'a']").unwrap();
    assert_eq!(
        arr.as_array(),
        Some(&[Value::from(1.0), Value::from("a")][..])
    );
    assert_eq!(Value::from(vec![]).as_array(), Some(&[][..]));
    assert_eq!(Value::from("[1]").as_array(), None);
    assert_eq!(Value::Null.as_array(), None);
}

#[test]
fn from_conversions() {
    assert_eq!(Value::from("a"), Value::String("a".to_string()));
    assert_eq!(
        Value::from(String::from("a")),
        Value::String("a".to_string())
    );
    assert_eq!(Value::from(2.5), Value::Number(2.5));
    assert_eq!(Value::from(false), Value::Boolean(false));
    assert_eq!(
        Value::from(vec![Value::from(1.0), Value::from(true)]),
        Value::Array(vec![Value::Number(1.0), Value::Boolean(true)])
    );
    let into: Value = "b".into();
    assert_eq!(into, Value::from("b"));
}
