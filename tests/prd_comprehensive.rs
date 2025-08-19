use skillet::{evaluate, Value};

fn approx(v: Value, expected: f64) -> bool {
    matches!(v, Value::Number(a) if (a - expected).abs() < 1e-6)
}

fn as_bool(v: Value) -> bool {
    matches!(v, Value::Boolean(b) if b)
}

fn as_string(v: Value) -> String {
    match v { Value::String(s) => s, _ => panic!("Expected string, got {:?}", v) }
}

#[test]
fn test_logical_formulas_basic() {
    // Basic boolean logic
    assert!(as_bool(evaluate("=AND(TRUE, TRUE)").unwrap()));
    assert!(!as_bool(evaluate("=AND(TRUE, FALSE)").unwrap()));
    assert!(!as_bool(evaluate("=OR(FALSE, FALSE)").unwrap()));
    assert!(as_bool(evaluate("=OR(TRUE, FALSE)").unwrap()));
    
    // XOR logic
    assert!(!as_bool(evaluate("=XOR(TRUE, TRUE)").unwrap()));
    assert!(as_bool(evaluate("=XOR(TRUE, FALSE)").unwrap()));
    
    // NOT logic
    assert!(!as_bool(evaluate("=NOT(TRUE)").unwrap()));
    assert!(as_bool(evaluate("=NOT(FALSE)").unwrap()));
    
    // NOR via NOT(OR())
    assert!(as_bool(evaluate("=NOT(OR(FALSE, FALSE))").unwrap()));
    assert!(!as_bool(evaluate("=NOT(OR(TRUE, FALSE))").unwrap()));
}

#[test]
fn test_logical_formulas_nested_if() {
    // Nested IF as ELSE emulation
    assert_eq!(as_string(evaluate("=IF(5>3, \"A>B\", IF(5=3, \"A=B\", \"A<B\"))").unwrap()), "A>B");
    assert_eq!(as_string(evaluate("=IF(3>5, \"A>B\", IF(3=5, \"A=B\", \"A<B\"))").unwrap()), "A<B");
    assert_eq!(as_string(evaluate("=IF(10=10, \"A=B\", IF(10>10, \"A>B\", \"A<B\"))").unwrap()), "A=B");
}

#[test]
fn test_logical_formulas_grading() {
    // Grading system with nested IF
    assert_eq!(as_string(evaluate("=IF(AND(95>=90, 95<=100), \"A\", IF(95>=80, \"B\", IF(95>=70, \"C\", IF(95>=60, \"D\", \"F\"))))").unwrap()), "A");
    assert_eq!(as_string(evaluate("=IF(AND(88>=90, 88<=100), \"A\", IF(88>=80, \"B\", IF(88>=70, \"C\", IF(88>=60, \"D\", \"F\"))))").unwrap()), "B");
    assert_eq!(as_string(evaluate("=IF(AND(72>=90, 72<=100), \"A\", IF(72>=80, \"B\", IF(72>=70, \"C\", IF(72>=60, \"D\", \"F\"))))").unwrap()), "C");
    assert_eq!(as_string(evaluate("=IF(AND(59>=90, 59<=100), \"A\", IF(59>=80, \"B\", IF(59>=70, \"C\", IF(59>=60, \"D\", \"F\"))))").unwrap()), "F");
}

#[test]
fn test_logical_formulas_ifs() {
    // IFS with temperature labels
    assert_eq!(as_string(evaluate("=IFS(5<10, \"Cold\", 5<20, \"Cool\", 5<30, \"Warm\", 5<40, \"Hot\", TRUE, \"Extreme\")").unwrap()), "Cold");
    assert_eq!(as_string(evaluate("=IFS(15<10, \"Cold\", 15<20, \"Cool\", 15<30, \"Warm\", 15<40, \"Hot\", TRUE, \"Extreme\")").unwrap()), "Cool");
    assert_eq!(as_string(evaluate("=IFS(25<10, \"Cold\", 25<20, \"Cool\", 25<30, \"Warm\", 25<40, \"Hot\", TRUE, \"Extreme\")").unwrap()), "Warm");
    assert_eq!(as_string(evaluate("=IFS(35<10, \"Cold\", 35<20, \"Cool\", 35<30, \"Warm\", 35<40, \"Hot\", TRUE, \"Extreme\")").unwrap()), "Hot");
    assert_eq!(as_string(evaluate("=IFS(45<10, \"Cold\", 45<20, \"Cool\", 45<30, \"Warm\", 45<40, \"Hot\", TRUE, \"Extreme\")").unwrap()), "Extreme");
}

#[test]
fn test_arithmetic_formulas_basic() {
    // Operator precedence + MOD
    assert!(approx(evaluate("=(8+3*2)-MOD(8,3)").unwrap(), 12.0));
    assert!(approx(evaluate("=(15+4*2)-MOD(15,4)").unwrap(), 20.0));
    assert!(approx(evaluate("=(100+25*2)-MOD(100,25)").unwrap(), 150.0));
    
    // Percent increase
    assert!(approx(evaluate("=200*(1+0.15)").unwrap(), 230.0));
    assert!(approx(evaluate("=500*(1+0.07)").unwrap(), 535.0));
    
    // Percent decrease
    assert!(approx(evaluate("=200*(1-0.15)").unwrap(), 170.0));
    assert!(approx(evaluate("=500*(1-0.07)").unwrap(), 465.0));
}

#[test]
fn test_arithmetic_formulas_power_functions() {
    // POWER, SQRT, ABS combinations
    assert!(approx(evaluate("=POWER(2,3)+SQRT(4)-ABS(-2)").unwrap(), 8.0 + 2.0 - 2.0));
    assert!(approx(evaluate("=POWER(3,3)+SQRT(9)-ABS(-3)").unwrap(), 27.0 + 3.0 - 3.0));
}

#[test]
fn test_arithmetic_formulas_rounding() {
    // ROUND function
    assert!(approx(evaluate("=ROUND(1.2345,2)").unwrap(), 1.23));
    assert!(approx(evaluate("=ROUND(2.71828,2)").unwrap(), 2.72));
    assert!(approx(evaluate("=ROUND(3.14159,2)").unwrap(), 3.14));
    assert!(approx(evaluate("=ROUND(9.87654,2)").unwrap(), 9.88));
    
    // INT, CEILING, FLOOR functions
    assert!(approx(evaluate("=INT(5.9)").unwrap(), 5.0));
    assert!(approx(evaluate("=CEILING(5.9,1)").unwrap(), 6.0));
    assert!(approx(evaluate("=FLOOR(5.9,1)").unwrap(), 5.0));
    
    assert!(approx(evaluate("=INT(-3.2)").unwrap(), -4.0));
    assert!(approx(evaluate("=CEILING(-3.2,1)").unwrap(), -3.0));
    assert!(approx(evaluate("=FLOOR(-3.2,1)").unwrap(), -4.0));
}

#[test]
fn test_arithmetic_formulas_modulo() {
    // MOD operations
    assert!(approx(evaluate("=MOD(10,3)").unwrap(), 1.0));
    assert!(approx(evaluate("=MOD(11,4)").unwrap(), 3.0));
    assert!(approx(evaluate("=MOD(20,6)").unwrap(), 2.0));
    assert!(approx(evaluate("=MOD(25,7)").unwrap(), 4.0));
    assert!(approx(evaluate("=MOD(100,9)").unwrap(), 1.0));
}

#[test]
fn test_statistical_formulas_basic() {
    // SUM operations
    assert!(approx(evaluate("=SUM(2,2,3,4,5,5,5,8,9)").unwrap(), 43.0));
    assert!(approx(evaluate("=SUM(10,10,10,12,14,14,16,18,18,18)").unwrap(), 140.0));
    
    // AVERAGE operations
    assert!(approx(evaluate("=AVERAGE(2,2,3,4,5,5,5,8,9)").unwrap(), 4.777778));
    assert!(approx(evaluate("=AVERAGE(10,10,10,12,14,14,16,18,18,18)").unwrap(), 14.0));
    
    // MEDIAN operations
    assert!(approx(evaluate("=MEDIAN(2,2,3,4,5,5,5,8,9)").unwrap(), 5.0));
    assert!(approx(evaluate("=MEDIAN(10,10,10,12,14,14,16,18,18,18)").unwrap(), 14.0));
    
    // MODE.SNGL operations (using underscore version)
    assert!(approx(evaluate("=MODE_SNGL(2,2,3,4,5,5,5,8,9)").unwrap(), 5.0));
    assert!(approx(evaluate("=MODE_SNGL(10,10,10,12,14,14,16,18,18,18)").unwrap(), 10.0));
}

#[test]
fn test_statistical_formulas_advanced() {
    // Standard deviation and variance (using underscore versions)
    let result = evaluate("=STDEV_P(2,2,3,4,5,5,5,8,9)").unwrap();
    if let Value::Number(n) = result {
        assert!((n - 2.298685).abs() < 0.001);
    }
    
    let result = evaluate("=VAR_P(2,2,3,4,5,5,5,8,9)").unwrap();
    if let Value::Number(n) = result {
        assert!((n - 5.283951).abs() < 0.001);
    }
}

#[test]
fn test_statistical_formulas_percentiles() {
    // PERCENTILE.INC operations (using underscore versions)
    assert!(approx(evaluate("=PERCENTILE_INC(2,2,3,4,5,5,5,8,9,0.1)").unwrap(), 2.0));
    assert!(approx(evaluate("=PERCENTILE_INC(2,2,3,4,5,5,5,8,9,0.25)").unwrap(), 3.0));
    assert!(approx(evaluate("=PERCENTILE_INC(2,2,3,4,5,5,5,8,9,0.5)").unwrap(), 5.0));
    
    // QUARTILE.INC operations (using underscore versions)
    assert!(approx(evaluate("=QUARTILE_INC(2,2,3,4,5,5,5,8,9,0)").unwrap(), 2.0));
    assert!(approx(evaluate("=QUARTILE_INC(2,2,3,4,5,5,5,8,9,1)").unwrap(), 3.0));
    assert!(approx(evaluate("=QUARTILE_INC(2,2,3,4,5,5,5,8,9,2)").unwrap(), 5.0));
    assert!(approx(evaluate("=QUARTILE_INC(2,2,3,4,5,5,5,8,9,4)").unwrap(), 9.0));
}

#[test]
fn test_mixed_formulas_bonuses() {
    // Bonus calculations with AND conditions
    assert!(approx(evaluate("=IF(AND(55000>50000,6>=5), 55000*0.1, 55000*0.05)").unwrap(), 5500.0));
    assert!(approx(evaluate("=IF(AND(45000>50000,3>=5), 45000*0.1, 45000*0.05)").unwrap(), 2250.0));
    assert!(approx(evaluate("=IF(AND(70000>50000,10>=5), 70000*0.1, 70000*0.05)").unwrap(), 7000.0));
}

#[test]
fn test_mixed_formulas_tiered_discounts() {
    // Tiered discount with IFS
    assert!(approx(evaluate("=IFS(1200>=1000,1200*0.8,1200>=500,1200*0.9,TRUE,1200)").unwrap(), 960.0));
    assert!(approx(evaluate("=IFS(800>=1000,800*0.8,800>=500,800*0.9,TRUE,800)").unwrap(), 720.0));
    assert!(approx(evaluate("=IFS(400>=1000,400*0.8,400>=500,400*0.9,TRUE,400)").unwrap(), 400.0));
}

#[test]
fn test_mixed_formulas_shipping() {
    // Shipping fee by weight
    assert!(approx(evaluate("=IF(0.5<=1,5,IF(0.5<=5,10,20))").unwrap(), 5.0));
    assert!(approx(evaluate("=IF(2.5<=1,5,IF(2.5<=5,10,20))").unwrap(), 10.0));
    assert!(approx(evaluate("=IF(10.0<=1,5,IF(10.0<=5,10,20))").unwrap(), 20.0));
}

#[test]
fn test_mixed_formulas_progressive_tax() {
    // Progressive tax calculation
    assert!(approx(evaluate("=ROUND(IF(15000<=30000,15000*0.1,IF(15000<=70000,15000*0.2,15000*0.3)),2)").unwrap(), 1500.0));
    assert!(approx(evaluate("=ROUND(IF(45000<=30000,45000*0.1,IF(45000<=70000,45000*0.2,45000*0.3)),2)").unwrap(), 9000.0));
    assert!(approx(evaluate("=ROUND(IF(75000<=30000,75000*0.1,IF(75000<=70000,75000*0.2,75000*0.3)),2)").unwrap(), 22500.0));
}

#[test]
fn test_mixed_formulas_xor_rules() {
    // XOR-based eligibility rules
    assert_eq!(as_string(evaluate("=IF(XOR(750>=700,20<=20),\"Eligible\",\"Review\")").unwrap()), "Review");
    assert_eq!(as_string(evaluate("=IF(XOR(800>=700,50<=20),\"Eligible\",\"Review\")").unwrap()), "Eligible");
    assert_eq!(as_string(evaluate("=IF(XOR(650>=700,15<=20),\"Eligible\",\"Review\")").unwrap()), "Eligible");
}

#[test]
fn test_mixed_formulas_conditional_discounts() {
    // Conditional discount with rounding
    assert!(approx(evaluate("=ROUND(IF(19.99>=50,19.99*0.9,19.99),2)").unwrap(), 19.99));
    assert!(approx(evaluate("=ROUND(IF(75.25>=50,75.25*0.9,75.25),2)").unwrap(), 67.73));
    assert!(approx(evaluate("=ROUND(IF(100.0>=50,100.0*0.9,100.0),2)").unwrap(), 90.0));
}

#[test]
fn test_mixed_formulas_billable_hours() {
    // Billable hours calculation with CEILING
    assert!(approx(evaluate("=CEILING(15/60,1)").unwrap(), 1.0));
    assert!(approx(evaluate("=CEILING(61/60,1)").unwrap(), 2.0));
    assert!(approx(evaluate("=CEILING(125/60,1)").unwrap(), 3.0));
}

#[test]
fn test_mixed_formulas_bulk_discount() {
    // Bulk discount after quantity threshold
    assert!(approx(evaluate("=ROUND(94*5*IF(5>=5,0.95,1),2)").unwrap(), 446.5));
    assert!(approx(evaluate("=ROUND(95*6*IF(6>=5,0.95,1),2)").unwrap(), 541.5));
    assert!(approx(evaluate("=ROUND(97*1*IF(1>=5,0.95,1),2)").unwrap(), 97.0));
}