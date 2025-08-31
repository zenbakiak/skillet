use skillet::{evaluate_with_custom, evaluate_with_assignments, Value, JSPluginLoader};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{TcpListener, TcpStream};
use std::io::{BufRead, BufReader, Write};
use std::sync::{Arc, atomic::{AtomicU64, AtomicBool, Ordering}};
use std::time::Instant;

/// High-performance Skillet evaluation server
/// Eliminates process spawn overhead by keeping interpreter in memory
/// Supports concurrent request processing with connection pooling

#[derive(Debug, Deserialize)]
struct EvalRequest {
    expression: String,
    variables: Option<HashMap<String, serde_json::Value>>,
    output_json: Option<bool>,
}

#[derive(Debug, Serialize)]
struct EvalResponse {
    success: bool,
    result: Option<serde_json::Value>,
    error: Option<String>,
    execution_time_ms: f64,
    request_id: u64,
}

struct ServerStats {
    requests_processed: AtomicU64,
    total_execution_time: AtomicU64, // in microseconds
}

impl ServerStats {
    fn new() -> Self {
        Self {
            requests_processed: AtomicU64::new(0),
            total_execution_time: AtomicU64::new(0),
        }
    }
    
    fn record_request(&self, execution_time_us: u64) {
        self.requests_processed.fetch_add(1, Ordering::Relaxed);
        self.total_execution_time.fetch_add(execution_time_us, Ordering::Relaxed);
    }
    
    fn get_stats(&self) -> (u64, f64) {
        let count = self.requests_processed.load(Ordering::Relaxed);
        let total_time = self.total_execution_time.load(Ordering::Relaxed);
        let avg_time_ms = if count > 0 { 
            total_time as f64 / count as f64 / 1000.0 
        } else { 0.0 };
        (count, avg_time_ms)
    }
}

fn handle_client(mut stream: TcpStream, stats: Arc<ServerStats>, request_counter: Arc<AtomicU64>) {
    let reader = BufReader::new(stream.try_clone().unwrap());
    
    for line in reader.lines() {
        let line = match line {
            Ok(line) => line,
            Err(_) => break,
        };
        
        if line.trim().is_empty() {
            continue;
        }
        
        let request_id = request_counter.fetch_add(1, Ordering::Relaxed);
        let start_time = Instant::now();
        
        let response = match serde_json::from_str::<EvalRequest>(&line) {
            Ok(req) => process_request(req, request_id),
            Err(e) => EvalResponse {
                success: false,
                result: None,
                error: Some(format!("Invalid JSON request: {}", e)),
                execution_time_ms: 0.0,
                request_id,
            },
        };
        
        let execution_time = start_time.elapsed();
        stats.record_request(execution_time.as_micros() as u64);
        
        let response_json = serde_json::to_string(&response).unwrap_or_else(|_| {
            format!(r#"{{"success":false,"error":"Failed to serialize response","request_id":{}}}"#, request_id)
        });
        
        if let Err(_) = writeln!(stream, "{}", response_json) {
            break;
        }
        
        // Log request for monitoring
        if request_id % 1000 == 0 {
            let (total_requests, avg_time) = stats.get_stats();
            eprintln!("Processed {} requests, avg execution time: {:.2}ms", 
                total_requests, avg_time);
        }
    }
}

fn process_request(req: EvalRequest, request_id: u64) -> EvalResponse {
    let start_time = Instant::now();
    
    // Convert JSON variables to Skillet values
    let vars = match req.variables {
        Some(json_vars) => {
            let mut result = HashMap::new();
            for (key, value) in json_vars {
                match skillet::json_to_value(value) {
                    Ok(v) => { result.insert(key, v); }
                    Err(e) => {
                        return EvalResponse {
                            success: false,
                            result: None,
                            error: Some(format!("Error converting variable '{}': {}", key, e)),
                            execution_time_ms: start_time.elapsed().as_secs_f64() * 1000.0,
                            request_id,
                        };
                    }
                }
            }
            result
        }
        None => HashMap::new(),
    };
    
    // Evaluate expression
    let result = if req.expression.contains(";") || req.expression.contains(":=") {
        evaluate_with_assignments(&req.expression, &vars)
    } else {
        evaluate_with_custom(&req.expression, &vars)
    };
    
    let execution_time_ms = start_time.elapsed().as_secs_f64() * 1000.0;
    
    match result {
        Ok(val) => {
            let result_json = if req.output_json.unwrap_or(false) {
                // Format as structured JSON output
                let (result_value, type_name) = match val {
                    Value::Number(n) => (serde_json::json!(n), "Number"),
                    Value::String(s) => (serde_json::json!(s), "String"), 
                    Value::Boolean(b) => (serde_json::json!(b), "Boolean"),
                    Value::Currency(c) => (serde_json::json!(c), "Currency"),
                    Value::DateTime(dt) => (serde_json::json!(dt), "DateTime"),
                    Value::Array(arr) => {
                        let json_arr: Vec<serde_json::Value> = arr.iter().map(|v| match v {
                            Value::Number(n) => serde_json::json!(n),
                            Value::String(s) => serde_json::json!(s),
                            Value::Boolean(b) => serde_json::json!(b),
                            Value::Currency(c) => serde_json::json!(c),
                            Value::DateTime(dt) => serde_json::json!(dt),
                            Value::Null => serde_json::json!(null),
                            Value::Array(_) => serde_json::json!(format!("{:?}", v)),
                            Value::Json(s) => serde_json::from_str(&s).unwrap_or_else(|_| serde_json::json!(s)),
                        }).collect();
                        (serde_json::json!(json_arr), "Array")
                    },
                    Value::Null => (serde_json::json!(null), "Null"),
                    Value::Json(s) => {
                        match serde_json::from_str(&s) {
                            Ok(parsed) => (parsed, "Json"),
                            Err(_) => (serde_json::json!(s), "Json")
                        }
                    }
                };
                
                serde_json::json!({
                    "result": result_value,
                    "type": type_name,
                    "execution_time": format!("{:.2} ms", execution_time_ms)
                })
            } else {
                // Simple value output
                match val {
                    Value::Number(n) => serde_json::json!(n),
                    Value::String(s) => serde_json::json!(s),
                    Value::Boolean(b) => serde_json::json!(b),
                    Value::Currency(c) => serde_json::json!(c),
                    Value::DateTime(dt) => serde_json::json!(dt.to_string()),
                    Value::Array(arr) => {
                        let json_arr: Vec<serde_json::Value> = arr.iter().map(|v| match v {
                            Value::Number(n) => serde_json::json!(n),
                            Value::String(s) => serde_json::json!(s),
                            Value::Boolean(b) => serde_json::json!(b),
                            Value::Currency(c) => serde_json::json!(c),
                            Value::DateTime(dt) => serde_json::json!(dt.to_string()),
                            Value::Null => serde_json::json!(null),
                            Value::Array(_) => serde_json::json!(format!("{:?}", v)),
                            Value::Json(s) => serde_json::from_str(&s).unwrap_or_else(|_| serde_json::json!(s)),
                        }).collect();
                        serde_json::json!(json_arr)
                    },
                    Value::Null => serde_json::json!(null),
                    Value::Json(s) => serde_json::from_str(&s).unwrap_or_else(|_| serde_json::json!(s)),
                }
            };
            
            EvalResponse {
                success: true,
                result: Some(result_json),
                error: None,
                execution_time_ms,
                request_id,
            }
        }
        Err(e) => EvalResponse {
            success: false,
            result: None,
            error: Some(e.to_string()),
            execution_time_ms,
            request_id,
        },
    }
}

fn daemonize() -> Result<(), Box<dyn std::error::Error>> {
    use std::fs::OpenOptions;
    use std::os::unix::io::AsRawFd;
    
    // Fork the process
    match unsafe { libc::fork() } {
        -1 => return Err("Failed to fork process".into()),
        0 => {
            // Child process continues
        }
        _ => {
            // Parent process exits
            std::process::exit(0);
        }
    }
    
    // Create a new session
    if unsafe { libc::setsid() } == -1 {
        return Err("Failed to create new session".into());
    }
    
    // Fork again to ensure we're not a session leader
    match unsafe { libc::fork() } {
        -1 => return Err("Failed to fork second time".into()),
        0 => {
            // Grandchild continues
        }
        _ => {
            // Child exits
            std::process::exit(0);
        }
    }
    
    // DON'T change working directory to root - stay in current directory
    // This allows relative paths to work (like hooks directory)
    // std::env::set_current_dir("/").ok();
    
    // Close standard file descriptors
    unsafe {
        libc::close(libc::STDIN_FILENO);
        libc::close(libc::STDOUT_FILENO);
        libc::close(libc::STDERR_FILENO);
    }
    
    // Redirect stdin, stdout, stderr to /dev/null
    let dev_null = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/null")?;
    
    let null_fd = dev_null.as_raw_fd();
    unsafe {
        libc::dup2(null_fd, libc::STDIN_FILENO);
        libc::dup2(null_fd, libc::STDOUT_FILENO);
        libc::dup2(null_fd, libc::STDERR_FILENO);
    }
    
    Ok(())
}

fn write_pid_file(pid_file: &str) -> Result<(), Box<dyn std::error::Error>> {
    use std::fs::File;
    use std::io::Write;
    
    let pid = std::process::id();
    let mut file = File::create(pid_file)?;
    writeln!(file, "{}", pid)?;
    Ok(())
}

fn setup_signal_handlers() -> Arc<AtomicBool> {
    // Handle SIGTERM and SIGINT gracefully
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        eprintln!("Received shutdown signal, gracefully stopping...");
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting signal handler");
    running
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    
    if args.len() < 2 {
        eprintln!("Usage: sk_server <port> [num_threads] [options]");
        eprintln!("");
        eprintln!("Options:");
        eprintln!("  -d, --daemon         Run as daemon (background process)");
        eprintln!("  --pid-file <file>    Write PID to file (default: skillet-server.pid)");
        eprintln!("  --log-file <file>    Write logs to file (daemon mode only)");
        eprintln!("");
        eprintln!("Examples:");
        eprintln!("  sk_server 8080              # Start server on port 8080");
        eprintln!("  sk_server 8080 16           # Start with 16 worker threads");
        eprintln!("  sk_server 8080 8 -d         # Run as daemon with 8 threads");
        eprintln!("  sk_server 8080 -d --pid-file /var/run/skillet.pid");
        eprintln!("");
        eprintln!("Protocol: Send JSON requests as newline-delimited messages");
        eprintln!("Request format: {{\"expression\": \"=2+3\", \"variables\": {{\"x\": 10}}, \"output_json\": true}}");
        std::process::exit(1);
    }
    
    let port: u16 = args[1].parse().unwrap_or_else(|_| {
        eprintln!("Error: Invalid port number");
        std::process::exit(1);
    });
    
    // Parse arguments
    let mut num_threads: usize = num_cpus::get();
    let mut daemon_mode = false;
    let mut pid_file = "skillet-server.pid".to_string();
    let mut _log_file: Option<String> = None;
    let mut i = 2;
    
    while i < args.len() {
        match args[i].as_str() {
            "-d" | "--daemon" => {
                daemon_mode = true;
            }
            "--pid-file" => {
                if i + 1 < args.len() {
                    pid_file = args[i + 1].clone();
                    i += 1;
                } else {
                    eprintln!("Error: --pid-file requires a filename");
                    std::process::exit(1);
                }
            }
            "--log-file" => {
                if i + 1 < args.len() {
                    _log_file = Some(args[i + 1].clone());
                    i += 1;
                } else {
                    eprintln!("Error: --log-file requires a filename");
                    std::process::exit(1);
                }
            }
            arg => {
                // Try to parse as thread count if it's a number
                if let Ok(threads) = arg.parse::<usize>() {
                    num_threads = threads;
                } else {
                    eprintln!("Error: Unknown argument: {}", arg);
                    std::process::exit(1);
                }
            }
        }
        i += 1;
    }
    
    // Handle daemon mode before any output
    if daemon_mode {
        #[cfg(unix)]
        {
            // Print startup message before daemonizing
            eprintln!("Starting Skillet server as daemon...");
            eprintln!("Port: {}, Threads: {}, PID file: {}", port, num_threads, pid_file);
            
            if let Err(e) = daemonize() {
                eprintln!("Failed to daemonize: {}", e);
                std::process::exit(1);
            }
            
            // Write PID file after successful daemonization
            if let Err(_e) = write_pid_file(&pid_file) {
                // Log to syslog or a file since we can't use stderr
                std::process::exit(1);
            }
        }
        #[cfg(not(unix))]
        {
            eprintln!("Error: Daemon mode not supported on this platform");
            std::process::exit(1);
        }
    }
    
    // Setup signal handlers and running flag
    let running = setup_signal_handlers();
    
    // Auto-load JavaScript functions
    let hooks_dir = std::env::var("SKILLET_HOOKS_DIR").unwrap_or_else(|_| "hooks".to_string());
    let js_loader = JSPluginLoader::new(hooks_dir);
    
    match js_loader.auto_register() {
        Ok(count) => {
            if count > 0 && !daemon_mode {
                eprintln!("Loaded {} custom JavaScript function(s)", count);
            }
        }
        Err(e) => {
            if !daemon_mode {
                eprintln!("Warning: Failed to load JavaScript functions: {}", e);
            }
        }
    }
    
    let listener = TcpListener::bind(format!("127.0.0.1:{}", port))
        .unwrap_or_else(|e| {
            eprintln!("Error: Failed to bind to port {}: {}", port, e);
            std::process::exit(1);
        });

    // Make listener non-blocking so we can check shutdown flag
    listener
        .set_nonblocking(true)
        .unwrap_or_else(|e| {
            eprintln!("Error: Failed to set non-blocking mode: {}", e);
            std::process::exit(1);
        });
    
    let stats = Arc::new(ServerStats::new());
    let request_counter = Arc::new(AtomicU64::new(0));
    
    if !daemon_mode {
        eprintln!("🚀 Skillet Server started on port {}", port);
        eprintln!("📊 Worker threads: {}", num_threads);
        eprintln!("🔧 Ready for high-throughput expression evaluation");
        eprintln!("");
    }
    
    // Use a thread pool for handling connections
    let pool = threadpool::ThreadPool::new(num_threads);

    // Accept loop that can be interrupted by Ctrl+C
    while running.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _addr)) => {
                let stats = Arc::clone(&stats);
                let request_counter = Arc::clone(&request_counter);
                pool.execute(move || {
                    handle_client(stream, stats, request_counter);
                });
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No pending connections; sleep briefly and check again
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => {
                eprintln!("Error accepting connection: {}", e);
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    }

    // Wait for outstanding tasks to complete
    pool.join();
    eprintln!("Server shutdown complete.");
}
