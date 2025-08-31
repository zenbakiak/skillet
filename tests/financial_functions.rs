use skillet::{evaluate, Value};

fn approx(v: Value, expected: f64) -> bool {
    matches!(v, Value::Number(a) if (a - expected).abs() < 1.0)
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

#[test]
fn test_db_basic() {
    // Basic depreciation: $10,000 cost, $1,000 salvage, 5 years, period 1
    let result = evaluate("=DB(10000, 1000, 5, 1)").unwrap();
    // Expected: fixed-declining balance depreciation
    assert!(approx(result, 3690.00));
}

#[test]
fn test_db_with_months() {
    // Depreciation with partial first year (6 months)
    let result = evaluate("=DB(10000, 1000, 5, 1, 6)").unwrap();
    // Should be prorated for 6 months
    assert!(approx(result, 1845.00));
}

#[test]
fn test_db_multiple_periods() {
    // Test multiple periods
    let result1 = evaluate("=DB(10000, 1000, 5, 1)").unwrap();
    let result2 = evaluate("=DB(10000, 1000, 5, 2)").unwrap();
    let result3 = evaluate("=DB(10000, 1000, 5, 3)").unwrap();
    
    // Each period should be different and generally decreasing
    assert!(matches!(result1, Value::Number(a) if a > 0.0));
    assert!(matches!(result2, Value::Number(b) if b > 0.0));
    assert!(matches!(result3, Value::Number(c) if c > 0.0));
}

#[test]
fn test_fv_basic() {
    // Future value: 5% rate, 10 periods, $1000 payment
    let result = evaluate("=FV(0.05, 10, -1000)").unwrap();
    // Expected: approximately $12,578
    assert!(approx(result, 12577.89));
}

#[test]
fn test_fv_with_present_value() {
    // FV with present value: $5000 invested now plus $100/month for 12 months at 6% annual
    let result = evaluate("=FV(0.06/12, 12, -100, -5000)").unwrap();
    // Expected: approximately $6,542
    assert!(approx(result, 6542.0));
}

#[test]
fn test_fv_beginning_of_period() {
    // FV with payments at beginning of period
    let result = evaluate("=FV(0.05/12, 12, -100, 0, 1)").unwrap();
    // Should be slightly higher than end-of-period payments
    assert!(approx(result, 1233.0));
}

#[test]
fn test_fv_zero_rate() {
    // FV with zero interest rate - should just be sum of payments
    let result = evaluate("=FV(0, 12, -100)").unwrap();
    assert!(approx(result, 1200.00)); // 12 * 100
}

#[test]
fn test_ipmt_basic() {
    // Interest payment for period 1 of a $100,000 loan at 5% for 30 years
    let result = evaluate("=IPMT(0.05/12, 1, 360, 100000)").unwrap();
    // First payment should be mostly interest: approximately $416.67
    assert!(approx(result, 416.67));
}

#[test]
fn test_ipmt_later_period() {
    // Interest payment for period 180 (halfway through) - should be less than period 1
    let result = evaluate("=IPMT(0.05/12, 180, 360, 100000)").unwrap();
    // Should be significantly less than first payment (note: result is negative)
    assert!(matches!(result, Value::Number(i) if i < 0.0 && i > -100.0));
}

#[test]
fn test_ipmt_with_future_value() {
    // IPMT with future value (balloon payment)
    let result = evaluate("=IPMT(0.04/12, 1, 60, 50000, 10000)").unwrap();
    assert!(matches!(result, Value::Number(i) if i > 0.0));
}

#[test]
fn test_ipmt_beginning_of_period() {
    // IPMT with payments at beginning of period - first period should be 0
    let result = evaluate("=IPMT(0.05/12, 1, 360, 100000, 0, 1)").unwrap();
    assert!(approx(result, 0.0)); // No interest on first payment when paid at beginning
}

#[test]
fn test_ipmt_zero_rate() {
    // IPMT with zero interest rate - should always be 0
    let result = evaluate("=IPMT(0, 1, 12, 12000)").unwrap();
    assert!(approx(result, 0.0));
}

#[test]
fn test_financial_error_cases() {
    // DB errors
    assert!(evaluate("=DB(10000, 1000, 5)").is_err()); // Too few arguments
    assert!(evaluate("=DB(-1000, 1000, 5, 1)").is_err()); // Negative cost
    assert!(evaluate("=DB(10000, 1000, 0, 1)").is_err()); // Zero life
    
    // FV errors  
    assert!(evaluate("=FV(0.05, 10)").is_err()); // Too few arguments
    assert!(evaluate("=FV(0.05, -10, 1000)").is_err()); // Negative periods
    
    // IPMT errors
    assert!(evaluate("=IPMT(0.05, 1, 12)").is_err()); // Too few arguments
    assert!(evaluate("=IPMT(0.05, 0, 12, 1000)").is_err()); // Period < 1
    assert!(evaluate("=IPMT(0.05, 13, 12, 1000)").is_err()); // Period > nper
    assert!(evaluate("=IPMT(0.05, 1, 0, 1000)").is_err()); // Zero periods
}