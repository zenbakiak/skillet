use std::process::Command;

fn run_sk(args: &[&str]) -> Result<(String, String, i32), Box<dyn std::error::Error>> {
    let output = Command::new("cargo")
        .args(&["run", "--bin", "sk", "--"])
        .args(args)
        .output()?;
    
    let stdout = String::from_utf8(output.stdout)?;
    let stderr = String::from_utf8(output.stderr)?;
    
    // Remove cargo compilation messages from stderr, keep only our error messages
    let clean_stderr = stderr.lines()
        .filter(|line| {
            let line = line.trim();
            !line.is_empty() 
            && !line.contains("Compiling") 
            && !line.contains("Finished") 
            && !line.contains("Running")
            && !line.starts_with("warning:")  // Remove cargo warnings too
        })
        .collect::<Vec<_>>()
        .join("\n");
    
    // Clean stdout to remove cargo messages that might appear there
    let clean_stdout = stdout.lines()
        .filter(|line| !line.trim().is_empty() && !line.contains("Compiling") && !line.contains("Finished") && !line.contains("Running"))
        .collect::<Vec<_>>()
        .join("\n");
    
    
    // If cargo compilation succeeded (contains "Finished"), trust the original exit code
    let exit_code = if stderr.contains("Finished") {
        output.status.code().unwrap_or(0) // Use original exit code, default to 0 if none
    } else {
        output.status.code().unwrap_or(-1) // Use actual exit code if cargo failed
    };
    
    Ok((clean_stdout.trim().to_string(), clean_stderr.trim().to_string(), exit_code))
}

#[test]
fn test_cli_basic_arithmetic() {
    let (stdout, _stderr, code) = run_sk(&["=2 + 3 * 4"]).unwrap();
    assert_eq!(code, 0);
    assert_eq!(stdout, "Number(14.0)");
}

#[test]
fn test_cli_with_single_variable() {
    let (stdout, _stderr, code) = run_sk(&["=SUM(:sales, 1000)", "sales=5000"]).unwrap();
    assert_eq!(code, 0);
    assert_eq!(stdout, "Number(6000.0)");
}

#[test]
fn test_cli_with_multiple_variables() {
    let (stdout, _stderr, code) = run_sk(&["=:price * :quantity", "price=19.99", "quantity=3"]).unwrap();
    assert_eq!(code, 0);
    assert_eq!(stdout, "Number(59.97)");
}

#[test]
fn test_cli_with_string_variable() {
    let (stdout, _stderr, code) = run_sk(&["=:name.upper()", "name=\"hello\""]).unwrap();
    assert_eq!(code, 0);
    assert_eq!(stdout, "String(\"HELLO\")");
}

#[test]
fn test_cli_with_boolean_variable() {
    let (stdout, _stderr, code) = run_sk(&["=IF(:active, \"YES\", \"NO\")", "active=true"]).unwrap();
    assert_eq!(code, 0);
    assert_eq!(stdout, "String(\"YES\")");
}

#[test]
fn test_cli_with_array_variable() {
    let (stdout, _stderr, code) = run_sk(&["=:numbers.length()", "numbers=[1,2,3,4,5]"]).unwrap();
    assert_eq!(code, 0);
    assert_eq!(stdout, "Number(5.0)");
}

#[test]
fn test_cli_with_null_variable() {
    let (stdout, _stderr, code) = run_sk(&["=:value.nil?", "value=null"]).unwrap();
    assert_eq!(code, 0);
    assert_eq!(stdout, "Boolean(true)");
}

#[test]
fn test_cli_error_handling() {
    let (_stdout, stderr, code) = run_sk(&["=UNKNOWN_FUNCTION()"]).unwrap();
    assert_ne!(code, 0);
    assert!(stderr.contains("Error:"));
}

#[test]
fn test_cli_invalid_variable_format() {
    let (_stdout, stderr, code) = run_sk(&["=:x", "invalid_format"]).unwrap();
    assert_ne!(code, 0);
    assert!(stderr.contains("Invalid variable assignment"));
}

#[test]
fn test_cli_help_message() {
    let (_stdout, stderr, code) = run_sk(&[]).unwrap();
    assert_ne!(code, 0);
    assert!(stderr.contains("Usage: sk"));
    assert!(stderr.contains("Examples:"));
}

// JSONPath Tests
#[test]
fn test_cli_jsonpath_sum() {
    let json_data = r#"{"accounts": [{"id": 1, "amount": 300.1}, {"id": 4, "amount": 890.1}]}"#;
    let (stdout, _stderr, code) = run_sk(&["=SUM(JQ(:arguments, \"$.accounts[*].amount\"))", "--json", json_data]).unwrap();
    assert_eq!(code, 0);
    assert_eq!(stdout, "Number(1190.2)");
}

#[test]
fn test_cli_jsonpath_avg() {
    let json_data = r#"{"scores": [{"value": 85}, {"value": 92}, {"value": 78}, {"value": 95}]}"#;
    let (stdout, _stderr, code) = run_sk(&["=AVG(JQ(:arguments, \"$.scores[*].value\"))", "--json", json_data]).unwrap();
    assert_eq!(code, 0);
    assert_eq!(stdout, "Number(87.5)");
}

#[test]
fn test_cli_jsonpath_max() {
    let json_data = r#"{"temperatures": [{"reading": 22.5}, {"reading": 31.2}, {"reading": 18.7}, {"reading": 29.8}]}"#;
    let (stdout, _stderr, code) = run_sk(&["=MAX(JQ(:arguments, \"$.temperatures[*].reading\"))", "--json", json_data]).unwrap();
    assert_eq!(code, 0);
    assert_eq!(stdout, "Number(31.2)");
}

#[test]
fn test_cli_jsonpath_min() {
    let json_data = r#"{"prices": [{"cost": 15.99}, {"cost": 8.50}, {"cost": 23.75}, {"cost": 12.30}]}"#;
    let (stdout, _stderr, code) = run_sk(&["=MIN(JQ(:arguments, \"$.prices[*].cost\"))", "--json", json_data]).unwrap();
    assert_eq!(code, 0);
    assert_eq!(stdout, "Number(8.5)");
}

#[test]
fn test_cli_jsonpath_nested() {
    let json_data = r#"{"departments": [{"name": "Sales", "employees": [{"salary": 50000}, {"salary": 55000}]}, {"name": "Engineering", "employees": [{"salary": 75000}, {"salary": 80000}, {"salary": 70000}]}]}"#;
    let (stdout, _stderr, code) = run_sk(&["=SUM(JQ(:arguments, \"$.departments[*].employees[*].salary\"))", "--json", json_data]).unwrap();
    assert_eq!(code, 0);
    assert_eq!(stdout, "Number(330000.0)");
}

#[test]
fn test_cli_jsonpath_with_filter() {
    let json_data = r#"{"products": [{"name": "A", "price": 10, "category": "electronics"}, {"name": "B", "price": 20, "category": "books"}, {"name": "C", "price": 30, "category": "electronics"}, {"name": "D", "price": 15, "category": "books"}]}"#;
    let (stdout, _stderr, code) = run_sk(&["=SUM(JQ(:arguments, \"$.products[?(@.category == 'electronics')].price\"))", "--json", json_data]).unwrap();
    assert_eq!(code, 0);
    assert_eq!(stdout, "Number(40.0)");
}

#[test]
fn test_cli_jsonpath_output_json() {
    let json_data = r#"{"accounts": [{"amount": 100.0}, {"amount": 200.0}]}"#;
    let (stdout, _stderr, code) = run_sk(&["=SUM(JQ(:arguments, \"$.accounts[*].amount\"))", "--json", json_data, "--output-json"]).unwrap();
    assert_eq!(code, 0);
    // Check that the output contains JSON structure with the result
    assert!(stdout.contains("\"result\""));
    assert!(stdout.contains("300"));
}

#[test]
fn test_cli_jsonpath_complex_expression() {
    let json_data = r#"{"sales": [{"amount": 100}, {"amount": 200}], "bonus": 50}"#;
    let (stdout, _stderr, code) = run_sk(&["=SUM(JQ(:arguments, \"$.sales[*].amount\")) + :bonus", "--json", json_data]).unwrap();
    assert_eq!(code, 0);
    assert_eq!(stdout, "Number(350.0)");
}