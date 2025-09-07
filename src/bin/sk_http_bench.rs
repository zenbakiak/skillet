use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use std::thread;
use std::time::{Duration, Instant};
use std::io::{Read, Write};
use std::net::TcpStream;

/// Simple HTTP client for benchmarking the Skillet HTTP server
/// Usage: sk_http_bench <host:port> [options]

#[derive(Clone)]
struct BenchConfig {
    host: String,
    port: u16,
    concurrent: usize,
    requests: usize,
    duration: Option<Duration>,
    warmup: usize,
}

#[derive(Debug)]
struct BenchResult {
    success_count: u64,
    error_count: u64,
    total_time: Duration,
    min_latency: Duration,
    max_latency: Duration,
    avg_latency: Duration,
    throughput: f64,
}

struct TestCase {
    name: &'static str,
    expression: &'static str,
    expected_complexity: &'static str,
}

fn main() {
    println!("🚀 Skillet HTTP Server Benchmark");
    println!("=================================");
    println!();

    let args: Vec<String> = std::env::args().collect();
    let config = parse_args(&args).unwrap_or_else(|e| {
        eprintln!("Error: {}", e);
        print_usage();
        std::process::exit(1);
    });

    println!("Configuration:");
    println!("  Target: http://{}:{}", config.host, config.port);
    println!("  Concurrent connections: {}", config.concurrent);
    println!("  Requests per test: {}", config.requests);
    println!("  Warmup requests: {}", config.warmup);
    println!();

    // Check server health
    if !check_server_health(&config) {
        eprintln!("❌ Server health check failed");
        std::process::exit(1);
    }

    println!("✅ Server is healthy");
    println!();

    // Run benchmark suite
    run_benchmark_suite(&config);
}

fn parse_args(args: &[String]) -> Result<BenchConfig, String> {
    if args.len() < 2 {
        return Err("Missing host:port argument".to_string());
    }

    let host_port = &args[1];
    let (host, port) = if let Some(pos) = host_port.rfind(':') {
        let host = host_port[..pos].to_string();
        let port = host_port[pos + 1..].parse::<u16>()
            .map_err(|_| "Invalid port number".to_string())?;
        (host, port)
    } else {
        return Err("Host:port format required (e.g., 127.0.0.1:8080)".to_string());
    };

    let mut config = BenchConfig {
        host,
        port,
        concurrent: 10,
        requests: 100,
        duration: None,
        warmup: 20,
    };

    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "-c" | "--concurrent" => {
                if i + 1 < args.len() {
                    config.concurrent = args[i + 1].parse()
                        .map_err(|_| "Invalid concurrent value".to_string())?;
                    i += 1;
                } else {
                    return Err("--concurrent requires a value".to_string());
                }
            }
            "-n" | "--requests" => {
                if i + 1 < args.len() {
                    config.requests = args[i + 1].parse()
                        .map_err(|_| "Invalid requests value".to_string())?;
                    i += 1;
                } else {
                    return Err("--requests requires a value".to_string());
                }
            }
            "-w" | "--warmup" => {
                if i + 1 < args.len() {
                    config.warmup = args[i + 1].parse()
                        .map_err(|_| "Invalid warmup value".to_string())?;
                    i += 1;
                } else {
                    return Err("--warmup requires a value".to_string());
                }
            }
            _ => return Err(format!("Unknown argument: {}", args[i])),
        }
        i += 1;
    }

    Ok(config)
}

fn print_usage() {
    println!("Usage: sk_http_bench <host:port> [options]");
    println!();
    println!("Options:");
    println!("  -c, --concurrent <num>  Number of concurrent connections (default: 10)");
    println!("  -n, --requests <num>    Number of requests per test (default: 100)");
    println!("  -w, --warmup <num>      Number of warmup requests (default: 20)");
    println!();
    println!("Examples:");
    println!("  sk_http_bench 127.0.0.1:8080");
    println!("  sk_http_bench localhost:8080 -c 50 -n 1000");
}

fn check_server_health(config: &BenchConfig) -> bool {
    match make_request(&config.host, config.port, "GET /health HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n") {
        Ok(response) => response.contains("\"status\":\"healthy\""),
        Err(_) => false,
    }
}

fn make_request(host: &str, port: u16, request: &str) -> Result<String, std::io::Error> {
    let mut stream = TcpStream::connect((host, port))?;
    stream.set_read_timeout(Some(Duration::from_secs(10)))?;
    stream.set_write_timeout(Some(Duration::from_secs(5)))?;
    
    let formatted_request = request.replace("{}", host);
    stream.write_all(formatted_request.as_bytes())?;
    
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    
    Ok(response)
}

fn run_benchmark_suite(config: &BenchConfig) {
    // Warmup
    println!("🔥 Warming up with {} requests...", config.warmup);
    run_warmup(config);
    println!("✅ Warmup complete");
    println!();

    // Expression complexity tests
    run_expression_tests(config);
    
    // Concurrent performance tests
    run_concurrent_tests(config);
    
    // Variable inclusion tests
    run_variable_tests(config);
}

fn run_warmup(config: &BenchConfig) {
    let concurrent = std::cmp::min(config.warmup, 10);
    let requests_per_thread = config.warmup / concurrent;
    
    let mut handles = Vec::new();
    
    for _ in 0..concurrent {
        let config = config.clone();
        let handle = thread::spawn(move || {
            for _ in 0..requests_per_thread {
                let _ = make_eval_request(&config.host, config.port, "2+2");
                thread::sleep(Duration::from_millis(10));
            }
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
}

fn run_expression_tests(config: &BenchConfig) {
    println!("📊 Expression Performance Tests");
    println!("===============================");
    
    let test_cases = [
        TestCase { name: "Simple arithmetic", expression: "2+2", expected_complexity: "Low" },
        TestCase { name: "Mixed operations", expression: "10*5+3/2-1", expected_complexity: "Low" },
        TestCase { name: "Math functions", expression: "2^10+sqrt(144)*3", expected_complexity: "Medium" },
        TestCase { name: "Variables", expression: ":a:=10;:b:=20;:c:=:a*:b", expected_complexity: "Medium" },
        TestCase { name: "Range operations", expression: "range(1,50).sum()", expected_complexity: "High" },
        TestCase { name: "Complex arrays", expression: "[1,2,3,4,5,6,7,8,9,10].map(:x*2).filter(:x>10).sum()", expected_complexity: "High" },
    ];
    
    for test_case in &test_cases {
        println!("Testing: {} ({})", test_case.name, test_case.expected_complexity);
        
        let _start = Instant::now();
        let mut success_count = 0;
        let mut latencies = Vec::new();
        
        for _ in 0..10 {
            let req_start = Instant::now();
            match make_eval_request(&config.host, config.port, test_case.expression) {
                Ok(response) if response.contains("\"success\":true") => {
                    latencies.push(req_start.elapsed());
                    success_count += 1;
                }
                _ => {}
            }
        }
        
        if success_count > 0 {
            let avg_latency = latencies.iter().sum::<Duration>() / latencies.len() as u32;
            let min_latency = *latencies.iter().min().unwrap();
            let max_latency = *latencies.iter().max().unwrap();
            
            println!("  ✅ {}/{} successful", success_count, 10);
            println!("  📈 Latency - Avg: {:.2}ms, Min: {:.2}ms, Max: {:.2}ms", 
                avg_latency.as_secs_f64() * 1000.0,
                min_latency.as_secs_f64() * 1000.0,
                max_latency.as_secs_f64() * 1000.0);
        } else {
            println!("  ❌ All requests failed");
        }
        println!();
    }
}

fn run_concurrent_tests(config: &BenchConfig) {
    println!("🔄 Concurrent Performance Tests");
    println!("===============================");
    
    let concurrent_levels = [1, 5, 10, 20, 50];
    
    for &concurrent in &concurrent_levels {
        if concurrent > config.requests {
            continue;
        }
        
        println!("Testing with {} concurrent connections...", concurrent);
        
        let result = run_concurrent_benchmark(config, concurrent, "10*5+sqrt(25)");
        
        println!("  📊 Success: {}/{}", result.success_count, result.success_count + result.error_count);
        println!("  ⏱️  Latency - Avg: {:.2}ms, Min: {:.2}ms, Max: {:.2}ms",
            result.avg_latency.as_secs_f64() * 1000.0,
            result.min_latency.as_secs_f64() * 1000.0,
            result.max_latency.as_secs_f64() * 1000.0);
        println!("  🚀 Throughput: {:.1} req/sec", result.throughput);
        println!();
    }
}

fn run_variable_tests(config: &BenchConfig) {
    println!("📋 Variable Inclusion Performance");
    println!("=================================");
    
    let expression = ":a:=10;:b:=20;:c:=30;:d:=40;:result:=:a*:b+:c*:d";
    
    let scenarios = [
        ("No variables", "false"),
        ("All variables", "true"),
        ("Selected variables", ":a,:result"),
    ];
    
    for (name, include_vars) in &scenarios {
        println!("Testing: {} (include_variables: {})", name, include_vars);
        
        let mut total_time = Duration::new(0, 0);
        let mut total_size = 0;
        let mut success_count = 0;
        
        for _ in 0..10 {
            let start = Instant::now();
            match make_json_request(&config.host, config.port, expression, include_vars) {
                Ok(response) if response.contains("\"success\":true") => {
                    total_time += start.elapsed();
                    total_size += response.len();
                    success_count += 1;
                }
                _ => {}
            }
        }
        
        if success_count > 0 {
            let avg_time = total_time / success_count as u32;
            let avg_size = total_size / success_count as usize;
            println!("  ✅ Avg time: {:.2}ms, Avg response size: {} bytes",
                avg_time.as_secs_f64() * 1000.0, avg_size);
        } else {
            println!("  ❌ All requests failed");
        }
        println!();
    }
}

fn run_concurrent_benchmark(config: &BenchConfig, concurrent: usize, expression: &str) -> BenchResult {
    let total_requests = config.requests;
    let requests_per_thread = total_requests / concurrent;
    let extra_requests = total_requests % concurrent;
    
    let success_count = Arc::new(AtomicU64::new(0));
    let error_count = Arc::new(AtomicU64::new(0));
    let latencies = Arc::new(std::sync::Mutex::new(Vec::new()));
    
    let start_time = Instant::now();
    let mut handles = Vec::new();
    
    for i in 0..concurrent {
        let requests = if i < extra_requests {
            requests_per_thread + 1
        } else {
            requests_per_thread
        };
        
        let config = config.clone();
        let expression = expression.to_string();
        let success_count = Arc::clone(&success_count);
        let error_count = Arc::clone(&error_count);
        let latencies = Arc::clone(&latencies);
        
        let handle = thread::spawn(move || {
            for _ in 0..requests {
                let req_start = Instant::now();
                match make_eval_request(&config.host, config.port, &expression) {
                    Ok(response) if response.contains("\"success\":true") => {
                        let latency = req_start.elapsed();
                        latencies.lock().unwrap().push(latency);
                        success_count.fetch_add(1, Ordering::Relaxed);
                    }
                    _ => {
                        error_count.fetch_add(1, Ordering::Relaxed);
                    }
                }
            }
        });
        handles.push(handle);
    }
    
    for handle in handles {
        handle.join().unwrap();
    }
    
    let total_time = start_time.elapsed();
    let success_count = success_count.load(Ordering::Relaxed);
    let error_count = error_count.load(Ordering::Relaxed);
    
    let latency_vec = latencies.lock().unwrap();
    let (min_latency, max_latency, avg_latency) = if !latency_vec.is_empty() {
        let min = *latency_vec.iter().min().unwrap();
        let max = *latency_vec.iter().max().unwrap();
        let avg = latency_vec.iter().sum::<Duration>() / latency_vec.len() as u32;
        (min, max, avg)
    } else {
        (Duration::new(0, 0), Duration::new(0, 0), Duration::new(0, 0))
    };
    
    let throughput = if total_time.as_secs_f64() > 0.0 {
        success_count as f64 / total_time.as_secs_f64()
    } else {
        0.0
    };
    
    BenchResult {
        success_count,
        error_count,
        total_time,
        min_latency,
        max_latency,
        avg_latency,
        throughput,
    }
}

fn make_eval_request(host: &str, port: u16, expression: &str) -> Result<String, std::io::Error> {
    let encoded_expr = url_encode(expression);
    let request = format!(
        "GET /eval?expr={} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        encoded_expr, host
    );
    make_request(host, port, &request)
}

fn make_json_request(host: &str, port: u16, expression: &str, include_variables: &str) -> Result<String, std::io::Error> {
    let json_body = if include_variables == "true" || include_variables == "false" {
        format!("{{\"expression\":\"{}\",\"include_variables\":{}}}", expression, include_variables)
    } else {
        format!("{{\"expression\":\"{}\",\"include_variables\":\"{}\"}}", expression, include_variables)
    };
    
    let request = format!(
        "POST /eval HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        host, json_body.len(), json_body
    );
    make_request(host, port, &request)
}

fn url_encode(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            '+' => "%2B".to_string(),
            ':' => "%3A".to_string(),
            ';' => "%3B".to_string(),
            '=' => "%3D".to_string(),
            '*' => "%2A".to_string(),
            '(' => "%28".to_string(),
            ')' => "%29".to_string(),
            '[' => "%5B".to_string(),
            ']' => "%5D".to_string(),
            ',' => "%2C".to_string(),
            ' ' => "%20".to_string(),
            c if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' || c == '~' => c.to_string(),
            c => format!("%{:02X}", c as u8),
        })
        .collect()
}