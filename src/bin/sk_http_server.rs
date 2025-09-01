use skillet::{evaluate_with_custom, evaluate_with_assignments, Value, JSPluginLoader};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, atomic::{AtomicU64, AtomicBool, Ordering}};
use std::time::Instant;

/// HTTP-compatible Skillet evaluation server
/// Works with all standard HTTP clients

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

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
    version: String,
    requests_processed: u64,
    avg_execution_time_ms: f64,
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

fn sanitize_json_key(key: &str) -> String {
    key.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn handle_http_request(
    mut stream: TcpStream,
    stats: Arc<ServerStats>,
    request_counter: Arc<AtomicU64>,
    server_token: Arc<Option<String>>,
) {
    let mut buffer = [0; 8192];
    let bytes_read = match stream.read(&mut buffer) {
        Ok(size) => size,
        Err(_) => return,
    };

    let request = match std::str::from_utf8(&buffer[..bytes_read]) {
        Ok(req) => req,
        Err(_) => return,
    };

    // Parse HTTP request
    let lines: Vec<&str> = request.lines().collect();
    if lines.is_empty() {
        return;
    }

    let request_line = lines[0];
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 3 {
        send_http_error(&mut stream, 400, "Bad Request");
        return;
    }

    let method = parts[0];
    let path = parts[1];

    // Handle paths that might have query parameters
    let path_only = path.split('?').next().unwrap_or(path);

    match (method, path_only) {
        ("GET", "/health") => handle_health(&mut stream, &stats, request, server_token),
        ("GET", "/") => handle_root(&mut stream),
        ("POST", "/eval") => handle_eval_post(&mut stream, request, stats, request_counter, server_token),
        ("GET", "/eval") => handle_eval_get(&mut stream, request, stats, request_counter, server_token),
        ("OPTIONS", _) => handle_cors_preflight(&mut stream),
        _ => send_http_error(&mut stream, 404, "Not Found"),
    }
}

fn handle_health(
    stream: &mut TcpStream, 
    stats: &ServerStats, 
    request: &str, 
    server_token: Arc<Option<String>>
) {
    // Check authentication
    if let Some(error_response) = check_authentication(request, &server_token) {
        send_http_response(stream, 401, "application/json", &error_response);
        return;
    }

    let (requests, avg_time) = stats.get_stats();
    let response = HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        requests_processed: requests,
        avg_execution_time_ms: avg_time,
    };

    let json = serde_json::to_string(&response).unwrap_or_default();
    send_http_response(stream, 200, "application/json", &json);
}

fn handle_root(stream: &mut TcpStream) {
    let html = r#"<!DOCTYPE html>
<html>
<head>
    <title>Skillet Expression Server</title>
    <style>
        body { font-family: Arial, sans-serif; max-width: 800px; margin: 0 auto; padding: 20px; }
        .endpoint { background: #f5f5f5; padding: 10px; margin: 10px 0; border-radius: 5px; }
        code { background: #e8e8e8; padding: 2px 4px; border-radius: 3px; }
        pre { background: #f0f0f0; padding: 10px; border-radius: 5px; overflow-x: auto; }
    </style>
</head>
<body>
    <h1>🥘 Skillet Expression Server</h1>
    <p>A high-performance mathematical and logical expression evaluation server.</p>

    <h2>API Endpoints</h2>

    <div class="endpoint">
        <h3>GET /health</h3>
        <p>Server health check and statistics</p>
    </div>

    <div class="endpoint">
        <h3>POST /eval</h3>
        <p>Evaluate expressions via JSON POST request</p>
        <pre>{
  "expression": "=2 + 3 * 4",
  "variables": {"x": 10, "y": 20},
  "output_json": true
}</pre>
    </div>

    <div class="endpoint">
        <h3>GET /eval?expr=EXPRESSION&vars=JSON</h3>
        <p>Evaluate expressions via GET request</p>
        <p>Example: <code>/eval?expr=2+3*4&x=10&y=20</code></p>
    </div>

    <h2>Examples</h2>
    <pre># Health check
curl http://localhost:5074/health

# Simple evaluation
curl -X POST http://localhost:5074/eval \
  -H "Content-Type: application/json" \
  -d '{"expression": "=2 + 3 * 4"}'

# With variables
curl -X POST http://localhost:5074/eval \
  -H "Content-Type: application/json" \
  -d '{"expression": "=:x + :y", "variables": {"x": 10, "y": 20}}'

# GET request
curl "http://localhost:5074/eval?expr=2%2B3*4"</pre>
</body>
</html>"#;

    send_http_response(stream, 200, "text/html", html);
}

fn handle_cors_preflight(stream: &mut TcpStream) {
    let response = "HTTP/1.1 200 OK\r\n\
        Access-Control-Allow-Origin: *\r\n\
        Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
        Access-Control-Allow-Headers: Content-Type, Authorization\r\n\
        Content-Length: 0\r\n\
        \r\n";
    let _ = stream.write_all(response.as_bytes());
}

fn extract_auth_header(request: &str) -> Option<String> {
    // Look for Authorization header in request
    for line in request.lines() {
        let line = line.trim();
        if line.to_lowercase().starts_with("authorization:") {
            let auth_value = line[14..].trim(); // Skip "authorization:"
            if auth_value.to_lowercase().starts_with("bearer ") {
                return Some(auth_value[7..].trim().to_string()); // Skip "bearer "
            }
            // Also support direct token without "Bearer" prefix
            return Some(auth_value.to_string());
        }
    }
    None
}

fn check_authentication(request: &str, server_token: &Option<String>) -> Option<String> {
    if let Some(cfg_token) = server_token {
        let auth_token = extract_auth_header(request);
        let supplied = auth_token.as_deref().unwrap_or("");
        if supplied != cfg_token {
            let error_response = serde_json::json!({
                "success": false,
                "error": "Unauthorized: invalid token"
            });
            return Some(error_response.to_string());
        }
    }
    None
}

fn handle_eval_post(
    stream: &mut TcpStream,
    request: &str,
    stats: Arc<ServerStats>,
    request_counter: Arc<AtomicU64>,
    server_token: Arc<Option<String>>,
) {
    // Check authentication first
    if let Some(error_response) = check_authentication(request, &server_token) {
        send_http_response(stream, 401, "application/json", &error_response);
        return;
    }

    // Find the JSON body after headers
    let body_start = match request.find("\r\n\r\n") {
        Some(pos) => pos + 4,
        None => {
            send_http_error(stream, 400, "Invalid HTTP request");
            return;
        }
    };

    let body = &request[body_start..];
    let eval_request: EvalRequest = match serde_json::from_str(body) {
        Ok(req) => req,
        Err(e) => {
            send_http_error(stream, 400, &format!("Invalid JSON: {}", e));
            return;
        }
    };

    let response = process_eval_request(eval_request, stats, request_counter);
    let json = serde_json::to_string(&response).unwrap_or_default();
    send_http_response(stream, if response.success { 200 } else { 400 }, "application/json", &json);
}

fn handle_eval_get(
    stream: &mut TcpStream,
    request: &str,
    stats: Arc<ServerStats>,
    request_counter: Arc<AtomicU64>,
    server_token: Arc<Option<String>>,
) {
    // Check authentication first
    if let Some(error_response) = check_authentication(request, &server_token) {
        send_http_response(stream, 401, "application/json", &error_response);
        return;
    }

    let request_line = request.lines().next().unwrap_or("");
    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        send_http_error(stream, 400, "Bad Request");
        return;
    }

    let path_and_query = parts[1];
    let (_, query) = path_and_query.split_once('?').unwrap_or(("", ""));

    // Parse query parameters
    let mut expression = String::new();
    let mut variables = HashMap::new();
    let mut output_json = false;

    for param in query.split('&') {
        if let Some((key, value)) = param.split_once('=') {
            let decoded_value = urlencoding::decode(value).unwrap_or_default();
            match key {
                "expr" | "expression" => expression = decoded_value.to_string(),
                "output_json" => output_json = decoded_value == "true",
                _ => {
                    // Treat as variable
                    if let Ok(num) = decoded_value.parse::<f64>() {
                        variables.insert(key.to_string(), serde_json::json!(num));
                    } else if decoded_value == "true" {
                        variables.insert(key.to_string(), serde_json::json!(true));
                    } else if decoded_value == "false" {
                        variables.insert(key.to_string(), serde_json::json!(false));
                    } else {
                        variables.insert(key.to_string(), serde_json::json!(decoded_value.to_string()));
                    }
                }
            }
        }
    }

    if expression.is_empty() {
        send_http_error(stream, 400, "Missing expression parameter");
        return;
    }

    let eval_request = EvalRequest {
        expression,
        variables: if variables.is_empty() { None } else { Some(variables) },
        output_json: Some(output_json),
    };

    let response = process_eval_request(eval_request, stats, request_counter);
    let json = serde_json::to_string(&response).unwrap_or_default();
    send_http_response(stream, if response.success { 200 } else { 400 }, "application/json", &json);
}

fn process_eval_request(
    req: EvalRequest,
    stats: Arc<ServerStats>,
    request_counter: Arc<AtomicU64>,
) -> EvalResponse {
    let request_id = request_counter.fetch_add(1, Ordering::Relaxed);
    let start_time = Instant::now();

    // Convert JSON variables to Skillet values with key sanitization
    let vars = match req.variables {
        Some(json_vars) => {
            let mut result = HashMap::new();
            for (key, value) in json_vars {
                match skillet::json_to_value(value) {
                    Ok(v) => {
                        let sanitized_key = sanitize_json_key(&key);
                        result.insert(sanitized_key, v);
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
    let result = if req.expression.contains(";") || req.expression.contains(":=") {
        evaluate_with_assignments(&req.expression, &vars)
    } else {
        evaluate_with_custom(&req.expression, &vars)
    };

    let execution_time = start_time.elapsed();
    let execution_time_ms = execution_time.as_secs_f64() * 1000.0;
    stats.record_request(execution_time.as_micros() as u64);

    match result {
        Ok(val) => {
            let result_json = if req.output_json.unwrap_or(false) {
                format_structured_output(&val, execution_time_ms)
            } else {
                format_simple_output(&val)
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

fn format_structured_output(val: &Value, execution_time_ms: f64) -> serde_json::Value {
    let (result_value, type_name) = match val {
        Value::Number(n) => (serde_json::json!(n), "Number"),
        Value::String(s) => (serde_json::json!(s), "String"),
        Value::Boolean(b) => (serde_json::json!(b), "Boolean"),
        Value::Currency(c) => (serde_json::json!(c), "Currency"),
        Value::DateTime(dt) => (serde_json::json!(dt), "DateTime"),
        Value::Array(arr) => {
            let json_arr: Vec<serde_json::Value> = arr.iter().map(format_simple_output).collect();
            (serde_json::json!(json_arr), "Array")
        },
        Value::Null => (serde_json::json!(null), "Null"),
        Value::Json(s) => {
            match serde_json::from_str(s) {
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
}

fn format_simple_output(val: &Value) -> serde_json::Value {
    match val {
        Value::Number(n) => serde_json::json!(n),
        Value::String(s) => serde_json::json!(s),
        Value::Boolean(b) => serde_json::json!(b),
        Value::Currency(c) => serde_json::json!(c),
        Value::DateTime(dt) => serde_json::json!(dt.to_string()),
        Value::Array(arr) => {
            let json_arr: Vec<serde_json::Value> = arr.iter().map(format_simple_output).collect();
            serde_json::json!(json_arr)
        },
        Value::Null => serde_json::json!(null),
        Value::Json(s) => serde_json::from_str(s).unwrap_or_else(|_| serde_json::json!(s)),
    }
}

fn send_http_response(stream: &mut TcpStream, status: u16, content_type: &str, body: &str) {
    let status_text = match status {
        200 => "OK",
        400 => "Bad Request",
        404 => "Not Found",
        500 => "Internal Server Error",
        _ => "Unknown",
    };

    let response = format!(
        "HTTP/1.1 {} {}\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
         Access-Control-Allow-Headers: Content-Type, Authorization\r\n\
         Content-Type: {}\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        status, status_text, content_type, body.len(), body
    );

    let _ = stream.write_all(response.as_bytes());
}

fn send_http_error(stream: &mut TcpStream, status: u16, message: &str) {
    let error_json = serde_json::json!({
        "success": false,
        "error": message
    });
    send_http_response(stream, status, "application/json", &error_json.to_string());
}

fn setup_signal_handlers() -> Arc<AtomicBool> {
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
        eprintln!("Usage: sk_http_server <port> [options]");
        eprintln!("");
        eprintln!("Options:");
        eprintln!("  -H, --host <addr>    Bind host/interface (default: 127.0.0.1)");
        eprintln!("  --token <value>      Require token for requests");
        eprintln!("");
        eprintln!("Examples:");
        eprintln!("  sk_http_server 5074");
        eprintln!("  sk_http_server 5074 --host 0.0.0.0");
        eprintln!("  sk_http_server 5074 --host 0.0.0.0 --token secret123");
        eprintln!("");
        eprintln!("Endpoints:");
        eprintln!("  GET  /health          - Health check");
        eprintln!("  GET  /                - API documentation");
        eprintln!("  POST /eval            - Evaluate expressions (JSON)");
        eprintln!("  GET  /eval?expr=...   - Evaluate expressions (query params)");
        std::process::exit(1);
    }

    let port: u16 = args[1].parse().unwrap_or_else(|_| {
        eprintln!("Error: Invalid port number");
        std::process::exit(1);
    });

    let mut bind_host = "127.0.0.1".to_string();
    let mut auth_token: Option<String> = None;
    let mut i = 2;

    while i < args.len() {
        match args[i].as_str() {
            "-H" | "--host" => {
                if i + 1 < args.len() {
                    bind_host = args[i + 1].clone();
                    i += 1;
                } else {
                    eprintln!("Error: --host requires an address");
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
            _ => {
                eprintln!("Error: Unknown argument: {}", args[i]);
                std::process::exit(1);
            }
        }
        i += 1;
    }

    // Setup signal handlers
    let running = setup_signal_handlers();

    // Load JavaScript functions
    let hooks_dir = std::env::var("SKILLET_HOOKS_DIR").unwrap_or_else(|_| "hooks".to_string());
    let js_loader = JSPluginLoader::new(hooks_dir);

    match js_loader.auto_register() {
        Ok(count) => {
            if count > 0 {
                eprintln!("Loaded {} custom JavaScript function(s)", count);
            }
        }
        Err(e) => {
            eprintln!("Warning: Failed to load JavaScript functions: {}", e);
        }
    }

    // Start server
    let listener = TcpListener::bind(format!("{}:{}", bind_host, port))
        .unwrap_or_else(|e| {
            eprintln!("Error: Failed to bind to {}:{}: {}", bind_host, port, e);
            std::process::exit(1);
        });

    listener.set_nonblocking(true).unwrap_or_else(|e| {
        eprintln!("Error: Failed to set non-blocking mode: {}", e);
        std::process::exit(1);
    });

    let stats = Arc::new(ServerStats::new());
    let request_counter = Arc::new(AtomicU64::new(0));
    let server_token = Arc::new(auth_token.clone());

    eprintln!("🚀 Skillet HTTP Server started on http://{}:{}", bind_host, port);
    if auth_token.is_some() { eprintln!("🔒 Token auth: enabled"); }
    eprintln!("🌐 Ready for HTTP requests and Cloudflare tunneling");
    eprintln!("📖 Visit http://{}:{} for API documentation", bind_host, port);
    eprintln!("");

    // Accept loop
    while running.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _addr)) => {
                let stats = Arc::clone(&stats);
                let request_counter = Arc::clone(&request_counter);
                let server_token = Arc::clone(&server_token);

                std::thread::spawn(move || {
                    handle_http_request(stream, stats, request_counter, server_token);
                });
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            Err(e) => {
                eprintln!("Error accepting connection: {}", e);
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
    }

    eprintln!("Server shutdown complete.");
}