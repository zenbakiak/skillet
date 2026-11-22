mod http_server;

#[cfg(feature = "plugins")]
use skillet::JSPluginLoader;
use std::net::{TcpListener, TcpStream};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use threadpool::Builder as ThreadPoolBuilder;

use http_server::auth::TokenConfig;
use http_server::conn_limit::{self, Admission, ConnLimiter};
use http_server::daemon::{setup_signal_handlers, wake_address};
use http_server::eval::{handle_cache_clear, handle_eval_get, handle_eval_post, handle_health};
#[cfg(feature = "plugins")]
use http_server::js_management::{
    handle_delete_js, handle_list_js, handle_reload_hooks, handle_update_js, handle_upload_js,
};
use http_server::pid_file::write_pid_file;
use http_server::stats::ServerStats;
use http_server::utils::{
    handle_cors_preflight, load_html_file, read_complete_http_request, send_http_error,
    send_http_response,
};

#[cfg(unix)]
use http_server::daemon::daemonize;

/// HTTP-compatible Skillet evaluation server
/// Works with all standard HTTP clients
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
        Err(e) => {
            // Log error for debugging but don't panic
            eprintln!("HTTP request read error: {}", e);
            // Send proper HTTP error response
            let error_msg = match e.kind() {
                std::io::ErrorKind::InvalidData => {
                    if e.to_string().contains("too large") {
                        "413 Payload Too Large"
                    } else {
                        "400 Bad Request"
                    }
                }
                std::io::ErrorKind::TimedOut => "408 Request Timeout",
                _ => "500 Internal Server Error",
            };
            send_http_error(&mut stream, 400, error_msg);
            return;
        }
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

    // Before dispatch, so no handler runs for a browser in development mode or parses a body
    // that is not JSON (ADR-0019).
    if let Err(message) =
        http_server::cors::check_browser_admin(path_only, &request, server_admin_token.is_some())
    {
        send_http_error(&mut stream, 403, message);
        return;
    }
    if let Err(message) = http_server::cors::check_content_type(method, path_only, &request) {
        send_http_error(&mut stream, 415, message);
        return;
    }

    match (method, path_only) {
        ("GET", "/health") => handle_health(&mut stream, &stats, &request, server_token),
        ("GET", "/") => handle_root(&mut stream),
        ("GET", "/docs") => handle_api_docs(&mut stream),
        ("GET", "/openapi.yml") => handle_openapi_spec(&mut stream),
        ("POST", "/eval") => {
            handle_eval_post(&mut stream, &request, stats, request_counter, server_token)
        }
        ("GET", "/eval") => {
            handle_eval_get(&mut stream, &request, stats, request_counter, server_token)
        }
        #[cfg(feature = "plugins")]
        ("POST", "/upload-js") => handle_upload_js(&mut stream, &request, server_admin_token),
        #[cfg(feature = "plugins")]
        ("PUT", "/update-js") => handle_update_js(&mut stream, &request, server_admin_token),
        #[cfg(feature = "plugins")]
        ("DELETE", "/delete-js") => handle_delete_js(&mut stream, &request, server_admin_token),
        #[cfg(feature = "plugins")]
        ("GET", "/list-js") => handle_list_js(&mut stream, &request, server_admin_token),
        #[cfg(feature = "plugins")]
        ("POST", "/reload-hooks") => handle_reload_hooks(&mut stream, &request, server_admin_token),
        #[cfg(not(feature = "plugins"))]
        (_, "/upload-js" | "/update-js" | "/delete-js" | "/list-js" | "/reload-hooks") => {
            handle_js_disabled(&mut stream, &request, server_admin_token)
        }
        ("DELETE", "/cache") => handle_cache_clear(&mut stream, &request, server_admin_token),
        ("OPTIONS", _) => handle_cors_preflight(&mut stream),
        _ => send_http_error(&mut stream, 404, "Not Found"),
    }
}

/// JS management endpoints in a build without the `plugins` feature: same admin
/// check as the real handlers, then 404 with a message saying why.
#[cfg(not(feature = "plugins"))]
fn handle_js_disabled(
    stream: &mut TcpStream,
    request: &str,
    server_admin_token: Arc<Option<String>>,
) {
    if let Some(error_response) =
        http_server::auth::check_admin_authentication(request, &server_admin_token)
    {
        send_http_response(stream, 401, "application/json", &error_response);
        return;
    }
    send_http_error(
        stream,
        404,
        "JavaScript plugins are not enabled in this build",
    );
}

fn handle_root(stream: &mut TcpStream) {
    send_http_response(stream, 200, "text/html", load_html_file());
}

fn handle_api_docs(stream: &mut TcpStream) {
    // Static Scalar page that loads our OpenAPI spec from /openapi.yml (ADR-0020).
    send_http_response(
        stream,
        200,
        "text/html",
        include_str!("http_server/api_docs.html"),
    );
}

fn handle_openapi_spec(stream: &mut TcpStream) {
    // Serve the OpenAPI specification YAML file
    let openapi_spec = include_str!("../../openapi.yml");
    send_http_response(stream, 200, "application/x-yaml", openapi_spec);
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage();
        std::process::exit(1);
    }

    let port: u16 = args[1].parse().unwrap_or_else(|_| {
        eprintln!("Error: Invalid port number");
        std::process::exit(1);
    });

    let ServerArgs {
        auth_token,
        admin_token,
        cors_origin,
        daemon_mode,
        pid_file,
        bind_host,
        thread_count,
        max_conns_per_ip,
    } = parse_args(&args[2..]);

    // Apply intelligent token logic
    let token_config = TokenConfig::new(auth_token, admin_token);
    let auth_token = token_config.auth_token.clone();
    let admin_token = token_config.admin_token.clone();

    let allowed_origin =
        http_server::cors::allowed_origin(cors_origin.as_deref(), auth_token.is_some());
    http_server::cors::init(allowed_origin.as_deref());

    // Handle daemon mode before any output
    if daemon_mode {
        handle_daemon_mode(port, &bind_host, &pid_file, &token_config, thread_count);
    }

    // Load JavaScript functions
    load_js_functions(daemon_mode);

    // Start server, then the shutdown handler (it needs the bound address to wake `accept`)
    let listener = start_server(port, &bind_host);
    let running = match listener.local_addr() {
        Ok(addr) => setup_signal_handlers(wake_address(addr)),
        Err(e) => {
            eprintln!("Error: Failed to read the listening address: {}", e);
            std::process::exit(1);
        }
    };
    let stats = Arc::new(ServerStats::new());
    let request_counter = Arc::new(AtomicU64::new(0));
    let server_token = Arc::new(auth_token.clone());
    let server_admin_token = Arc::new(admin_token.clone());
    let limiter = max_conns_per_ip.map(ConnLimiter::new);

    // Create thread pool. Workers get a 16 MB stack (default 2 MB) so expressions up to
    // MAX_NESTING_DEPTH have headroom in debug builds too (BL-017).
    let pool = ThreadPoolBuilder::new()
        .num_threads(thread_count)
        .thread_stack_size(16 * 1024 * 1024)
        .build();

    // Print startup messages
    print_startup_messages(
        daemon_mode,
        port,
        &bind_host,
        &token_config,
        allowed_origin.as_deref(),
        thread_count,
    );
    if let (false, Some(max)) = (daemon_mode, max_conns_per_ip) {
        eprintln!("🚦 Max connections per IP: {}", max);
    }

    // Accept loop. `accept` blocks (no polling delay, BL-039); the shutdown handler clears
    // `running` and connects once to wake it, and that connection is dropped unserved.
    loop {
        let accepted = listener.accept();
        if !running.load(Ordering::SeqCst) {
            break;
        }
        match accepted {
            Ok((stream, _addr)) => {
                // Counted before the connection is queued, so a rejected one never reaches a
                // worker (ADR-0025).
                let guard = match conn_limit::admit(limiter.as_ref(), &stream) {
                    Admission::Accept(guard) => guard,
                    Admission::Reject(max) => {
                        conn_limit::reject(stream, too_many_connections(max).as_bytes());
                        continue;
                    }
                };
                let stats = Arc::clone(&stats);
                let request_counter = Arc::clone(&request_counter);
                let server_token = Arc::clone(&server_token);
                let server_admin_token = Arc::clone(&server_admin_token);

                pool.execute(move || {
                    let _guard = guard; // released when the connection is closed
                    handle_http_request(
                        stream,
                        stats,
                        request_counter,
                        server_token,
                        server_admin_token,
                    );
                });
            }
            Err(e) => {
                // e.g. out of file descriptors: back off instead of spinning
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

/// The full HTTP 429 response for a connection over the per-IP limit.
fn too_many_connections(max: usize) -> String {
    let body = conn_limit::rejection_json(max);
    format!(
        "HTTP/1.1 429 Too Many Requests\r\n\
         {}\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\
         \r\n\
         {}",
        http_server::cors::response_headers(),
        body.len(),
        body
    )
}

fn print_usage() {
    eprintln!("Usage: sk_http_server <port> [options]");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -d, --daemon         Run as daemon (background process)");
    eprintln!("  -H, --host <addr>    Bind host/interface (default: 127.0.0.1)");
    eprintln!("  -t, --threads <num>  Number of worker threads (default: CPU count)");
    eprintln!("  --pid-file <file>    Write PID to file (default: skillet-http-server.pid)");
    eprintln!("  --log-file <file>    Write logs to file (daemon mode only)");
    eprintln!("  --token <value>      Require token for eval requests (or set SKILLET_AUTH_TOKEN;");
    eprintln!("                       the flag wins)");
    eprintln!("  --admin-token <val>  Require admin token for JS function management and");
    eprintln!("                       DELETE /cache (or set SKILLET_ADMIN_TOKEN; the flag wins)");
    eprintln!("  --cors-origin <org>  Access-Control-Allow-Origin value, e.g. https://app.example");
    eprintln!("                       or * (or set SKILLET_CORS_ORIGIN; the flag wins). Default:");
    eprintln!("                       * when a token is set, otherwise no CORS headers");
    eprintln!("  --max-connections-per-ip <n>");
    eprintln!("                       Open connections allowed from one IP address; more get");
    eprintln!("                       429 (or set SKILLET_MAX_CONNECTIONS_PER_IP; the flag wins).");
    eprintln!("                       Default: no limit");
    eprintln!();
    eprintln!("Requests with a body must send Content-Type: application/json (415 otherwise);");
    eprintln!("/upload-js and /update-js also accept multipart/form-data.");
    eprintln!();
    eprintln!("Examples:");
    eprintln!("  sk_http_server 5074");
    eprintln!("  sk_http_server 5074 --host 0.0.0.0 --threads 8");
    eprintln!("  sk_http_server 5074 --host 0.0.0.0 --token secret123");
    eprintln!("  sk_http_server 5074 --admin-token admin456 --threads 16");
    eprintln!("  sk_http_server 5074 --token secret123 --admin-token admin456");
    eprintln!("  sk_http_server 5074 -d --pid-file /var/run/skillet-http.pid --threads 12");
    eprintln!("  sk_http_server 5074 -d --host 0.0.0.0 --token secret123 --admin-token admin456");
    eprintln!();
    eprintln!("Endpoints:");
    eprintln!("  GET  /health          - Health check with cache stats");
    eprintln!("  GET  /                - API documentation");
    eprintln!("  POST /eval            - Evaluate expressions (JSON)");
    eprintln!("  GET  /eval?expr=...   - Evaluate expressions (query params)");
    eprintln!("  DELETE /cache         - Clear expression cache (admin token required)");
}

struct ServerArgs {
    auth_token: Option<String>,
    admin_token: Option<String>,
    cors_origin: Option<String>,
    daemon_mode: bool,
    pid_file: String,
    bind_host: String,
    thread_count: usize,
    max_conns_per_ip: Option<usize>,
}

fn exit_with(message: &str) -> ! {
    eprintln!("Error: {message}");
    std::process::exit(1);
}

/// The value after flag `args[*i]`; advances `i` past it.
fn flag_value(args: &[String], i: &mut usize, what: &str) -> String {
    *i += 1;
    match args.get(*i) {
        Some(value) => value.clone(),
        None => exit_with(&format!("{} requires {what}", args[*i - 1])),
    }
}

fn parse_args(args: &[String]) -> ServerArgs {
    let mut parsed = ServerArgs {
        auth_token: None,
        admin_token: None,
        cors_origin: None,
        daemon_mode: false,
        pid_file: "skillet-http-server.pid".to_string(),
        bind_host: "127.0.0.1".to_string(),
        thread_count: num_cpus::get(),
        max_conns_per_ip: None,
    };
    let mut max_conns_flag: Option<String> = None;
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "-d" | "--daemon" => parsed.daemon_mode = true,
            "-H" | "--host" => parsed.bind_host = flag_value(args, &mut i, "an address"),
            "-t" | "--threads" => {
                let n = flag_value(args, &mut i, "a number");
                parsed.thread_count = n
                    .parse()
                    .unwrap_or_else(|_| exit_with("Invalid thread count"));
                if parsed.thread_count == 0 {
                    exit_with("Thread count must be greater than 0");
                }
            }
            "--pid-file" => parsed.pid_file = flag_value(args, &mut i, "a filename"),
            // Accepted for compatibility; logs go to stderr.
            "--log-file" => drop(flag_value(args, &mut i, "a filename")),
            "--token" => parsed.auth_token = Some(flag_value(args, &mut i, "a value")),
            "--admin-token" => parsed.admin_token = Some(flag_value(args, &mut i, "a value")),
            "--cors-origin" => parsed.cors_origin = Some(flag_value(args, &mut i, "an origin")),
            conn_limit::FLAG => max_conns_flag = Some(flag_value(args, &mut i, "a number")),
            other => exit_with(&format!("Unknown argument: {other}")),
        }
        i += 1;
    }

    // Same as sk_server: the flag wins, the env var is the fallback.
    env_fallback(&mut parsed.auth_token, "SKILLET_AUTH_TOKEN");
    env_fallback(&mut parsed.admin_token, "SKILLET_ADMIN_TOKEN");
    env_fallback(&mut parsed.cors_origin, "SKILLET_CORS_ORIGIN");

    // An empty token would match a request with no token, so auth would be off while the
    // banner says it is on (BL-048).
    for (flag, token) in [
        ("--token", &parsed.auth_token),
        ("--admin-token", &parsed.admin_token),
    ] {
        if token.as_deref() == Some("") {
            exit_with(&format!("{flag} must not be empty"));
        }
    }
    if let Some(origin) = &parsed.cors_origin {
        if !http_server::cors::valid_origin(origin) {
            exit_with(
                "--cors-origin must be a non-empty origin without spaces or control characters",
            );
        }
    }
    parsed.max_conns_per_ip =
        conn_limit::resolve(max_conns_flag.as_deref()).unwrap_or_else(|e| exit_with(&e));
    parsed
}

/// Fill `slot` from env var `var` when no flag set it. An empty value is an error: it must
/// not silently turn a setting off (BL-048).
fn env_fallback(slot: &mut Option<String>, var: &str) {
    if slot.is_some() {
        return;
    }
    if let Ok(value) = std::env::var(var) {
        if value.is_empty() {
            exit_with(&format!(
                "{var} must not be empty; give a non-empty value or unset it"
            ));
        }
        *slot = Some(value);
    }
}

#[cfg(unix)]
fn handle_daemon_mode(
    port: u16,
    bind_host: &str,
    pid_file: &str,
    token_config: &TokenConfig,
    thread_count: usize,
) {
    // Print startup message before daemonizing
    eprintln!("Starting Skillet HTTP server as daemon...");
    eprintln!(
        "Port: {}, Host: {}, Threads: {}, PID file: {}",
        port, bind_host, thread_count, pid_file
    );
    if token_config.auth_token.is_some() {
        eprintln!("Eval token auth: enabled");
    }
    if token_config.admin_token.is_some() {
        eprintln!("Admin token auth: enabled");
    }

    // Print warnings before daemonizing
    token_config.print_warnings();

    if let Err(e) = daemonize() {
        eprintln!("Failed to daemonize: {}", e);
        std::process::exit(1);
    }

    // Write PID file after successful daemonization
    if let Err(_e) = write_pid_file(pid_file) {
        // Log to syslog or a file since we can't use stderr
        std::process::exit(1);
    }
}

#[cfg(not(unix))]
fn handle_daemon_mode(
    _port: u16,
    _bind_host: &str,
    _pid_file: &str,
    _token_config: &TokenConfig,
    _thread_count: usize,
) {
    eprintln!("Error: Daemon mode not supported on this platform");
    std::process::exit(1);
}

#[cfg(not(feature = "plugins"))]
fn load_js_functions(_daemon_mode: bool) {}

#[cfg(feature = "plugins")]
fn load_js_functions(daemon_mode: bool) {
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

fn start_server(port: u16, bind_host: &str) -> TcpListener {
    TcpListener::bind(format!("{}:{}", bind_host, port)).unwrap_or_else(|e| {
        eprintln!("Error: Failed to bind to {}:{}: {}", bind_host, port, e);
        std::process::exit(1);
    })
}

fn print_startup_messages(
    daemon_mode: bool,
    port: u16,
    bind_host: &str,
    token_config: &TokenConfig,
    allowed_origin: Option<&str>,
    thread_count: usize,
) {
    if !daemon_mode {
        eprintln!(
            "🚀 Skillet HTTP Server started on http://{}:{}",
            bind_host, port
        );
        eprintln!("🧵 Worker threads: {}", thread_count);
        if token_config.auth_token.is_some() {
            eprintln!("🔒 Eval token auth: enabled");
        }
        if token_config.admin_token.is_some() {
            eprintln!("🔐 Admin token auth: enabled");
        }
        match allowed_origin {
            Some(origin) => eprintln!("🌍 CORS: Access-Control-Allow-Origin: {}", origin),
            None => eprintln!("🌍 CORS: off (no token, no --cors-origin)"),
        }

        // Print security warnings
        token_config.print_warnings();

        eprintln!("🌐 Ready for HTTP requests");
        eprintln!(
            "📖 Visit http://{}:{} for API documentation",
            bind_host, port
        );
        eprintln!();
    }
}
