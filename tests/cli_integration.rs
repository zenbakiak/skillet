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