use skillet::{evaluate, Value};

fn s(v: Value) -> String { if let Value::String(s) = v { s } else { panic!("expected string") } }
fn n(v: Value) -> f64 { if let Value::Number(n) = v { n } else { panic!("expected number") } }
fn b(v: Value) -> bool { if let Value::Boolean(b) = v { b } else { panic!("expected bool") } }
fn a(v: Value) -> Vec<Value> { if let Value::Array(a) = v { a } else { panic!("expected array") } }
fn j(v: Value) -> String { if let Value::Json(j) = v { j } else { panic!("expected json") } }

#[test]
fn null_conversions() {
    // Null conversion methods
    assert_eq!(s(evaluate("NULL.to_s()").unwrap()), "");
    assert_eq!(n(evaluate("NULL.to_i()").unwrap()), 0.0);
    assert_eq!(n(evaluate("NULL.to_f()").unwrap()), 0.0);
    assert_eq!(a(evaluate("NULL.to_a()").unwrap()).len(), 0);
    assert_eq!(j(evaluate("NULL.to_json()").unwrap()), "{}");
    assert_eq!(b(evaluate("NULL.to_bool()").unwrap()), false);
}

#[test]
fn string_conversions() {
    // String conversion methods
    assert_eq!(s(evaluate("\"hello\".to_s()").unwrap()), "hello");
    assert_eq!(n(evaluate("\"123\".to_i()").unwrap()), 123.0);
    assert_eq!(n(evaluate("\"123.45\".to_f()").unwrap()), 123.45);
    assert_eq!(n(evaluate("\"abc\".to_i()").unwrap()), 0.0); // Invalid string to 0
    assert_eq!(a(evaluate("\"hi\".to_a()").unwrap()), vec![Value::String("h".to_string()), Value::String("i".to_string())]);
    assert_eq!(b(evaluate("\"\".to_bool()").unwrap()), false);
    assert_eq!(b(evaluate("\"hello\".to_bool()").unwrap()), true);
}

#[test]
fn number_conversions() {
    // Number conversion methods
    assert_eq!(s(evaluate("123.to_s()").unwrap()), "123");
    assert_eq!(s(evaluate("123.45.to_s()").unwrap()), "123.45");
    assert_eq!(n(evaluate("123.45.to_i()").unwrap()), 123.0); // Truncates
    assert_eq!(n(evaluate("123.45.to_f()").unwrap()), 123.45);
    assert_eq!(a(evaluate("42.to_a()").unwrap()), vec![Value::Number(42.0)]);
    assert_eq!(b(evaluate("0.to_bool()").unwrap()), false);
    assert_eq!(b(evaluate("123.to_bool()").unwrap()), true);
}

#[test]
fn boolean_conversions() {
    // Boolean conversion methods
    assert_eq!(s(evaluate("true.to_s()").unwrap()), "true");
    assert_eq!(s(evaluate("false.to_s()").unwrap()), "false");
    assert_eq!(n(evaluate("true.to_i()").unwrap()), 1.0);
    assert_eq!(n(evaluate("false.to_i()").unwrap()), 0.0);
    assert_eq!(n(evaluate("true.to_f()").unwrap()), 1.0);
    assert_eq!(n(evaluate("false.to_f()").unwrap()), 0.0);
    assert_eq!(b(evaluate("true.to_bool()").unwrap()), true);
    assert_eq!(b(evaluate("false.to_bool()").unwrap()), false);
}

#[test]
fn array_conversions() {
    // Array conversion methods
    assert_eq!(s(evaluate("[1, 2, 3].to_s()").unwrap()), "[1, 2, 3]");
    assert_eq!(n(evaluate("[1, 2, 3].to_i()").unwrap()), 3.0); // Array length
    assert_eq!(n(evaluate("[].to_i()").unwrap()), 0.0);
    assert_eq!(a(evaluate("[1, 2, 3].to_a()").unwrap()), vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)]);
    assert_eq!(b(evaluate("[].to_bool()").unwrap()), false);
    assert_eq!(b(evaluate("[1].to_bool()").unwrap()), true);
}

#[test]
fn long_form_method_names() {
    // Test long form method names
    assert_eq!(s(evaluate("NULL.to_string()").unwrap()), "");
    assert_eq!(n(evaluate("\"123\".to_int()").unwrap()), 123.0);
    assert_eq!(n(evaluate("\"123.45\".to_float()").unwrap()), 123.45);
    assert_eq!(a(evaluate("\"hi\".to_array()").unwrap()), vec![Value::String("h".to_string()), Value::String("i".to_string())]);
    assert_eq!(b(evaluate("\"\".to_boolean()").unwrap()), false);
}

#[test]
fn practical_null_handling() {
    // Test the practical use case that motivated this feature
    use skillet::evaluate_with_assignments;
    use std::collections::HashMap;
    
    let result = evaluate_with_assignments(r#"
        :cuentas := [{"FechaCierreCuenta": null}, {"FechaCierreCuenta": ""}, {"FechaCierreCuenta": "2023-01-01"}]; 
        :cuentas.filter(:x.FechaCierreCuenta.to_s().length() == 0)
    "#, &HashMap::new()).unwrap();
    
    if let Value::Array(arr) = result {
        assert_eq!(arr.len(), 2); // null and empty string both convert to empty string
    } else {
        panic!("Expected array result");
    }
}

#[test]
fn chained_conversions() {
    // Test chaining conversion methods
    assert_eq!(s(evaluate("123.to_s().to_s()").unwrap()), "123");
    assert_eq!(n(evaluate("\"123\".to_i().to_f()").unwrap()), 123.0);
    assert_eq!(b(evaluate("\"hello\".to_a().length().to_bool()").unwrap()), true);
}