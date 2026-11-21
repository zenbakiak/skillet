use serde::{Deserialize, Serialize};
#[cfg(feature = "plugins")]
use skillet::JSPluginLoader;
use skillet::{evaluate_with_assignments, evaluate_with_custom, Value};
use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc,
};
use std::time::Instant;

/// High-performance Skillet evaluation server
/// Eliminates process spawn overhead by keeping interpreter in memory
/// Supports concurrent request processing with connection pooling

#[derive(Debug, Deserialize)]
struct EvalRequest {
    expression: String,
    variables: Option<HashMap<String, serde_json::Value>>,
    output_json: Option<bool>,
    token: Option<String>,
}

#[derive(Debug, Serialize)]
struct EvalResponse {
    success: bool,
    result: Option<Wire>,
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
        self.total_execution_time
            .fetch_add(execution_time_us, Ordering::Relaxed);
    }

    fn get_stats(&self) -> (u64, f64) {
        let count = self.requests_processed.load(Ordering::Relaxed);
        let total_time = self.total_execution_time.load(Ordering::Relaxed);
        let avg_time_ms = if count > 0 {
            total_time as f64 / count as f64 / 1000.0
        } else {
            0.0
        };
        (count, avg_time_ms)
    }
}

#[path = "http_server/response_size.rs"]
mod response_size;

#[path = "http_server/request_limits.rs"]
mod request_limits;
use request_limits::{DeadlineReader, MAX_REQUEST_SIZE, WRITE_TIMEOUT};

#[path = "http_server/token_eq.rs"]
mod token_eq;
use token_eq::tokens_match;

/// Wall-clock limit for one evaluation (ADR-0011), same as `sk_http_server`. No flag or env var.
const SERVER_EVAL_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

fn server_limits() -> skillet::EvalLimits {
    let mut limits = skillet::EvalLimits::default();
    limits.timeout = Some(SERVER_EVAL_TIMEOUT);
    limits
}

/// One request line read from a connection.
enum Line {
    Request(String),
    /// Connection closed, idle until the deadline, or not UTF-8: close quietly.
    Closed,
    /// Over `MAX_REQUEST_SIZE` or timed out part-way: answer with this error, then close (the
    /// rest of the stream cannot be framed).
    Rejected(&'static str),
}

/// Read one `\n`-terminated line of at most `MAX_REQUEST_SIZE` bytes within
/// `REQUEST_READ_DEADLINE` (BL-033). Memory stays bounded whatever the client sends.
fn read_request_line(reader: &mut BufReader<DeadlineReader<'_>>, buf: &mut Vec<u8>) -> Line {
    buf.clear();
    let limit = MAX_REQUEST_SIZE as u64 + 1; // room for the newline
    match reader.by_ref().take(limit).read_until(b'\n', buf) {
        Ok(0) => return Line::Closed,
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::TimedOut && !buf.is_empty() => {
            return Line::Rejected("Request read timed out (max 30 s)");
        }
        Err(_) => return Line::Closed,
    }
    if buf.last() == Some(&b'\n') {
        buf.pop();
        if buf.last() == Some(&b'\r') {
            buf.pop();
        }
    } else if buf.len() > MAX_REQUEST_SIZE {
        return Line::Rejected("Request too large (max 1MB)");
    }
    match std::str::from_utf8(buf) {
        Ok(text) => Line::Request(text.to_string()),
        // As before (`BufRead::lines`): a line that is not UTF-8 ends the connection.
        Err(_) => Line::Closed,
    }
}

fn handle_client(
    stream: TcpStream,
    stats: Arc<ServerStats>,
    request_counter: Arc<AtomicU64>,
    server_token: Arc<Option<String>>,
) {
    let _ = stream.set_write_timeout(Some(WRITE_TIMEOUT));
    let mut reader = BufReader::new(DeadlineReader::new(&stream));
    let mut buf = Vec::new();

    loop {
        // Each request gets the full deadline, counted from when the server starts reading it.
        *reader.get_mut() = DeadlineReader::new(&stream);
        let line = match read_request_line(&mut reader, &mut buf) {
            Line::Request(line) => line,
            Line::Closed => break,
            Line::Rejected(message) => {
                let request_id = request_counter.fetch_add(1, Ordering::Relaxed);
                let response = EvalResponse {
                    success: false,
                    result: None,
                    error: Some(message.to_string()),
                    execution_time_ms: 0.0,
                    request_id,
                };
                if write_response(&stream, &response).is_ok() {
                    close_after_reject(&stream, reader.get_mut());
                }
                break;
            }
        };

        if line.trim().is_empty() {
            continue;
        }

        let request_id = request_counter.fetch_add(1, Ordering::Relaxed);
        let start_time = Instant::now();

        let response = match serde_json::from_str::<EvalRequest>(&line) {
            Ok(req) => {
                if let Some(cfg_token) = server_token.as_ref() {
                    let supplied = req.token.as_deref().unwrap_or("");
                    if !tokens_match(supplied, cfg_token) {
                        EvalResponse {
                            success: false,
                            result: None,
                            error: Some("Unauthorized: invalid token".to_string()),
                            execution_time_ms: 0.0,
                            request_id,
                        }
                    } else {
                        process_request(req, request_id)
                    }
                } else {
                    process_request(req, request_id)
                }
            }
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

        if write_response(&stream, &response).is_err() {
            break;
        }

        // Log request for monitoring
        if request_id % 1000 == 0 {
            let (total_requests, avg_time) = stats.get_stats();
            eprintln!(
                "Processed {} requests, avg execution time: {:.2}ms",
                total_requests, avg_time
            );
        }
    }
}

/// After rejecting a request, close so the client still receives the error: closing a socket
/// with unread input sends a reset, which can discard the response before the client reads it.
/// Send FIN, then discard input until the client closes or the request deadline passes.
fn close_after_reject(stream: &TcpStream, reader: &mut DeadlineReader<'_>) {
    let _ = stream.shutdown(std::net::Shutdown::Write);
    let mut sink = [0u8; 8192];
    while matches!(reader.read(&mut sink), Ok(n) if n > 0) {}
}

fn process_request(req: EvalRequest, request_id: u64) -> EvalResponse {
    let start_time = Instant::now();

    // Convert JSON variables to Skillet values
    let vars = match req.variables {
        Some(json_vars) => {
            let mut result = HashMap::new();
            for (key, value) in json_vars {
                match skillet::json_to_value(value) {
                    Ok(v) => {
                        result.insert(key, v);
                    }
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
    let result = skillet::with_limits(server_limits(), || {
        if req.expression.contains(";") || req.expression.contains(":=") {
            evaluate_with_assignments(&req.expression, &vars)
        } else {
            evaluate_with_custom(&req.expression, &vars)
        }
    });

    let execution_time_ms = start_time.elapsed().as_secs_f64() * 1000.0;

    // Size the response before building it: escaping can make it up to 6x the value.
    let result = result.and_then(|val| {
        response_size::check(response_size::json_size(&val, true))
            .map(|()| val)
            .map_err(|e| skillet::Error::new(e, None))
    });

    match result {
        Ok(val) => {
            let result_json = if req.output_json.unwrap_or(false) {
                // Structured output; keys in the order `serde_json::json!` sorted them.
                let type_name = type_name(&val);
                let mut fields = BTreeMap::new();
                fields.insert("result", to_wire(val, false));
                fields.insert("type", Wire::Json(serde_json::json!(type_name)));
                let time = format!("{:.2} ms", execution_time_ms);
                fields.insert("execution_time", Wire::Json(serde_json::json!(time)));
                Wire::Fields(fields)
            } else {
                to_wire(val, true)
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

/// The `result` of a response, serialized as it is written to the socket.
#[derive(Debug)]
enum Wire {
    Json(serde_json::Value),
    Array(Vec<Wire>),
    /// An array nested in an array result: sent as its `Debug` text in a JSON string, which is
    /// formatted straight into the JSON escaper and never built as a `String` (BL-027f).
    DebugText(Value),
    Fields(BTreeMap<&'static str, Wire>),
}

impl Serialize for Wire {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Wire::Json(v) => v.serialize(serializer),
            Wire::Array(items) => serializer.collect_seq(items),
            Wire::DebugText(v) => serializer.collect_str(&format_args!("{:?}", v)),
            Wire::Fields(fields) => serializer.collect_map(fields),
        }
    }
}

fn type_name(val: &Value) -> &'static str {
    match val {
        Value::Number(_) => "Number",
        Value::String(_) => "String",
        Value::Boolean(_) => "Boolean",
        Value::Currency(_) => "Currency",
        Value::DateTime(_) => "DateTime",
        Value::Array(_) => "Array",
        Value::Null => "Null",
        Value::Json(_) => "Json",
    }
}

/// A result in sk_server's wire format: arrays as JSON arrays whose nested arrays are `Debug`
/// text, `Json` re-parsed (or sent as a string when it does not parse). `datetime_text`: send
/// a `DateTime` as its text (simple output) instead of a number (`output_json`).
fn to_wire(val: Value, datetime_text: bool) -> Wire {
    match val {
        Value::Array(items) => Wire::Array(
            items
                .into_iter()
                .map(|v| match v {
                    Value::Array(_) => Wire::DebugText(v),
                    v => Wire::Json(scalar_json(v, datetime_text)),
                })
                .collect(),
        ),
        v => Wire::Json(scalar_json(v, datetime_text)),
    }
}

fn scalar_json(val: Value, datetime_text: bool) -> serde_json::Value {
    match val {
        Value::Number(n) => serde_json::json!(n),
        Value::String(s) => serde_json::json!(s),
        Value::Boolean(b) => serde_json::json!(b),
        Value::Currency(c) => serde_json::json!(c),
        Value::DateTime(dt) if datetime_text => serde_json::json!(dt.to_string()),
        Value::DateTime(dt) => serde_json::json!(dt),
        Value::Null => serde_json::json!(null),
        Value::Json(s) => serde_json::from_str(&s).unwrap_or(serde_json::Value::String(s)),
        // Arrays are handled by `to_wire`; kept total for the old format.
        Value::Array(_) => serde_json::Value::String(format!("{:?}", val)),
    }
}

/// Stream one response line to the client, with no copy of the response in memory.
fn write_response(stream: &TcpStream, response: &EvalResponse) -> std::io::Result<()> {
    let mut out = BufWriter::new(stream);
    serde_json::to_writer(&mut out, response)?;
    out.write_all(b"\n")?;
    out.flush()
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
    })
    .expect("Error setting signal handler");
    running
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: sk_server <port> [num_threads] [options]");
        eprintln!();
        eprintln!("Options:");
        eprintln!("  -d, --daemon         Run as daemon (background process)");
        eprintln!("  -H, --host <addr>    Bind host/interface (default: 127.0.0.1)");
        eprintln!("  --pid-file <file>    Write PID to file (default: skillet-server.pid)");
        eprintln!("  --log-file <file>    Write logs to file (daemon mode only)");
        eprintln!("  --token <value>      Require token for requests (or set SKILLET_AUTH_TOKEN)");
        eprintln!();
        eprintln!("Examples:");
        eprintln!("  sk_server 8080              # Start server on port 8080");
        eprintln!("  sk_server 8080 16           # Start with 16 worker threads");
        eprintln!("  sk_server 8080 8 -d         # Run as daemon with 8 threads");
        eprintln!("  sk_server 8080 --host 0.0.0.0   # Expose on all interfaces");
        eprintln!("  sk_server 8080 --host 0.0.0.0 --token <secret>");
        eprintln!("  sk_server 8080 -d --pid-file /var/run/skillet.pid");
        eprintln!();
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
    let mut bind_host: String =
        std::env::var("SKILLET_BIND_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let mut auth_token: Option<String> = std::env::var("SKILLET_AUTH_TOKEN").ok();
    let mut daemon_mode = false;
    let mut pid_file = "skillet-server.pid".to_string();
    let mut _log_file: Option<String> = None;
    let mut i = 2;

    while i < args.len() {
        match args[i].as_str() {
            "-d" | "--daemon" => {
                daemon_mode = true;
            }
            "-H" | "--host" => {
                if i + 1 < args.len() {
                    bind_host = args[i + 1].clone();
                    i += 1;
                } else {
                    eprintln!("Error: --host requires an address (e.g. 0.0.0.0)");
                    std::process::exit(1);
                }
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
            "--token" => {
                if i + 1 < args.len() {
                    auth_token = Some(args[i + 1].clone());
                    i += 1;
                } else {
                    eprintln!("Error: --token requires a value");
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
            eprintln!(
                "Port: {}, Threads: {}, PID file: {}",
                port, num_threads, pid_file
            );

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

    // Auto-load JavaScript functions (only with the `plugins` feature)
    #[cfg(feature = "plugins")]
    {
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
    }

    let listener = TcpListener::bind(format!("{}:{}", bind_host, port)).unwrap_or_else(|e| {
        eprintln!("Error: Failed to bind to {}:{}: {}", bind_host, port, e);
        std::process::exit(1);
    });

    // Make listener non-blocking so we can check shutdown flag
    listener.set_nonblocking(true).unwrap_or_else(|e| {
        eprintln!("Error: Failed to set non-blocking mode: {}", e);
        std::process::exit(1);
    });

    let stats = Arc::new(ServerStats::new());
    let request_counter = Arc::new(AtomicU64::new(0));

    if !daemon_mode {
        eprintln!("🚀 Skillet Server started on {}:{}", bind_host, port);
        eprintln!("📊 Worker threads: {}", num_threads);
        if auth_token.is_some() {
            eprintln!("🔒 Token auth: enabled");
        }
        eprintln!("🔧 Ready for high-throughput expression evaluation");
        eprintln!();
    }

    // Use a thread pool for handling connections. Workers get a 16 MB stack (default 2 MB)
    // so expressions up to MAX_NESTING_DEPTH have headroom in debug builds too (BL-017).
    let pool = threadpool::Builder::new()
        .num_threads(num_threads)
        .thread_stack_size(16 * 1024 * 1024)
        .build();

    // Accept loop that can be interrupted by Ctrl+C
    let server_token = Arc::new(auth_token);
    while running.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _addr)) => {
                // The listener is non-blocking; on macOS/BSD accepted sockets inherit that, and
                // a read that finds no data yet would end the connection instead of waiting.
                let _ = stream.set_nonblocking(false);
                let stats = Arc::clone(&stats);
                let request_counter = Arc::clone(&request_counter);
                let server_token = Arc::clone(&server_token);
                pool.execute(move || {
                    handle_client(stream, stats, request_counter, server_token);
                });
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                // No pending connections; sleep briefly and check again
                // Keep this very small to avoid per-connection latency
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            Err(e) => {
                eprintln!("Error accepting connection: {}", e);
                // Back off briefly on accept errors, but keep it tight
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
    }

    // Wait for outstanding tasks to complete
    pool.join();
    eprintln!("Server shutdown complete.");
}
