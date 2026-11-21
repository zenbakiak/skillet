//! D-8 / X-8 (BL-054): `IF` and `IFS` evaluate only the branch they pick, as in Excel.

use skillet::{evaluate, evaluate_with, Value};
use std::collections::HashMap;

fn ok(src: &str) -> Value {
    evaluate(src).unwrap_or_else(|e| panic!("{src}: {}", e.message))
}

#[test]
fn if_skips_the_branch_it_does_not_pick() {
    assert_eq!(ok("IF(TRUE, 1, UPPER(1))"), Value::Number(1.0));
    assert_eq!(ok("IF(FALSE, FIRST([]), 0)"), Value::Number(0.0));
    assert_eq!(ok("IF(1, 'a', LAST([]))"), Value::String("a".into()));
    assert_eq!(ok("IF(0, UPPER(1))"), Value::Boolean(false));
}

#[test]
fn if_still_evaluates_the_chosen_branch_and_condition() {
    assert!(evaluate("IF(FALSE, 1, UPPER(1))").is_err());
    assert!(evaluate("IF(TRUE, UPPER(1), 1)").is_err());
    assert!(evaluate("IF(UPPER(1), 1, 2)").is_err());
}

#[test]
fn guard_against_missing_or_zero() {
    let mut vars = HashMap::new();
    vars.insert("a".to_string(), Value::Number(0.0));
    vars.insert("items".to_string(), Value::Array(vec![]));
    assert_eq!(
        evaluate_with("IF(:a == 0, 0, 1 / :a)", &vars).unwrap(),
        Value::Number(0.0)
    );
    assert_eq!(
        evaluate_with("IF(:items.length() > 0, FIRST(:items), 'none')", &vars).unwrap(),
        Value::String("none".into())
    );
    // A missing variable in the branch not taken is not an error.
    assert_eq!(
        evaluate_with("IF(TRUE, 1, :undefined)", &vars).unwrap(),
        Value::Number(1.0)
    );
}

#[test]
fn ifs_stops_at_the_first_true_condition() {
    assert_eq!(
        ok("IFS(FALSE, UPPER(1), TRUE, 2, UPPER(1), 3)"),
        Value::Number(2.0)
    );
    assert_eq!(ok("IFS(TRUE, 1, FALSE, FIRST([]))"), Value::Number(1.0));
    assert_eq!(ok("IFS(FALSE, 1)"), Value::Boolean(false));
    assert!(evaluate("IFS(FALSE, 1, TRUE, UPPER(1))").is_err());
    assert!(evaluate("IFS(TRUE)").is_err());
}

#[test]
fn eager_results_are_unchanged() {
    assert_eq!(ok("IF(TRUE, 1, 2)"), Value::Number(1.0));
    assert_eq!(ok("IF(FALSE, 1)"), Value::Boolean(false));
    assert_eq!(ok("IF('x', 'a', 'b')"), Value::String("b".into()));
    assert_eq!(ok("IF(NULL, 'a', 'b')"), Value::String("b".into()));
    assert_eq!(ok("IFS(FALSE, 1, TRUE, 2)"), Value::Number(2.0));
    // A spread argument takes the eager path, with the same result.
    assert_eq!(ok("IF(...[TRUE, 'y', 'n'])"), Value::String("y".into()));
    assert_eq!(ok("IFS(...[FALSE, 1, 1, 2])"), Value::Number(2.0));
}

#[test]
fn branches_inside_lambdas() {
    assert_eq!(
        ok("[0, 2, 4].map(IF(:x == 0, 0, 8 / :x))"),
        Value::Array(vec![
            Value::Number(0.0),
            Value::Number(4.0),
            Value::Number(2.0)
        ])
    );
    assert_eq!(
        ok("MAP([[], [7]], IF(:x.length() == 0, NULL, FIRST(:x)))"),
        Value::Array(vec![Value::Null, Value::Number(7.0)])
    );
}
