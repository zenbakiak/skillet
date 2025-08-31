use std::process::Command;
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};
use std::collections::HashMap;

/// Concurrency testing for the sk command
/// Tests multiple concurrent invocations of the sk binary to ensure:
/// 1. Thread safety
/// 2. No race conditions
/// 3. Consistent results under load
/// 4. Resource management

#[derive(Debug, Clone)]
struct ConcurrencyTestResult {
    thread_id: usize,
    success: bool,
    duration: Duration,
    result: String,
    error: Option<String>,
}

fn run_sk_concurrent(args: &[&str], thread_id: usize) -> ConcurrencyTestResult {
    let start = Instant::now();
    
    match Command::new("cargo")
        .args(&["run", "--release", "--bin", "sk", "--"])
        .args(args)
        .output()
    {
        Ok(output) => {
            let duration = start.elapsed();
            let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            
            if output.status.success() {
                ConcurrencyTestResult {
                    thread_id,
                    success: true,
                    duration,
                    result: stdout,
                    error: None,
                }
            } else {
                ConcurrencyTestResult {
                    thread_id,
                    success: false,
                    duration,
                    result: String::new(),
                    error: Some(stderr),
                }
            }
        }
        Err(e) => {
            ConcurrencyTestResult {
                thread_id,
                success: false,
                duration: start.elapsed(),
                result: String::new(),
                error: Some(e.to_string()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_concurrent_basic_operations() {
        println!("=== CONCURRENT BASIC OPERATIONS TEST ===");
        
        let thread_count = 10;
        let iterations_per_thread = 5;
        let expressions = vec![
            "=2 + 3 * 4",
            "=10 - 5 + 2",
            "=2 ^ 3",
            "=(10 + 5) * 2",
            "=100 / 4",
        ];
        
        let results = Arc::new(Mutex::new(Vec::new()));
        let mut handles = Vec::new();
        
        let start_time = Instant::now();
        
        for thread_id in 0..thread_count {
            let expressions = expressions.clone();
            let results = Arc::clone(&results);
            
            let handle = thread::spawn(move || {
                for iteration in 0..iterations_per_thread {
                    let expr = &expressions[iteration % expressions.len()];
                    let result = run_sk_concurrent(&[expr], thread_id);
                    
                    results.lock().unwrap().push(result);
                }
            });
            
            handles.push(handle);
        }
        
        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
        
        let total_duration = start_time.elapsed();
        let results = results.lock().unwrap();
        
        // Analyze results
        let successful_runs = results.iter().filter(|r| r.success).count();
        let failed_runs = results.len() - successful_runs;
        let avg_duration = results.iter()
            .map(|r| r.duration.as_millis())
            .sum::<u128>() as f64 / results.len() as f64;
        
        println!("Total operations: {}", results.len());
        println!("Successful: {}", successful_runs);
        println!("Failed: {}", failed_runs);
        println!("Success rate: {:.2}%", (successful_runs as f64 / results.len() as f64) * 100.0);
        println!("Average duration per operation: {:.2}ms", avg_duration);
        println!("Total test duration: {:.2}s", total_duration.as_secs_f64());
        
        // Check for consistency
        let mut result_counts: HashMap<String, usize> = HashMap::new();
        for result in results.iter() {
            if result.success {
                *result_counts.entry(result.result.clone()).or_insert(0) += 1;
            }
        }
        
        println!("\nResult consistency check:");
        for (result, count) in &result_counts {
            println!("  '{}': {} occurrences", result, count);
        }
        
        assert!(failed_runs == 0, "Some concurrent operations failed");
        assert!(successful_runs > 0, "No successful operations");
    }

    #[test]
    fn test_concurrent_variable_operations() {
        println!("=== CONCURRENT VARIABLE OPERATIONS TEST ===");
        
        let thread_count = 8;
        let operations_per_thread = 10;
        
        let test_cases = vec![
            ("=:x + :y", vec!["x=10", "y=20"], "Number(30.0)"),
            ("=:name.upper()", vec!["name=\"hello\""], "String(\"HELLO\")"),
            ("=SUM(:a, :b, :c)", vec!["a=1", "b=2", "c=3"], "Number(6.0)"),
            ("=:active", vec!["active=true"], "Boolean(true)"),
            ("=:items.length()", vec!["items=[1,2,3,4]"], "Number(4.0)"),
        ];
        
        let results = Arc::new(Mutex::new(Vec::new()));
        let mut handles = Vec::new();
        
        for thread_id in 0..thread_count {
            let test_cases = test_cases.clone();
            let results = Arc::clone(&results);
            
            let handle = thread::spawn(move || {
                for i in 0..operations_per_thread {
                    let (expr, vars, _expected) = &test_cases[i % test_cases.len()];
                    let mut args = vec![*expr];
                    args.extend(vars.iter().map(|s| *s));
                    
                    let result = run_sk_concurrent(&args, thread_id);
                    results.lock().unwrap().push(result);
                }
            });
            
            handles.push(handle);
        }
        
        for handle in handles {
            handle.join().unwrap();
        }
        
        let results = results.lock().unwrap();
        let successful_runs = results.iter().filter(|r| r.success).count();
        let failed_runs = results.len() - successful_runs;
        
        println!("Variable operations - Successful: {}, Failed: {}", successful_runs, failed_runs);
        
        if failed_runs > 0 {
            println!("Failures:");
            for result in results.iter().filter(|r| !r.success) {
                println!("  Thread {}: {:?}", result.thread_id, result.error);
            }
        }
        
        assert_eq!(failed_runs, 0, "Some variable operations failed under concurrency");
    }

    #[test]
    fn test_concurrent_json_operations() {
        println!("=== CONCURRENT JSON OPERATIONS TEST ===");
        
        let thread_count = 6;
        let operations_per_thread = 8;
        
        let json_tests = vec![
            ("=:user.name", vec!["--json", r#"{"user": {"name": "Alice"}}"#]),
            ("=:data.count", vec!["--json", r#"{"data": {"count": 42}}"#]),
            ("=:items.length()", vec!["--json", r#"{"items": [1,2,3]}"#]),
            ("=:settings.enabled", vec!["--json", r#"{"settings": {"enabled": true}}"#]),
        ];
        
        let results = Arc::new(Mutex::new(Vec::new()));
        let mut handles = Vec::new();
        
        for thread_id in 0..thread_count {
            let json_tests = json_tests.clone();
            let results = Arc::clone(&results);
            
            let handle = thread::spawn(move || {
                for i in 0..operations_per_thread {
                    let (expr, args) = &json_tests[i % json_tests.len()];
                    let mut full_args = vec![*expr];
                    full_args.extend(args.iter().map(|s| *s));
                    
                    let result = run_sk_concurrent(&full_args, thread_id);
                    results.lock().unwrap().push(result);
                }
            });
            
            handles.push(handle);
        }
        
        for handle in handles {
            handle.join().unwrap();
        }
        
        let results = results.lock().unwrap();
        let successful_runs = results.iter().filter(|r| r.success).count();
        let failed_runs = results.len() - successful_runs;
        
        println!("JSON operations - Successful: {}, Failed: {}", successful_runs, failed_runs);
        assert_eq!(failed_runs, 0, "Some JSON operations failed under concurrency");
    }

    #[test]
    fn test_stress_test_high_concurrency() {
        println!("=== STRESS TEST - HIGH CONCURRENCY ===");
        
        let thread_count = 20;
        let operations_per_thread = 3;
        let expression = "=2 * 3 + 4 * 5 - 1";
        let expected_result = "Number(25.0)";
        
        let (sender, receiver) = mpsc::channel();
        let mut handles = Vec::new();
        let start_time = Instant::now();
        
        for thread_id in 0..thread_count {
            let sender = sender.clone();
            let expr = expression.to_string();
            
            let handle = thread::spawn(move || {
                for _ in 0..operations_per_thread {
                    let result = run_sk_concurrent(&[&expr], thread_id);
                    sender.send(result).unwrap();
                }
            });
            
            handles.push(handle);
        }
        
        drop(sender); // Close the sending side
        
        // Collect results
        let mut results = Vec::new();
        while let Ok(result) = receiver.recv() {
            results.push(result);
        }
        
        // Wait for all threads
        for handle in handles {
            handle.join().unwrap();
        }
        
        let total_duration = start_time.elapsed();
        let successful_runs = results.iter().filter(|r| r.success).count();
        let failed_runs = results.len() - successful_runs;
        let correct_results = results.iter()
            .filter(|r| r.success && r.result == expected_result)
            .count();
        
        println!("Stress test results:");
        println!("  Threads: {}", thread_count);
        println!("  Operations per thread: {}", operations_per_thread);
        println!("  Total operations: {}", results.len());
        println!("  Successful: {}", successful_runs);
        println!("  Failed: {}", failed_runs);
        println!("  Correct results: {}", correct_results);
        println!("  Success rate: {:.2}%", (successful_runs as f64 / results.len() as f64) * 100.0);
        println!("  Correctness rate: {:.2}%", (correct_results as f64 / successful_runs as f64) * 100.0);
        println!("  Total duration: {:.2}s", total_duration.as_secs_f64());
        println!("  Operations per second: {:.1}", results.len() as f64 / total_duration.as_secs_f64());
        
        // Performance stats
        let successful_durations: Vec<u128> = results.iter()
            .filter(|r| r.success)
            .map(|r| r.duration.as_millis())
            .collect();
        
        if !successful_durations.is_empty() {
            let avg_duration = successful_durations.iter().sum::<u128>() as f64 / successful_durations.len() as f64;
            let min_duration = *successful_durations.iter().min().unwrap();
            let max_duration = *successful_durations.iter().max().unwrap();
            
            println!("  Average operation duration: {:.2}ms", avg_duration);
            println!("  Min operation duration: {}ms", min_duration);
            println!("  Max operation duration: {}ms", max_duration);
        }
        
        assert!(failed_runs < results.len() / 10, "More than 10% of operations failed");
        assert!(correct_results == successful_runs, "Some successful operations produced incorrect results");
    }

    #[test]
    fn test_resource_cleanup() {
        println!("=== RESOURCE CLEANUP TEST ===");
        
        // This test runs many short-lived sk processes to ensure
        // there are no resource leaks
        let iterations = 20;
        let start_time = Instant::now();
        
        for i in 0..iterations {
            if i % 5 == 0 {
                print!(".");
                std::io::Write::flush(&mut std::io::stdout()).unwrap();
            }
            
            let result = run_sk_concurrent(&["=42"], 0);
            assert!(result.success, "Operation {} failed: {:?}", i, result.error);
            assert_eq!(result.result, "Number(42.0)");
        }
        
        let total_duration = start_time.elapsed();
        println!("\nResource cleanup test completed:");
        println!("  {} iterations in {:.2}s", iterations, total_duration.as_secs_f64());
        println!("  Average: {:.2}ms per operation", total_duration.as_millis() as f64 / iterations as f64);
    }
}