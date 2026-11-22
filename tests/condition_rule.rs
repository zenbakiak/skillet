//! BL-089, ADR-0029 amendment 1: one condition rule (`logical::condition`) for `IF`, `IFS`,
//! `? :`, the `AND`/`OR`/`&&`/`||` operators, `NOT`/`!`, `filter`/`find`/`FILTER`/`FIND` and
//! lambda criteria. The `AND()`/`OR()`/`XOR()` builtins keep skipping NULL (A4).
//!
//! The matrix tests walk the columns of the amendment's "Before and after" table.

use skillet::{evaluate, evaluate_with, evaluate_with_json, Value};
use std::collections::HashMap;

/// How a column reads as a condition under the A1 rule.
#[derive(Clone, Copy)]
enum Rule {
    T,
    F,
    /// Not a condition: `#VALUE!`, with the type name in the message.
    V(&'static str),
}
use Rule::{F, T, V};

/// The columns of the matrix and their A1 reading.
const COLUMNS: &[(&str, Rule)] = &[
    ("TRUE", T),
    ("FALSE", F),
    ("NULL", F),
    ("0", F),
    ("1", T),
    ("2.5", T),
    ("\"a\"", V("string")),
    ("\"\"", V("string")),
    ("2::Currency", T),
    ("0::Currency", F),
    ("DATE(2026,1,1)", V("datetime")),
    ("[1]", V("array")),
];

fn eval(src: &str) -> Result<Value, String> {
    evaluate(src).map_err(|e| e.message)
}

fn value(src: &str) -> Value {
    evaluate(src).unwrap_or_else(|e| panic!("{src}: {}", e.message))
}

fn condition_error(name: &str, ty: &str) -> String {
    format!("{name} condition must be TRUE, FALSE or a number, got {ty} (#VALUE!)")
}

fn not_error(ty: &str) -> String {
    format!("NOT expects a logical value, got {ty} (#VALUE!)")
}

/// For every column, `form` (with `c` replaced by the column) evaluates to the expression
/// `on_true`/`on_false` picks, or to the error `on_error` builds.
fn row(
    form: &str,
    on_true: impl Fn(&str) -> String,
    on_false: impl Fn(&str) -> String,
    on_error: impl Fn(&str) -> String,
) {
    for &(c, rule) in COLUMNS {
        let src = form.replace('c', &format!("({c})"));
        let want = match rule {
            T => Ok(value(&on_true(c))),
            F => Ok(value(&on_false(c))),
            V(ty) => Err(on_error(ty)),
        };
        assert_eq!(eval(&src), want, "{src}");
    }
}

fn fixed(s: &'static str) -> impl Fn(&str) -> String {
    move |_| s.to_string()
}

#[test]
fn if_and_ifs_keep_the_rule() {
    row("IF(c, 'T', 'F')", fixed("'T'"), fixed("'F'"), |t| {
        condition_error("IF", t)
    });
    row("IFS(c, 'T', TRUE, 'F')", fixed("'T'"), fixed("'F'"), |t| {
        condition_error("IFS", t)
    });
}

#[test]
fn ternary_uses_the_rule() {
    row("c ? 'T' : 'F'", fixed("'T'"), fixed("'F'"), |t| {
        condition_error("Ternary", t)
    });
}

#[test]
fn and_operators_use_the_rule() {
    for form in ["c AND TRUE", "TRUE AND c", "c && TRUE", "TRUE && c"] {
        row(form, fixed("TRUE"), fixed("FALSE"), |t| {
            condition_error("AND", t)
        });
    }
    row("c AND FALSE", fixed("FALSE"), fixed("FALSE"), |t| {
        condition_error("AND", t)
    });
    // The right side is never reached.
    for &(c, _) in COLUMNS {
        assert_eq!(
            value(&format!("FALSE AND ({c})")),
            Value::Boolean(false),
            "{c}"
        );
    }
}

#[test]
fn or_operators_use_the_rule() {
    for form in ["c OR FALSE", "FALSE OR c", "c || FALSE", "FALSE || c"] {
        row(form, fixed("TRUE"), fixed("FALSE"), |t| {
            condition_error("OR", t)
        });
    }
    row("c OR TRUE", fixed("TRUE"), fixed("TRUE"), |t| {
        condition_error("OR", t)
    });
    for &(c, _) in COLUMNS {
        assert_eq!(
            value(&format!("TRUE OR ({c})")),
            Value::Boolean(true),
            "{c}"
        );
    }
}

#[test]
fn bang_is_not() {
    for form in ["!c", "NOT c", "NOT(c)"] {
        row(form, fixed("FALSE"), fixed("TRUE"), not_error);
    }
}

#[test]
fn filter_and_find_use_the_rule() {
    row(
        "[c].filter(:x)",
        |c| format!("[{c}]"),
        fixed("[]"),
        |t| condition_error("filter", t),
    );
    row(
        "FILTER([c], :x)",
        |c| format!("[{c}]"),
        fixed("[]"),
        |t| condition_error("FILTER", t),
    );
    row(
        "[c].find(:x)",
        |c| c.to_string(),
        fixed("NULL"),
        |t| condition_error("find", t),
    );
    row(
        "FIND([c], :x)",
        |c| c.to_string(),
        fixed("NULL"),
        |t| condition_error("FIND", t),
    );
}

#[test]
fn lambda_criteria_use_the_rule() {
    row("COUNTIF([c], :x)", fixed("1"), fixed("0"), |t| {
        condition_error("COUNTIF", t)
    });
    row("SUMIF([c], :x, [10])", fixed("10"), fixed("0"), |t| {
        condition_error("SUMIF", t)
    });
    for form in ["AVERAGEIF([c], :x, [10])", "AVGIF([c], :x, [10])"] {
        for &(c, rule) in COLUMNS {
            let src = form.replace('c', &format!("({c})"));
            let want = match rule {
                T => Ok(Value::Number(10.0)),
                F => Err("AVERAGEIF: no matching numbers (#DIV/0!)".to_string()),
                V(ty) => Err(condition_error("AVERAGEIF", ty)),
            };
            assert_eq!(eval(&src), want, "{src}");
        }
    }
}

#[test]
fn builtins_keep_skipping_null() {
    // A4: unchanged. `T F T F T T #V #V T F #V T` for AND(c, TRUE).
    let and = [true, false, true, false, true, true];
    let or = [true, false, false, false, true, true];
    for (i, &(c, rule)) in COLUMNS.iter().enumerate() {
        let a = eval(&format!("AND({c}, TRUE)"));
        let o = eval(&format!("OR({c}, FALSE)"));
        let x = eval(&format!("XOR({c})"));
        match (i, rule) {
            (11, _) => {
                // An array is flattened.
                assert_eq!(a, Ok(Value::Boolean(true)), "{c}");
                assert_eq!(o, Ok(Value::Boolean(true)), "{c}");
                assert_eq!(x, Ok(Value::Boolean(true)), "{c}");
            }
            (_, V(_)) => {
                for r in [a, o, x] {
                    assert!(r.unwrap_err().contains("#VALUE!"), "{c}");
                }
            }
            (8, _) | (9, _) => {
                let want = i == 8;
                assert_eq!(a, Ok(Value::Boolean(want)), "{c}");
                assert_eq!(o, Ok(Value::Boolean(want)), "{c}");
                assert_eq!(x, Ok(Value::Boolean(want)), "{c}");
            }
            _ => {
                assert_eq!(a, Ok(Value::Boolean(and[i])), "{c}");
                assert_eq!(o, Ok(Value::Boolean(or[i])), "{c}");
                if c == "NULL" {
                    assert!(x.unwrap_err().contains("#VALUE!"));
                } else {
                    assert_eq!(x, Ok(Value::Boolean(or[i])), "{c}");
                }
            }
        }
    }
    assert_eq!(value("AND(TRUE, NULL)"), Value::Boolean(true));
}

#[test]
fn operators_do_not_depend_on_operand_order() {
    let valid: Vec<&str> = COLUMNS
        .iter()
        .filter(|(_, r)| !matches!(r, V(_)))
        .map(|(c, _)| *c)
        .collect();
    for a in &valid {
        for b in &valid {
            for op in ["AND", "OR", "&&", "||"] {
                let ab = value(&format!("({a}) {op} ({b})"));
                let ba = value(&format!("({b}) {op} ({a})"));
                assert_eq!(ab, ba, "{a} {op} {b}");
                assert!(matches!(ab, Value::Boolean(_)), "{a} {op} {b}: {ab:?}");
            }
        }
    }
}

#[test]
fn other_changed_results() {
    let b = |src: &str, want: bool| assert_eq!(value(src), Value::Boolean(want), "{src}");
    b("NULL AND FALSE", false);
    b("TRUE AND NULL", false);
    b("NULL OR TRUE", true);
    b("1 AND 2", true);
    b("0 || NULL", false);
    b("!0 == FALSE", false);
    b("!NULL", true);
    assert_eq!(eval("\"a\" AND 1/0"), Err(condition_error("AND", "string")));
    assert_eq!(value("1 ? 2 : 3"), Value::Number(2.0));
    assert_eq!(value("NULL ? 1 : 2"), Value::Number(2.0));
    assert_eq!(value("[0, 1, 2].filter(:x)"), value("[1, 2]"));
    assert_eq!(value("FILTER([0, 1, 2], :x)"), value("[1, 2]"));
    assert_eq!(
        eval("[1, 2].filter(\"yes\")"),
        Err(condition_error("filter", "string"))
    );
    assert_eq!(value("[2, \"a\"].find(:x)"), Value::Number(2.0));
    assert_eq!(
        eval("[\"a\", 2].find(:x)"),
        Err(condition_error("find", "string"))
    );
    assert_eq!(value("COUNTIF([1, 2, 3], :x)"), Value::Number(3.0));
    assert_eq!(
        value("SUMIF([0, 1, 2], :x, [10, 20, 30])"),
        Value::Number(50.0)
    );
}

#[test]
fn a_missing_flag_is_false_in_the_operators_but_skipped_by_and() {
    let vars = r#"{"u": {"name": "ann"}, "on": true}"#;
    let ev = |src: &str| evaluate_with_json(src, vars).map_err(|e| e.message);
    assert_eq!(ev(":u&.is_admin AND :on"), Ok(Value::Boolean(false)));
    assert_eq!(ev(":u&.is_admin ? :on : FALSE"), Ok(Value::Boolean(false)));
    assert_eq!(ev("!:u&.is_admin"), Ok(Value::Boolean(true)));
    assert_eq!(
        ev("IF(:u&.is_admin, :on, FALSE)"),
        Ok(Value::Boolean(false))
    );
    // A4, the warning in the reference.
    assert_eq!(ev("AND(:u&.is_admin, :on)"), Ok(Value::Boolean(true)));
    assert_eq!(
        ev("AND(:u&.is_admin == TRUE, :on)"),
        Ok(Value::Boolean(false))
    );
}

#[test]
fn errors_inside_a_lambda_predicate_are_the_result() {
    // A6: never dropped, no partial array.
    for src in [
        "[1, 0].filter(10 / :x > 1)",
        "FILTER([1, 0], 10 / :x > 1)",
        "[1, 0].find(10 / :x > 100)",
        "COUNTIF([1, 0], 10 / :x > 1)",
    ] {
        assert!(eval(src).unwrap_err().contains("#DIV/0!"), "{src}");
    }
    assert_eq!(
        eval("[1, \"a\", 2].filter(:x)"),
        Err(condition_error("filter", "string"))
    );
    assert_eq!(value("[NULL, TRUE].filter(:x)"), value("[TRUE]"));
}

#[test]
fn if_takes_at_most_three_arguments() {
    let arity = Err("IF expects at most 3 arguments".to_string());
    assert_eq!(eval("IF(1, 2, 3, 4)"), arity);
    assert_eq!(eval("IF(0, 2, 3, 1/0)"), arity); // before any argument is evaluated
    assert_eq!(eval("IF(1/0, 2, 3, 4)"), arity);
    assert_eq!(eval("IF(...[TRUE, 1, 2, 3])"), arity); // eager path, after spreading
    assert_eq!(
        eval("IF(TRUE)"),
        Err("IF expects at least 2 arguments".to_string())
    );
    assert_eq!(value("IF(...[TRUE, 1, 2])"), Value::Number(1.0));
    assert_eq!(value("IF(FALSE, 1)"), Value::Boolean(false));
}

#[test]
fn ifs_needs_at_least_one_pair() {
    let pairs = Err("IFS expects pairs of condition,value arguments".to_string());
    assert_eq!(eval("IFS()"), pairs);
    assert_eq!(eval("IFS(...[])"), pairs);
    assert_eq!(eval("IFS(TRUE)"), pairs);
    assert_eq!(
        eval("IFS(FALSE, 1)"),
        Err("IFS: no condition is TRUE (#N/A)".to_string())
    );
}

#[test]
fn unchanged_results() {
    let b = |src: &str, want: bool| assert_eq!(value(src), Value::Boolean(want), "{src}");
    b("FALSE AND 1/0", false);
    b("TRUE OR 1/0", true);
    b("NOT TRUE AND NULL", false);
    b("FALSE AND \"a\"", false);
    assert!(eval("TRUE AND 1/0").unwrap_err().contains("#DIV/0!"));
    assert!(eval("FALSE OR 1/0").unwrap_err().contains("#DIV/0!"));
    assert_eq!(
        eval("\"a\" AND FALSE"),
        Err(condition_error("AND", "string"))
    );
    assert_eq!(value("TRUE ? 1 : 1/0"), Value::Number(1.0));
    assert_eq!(value("FALSE ? 1/0 : 2"), Value::Number(2.0));
    // `!` is still a tight unary (A5): `!:x > 1` is `(!:x) > 1`, a comparison error.
    let vars = HashMap::from([("x".to_string(), Value::Number(5.0))]);
    assert!(evaluate_with("!:x > 1", &vars).is_err());
    b("NOT 0 == FALSE", true);
    // Excel criteria keep their own rules.
    assert_eq!(value("COUNTIF([1, 2, 3], \">1\")"), Value::Number(2.0));
}

// --- NaN (A1, A4) ---------------------------------------------------------------------------

fn with_nan(src: &str, nan: Value) -> Result<Value, String> {
    let vars = HashMap::from([("n".to_string(), nan)]);
    evaluate_with(src, &vars).map_err(|e| e.message)
}

fn nan_error(name: &str) -> Result<Value, String> {
    Err(format!("{name} condition is not a number (#NUM!)"))
}

#[test]
fn nan_is_a_num_error() {
    for nan in [Value::Number(f64::NAN), Value::Currency(f64::NAN)] {
        for (src, name) in [
            ("IF(:n, 'T', 'F')", "IF"),
            ("IF(...[:n, 'T', 'F'])", "IF"),
            ("IFS(:n, 'T', TRUE, 'F')", "IFS"),
            ("NOT :n", "NOT"),
            ("NOT(:n)", "NOT"),
            ("!(:n)", "NOT"),
            (":n ? 'T' : 'F'", "Ternary"),
            (":n AND TRUE", "AND"),
            ("TRUE AND :n", "AND"),
            (":n AND FALSE", "AND"),
            (":n && TRUE", "AND"),
            (":n OR FALSE", "OR"),
            ("FALSE || :n", "OR"),
            ("[:n].filter(:x)", "filter"),
            ("FILTER([:n], :x)", "FILTER"),
            ("[:n].find(:x)", "find"),
            ("FIND([:n], :x)", "FIND"),
            ("COUNTIF([:n], :x)", "COUNTIF"),
            ("SUMIF([:n], :x, [10])", "SUMIF"),
            ("AVERAGEIF([:n], :x, [10])", "AVERAGEIF"),
            ("AND(:n, TRUE)", "AND"),
            ("AND([:n])", "AND"),
            ("AND([[TRUE, :n]])", "AND"),
            ("OR(:n, FALSE)", "OR"),
            ("XOR(:n)", "XOR"),
            ("XOR([:n])", "XOR"),
        ] {
            assert_eq!(with_nan(src, nan.clone()), nan_error(name), "{src} {nan:?}");
        }
        // The right side is not evaluated.
        assert_eq!(
            with_nan("FALSE AND :n", nan.clone()),
            Ok(Value::Boolean(false))
        );
        assert_eq!(
            with_nan("TRUE OR :n", nan.clone()),
            Ok(Value::Boolean(true))
        );
    }
}
