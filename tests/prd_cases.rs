use skillet::{evaluate, evaluate_with, Value};
use std::collections::HashMap;

fn approxn(v: Value, b: f64) -> bool { matches!(v, Value::Number(a) if (a - b).abs() < 1e-9) }
fn as_str(v: Value) -> String { if let Value::String(s) = v { s } else { panic!("expected string") } }
fn as_bool(v: Value) -> bool { if let Value::Boolean(b) = v { b } else { panic!("expected bool, got {:?}", v) } }

#[test]
fn prd_basic_arithmetic() {
    assert!(approxn(evaluate("=10 + 20 * 3").unwrap(), 70.0));
    assert!(approxn(evaluate("=(10 + 20) * 3").unwrap(), 90.0));
}

#[test]
fn prd_variables() {
    let mut vars = HashMap::new();
    vars.insert("price".into(), Value::Number(19.99));
    vars.insert("quantity".into(), Value::Number(3.0));
    vars.insert("base_salary".into(), Value::Number(1000.0));
    vars.insert("bonus".into(), Value::Number(250.0));
    assert!(approxn(evaluate_with("=:price * :quantity", &vars).unwrap(), 59.97));
    assert!(approxn(evaluate_with("=:base_salary + :bonus", &vars).unwrap(), 1250.0));
}

#[test]
fn prd_arrays() {
    // literal
    match evaluate("=[1, 2, 3, 4, 5]").unwrap() { Value::Array(v) => assert_eq!(v.len(), 5), _ => panic!() }
    // indexing and slicing
    let mut vars = HashMap::new();
    vars.insert("numbers".into(), Value::Array(vec![1.0,2.0,3.0,4.0,5.0].into_iter().map(Value::Number).collect()));
    vars.insert("items".into(), Value::Array(vec![10.0,20.0,30.0,40.0].into_iter().map(Value::Number).collect()));
    vars.insert("array".into(), Value::Array(vec![1.0,2.0,3.0].into_iter().map(Value::Number).collect()));
    assert!(approxn(evaluate_with("=:numbers[0]", &vars).unwrap(), 1.0));
    match evaluate_with("=:items[1:3]", &vars).unwrap() { Value::Array(v) => assert_eq!(v, vec![Value::Number(20.0), Value::Number(30.0)]), _ => panic!() }
    assert!(approxn(evaluate_with("=SUM(...:array)", &vars).unwrap(), 6.0));
    // mixed types
    let mut vars2 = HashMap::new();
    vars2.insert("variable".into(), Value::Number(7.0));
    match evaluate_with("=[1, \"text\", true, :variable]", &vars2).unwrap() { Value::Array(v) => assert_eq!(v.len(), 4), _ => panic!() }
}

#[test]
fn prd_functions() {
    let mut vars = HashMap::new();
    vars.insert("total".into(), Value::Number(3.14159));
    vars.insert("name".into(), Value::String("World".into()));
    vars.insert("array".into(), Value::Array(vec![1.0,2.0,3.0].into_iter().map(Value::Number).collect()));
    assert!(approxn(evaluate("=SUM(1, 2, 3, 4, 5)").unwrap(), 15.0));
    assert!(approxn(evaluate_with("=ROUND(:total, 2)", &vars).unwrap(), 3.14));
    assert_eq!(as_str(evaluate_with("=CONCAT(\"Hello, \", :name)", &vars).unwrap()), "Hello, World");
    assert!(approxn(evaluate_with("=LENGTH(:array)", &vars).unwrap(), 3.0));
}

#[test]
fn prd_functions_filter_param_inference() {
    // Pending feature: infer lambda param from symbol name (:value)
    let mut vars = HashMap::new();
    vars.insert("numbers".into(), Value::Array(vec![5.0,12.0,30.0].into_iter().map(Value::Number).collect()));
    let _ = evaluate_with("=FILTER(:numbers, :x > 10)", &vars).unwrap();
}

#[test]

fn prd_chained_methods() {
    let mut vars = HashMap::new();
    vars.insert("some_var".into(), Value::Number(5.0));
    vars.insert("text".into(), Value::String("  hello  ".into()));
    vars.insert("number".into(), Value::Number(-3.14159));
    vars.insert("array".into(), Value::Array(vec![1.0,2.0,3.0].into_iter().map(Value::Number).collect()));
    vars.insert("values".into(), Value::Array(vec![-1.0,2.0,-3.0,4.0].into_iter().map(Value::Number).collect()));
    vars.insert("name".into(), Value::Null);
    vars.insert("prices".into(), Value::Array(vec![10.0,15.0].into_iter().map(Value::Number).collect()));
    // TODO: investigate :some_var.positive? in this context
    // assert!(as_bool(evaluate_with("=:some_var.positive?", &vars).unwrap()));
    assert!(!as_bool(evaluate("=0.nil?").unwrap()));
    assert_eq!(as_str(evaluate_with("=:text.upper().trim()", &vars).unwrap()), "HELLO");
    assert!(approxn(evaluate_with("=:number.abs().round(2)", &vars).unwrap(), 3.14));
    assert!(as_bool(evaluate_with("=:array.length().positive?", &vars).unwrap()));
    assert!(approxn(evaluate_with("=:values.filter(:x > 0).sum()", &vars).unwrap(), 6.0));
    assert_eq!(as_str(evaluate_with("=:name.blank? ? \"Anonymous\" : :name.upper()", &vars).unwrap()), "Anonymous");
    assert!(approxn(evaluate("=[1, 2, 3, 4].sum()").unwrap(), 10.0));
    assert!(approxn(evaluate_with("=:prices.map(:x * 1.1).sum().round(2)", &vars).unwrap(), 27.5));
}

#[test]
fn prd_conditionals_and_casting() {
    let mut vars = HashMap::new();
    vars.insert("score".into(), Value::Number(85.0));
    vars.insert("value".into(), Value::Null);
    assert_eq!(as_str(evaluate_with("=:score >= 90 ? \"A\" : (:score >= 80 ? \"B\" : \"C\")", &vars).unwrap()), "B");
    assert!(approxn(evaluate_with("=:value.nil? ? 0 : :value", &vars).unwrap(), 0.0));

    vars.insert("text_number".into(), Value::String("42".into()));
    assert!(approxn(evaluate_with("=:text_number::Integer + 10", &vars).unwrap(), 52.0));

    vars.insert("timestamp".into(), Value::Number(1_690_000_000.0));
    // Just ensure it parses and returns DateTime as number for now
    let _ = evaluate_with("=:timestamp::DateTime", &vars).unwrap();
}

#[test]
fn prd_conditionals_if_function() {
    let mut vars = HashMap::new();
    vars.insert("age".into(), Value::Number(20.0));
    let _ = evaluate_with("=IF(:age >= 18, 'Adult', 'Minor')", &vars).unwrap();
}

#[test]
fn prd_casting_string_to_array_and_array_element_cast() {
    let mut vars = HashMap::new();
    vars.insert("csv_data".into(), Value::String("1,2,3".into()));
    let _ = evaluate_with("=:csv_data::Array", &vars).unwrap();
    vars.insert("scores".into(), Value::Array(vec!["1","2","3"].into_iter().map(|s| Value::String(s.into())).collect()));
    let _ = evaluate_with("=ROUND(AVG(:scores.map(:x::Integer)), 2)", &vars).unwrap();
}

#[test]
fn prd_complex_objects_and_if() {
    let mut vars = HashMap::new();
    vars.insert("items".into(), Value::Array(vec![12.0, 100.0, 200.0, 4000.0].into_iter().map(Value::Number).collect()));
    vars.insert("price".into(), Value::Number(10.0));
    let _ = evaluate_with("=:items.filter(:price > 100).map(:price * 0.9).sum()", &vars).unwrap();
    vars.insert("sales".into(), Value::Number(12_000.0));
    let _ = evaluate_with("=IF(SUM(:sales) > 10000, :sales * 0.1, :sales * 0.05)", &vars).unwrap();
    vars.insert("user_input".into(), Value::String("  hi  ".into()));
    let _ = evaluate_with("=IF(:user_input.blank?, \"default\", :user_input.trim().lower())", &vars).unwrap();
}
