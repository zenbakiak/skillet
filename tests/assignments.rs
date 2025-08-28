use skillet::{evaluate_with_assignments, Value};
use std::collections::HashMap;

fn approx(v: Value, expected: f64) -> bool {
    matches!(v, Value::Number(a) if (a - expected).abs() < 1e-9)
}

#[test]
fn test_simple_assignment() {
    let vars = HashMap::new();
    let result = evaluate_with_assignments(":x := 42; :x", &vars).unwrap();
    assert!(matches!(result, Value::Number(42.0)));
}

#[test]
fn test_multiple_assignments() {
    let vars = HashMap::new();
    let result = evaluate_with_assignments(":x := 5; :y := 10; :x + :y", &vars).unwrap();
    assert!(matches!(result, Value::Number(15.0)));
}

#[test]
fn test_assignment_with_calculation() {
    let vars = HashMap::new();
    let result = evaluate_with_assignments(":x := 2 + 3; :y := :x * 2; :y", &vars).unwrap();
    assert!(matches!(result, Value::Number(10.0)));
}

#[test]
fn test_prd_example() {
    // Test the example from the PRD document
    let vars = HashMap::new();
    let input = ":sum_group_1 := SUM([1,2,3,4,5,6])/LENGTH([1,2,3,4,5,6]); :sum_group_2 := SUM([23,4,5,6,7,8])/LENGTH([23,4,5,6,7,8]); (:sum_group_1 + :sum_group_2) * 100 / 50";
    let result = evaluate_with_assignments(input, &vars).unwrap();
    
    // Calculate expected result:
    // sum_group_1 = (1+2+3+4+5+6)/6 = 21/6 = 3.5
    // sum_group_2 = (23+4+5+6+7+8)/6 = 53/6 = 8.833...
    // result = (3.5 + 8.833...) * 100 / 50 = 12.333... * 2 = 24.666...
    let expected = (21.0/6.0 + 53.0/6.0) * 100.0 / 50.0;
    assert!(approx(result, expected));
}

#[test]
fn test_complex_assignments_with_functions() {
    let vars = HashMap::new();
    let input = ":data := [10, 20, 30, 40, 50]; :avg := SUM(:data) / LENGTH(:data); :total := :avg * COUNT(:data)";
    let result = evaluate_with_assignments(input, &vars).unwrap();
    // Average is 30, count is 5, total should be 150
    assert!(matches!(result, Value::Number(150.0)));
}

#[test]
fn test_trailing_semicolon() {
    let vars = HashMap::new();
    let result = evaluate_with_assignments(":x := 42;", &vars).unwrap();
    assert!(matches!(result, Value::Number(42.0)));
}

#[test]
fn test_no_assignments() {
    // Test that expressions without assignments still work
    let vars = HashMap::new();
    let result = evaluate_with_assignments("2 + 3 * 4", &vars).unwrap();
    assert!(matches!(result, Value::Number(14.0)));
}

#[test]
fn test_assignment_with_initial_vars() {
    let mut vars = HashMap::new();
    vars.insert("base".to_string(), Value::Number(100.0));
    
    let result = evaluate_with_assignments(":multiplier := 2; :base * :multiplier", &vars).unwrap();
    assert!(matches!(result, Value::Number(200.0)));
}

#[test]
fn test_new_array_functions_in_assignment() {
    let vars = HashMap::new();
    let input = ":arr := [1, 2, 3, 4, 5]; :size := COUNT(:arr); :has_three := IN(:arr, 3); IF(:has_three, :size, 0)";
    let result = evaluate_with_assignments(input, &vars).unwrap();
    assert!(matches!(result, Value::Number(5.0)));
}

#[test]
fn test_new_financial_functions_in_assignment() {
    let vars = HashMap::new();
    let input = ":rate := 0.05; :periods := 10; :payment := -1000; :future := FV(:rate, :periods, :payment); :future";
    let result = evaluate_with_assignments(input, &vars).unwrap();
    // Should be approximately 12577.89 (use looser tolerance for floating point)
    assert!(matches!(result, Value::Number(n) if (n - 12577.89).abs() < 1.0));
}

#[test]
fn test_variable_scoping() {
    let mut vars = HashMap::new();
    vars.insert("global_var".to_string(), Value::Number(1.0));
    
    let result = evaluate_with_assignments(":local_var := :global_var + 10; :another := :local_var * 2; :another", &vars).unwrap();
    assert!(matches!(result, Value::Number(22.0)));
}

#[test]
fn test_assignment_expression_returns_value() {
    let vars = HashMap::new();
    // The assignment itself should return the assigned value
    let result = evaluate_with_assignments(":x := 42", &vars).unwrap();
    assert!(matches!(result, Value::Number(42.0)));
}