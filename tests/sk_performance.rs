use std::process::Command;
use std::time::{Duration, Instant};

/// Performance and concurrency testing for the sk command
/// 
/// To run these tests:
/// - Performance: cargo bench --bench sk_performance
/// - Concurrency: cargo test --test sk_concurrency --release

#[derive(Debug)]
struct BenchResult {
    operation: String,
    duration: Duration,
    iterations: usize,
    ops_per_sec: f64,
}

impl BenchResult {
    fn new(operation: String, duration: Duration, iterations: usize) -> Self {
        let ops_per_sec = iterations as f64 / duration.as_secs_f64();
        Self { operation, duration, iterations, ops_per_sec }
    }
}

fn run_sk_command(args: &[&str]) -> Result<(String, Duration), Box<dyn std::error::Error>> {
    let start = Instant::now();
    let output = Command::new("cargo")
        .args(&["run", "--release", "--bin", "sk", "--"])
        .args(args)
        .output()?;
    let duration = start.elapsed();
    
    if !output.status.success() {
        return Err(format!("Command failed: {}", String::from_utf8_lossy(&output.stderr)).into());
    }
    
    let stdout = String::from_utf8(output.stdout)?;
    Ok((stdout.trim().to_string(), duration))
}

fn benchmark_expression(expr: &str, vars: &[&str], iterations: usize) -> BenchResult {
    let mut total_duration = Duration::new(0, 0);
    let mut successful_runs = 0;
    
    println!("Benchmarking: {} (iterations: {})", expr, iterations);
    
    for i in 0..iterations {
        if i % (iterations / 10).max(1) == 0 {
            print!(".");
            std::io::Write::flush(&mut std::io::stdout()).unwrap();
        }
        
        let mut args = vec![expr];
        args.extend_from_slice(vars);
        
        match run_sk_command(&args) {
            Ok((_output, duration)) => {
                total_duration += duration;
                successful_runs += 1;
            }
            Err(e) => {
                eprintln!("\nError in iteration {}: {}", i, e);
            }
        }
    }
    
    println!(" Done!");
    BenchResult::new(expr.to_string(), total_duration, successful_runs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_benchmark_arithmetic_operations() {
        let tests = vec![
            ("Simple Addition", "=2 + 3", vec![]),
            ("Complex Expression", "=(2 + 3) * 4 - 1", vec![]),
            ("Power Operation", "=2 ^ 10", vec![]),
            ("Nested Parentheses", "=((2 + 3) * (4 - 1)) ^ 2", vec![]),
        ];
        
        let iterations = 10;
        let mut results = Vec::new();
        
        for (name, expr, vars) in tests {
            println!("\n=== {} ===", name);
            let result = benchmark_expression(expr, &vars, iterations);
            results.push(result);
        }
        
        println!("\n=== PERFORMANCE RESULTS ===");
        for result in &results {
            println!("{:<25} | {:>8.2} ms avg | {:>8.1} ops/sec", 
                result.operation, 
                result.duration.as_millis() as f64 / result.iterations as f64,
                result.ops_per_sec
            );
        }
    }

    #[test]
    fn test_benchmark_function_operations() {
        let tests = vec![
            ("SUM Function", "=SUM(1, 2, 3, 4, 5)", vec![]),
            ("Variable Lookup", "=SUM(:sales, :bonus)", vec!["sales=1000", "bonus=500"]),
            ("String Operations", "=:name.upper()", vec!["name=\"hello world\""]),
            ("Array Operations", "=:numbers.length()", vec!["numbers=[1,2,3,4,5,6,7,8,9,10]"]),
            ("Date Operations", "=TODAY()", vec![]),
        ];
        
        let iterations = 8;
        let mut results = Vec::new();
        
        for (name, expr, vars) in tests {
            println!("\n=== {} ===", name);
            let result = benchmark_expression(expr, &vars, iterations);
            results.push(result);
        }
        
        println!("\n=== FUNCTION PERFORMANCE RESULTS ===");
        for result in &results {
            println!("{:<25} | {:>8.2} ms avg | {:>8.1} ops/sec", 
                result.operation, 
                result.duration.as_millis() as f64 / result.iterations as f64,
                result.ops_per_sec
            );
        }
    }

    #[test]
    fn test_benchmark_json_operations() {
        let large_json = format!(r#"{{"items": [{}]}}"#, (0..100).map(|i| i.to_string()).collect::<Vec<_>>().join(","));
        let tests = vec![
            ("Simple JSON", "=:user.name", vec!["--json", r#"{"user": {"name": "Alice"}}"#]),
            ("Complex JSON", "=:data.values.length()", vec!["--json", r#"{"data": {"values": [1,2,3,4,5]}}"#]),
            ("Large JSON", "=:items.length()", vec!["--json", &large_json]),
        ];
        
        let iterations = 6;
        let mut results = Vec::new();
        
        for (name, expr, vars) in tests {
            println!("\n=== {} ===", name);
            let result = benchmark_expression(expr, &vars, iterations);
            results.push(result);
        }
        
        println!("\n=== JSON PERFORMANCE RESULTS ===");
        for result in &results {
            println!("{:<25} | {:>8.2} ms avg | {:>8.1} ops/sec", 
                result.operation, 
                result.duration.as_millis() as f64 / result.iterations as f64,
                result.ops_per_sec
            );
        }
    }

    #[test]
    fn test_memory_usage_test() {
        println!("=== MEMORY USAGE TEST ===");
        
        // Test with progressively larger expressions
        let sizes = vec![10, 50, 100, 500, 1000];
        
        for size in sizes {
            let expr = format!("=SUM({})", (1..=size).map(|i| i.to_string()).collect::<Vec<_>>().join(","));
            println!("\nTesting expression with {} numbers...", size);
            
            let _start = Instant::now();
            match run_sk_command(&[&expr]) {
                Ok((result, duration)) => {
                    println!("Size: {:<4} | Duration: {:>6.2}ms | Result: {}", 
                        size, duration.as_millis(), result);
                }
                Err(e) => {
                    println!("Failed at size {}: {}", size, e);
                    break;
                }
            }
        }
    }
}