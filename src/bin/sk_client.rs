use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::time::Instant;

#[derive(Debug, Serialize)]
struct EvalRequest {
    expression: String,
    variables: Option<HashMap<String, serde_json::Value>>,
    output_json: Option<bool>,
    token: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct EvalResponse {
    success: bool,
    result: Option<serde_json::Value>,
    error: Option<String>,
    execution_time_ms: f64,
    request_id: u64,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() < 3 {
        eprintln!("Usage: sk_client <host:port> <expression> [var=value ...]");
        eprintln!("       sk_client <host:port> <expression> --json '{{\"var\": \"value\"}}'");
        eprintln!("       sk_client <host:port> --benchmark <expression> [iterations]");
        eprintln!("");
        eprintln!("Examples:");
        eprintln!("  sk_client localhost:8080 '=2 + 3 * 4'");
        eprintln!("  sk_client localhost:8080 '=SUM(:sales, :bonus)' sales=1000 bonus=500");
        eprintln!("  sk_client localhost:8080 '=:user.name' --json '{{\"user\": {{\"name\": \"Alice\"}}}}'");
        eprintln!("  sk_client localhost:8080 --benchmark '=2+3*4' 1000");
        eprintln!("  sk_client localhost:8080 '=2+3' --token <secret>");
        std::process::exit(1);
    }
    
    let server_addr = &args[1];
    
    // Check for benchmark mode
    if args.len() > 3 && args[2] == "--benchmark" {
        // Parse benchmark options: expression [--json JSON] [var=val ...] [--output-json] [--token TOKEN] [iterations]
        let expression = args[3].clone();
        let mut variables = HashMap::new();
        let mut json_input: Option<String> = None;
        let mut output_json = false;
        let mut token: Option<String> = std::env::var("SKILLET_SERVER_TOKEN").ok();
        let mut iterations: usize = 100;

        let mut i = 4;
        while i < args.len() {
            let arg = &args[i];
            if arg == "--json" {
                if i + 1 >= args.len() { eprintln!("Error: --json flag requires a JSON string argument"); std::process::exit(1); }
                json_input = Some(args[i + 1].clone());
                i += 1;
            } else if arg == "--output-json" {
                output_json = true;
            } else if arg == "--token" {
                if i + 1 >= args.len() { eprintln!("Error: --token flag requires a value"); std::process::exit(1); }
                token = Some(args[i + 1].clone());
                i += 1;
            } else if let Some((name, value_str)) = arg.split_once('=') {
                let value = parse_value_to_json(value_str);
                variables.insert(name.to_string(), value);
            } else if let Ok(n) = arg.parse::<usize>() {
                iterations = n;
            } else {
                eprintln!("Invalid argument in benchmark: '{}'", arg);
                std::process::exit(1);
            }
            i += 1;
        }

        // Build request
        let request = if let Some(json_str) = json_input {
            let json_vars: Result<HashMap<String, serde_json::Value>, _> = serde_json::from_str(&json_str);
            match json_vars {
                Ok(vars) => EvalRequest { expression, variables: Some(vars), output_json: Some(output_json), token },
                Err(e) => { eprintln!("Error: Invalid JSON: {}", e); std::process::exit(1); }
            }
        } else if !variables.is_empty() {
            EvalRequest { expression, variables: Some(variables), output_json: Some(output_json), token }
        } else {
            EvalRequest { expression, variables: None, output_json: Some(output_json), token }
        };

        run_benchmark_with_request(server_addr, request, iterations);
        return;
    }
    
    let expression = &args[2];
    
    // Parse arguments
    let mut variables = HashMap::new();
    let mut json_input = None;
    let mut output_json = false;
    let mut token: Option<String> = std::env::var("SKILLET_SERVER_TOKEN").ok();
    let mut i = 3;
    
    while i < args.len() {
        let arg = &args[i];
        
        if arg == "--json" {
            if i + 1 >= args.len() {
                eprintln!("Error: --json flag requires a JSON string argument");
                std::process::exit(1);
            }
            json_input = Some(args[i + 1].clone());
            i += 1;
        } else if arg == "--output-json" {
            output_json = true;
        } else if arg == "--token" {
            if i + 1 >= args.len() {
                eprintln!("Error: --token flag requires a value");
                std::process::exit(1);
            }
            token = Some(args[i + 1].clone());
            i += 1;
        } else if let Some((name, value_str)) = arg.split_once('=') {
            let value = parse_value_to_json(value_str);
            variables.insert(name.to_string(), value);
        } else {
            eprintln!("Invalid argument: '{}'. Use format: var=value", arg);
            std::process::exit(1);
        }
        
        i += 1;
    }
    
    // Prepare request
    let request = if let Some(json_str) = json_input {
        let json_vars: Result<HashMap<String, serde_json::Value>, _> = serde_json::from_str(&json_str);
        match json_vars {
            Ok(vars) => EvalRequest {
                expression: expression.clone(),
                variables: Some(vars),
                output_json: Some(output_json),
                token: token.clone(),
            },
            Err(e) => {
                eprintln!("Error: Invalid JSON: {}", e);
                std::process::exit(1);
            }
        }
    } else if !variables.is_empty() {
        EvalRequest {
            expression: expression.clone(),
            variables: Some(variables),
            output_json: Some(output_json),
            token: token.clone(),
        }
    } else {
        EvalRequest {
            expression: expression.clone(),
            variables: None,
            output_json: Some(output_json),
            token: token.clone(),
        }
    };
    
    // Send request
    match send_request(server_addr, &request) {
        Ok(response) => {
            if response.success {
                if let Some(result) = response.result {
                    println!("{}", serde_json::to_string_pretty(&result).unwrap_or_else(|_| "null".to_string()));
                } else {
                    println!("null");
                }
            } else {
                eprintln!("Error: {}", response.error.unwrap_or_else(|| "Unknown error".to_string()));
                std::process::exit(2);
            }
        }
        Err(e) => {
            eprintln!("Connection error: {}", e);
            std::process::exit(3);
        }
    }
}

fn send_request(server_addr: &str, request: &EvalRequest) -> Result<EvalResponse, Box<dyn std::error::Error>> {
    let mut stream = TcpStream::connect(server_addr)?;
    
    let request_json = serde_json::to_string(request)?;
    writeln!(stream, "{}", request_json)?;
    
    let mut reader = BufReader::new(stream);
    let mut response_line = String::new();
    reader.read_line(&mut response_line)?;
    
    let response: EvalResponse = serde_json::from_str(&response_line)?;
    Ok(response)
}

fn run_benchmark_with_request(server_addr: &str, request: EvalRequest, iterations: usize) {
    println!("🚀 Benchmarking Skillet Server Performance");
    println!("==========================================");
    println!("Server: {}", server_addr);
    println!("Expression: {}", request.expression);
    println!("Iterations: {}", iterations);
    println!("");
    
    // Warmup
    print!("Warming up...");
    std::io::stdout().flush().unwrap();
    for _ in 0..10 {
        if let Err(e) = send_request(server_addr, &request) {
            eprintln!("\nWarmup failed: {}", e);
            std::process::exit(1);
        }
    }
    println!(" Done!");
    
    // Benchmark
    println!("Running benchmark...");
    let mut durations = Vec::new();
    let mut server_times = Vec::new();
    let mut successful = 0;
    let mut failed = 0;
    
    let total_start = Instant::now();
    
    for i in 0..iterations {
        if i % (iterations / 10).max(1) == 0 {
            print!(".");
            std::io::stdout().flush().unwrap();
        }
        
        let start = Instant::now();
        match send_request(server_addr, &request) {
            Ok(response) => {
                let duration = start.elapsed();
                durations.push(duration.as_millis() as f64);
                server_times.push(response.execution_time_ms);
                
                if response.success {
                    successful += 1;
                } else {
                    failed += 1;
                    if failed <= 5 { // Show first few errors
                        eprintln!("\nError in iteration {}: {}", i, response.error.unwrap_or_else(|| "Unknown".to_string()));
                    }
                }
            }
            Err(e) => {
                failed += 1;
                if failed <= 5 {
                    eprintln!("\nConnection error in iteration {}: {}", i, e);
                }
                durations.push(f64::MAX); // Mark as failed
                server_times.push(0.0);
            }
        }
    }
    
    let total_duration = total_start.elapsed();
    println!(" Done!");
    
    // Calculate statistics
    let valid_durations: Vec<f64> = durations.iter().filter(|&&d| d != f64::MAX).cloned().collect();
    let valid_server_times: Vec<f64> = server_times.iter().filter(|&&t| t > 0.0).cloned().collect();
    
    if valid_durations.is_empty() {
        eprintln!("All requests failed!");
        std::process::exit(1);
    }
    
    let avg_client_time = valid_durations.iter().sum::<f64>() / valid_durations.len() as f64;
    let avg_server_time = valid_server_times.iter().sum::<f64>() / valid_server_times.len() as f64;
    let min_client_time = valid_durations.iter().fold(f64::MAX, |a, &b| a.min(b));
    let max_client_time = valid_durations.iter().fold(0.0f64, |a, &b| a.max(b));
    let min_server_time = valid_server_times.iter().fold(f64::MAX, |a, &b| a.min(b));
    let max_server_time = valid_server_times.iter().fold(0.0f64, |a, &b| a.max(b));
    
    let throughput = successful as f64 / total_duration.as_secs_f64();
    let success_rate = successful as f64 / iterations as f64 * 100.0;
    
    // Calculate percentiles
    let mut sorted_durations = valid_durations.clone();
    sorted_durations.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p50 = percentile(&sorted_durations, 50.0);
    let p95 = percentile(&sorted_durations, 95.0);
    let p99 = percentile(&sorted_durations, 99.0);
    
    // Results
    println!("");
    println!("📊 BENCHMARK RESULTS");
    println!("====================");
    println!("Total requests: {}", iterations);
    println!("Successful: {}", successful);
    println!("Failed: {}", failed);
    println!("Success rate: {:.2}%", success_rate);
    println!("Total time: {:.2}s", total_duration.as_secs_f64());
    println!("Throughput: {:.1} requests/second", throughput);
    println!("");
    println!("Client-side latency (includes network):");
    println!("  Average: {:.2}ms", avg_client_time);
    println!("  Min: {:.2}ms", min_client_time);
    println!("  Max: {:.2}ms", max_client_time);
    println!("  P50: {:.2}ms", p50);
    println!("  P95: {:.2}ms", p95);
    println!("  P99: {:.2}ms", p99);
    println!("");
    println!("Server-side execution time:");
    println!("  Average: {:.2}ms", avg_server_time);
    println!("  Min: {:.2}ms", min_server_time);
    println!("  Max: {:.2}ms", max_server_time);
    println!("");
    println!("Network overhead: {:.2}ms average", avg_client_time - avg_server_time);
    
    // Performance comparison
    let improvement_factor = 250.0 / avg_server_time; // vs original 0.25s per operation
    println!("");
    println!("🎯 PERFORMANCE IMPROVEMENT");
    println!("==========================");
    println!("Original sk command: ~250ms per operation");
    println!("Server mode: {:.2}ms per operation", avg_server_time);
    println!("Improvement: {:.1}x faster", improvement_factor);
    println!("Estimated max throughput: {:.0} ops/second", 1000.0 / avg_server_time);
}


fn parse_value_to_json(s: &str) -> serde_json::Value {
    // Try parsing as different JSON types
    if (s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')) {
        serde_json::Value::String(s[1..s.len()-1].to_string())
    } else if s == "true" {
        serde_json::Value::Bool(true)
    } else if s == "false" {
        serde_json::Value::Bool(false)
    } else if s == "null" {
        serde_json::Value::Null
    } else if s.starts_with('[') && s.ends_with(']') {
        serde_json::from_str(s).unwrap_or_else(|_| serde_json::Value::String(s.to_string()))
    } else if let Ok(num) = s.parse::<f64>() {
        serde_json::Value::Number(serde_json::Number::from_f64(num).unwrap_or_else(|| serde_json::Number::from(0)))
    } else {
        serde_json::Value::String(s.to_string())
    }
}

fn percentile(sorted_data: &[f64], p: f64) -> f64 {
    let index = (p / 100.0 * (sorted_data.len() - 1) as f64) as usize;
    sorted_data[index.min(sorted_data.len() - 1)]
}
