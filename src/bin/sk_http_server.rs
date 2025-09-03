use skillet::{evaluate_with_custom, evaluate_with_assignments, evaluate_with_assignments_and_context, Value, JSPluginLoader, CustomFunction};
use skillet::js_plugin::JavaScriptFunction;
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
    #[serde(deserialize_with = "deserialize_expression")]
    expression: String,
    arguments: Option<HashMap<String, serde_json::Value>>,
    output_json: Option<bool>,
    include_variables: Option<bool>,
}

fn deserialize_expression<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, Visitor};
    use std::fmt;

    struct ExpressionVisitor;

    impl<'de> Visitor<'de> for ExpressionVisitor {
        type Value = String;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string or array of strings")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(value.to_string())
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: de::SeqAccess<'de>,
        {
            let mut expressions = Vec::new();

            while let Some(expr) = seq.next_element::<String>()? {
                expressions.push(expr);
            }

            Ok(expressions.join(""))
        }
    }

    deserializer.deserialize_any(ExpressionVisitor)
}

#[derive(Debug, Serialize)]
struct EvalResponse {
    success: bool,
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    variables: Option<HashMap<String, serde_json::Value>>,
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

#[derive(Debug, Deserialize)]
struct UploadJSRequest {
    filename: String,
    js_code: String,
}

#[derive(Debug, Serialize)]
struct UploadJSResponse {
    success: bool,
    message: String,
    function_name: Option<String>,
    validation_results: Option<ValidationResults>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct ValidationResults {
    syntax_valid: bool,
    structure_valid: bool,
    example_test_passed: bool,
    example_result: Option<String>,
    example_error: Option<String>,
}

#[derive(Debug, Serialize)]
struct ReloadHooksResponse {
    success: bool,
    message: String,
    functions_loaded: usize,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct UpdateJSRequest {
    filename: String,
    js_code: String,
}

#[derive(Debug, Serialize)]
struct UpdateJSResponse {
    success: bool,
    message: String,
    function_name: Option<String>,
    validation_results: Option<ValidationResults>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeleteJSRequest {
    filename: String,
}

#[derive(Debug, Serialize)]
struct DeleteJSResponse {
    success: bool,
    message: String,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct JSFunctionInfo {
    filename: String,
    function_name: Option<String>,
    description: Option<String>,
    example: Option<String>,
    min_args: Option<usize>,
    max_args: Option<usize>,
    file_size: u64,
    last_modified: String,
    is_valid: bool,
    validation_error: Option<String>,
}

#[derive(Debug, Serialize)]
struct ListJSResponse {
    success: bool,
    functions: Vec<JSFunctionInfo>,
    total_count: usize,
    error: Option<String>,
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

fn read_complete_http_request(stream: &mut TcpStream) -> Result<String, std::io::Error> {
    let mut buffer = Vec::new();
    let mut temp_buffer = [0; 1024];
    let mut headers_complete = false;
    let mut content_length: usize = 0;
    let mut headers_end_pos = 0;

    // First, read until we have complete headers
    while !headers_complete {
        let bytes_read = stream.read(&mut temp_buffer)?;
        if bytes_read == 0 {
            break;
        }

        buffer.extend_from_slice(&temp_buffer[..bytes_read]);

        // Check if we have complete headers (ending with \r\n\r\n)
        if let Some(pos) = find_headers_end(&buffer) {
            headers_complete = true;
            headers_end_pos = pos + 4;

            // Parse the headers to get Content-Length
            let headers_str = String::from_utf8_lossy(&buffer[..pos]);
            content_length = parse_content_length(&headers_str);
        }
    }

    // Now read the remaining body if needed
    let body_bytes_read = buffer.len() - headers_end_pos;
    let remaining_bytes = content_length.saturating_sub(body_bytes_read);

    if remaining_bytes > 0 {
        let mut body_buffer = vec![0; remaining_bytes];
        let mut total_read = 0;

        while total_read < remaining_bytes {
            let bytes_read = stream.read(&mut body_buffer[total_read..])?;
            if bytes_read == 0 {
                break;
            }
            total_read += bytes_read;
        }

        buffer.extend_from_slice(&body_buffer[..total_read]);
    }

    String::from_utf8(buffer).map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid UTF-8"))
}

fn find_headers_end(buffer: &[u8]) -> Option<usize> {
    let pattern = b"\r\n\r\n";
    buffer.windows(pattern.len()).position(|window| window == pattern)
}

fn parse_content_length(headers: &str) -> usize {
    for line in headers.lines() {
        if line.to_lowercase().starts_with("content-length:") {
            if let Some(value) = line.split(':').nth(1) {
                return value.trim().parse().unwrap_or(0);
            }
        }
    }
    0
}

fn handle_http_request(
    mut stream: TcpStream,
    stats: Arc<ServerStats>,
    request_counter: Arc<AtomicU64>,
    server_token: Arc<Option<String>>,
    server_admin_token: Arc<Option<String>>,
) {
    // Read the complete HTTP request properly
    let request = match read_complete_http_request(&mut stream) {
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
        ("GET", "/health") => handle_health(&mut stream, &stats, &request, server_token),
        ("GET", "/") => handle_root(&mut stream),
        ("POST", "/eval") => handle_eval_post(&mut stream, &request, stats, request_counter, server_token),
        ("GET", "/eval") => handle_eval_get(&mut stream, &request, stats, request_counter, server_token),
        ("POST", "/upload-js") => handle_upload_js(&mut stream, &request, server_admin_token),
        ("PUT", "/update-js") => handle_update_js(&mut stream, &request, server_admin_token),
        ("DELETE", "/delete-js") => handle_delete_js(&mut stream, &request, server_admin_token),
        ("GET", "/list-js") => handle_list_js(&mut stream, &request, server_admin_token),
        ("POST", "/reload-hooks") => handle_reload_hooks(&mut stream, &request, server_admin_token),
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
    <h1>Skillet Expression Server</h1>
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
  "arguments": {"x": 10, "y": 20},
  "output_json": true
}</pre>
    </div>

    <div class="endpoint">
        <h3>GET /eval?expr=EXPRESSION&vars=JSON</h3>
        <p>Evaluate expressions via GET request</p>
        <p>Example: <code>/eval?expr=2+3*4&x=10&y=20</code></p>
    </div>

    <div class="endpoint">
        <h3>POST /upload-js</h3>
        <p>Upload and validate JavaScript functions</p>
        <p><strong>⚠️ Requires admin token authentication</strong></p>
        <pre>{
  "filename": "myfunction.js",
  "js_code": "// @name: MYFUNCTION\n// @min_args: 1\n// @max_args: 1\n// @example: MYFUNCTION(5) returns 10\nfunction execute(args) { return args[0] * 2; }"
}</pre>
    </div>

    <div class="endpoint">
        <h3>GET /list-js</h3>
        <p>List all JavaScript functions in hooks directory</p>
        <p>Returns detailed information about each function including validation status</p>
        <p><strong>⚠️ Requires admin token authentication</strong></p>
    </div>

    <div class="endpoint">
        <h3>PUT /update-js</h3>
        <p>Update an existing JavaScript function</p>
        <p><strong>⚠️ Requires admin token authentication</strong></p>
        <pre>{
  "filename": "myfunction.js",
  "js_code": "// @name: MYFUNCTION\n// @min_args: 1\n// @max_args: 1\n// @example: MYFUNCTION(10) returns 20\nfunction execute(args) { return args[0] * 2; }"
}</pre>
    </div>

    <div class="endpoint">
        <h3>DELETE /delete-js</h3>
        <p>Delete a JavaScript function file</p>
        <p><strong>⚠️ Requires admin token authentication</strong></p>
        <pre>{
  "filename": "myfunction.js"
}</pre>
    </div>

    <div class="endpoint">
        <h3>POST /reload-hooks</h3>
        <p>Reload all JavaScript functions from hooks directory</p>
        <p><strong>⚠️ Requires admin token authentication</strong></p>
        <pre>{}</pre>
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
  -d '{"expression": "=:x + :y", "arguments": {"x": 10, "y": 20}}'

# GET request
curl "http://localhost:5074/eval?expr=2%2B3*4"

# JavaScript function management (requires admin token)
# List all JS functions
curl -H "Authorization: admin456" http://localhost:5074/list-js

# Upload a JS function
curl -X POST http://localhost:5074/upload-js \
  -H "Content-Type: application/json" \
  -H "Authorization: admin456" \
  -d '{"filename": "double.js", "js_code": "// @name: DOUBLE\n// @example: DOUBLE(5) returns 10\nfunction execute(args) { return args[0] * 2; }"}'

# Update a JS function
curl -X PUT http://localhost:5074/update-js \
  -H "Content-Type: application/json" \
  -H "Authorization: admin456" \
  -d '{"filename": "double.js", "js_code": "// @name: DOUBLE\n// @example: DOUBLE(5) returns 20\nfunction execute(args) { return args[0] * 4; }"}'

# Delete a JS function
curl -X DELETE http://localhost:5074/delete-js \
  -H "Content-Type: application/json" \
  -H "Authorization: admin456" \
  -d '{"filename": "double.js"}'

# Reload hooks
curl -X POST http://localhost:5074/reload-hooks \
  -H "Content-Type: application/json" \
  -H "Authorization: admin456" \
  -d '{}'</pre>
</body>
</html>"#;

    send_http_response(stream, 200, "text/html", html);
}

fn handle_cors_preflight(stream: &mut TcpStream) {
    let response = "HTTP/1.1 200 OK\r\n\
        Access-Control-Allow-Origin: *\r\n\
        Access-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS\r\n\
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

fn check_admin_authentication(request: &str, server_admin_token: &Option<String>) -> Option<String> {
    if let Some(cfg_admin_token) = server_admin_token {
        let auth_token = extract_auth_header(request);
        let supplied = auth_token.as_deref().unwrap_or("");
        if supplied != cfg_admin_token {
            let error_response = serde_json::json!({
                "success": false,
                "error": "Unauthorized: invalid admin token. JavaScript function management requires admin authentication."
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
    let mut include_variables = false;

    for param in query.split('&') {
        if let Some((key, value)) = param.split_once('=') {
            let decoded_value = urlencoding::decode(value).unwrap_or_default();
            match key {
                "expr" | "expression" => expression = decoded_value.to_string(),
                "output_json" => output_json = decoded_value == "true",
                "include_variables" => include_variables = decoded_value == "true",
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
        arguments: if variables.is_empty() { None } else { Some(variables) },
        output_json: Some(output_json),
        include_variables: Some(include_variables),
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
    let vars = match req.arguments {
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
                            variables: None,
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
    let (result, variable_context) = if req.expression.contains(";") || req.expression.contains(":=") {
        // Use the new function that returns both result and variable context
        if req.include_variables.unwrap_or(false) {
            match evaluate_with_assignments_and_context(&req.expression, &vars) {
                Ok((val, ctx)) => (Ok(val), Some(ctx)),
                Err(e) => (Err(e), None),
            }
        } else {
            (evaluate_with_assignments(&req.expression, &vars), None)
        }
    } else {
        (evaluate_with_custom(&req.expression, &vars), None)
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

            // Convert variable context to JSON if requested
            let variables_json = if let Some(ctx) = variable_context {
                let mut json_vars = HashMap::new();
                for (key, value) in ctx {
                    // Include all variables that were assigned during evaluation
                    // Skip initial arguments that haven't changed
                    if !vars.contains_key(&key) || vars.get(&key) != Some(&value) {
                        json_vars.insert(key, format_simple_output(&value));
                    }
                }
                if json_vars.is_empty() { None } else { Some(json_vars) }
            } else {
                None
            };

            EvalResponse {
                success: true,
                result: Some(result_json),
                variables: variables_json,
                error: None,
                execution_time_ms,
                request_id,
            }
        }
        Err(e) => EvalResponse {
            success: false,
            result: None,
            variables: None,
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
         Access-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS\r\n\
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

fn handle_list_js(
    stream: &mut TcpStream,
    request: &str,
    server_admin_token: Arc<Option<String>>,
) {
    // Check admin authentication first
    if let Some(error_response) = check_admin_authentication(request, &server_admin_token) {
        send_http_response(stream, 401, "application/json", &error_response);
        return;
    }

    let hooks_dir = std::env::var("SKILLET_HOOKS_DIR").unwrap_or_else(|_| "hooks".to_string());

    match list_js_functions(&hooks_dir) {
        Ok(functions) => {
            let response = ListJSResponse {
                success: true,
                total_count: functions.len(),
                functions,
                error: None,
            };
            let json = serde_json::to_string(&response).unwrap_or_default();
            send_http_response(stream, 200, "application/json", &json);
        }
        Err(e) => {
            let response = ListJSResponse {
                success: false,
                functions: Vec::new(),
                total_count: 0,
                error: Some(e),
            };
            let json = serde_json::to_string(&response).unwrap_or_default();
            send_http_response(stream, 500, "application/json", &json);
        }
    }
}

fn handle_update_js(
    stream: &mut TcpStream,
    request: &str,
    server_admin_token: Arc<Option<String>>,
) {
    // Check admin authentication first
    if let Some(error_response) = check_admin_authentication(request, &server_admin_token) {
        send_http_response(stream, 401, "application/json", &error_response);
        return;
    }

    // Parse JSON body
    let body_start = match request.find("\r\n\r\n") {
        Some(pos) => pos + 4,
        None => {
            send_http_error(stream, 400, "Invalid HTTP request");
            return;
        }
    };

    let body = &request[body_start..];
    let update_request: UpdateJSRequest = match serde_json::from_str(body) {
        Ok(req) => req,
        Err(e) => {
            send_http_error(stream, 400, &format!("Invalid JSON: {}", e));
            return;
        }
    };

    // Validate filename ends with .js
    if !update_request.filename.ends_with(".js") {
        let response = UpdateJSResponse {
            success: false,
            message: "Filename must end with .js extension".to_string(),
            function_name: None,
            validation_results: None,
            error: Some("Invalid file extension".to_string()),
        };
        let json = serde_json::to_string(&response).unwrap_or_default();
        send_http_response(stream, 400, "application/json", &json);
        return;
    }

    let hooks_dir = std::env::var("SKILLET_HOOKS_DIR").unwrap_or_else(|_| "hooks".to_string());

    // Check if file exists
    let file_path = std::path::Path::new(&hooks_dir).join(&update_request.filename);
    if !file_path.exists() {
        let response = UpdateJSResponse {
            success: false,
            message: format!("File '{}' does not exist", update_request.filename),
            function_name: None,
            validation_results: None,
            error: Some("File not found".to_string()),
        };
        let json = serde_json::to_string(&response).unwrap_or_default();
        send_http_response(stream, 404, "application/json", &json);
        return;
    }

    // Validate and process the JS function
    match validate_js_function(&update_request.js_code) {
        Ok((js_func, validation_results)) => {
            // Update file in hooks directory
            match save_js_file(&hooks_dir, &update_request.filename, &update_request.js_code) {
                Ok(_) => {
                    let response = UpdateJSResponse {
                        success: true,
                        message: format!("JavaScript function '{}' updated successfully", js_func.name()),
                        function_name: Some(js_func.name().to_string()),
                        validation_results: Some(validation_results),
                        error: None,
                    };
                    let json = serde_json::to_string(&response).unwrap_or_default();
                    send_http_response(stream, 200, "application/json", &json);
                }
                Err(e) => {
                    let response = UpdateJSResponse {
                        success: false,
                        message: "Validation passed but failed to update file".to_string(),
                        function_name: Some(js_func.name().to_string()),
                        validation_results: Some(validation_results),
                        error: Some(e),
                    };
                    let json = serde_json::to_string(&response).unwrap_or_default();
                    send_http_response(stream, 500, "application/json", &json);
                }
            }
        }
        Err(e) => {
            let response = UpdateJSResponse {
                success: false,
                message: "JavaScript function validation failed".to_string(),
                function_name: None,
                validation_results: None,
                error: Some(e),
            };
            let json = serde_json::to_string(&response).unwrap_or_default();
            send_http_response(stream, 400, "application/json", &json);
        }
    }
}

fn handle_delete_js(
    stream: &mut TcpStream,
    request: &str,
    server_admin_token: Arc<Option<String>>,
) {
    // Check admin authentication first
    if let Some(error_response) = check_admin_authentication(request, &server_admin_token) {
        send_http_response(stream, 401, "application/json", &error_response);
        return;
    }

    // Parse JSON body
    let body_start = match request.find("\r\n\r\n") {
        Some(pos) => pos + 4,
        None => {
            send_http_error(stream, 400, "Invalid HTTP request");
            return;
        }
    };

    let body = &request[body_start..];
    let delete_request: DeleteJSRequest = match serde_json::from_str(body) {
        Ok(req) => req,
        Err(e) => {
            send_http_error(stream, 400, &format!("Invalid JSON: {}", e));
            return;
        }
    };

    // Validate filename ends with .js
    if !delete_request.filename.ends_with(".js") {
        let response = DeleteJSResponse {
            success: false,
            message: "Filename must end with .js extension".to_string(),
            error: Some("Invalid file extension".to_string()),
        };
        let json = serde_json::to_string(&response).unwrap_or_default();
        send_http_response(stream, 400, "application/json", &json);
        return;
    }

    let hooks_dir = std::env::var("SKILLET_HOOKS_DIR").unwrap_or_else(|_| "hooks".to_string());

    match delete_js_file(&hooks_dir, &delete_request.filename) {
        Ok(_) => {
            let response = DeleteJSResponse {
                success: true,
                message: format!("JavaScript function file '{}' deleted successfully", delete_request.filename),
                error: None,
            };
            let json = serde_json::to_string(&response).unwrap_or_default();
            send_http_response(stream, 200, "application/json", &json);
        }
        Err(e) => {
            let response = DeleteJSResponse {
                success: false,
                message: format!("Failed to delete file '{}'", delete_request.filename),
                error: Some(e),
            };
            let json = serde_json::to_string(&response).unwrap_or_default();
            send_http_response(stream, 500, "application/json", &json);
        }
    }
}

fn handle_upload_js(
    stream: &mut TcpStream,
    request: &str,
    server_admin_token: Arc<Option<String>>,
) {
    // Check admin authentication first
    if let Some(error_response) = check_admin_authentication(request, &server_admin_token) {
        send_http_response(stream, 401, "application/json", &error_response);
        return;
    }

    // Parse JSON body
    let body_start = match request.find("\r\n\r\n") {
        Some(pos) => pos + 4,
        None => {
            send_http_error(stream, 400, "Invalid HTTP request");
            return;
        }
    };

    let body = &request[body_start..];
    let upload_request: UploadJSRequest = match serde_json::from_str(body) {
        Ok(req) => req,
        Err(e) => {
            send_http_error(stream, 400, &format!("Invalid JSON: {}", e));
            return;
        }
    };

    // Validate filename ends with .js
    if !upload_request.filename.ends_with(".js") {
        let response = UploadJSResponse {
            success: false,
            message: "Filename must end with .js extension".to_string(),
            function_name: None,
            validation_results: None,
            error: Some("Invalid file extension".to_string()),
        };
        let json = serde_json::to_string(&response).unwrap_or_default();
        send_http_response(stream, 400, "application/json", &json);
        return;
    }

    // Validate and process the JS function
    match validate_js_function(&upload_request.js_code) {
        Ok((js_func, validation_results)) => {
            // Save file to hooks directory
            let hooks_dir = std::env::var("SKILLET_HOOKS_DIR").unwrap_or_else(|_| "hooks".to_string());
            match save_js_file(&hooks_dir, &upload_request.filename, &upload_request.js_code) {
                Ok(_) => {
                    let response = UploadJSResponse {
                        success: true,
                        message: format!("JavaScript function '{}' uploaded and validated successfully", js_func.name()),
                        function_name: Some(js_func.name().to_string()),
                        validation_results: Some(validation_results),
                        error: None,
                    };
                    let json = serde_json::to_string(&response).unwrap_or_default();
                    send_http_response(stream, 200, "application/json", &json);
                }
                Err(e) => {
                    let response = UploadJSResponse {
                        success: false,
                        message: "Validation passed but failed to save file".to_string(),
                        function_name: Some(js_func.name().to_string()),
                        validation_results: Some(validation_results),
                        error: Some(e),
                    };
                    let json = serde_json::to_string(&response).unwrap_or_default();
                    send_http_response(stream, 500, "application/json", &json);
                }
            }
        }
        Err(e) => {
            let response = UploadJSResponse {
                success: false,
                message: "JavaScript function validation failed".to_string(),
                function_name: None,
                validation_results: None,
                error: Some(e),
            };
            let json = serde_json::to_string(&response).unwrap_or_default();
            send_http_response(stream, 400, "application/json", &json);
        }
    }
}

fn handle_reload_hooks(
    stream: &mut TcpStream,
    request: &str,
    server_admin_token: Arc<Option<String>>,
) {
    // Check admin authentication first
    if let Some(error_response) = check_admin_authentication(request, &server_admin_token) {
        send_http_response(stream, 401, "application/json", &error_response);
        return;
    }

    let hooks_dir = std::env::var("SKILLET_HOOKS_DIR").unwrap_or_else(|_| "hooks".to_string());
    let js_loader = JSPluginLoader::new(hooks_dir);

    match js_loader.auto_register() {
        Ok(count) => {
            let response = ReloadHooksResponse {
                success: true,
                message: format!("Successfully reloaded {} JavaScript function(s)", count),
                functions_loaded: count,
                error: None,
            };
            let json = serde_json::to_string(&response).unwrap_or_default();
            send_http_response(stream, 200, "application/json", &json);
        }
        Err(e) => {
            let response = ReloadHooksResponse {
                success: false,
                message: "Failed to reload JavaScript functions".to_string(),
                functions_loaded: 0,
                error: Some(e.to_string()),
            };
            let json = serde_json::to_string(&response).unwrap_or_default();
            send_http_response(stream, 500, "application/json", &json);
        }
    }
}

fn validate_js_function(js_code: &str) -> Result<(JavaScriptFunction, ValidationResults), String> {
    let mut validation_results = ValidationResults {
        syntax_valid: false,
        structure_valid: false,
        example_test_passed: false,
        example_result: None,
        example_error: None,
    };

    // Step 1: Parse the JS function (validates syntax and structure)
    let js_func = match JavaScriptFunction::parse_js_function(js_code) {
        Ok(func) => {
            validation_results.syntax_valid = true;
            validation_results.structure_valid = true;
            func
        }
        Err(e) => {
            return Err(format!("Syntax/structure validation failed: {}", e));
        }
    };

    // Step 2: Test the example if provided
    if let Some(example) = js_func.example() {
        match test_js_function_example(&js_func, example) {
            Ok(result) => {
                validation_results.example_test_passed = true;
                validation_results.example_result = Some(result);
            }
            Err(e) => {
                validation_results.example_test_passed = false;
                validation_results.example_error = Some(e);
            }
        }
    } else {
        // No example provided, consider it passed
        validation_results.example_test_passed = true;
        validation_results.example_result = Some("No example provided to test".to_string());
    }

    Ok((js_func, validation_results))
}

fn test_js_function_example(js_func: &JavaScriptFunction, example: &str) -> Result<String, String> {
    // Parse the example to extract function call and expected result
    // Example format: "MYFUNCTION(5) returns 10"
    // or "MYFUNCTION(\"hello\") returns \"HELLO\""

    if let Some((call_part, expected_part)) = example.split_once(" returns ") {
        let function_call = call_part.trim();
        let expected_result = expected_part.trim();

        // Parse the function call to get arguments
        if let Some(args_str) = function_call.strip_prefix(&format!("{}(", js_func.name())).and_then(|s| s.strip_suffix(')')) {
            // Simple argument parsing - this could be enhanced
            let args = parse_function_arguments(args_str)?;

            // Execute the function
            match js_func.execute(args) {
                Ok(result) => {
                    let result_str = format_value_for_comparison(&result);
                    if result_str == expected_result || format!("\"{}\"", result_str) == expected_result {
                        Ok(format!("Expected: {}, Got: {} ✓", expected_result, result_str))
                    } else {
                        Err(format!("Expected: {}, Got: {}", expected_result, result_str))
                    }
                }
                Err(e) => {
                    Err(format!("Function execution failed: {}", e))
                }
            }
        } else {
            Err("Invalid example format: cannot parse function call".to_string())
        }
    } else {
        Err("Invalid example format: expected 'FUNCTION(args) returns result'".to_string())
    }
}

fn parse_function_arguments(args_str: &str) -> Result<Vec<Value>, String> {
    if args_str.trim().is_empty() {
        return Ok(Vec::new());
    }

    let mut args = Vec::new();

    // Simple argument parsing - handles basic cases
    for arg in args_str.split(',') {
        let arg = arg.trim();

        if arg.starts_with('"') && arg.ends_with('"') {
            // String argument
            let string_val = &arg[1..arg.len()-1]; // Remove quotes
            args.push(Value::String(string_val.to_string()));
        } else if arg == "true" {
            args.push(Value::Boolean(true));
        } else if arg == "false" {
            args.push(Value::Boolean(false));
        } else if let Ok(num) = arg.parse::<f64>() {
            // Number argument
            args.push(Value::Number(num));
        } else {
            return Err(format!("Cannot parse argument: {}", arg));
        }
    }

    Ok(args)
}

fn format_value_for_comparison(value: &Value) -> String {
    match value {
        Value::Number(n) => {
            if n.fract() == 0.0 {
                format!("{}", *n as i64)
            } else {
                format!("{}", n)
            }
        }
        Value::String(s) => s.clone(),
        Value::Boolean(b) => b.to_string(),
        Value::Null => "null".to_string(),
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(format_value_for_comparison).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Currency(c) => format!("{}", c),
        Value::DateTime(dt) => dt.to_string(),
        Value::Json(json) => json.clone(),
    }
}

fn save_js_file(hooks_dir: &str, filename: &str, js_code: &str) -> Result<(), String> {
    use std::fs;
    use std::path::Path;

    // Ensure hooks directory exists
    let hooks_path = Path::new(hooks_dir);
    if !hooks_path.exists() {
        fs::create_dir_all(hooks_path)
            .map_err(|e| format!("Failed to create hooks directory: {}", e))?;
    }

    // Save the file
    let file_path = hooks_path.join(filename);
    fs::write(&file_path, js_code)
        .map_err(|e| format!("Failed to write JS file: {}", e))?;

    Ok(())
}

fn delete_js_file(hooks_dir: &str, filename: &str) -> Result<(), String> {
    use std::fs;
    use std::path::Path;

    let hooks_path = Path::new(hooks_dir);
    let file_path = hooks_path.join(filename);

    // Check if file exists
    if !file_path.exists() {
        return Err(format!("File '{}' does not exist", filename));
    }

    // Delete the file
    fs::remove_file(&file_path)
        .map_err(|e| format!("Failed to delete JS file: {}", e))?;

    Ok(())
}

fn list_js_functions(hooks_dir: &str) -> Result<Vec<JSFunctionInfo>, String> {
    use std::fs;
    use std::path::Path;

    let hooks_path = Path::new(hooks_dir);

    if !hooks_path.exists() {
        // Return empty list if hooks directory doesn't exist
        return Ok(Vec::new());
    }

    let mut functions = Vec::new();

    // Recursively scan directory for JS files
    scan_directory_for_js(hooks_path, hooks_path, &mut functions)?;

    // Sort by filename for consistent ordering
    functions.sort_by(|a, b| a.filename.cmp(&b.filename));

    Ok(functions)
}

fn scan_directory_for_js(
    current_dir: &std::path::Path,
    hooks_root: &std::path::Path,
    functions: &mut Vec<JSFunctionInfo>
) -> Result<(), String> {
    use std::fs;

    let entries = fs::read_dir(current_dir)
        .map_err(|e| format!("Failed to read directory: {}", e))?;

    for entry in entries {
        let entry = entry
            .map_err(|e| format!("Failed to read directory entry: {}", e))?;

        let path = entry.path();

        if path.is_dir() {
            // Recursively scan subdirectories
            scan_directory_for_js(&path, hooks_root, functions)?;
        } else if path.extension().and_then(|s| s.to_str()) == Some("js") {
            // Get relative path from hooks root
            let relative_path = path.strip_prefix(hooks_root)
                .map_err(|_| "Failed to get relative path".to_string())?;

            let filename = relative_path.to_string_lossy().to_string();

            // Get file metadata
            let metadata = entry.metadata()
                .map_err(|e| format!("Failed to get file metadata: {}", e))?;

            let file_size = metadata.len();
            let last_modified = metadata.modified()
                .map(|time| {
                    use std::time::UNIX_EPOCH;
                    let duration = time.duration_since(UNIX_EPOCH).unwrap_or_default();
                    chrono::DateTime::from_timestamp(duration.as_secs() as i64, 0)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M:%S UTC").to_string())
                        .unwrap_or_else(|| "Unknown".to_string())
                })
                .unwrap_or_else(|_| "Unknown".to_string());

            // Try to parse the JS function to get metadata
            let (function_name, description, example, min_args, max_args, is_valid, validation_error) =
                match fs::read_to_string(&path) {
                    Ok(content) => {
                        match JavaScriptFunction::parse_js_function(&content) {
                            Ok(js_func) => (
                                Some(js_func.name().to_string()),
                                js_func.description().map(|s| s.to_string()),
                                js_func.example().map(|s| s.to_string()),
                                Some(js_func.min_args()),
                                js_func.max_args(),
                                true,
                                None,
                            ),
                            Err(e) => (None, None, None, None, None, false, Some(e.to_string())),
                        }
                    }
                    Err(e) => (None, None, None, None, None, false, Some(format!("Failed to read file: {}", e))),
                };

            functions.push(JSFunctionInfo {
                filename,
                function_name,
                description,
                example,
                min_args,
                max_args,
                file_size,
                last_modified,
                is_valid,
                validation_error,
            });
        }
    }

    Ok(())
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
        eprintln!("  -d, --daemon         Run as daemon (background process)");
        eprintln!("  -H, --host <addr>    Bind host/interface (default: 127.0.0.1)");
        eprintln!("  --pid-file <file>    Write PID to file (default: skillet-http-server.pid)");
        eprintln!("  --log-file <file>    Write logs to file (daemon mode only)");
        eprintln!("  --token <value>      Require token for eval requests");
        eprintln!("  --admin-token <val>  Require admin token for JS function management");
        eprintln!("");
        eprintln!("Examples:");
        eprintln!("  sk_http_server 5074");
        eprintln!("  sk_http_server 5074 --host 0.0.0.0");
        eprintln!("  sk_http_server 5074 --host 0.0.0.0 --token secret123");
        eprintln!("  sk_http_server 5074 --admin-token admin456");
        eprintln!("  sk_http_server 5074 --token secret123 --admin-token admin456");
        eprintln!("  sk_http_server 5074 -d --pid-file /var/run/skillet-http.pid");
        eprintln!("  sk_http_server 5074 -d --host 0.0.0.0 --token secret123 --admin-token admin456");
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
    let mut admin_token: Option<String> = None;
    let mut daemon_mode = false;
    let mut pid_file = "skillet-http-server.pid".to_string();
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
                    eprintln!("Error: --host requires an address");
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
            "--admin-token" => {
                if i + 1 < args.len() {
                    admin_token = Some(args[i + 1].clone());
                    i += 1;
                } else {
                    eprintln!("Error: --admin-token requires a value");
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

    // Implement intelligent token defaults and warnings
    let mut dev_mode_warning = false;
    let mut same_token_warning = false;
    let mut admin_inherited_warning = false;
    let mut eval_inherited_warning = false;

    match (&auth_token, &admin_token) {
        // No tokens provided - development mode
        (None, None) => {
            dev_mode_warning = true;
        }
        // Only eval token provided - admin inherits eval token
        (Some(eval_tok), None) => {
            admin_token = Some(eval_tok.clone());
            admin_inherited_warning = true;
        }
        // Only admin token provided - eval inherits admin token
        (None, Some(admin_tok)) => {
            auth_token = Some(admin_tok.clone());
            eval_inherited_warning = true;
        }
        // Both tokens provided - check if they're the same
        (Some(eval_tok), Some(admin_tok)) => {
            if eval_tok == admin_tok {
                same_token_warning = true;
            }
        }
    }

    // Handle daemon mode before any output
    if daemon_mode {
        #[cfg(unix)]
        {
            // Print startup message before daemonizing
            eprintln!("Starting Skillet HTTP server as daemon...");
            eprintln!("Port: {}, Host: {}, PID file: {}", port, bind_host, pid_file);
            if auth_token.is_some() { eprintln!("Eval token auth: enabled"); }
            if admin_token.is_some() { eprintln!("Admin token auth: enabled"); }

            // Print warnings before daemonizing
            if dev_mode_warning {
                eprintln!("⚠️  WARNING: Running in DEVELOPMENT MODE - no authentication required!");
                eprintln!("⚠️  This server is UNPROTECTED and should not be exposed to networks.");
            }
            if same_token_warning {
                eprintln!("⚠️  WARNING: Admin token and eval token are the same!");
                eprintln!("⚠️  Consider using different tokens for better security separation.");
            }
            if admin_inherited_warning {
                eprintln!("⚠️  WARNING: Admin token inherited from eval token!");
                eprintln!("⚠️  Admin operations use the same token as eval operations.");
            }
            if eval_inherited_warning {
                eprintln!("⚠️  WARNING: Eval token inherited from admin token!");
                eprintln!("⚠️  The same token has all privileges (eval and admin).");
            }

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

    // Setup signal handlers
    let running = setup_signal_handlers();

    // Load JavaScript functions
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
    let server_admin_token = Arc::new(admin_token.clone());

    if !daemon_mode {
        eprintln!("🚀 Skillet HTTP Server started on http://{}:{}", bind_host, port);
        if auth_token.is_some() { eprintln!("🔒 Eval token auth: enabled"); }
        if admin_token.is_some() { eprintln!("🔐 Admin token auth: enabled"); }

        // Print security warnings
        if dev_mode_warning {
            eprintln!("⚠️  WARNING: Running in DEVELOPMENT MODE - no authentication required!");
            eprintln!("⚠️  This server is UNPROTECTED and should not be exposed to networks.");
        }
        if same_token_warning {
            eprintln!("⚠️  WARNING: Admin token and eval token are the same!");
            eprintln!("⚠️  Consider using different tokens for better security separation.");
        }
        if admin_inherited_warning {
            eprintln!("⚠️  WARNING: Admin token inherited from eval token!");
            eprintln!("⚠️  Admin operations use the same token as eval operations.");
        }
        if eval_inherited_warning {
            eprintln!("⚠️  WARNING: Eval token inherited from admin token!");
            eprintln!("⚠️  The same token has all privileges (eval and admin).");
        }

        eprintln!("🌐 Ready for HTTP requests");
        eprintln!("📖 Visit http://{}:{} for API documentation", bind_host, port);
        eprintln!("");
    }

    // Accept loop
    while running.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _addr)) => {
                let stats = Arc::clone(&stats);
                let request_counter = Arc::clone(&request_counter);
                let server_token = Arc::clone(&server_token);
                let server_admin_token = Arc::clone(&server_admin_token);

                std::thread::spawn(move || {
                    handle_http_request(stream, stats, request_counter, server_token, server_admin_token);
                });
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            Err(e) => {
                if !daemon_mode {
                    eprintln!("Error accepting connection: {}", e);
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
    }

    if !daemon_mode {
        eprintln!("Server shutdown complete.");
    }
}