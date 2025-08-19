use skillet::{evaluate, Value};

fn approx(v: Value, expected: f64) -> bool {
    matches!(v, Value::Number(a) if (a - expected).abs() < 1e-2)
}

#[test]
fn test_pmt_basic_loan() {
    // Basic loan: $100,000 at 5% annual rate for 30 years
    // Monthly rate: 5%/12 = 0.004167, periods: 30*12 = 360
    let result = evaluate("=PMT(0.05/12, 30*12, 100000)").unwrap();
    // Expected monthly payment: approximately -$536.82
    assert!(approx(result, -536.82));
}

#[test]
fn test_pmt_with_future_value() {
    // Loan with balloon payment
    // $50,000 loan, 4% annual rate, 5 years, $10,000 balloon payment
    let result = evaluate("=PMT(0.04/12, 5*12, 50000, 10000)").unwrap();
    // Should be higher payment due to balloon payment
    assert!(approx(result, -1071.66));
}

#[test]
fn test_pmt_beginning_of_period() {
    // Payment at beginning of period (type = 1)
    let result = evaluate("=PMT(0.05/12, 30*12, 100000, 0, 1)").unwrap();
    // Should be slightly less than end-of-period payment
    assert!(approx(result, -534.59));
}

#[test]
fn test_pmt_zero_interest() {
    // No interest loan - should just be principal divided by periods
    let result = evaluate("=PMT(0, 12, 12000)").unwrap();
    assert!(approx(result, -1000.0)); // 12000 / 12 = 1000
}

#[test]
fn test_pmt_investment_annuity() {
    // Investment scenario: want $50,000 in 10 years at 6% interest
    // How much to save monthly?
    let result = evaluate("=PMT(0.06/12, 10*12, 0, 50000)").unwrap();
    // Should be negative (payment out) of approximately -$305
    assert!(approx(result, -305.10));
}

#[test]
fn test_pmt_car_loan() {
    // Car loan: $25,000 at 3.5% for 5 years
    let result = evaluate("=PMT(0.035/12, 5*12, 25000)").unwrap();
    assert!(approx(result, -454.79));
}

#[test]
fn test_pmt_short_term_loan() {
    // Short-term loan: $5,000 at 8% for 2 years
    let result = evaluate("=PMT(0.08/12, 2*12, 5000)").unwrap();
    assert!(approx(result, -226.14));
}

#[test]
fn test_pmt_error_cases() {
    // Test error cases
    
    // Too few arguments
    let result = evaluate("=PMT(0.05, 12)");
    assert!(result.is_err());
    
    // Too many arguments
    let result = evaluate("=PMT(0.05, 12, 1000, 0, 0, 0)");
    assert!(result.is_err());
    
    // Zero or negative periods
    let result = evaluate("=PMT(0.05, 0, 1000)");
    assert!(result.is_err());
    
    let result = evaluate("=PMT(0.05, -12, 1000)");
    assert!(result.is_err());
    
    // Non-numeric arguments
    let result = evaluate("=PMT(\"invalid\", 12, 1000)");
    assert!(result.is_err());
}

#[test]
fn test_pmt_real_world_scenarios() {
    // Mortgage: $300,000 house, 20% down, 30-year fixed at 6.5%
    let loan_amount = 300000.0 * 0.8; // 240,000
    let monthly_rate = 0.065 / 12.0;
    let months = 30.0 * 12.0;
    
    let result = evaluate(&format!("=PMT({}, {}, {})", monthly_rate, months, loan_amount)).unwrap();
    assert!(approx(result, -1516.96));
    
    // Business loan: $75,000 at 7% for 7 years
    let result = evaluate("=PMT(0.07/12, 7*12, 75000)").unwrap();
    assert!(approx(result, -1131.95));
    
    // Student loan: $40,000 at 4.5% for 10 years
    let result = evaluate("=PMT(0.045/12, 10*12, 40000)").unwrap();
    assert!(approx(result, -414.55));
}

#[test]
fn test_pmt_with_variables() {
    use skillet::evaluate_with;
    use std::collections::HashMap;
    
    let mut vars = HashMap::new();
    vars.insert("principal".to_string(), Value::Number(100000.0));
    vars.insert("annual_rate".to_string(), Value::Number(0.05));
    vars.insert("years".to_string(), Value::Number(30.0));
    
    let result = evaluate_with("=PMT(:annual_rate/12, :years*12, :principal)", &vars).unwrap();
    assert!(approx(result, -536.82));
}

#[test]
fn test_pmt_json_integration() {
    use skillet::evaluate_with_json;
    
    let json_vars = r#"{
        "principal": 50000,
        "annual_rate": 0.04,
        "years": 5
    }"#;
    
    let result = evaluate_with_json(
        "=PMT(:annual_rate/12, :years*12, :principal)", 
        json_vars
    ).unwrap();
    
    assert!(approx(result, -920.83));
}