mod http_server;

use skillet::JSPluginLoader;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};

use http_server::auth::TokenConfig;
use http_server::daemon::{setup_signal_handlers, write_pid_file};
use http_server::eval::{handle_eval_post, handle_eval_get, handle_health};
use http_server::js_management::{handle_list_js, handle_update_js, handle_delete_js, handle_upload_js, handle_reload_hooks};
use http_server::stats::ServerStats;
use http_server::utils::{read_complete_http_request, send_http_response, send_http_error, handle_cors_preflight, load_html_file};

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

fn handle_root(stream: &mut TcpStream) {
    let html = load_html_file();
    send_http_response(stream, 200, "text/html", &html);
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

    // Parse command line arguments
    let (mut auth_token, mut admin_token, daemon_mode, pid_file, bind_host) = parse_args(&args[2..]);

    // Apply intelligent token logic
    let token_config = TokenConfig::new(auth_token, admin_token);
    auth_token = token_config.auth_token.clone();
    admin_token = token_config.admin_token.clone();

    // Handle daemon mode before any output
    if daemon_mode {
        handle_daemon_mode(port, &bind_host, &pid_file, &token_config);
    }

    // Setup signal handlers
    let running = setup_signal_handlers();

    // Load JavaScript functions
    load_js_functions(daemon_mode);

    // Start server
    let listener = start_server(port, &bind_host);
    let stats = Arc::new(ServerStats::new());
    let request_counter = Arc::new(AtomicU64::new(0));
    let server_token = Arc::new(auth_token.clone());
    let server_admin_token = Arc::new(admin_token.clone());

    // Print startup messages
    print_startup_messages(daemon_mode, port, &bind_host, &auth_token, &admin_token, &token_config);

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

fn print_usage() {
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
}

fn parse_args(args: &[String]) -> (Option<String>, Option<String>, bool, String, String) {
    let mut auth_token: Option<String> = None;
    let mut admin_token: Option<String> = None;
    let mut daemon_mode = false;
    let mut pid_file = "skillet-http-server.pid".to_string();
    let mut bind_host = "127.0.0.1".to_string();
    let mut _log_file: Option<String> = None;
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "-d" | "--daemon" => daemon_mode = true,
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

    (auth_token, admin_token, daemon_mode, pid_file, bind_host)
}

#[cfg(unix)]
fn handle_daemon_mode(port: u16, bind_host: &str, pid_file: &str, token_config: &TokenConfig) {
    // Print startup message before daemonizing
    eprintln!("Starting Skillet HTTP server as daemon...");
    eprintln!("Port: {}, Host: {}, PID file: {}", port, bind_host, pid_file);
    if token_config.auth_token.is_some() { eprintln!("Eval token auth: enabled"); }
    if token_config.admin_token.is_some() { eprintln!("Admin token auth: enabled"); }
    
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
fn handle_daemon_mode(_port: u16, _bind_host: &str, _pid_file: &str, _token_config: &TokenConfig) {
    eprintln!("Error: Daemon mode not supported on this platform");
    std::process::exit(1);
}

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
    let listener = TcpListener::bind(format!("{}:{}", bind_host, port))
        .unwrap_or_else(|e| {
            eprintln!("Error: Failed to bind to {}:{}: {}", bind_host, port, e);
            std::process::exit(1);
        });

    listener.set_nonblocking(true).unwrap_or_else(|e| {
        eprintln!("Error: Failed to set non-blocking mode: {}", e);
        std::process::exit(1);
    });

    listener
}

fn print_startup_messages(
    daemon_mode: bool,
    port: u16,
    bind_host: &str,
    auth_token: &Option<String>,
    admin_token: &Option<String>,
    token_config: &TokenConfig,
) {
    if !daemon_mode {
        eprintln!("🚀 Skillet HTTP Server started on http://{}:{}", bind_host, port);
        if auth_token.is_some() { eprintln!("🔒 Eval token auth: enabled"); }
        if admin_token.is_some() { eprintln!("🔐 Admin token auth: enabled"); }
        
        // Print security warnings
        token_config.print_warnings();
        
        eprintln!("🌐 Ready for HTTP requests");
        eprintln!("📖 Visit http://{}:{} for API documentation", bind_host, port);
        eprintln!("");
    }
}