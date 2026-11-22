//! `sk_http_server` accept loop (BL-039): no per-request polling delay, and Ctrl-C still
//! stops a server that is blocked in `accept`. Also the embedded `GET /` page (BL-045).
//! Servers run on 127.0.0.1 only, from the temp directory (not the repo), like an installed
//! binary.

mod common;

use common::ServerProcess;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// `sk_http_server` on a free 127.0.0.1 port, killed on drop.
struct Server(ServerProcess);

impl Server {
    fn spawn() -> Server {
        Server(ServerProcess::start(|port| {
            let hooks = std::env::temp_dir().join("skillet-http-accept-no-hooks");
            let mut cmd = Command::new(env!("CARGO_BIN_EXE_sk_http_server"));
            cmd.current_dir(std::env::temp_dir())
                .args([port.to_string(), "--threads".into(), "2".into()])
                .env("SKILLET_HOOKS_DIR", hooks)
                .env("SKILLET_BIND_HOST", "127.0.0.1")
                .env_remove("SKILLET_AUTH_TOKEN")
                .env_remove("SKILLET_ADMIN_TOKEN")
                .env_remove("SKILLET_CORS_ORIGIN")
                .stdout(Stdio::null());
            cmd
        }))
    }

    /// One `GET` with `Connection: close`; returns the raw response.
    fn get(&self, path: &str) -> std::io::Result<String> {
        let mut s = TcpStream::connect(("127.0.0.1", self.0.port))?;
        s.set_read_timeout(Some(Duration::from_secs(10)))?;
        write!(
            s,
            "GET {} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
            path
        )?;
        let mut out = String::new();
        s.read_to_string(&mut out)?;
        Ok(out)
    }
}

/// Sequential requests used to wait out a 1 ms sleep each (~1.1 ms floor per request).
/// The fastest of 30 is now ~0.1 ms (debug build, load average ~19); before BL-039 it was
/// ~0.77 ms. 0.4 ms sits between the two.
/// Timing-based, so it is not part of the default run: on a loaded machine or a shared CI
/// runner a debug build lands at ~0.45-0.55 ms, too close to the threshold. Run it with
/// `cargo test --test http_server_accept -- --ignored` on a quiet machine; the HTTP layer of
/// `cargo bench --bench baseline` is the regular guard.
#[test]
#[ignore = "timing-sensitive: run with --ignored on a quiet machine"]
fn sequential_requests_have_no_polling_floor() {
    let server = Server::spawn();
    let mut fastest = Duration::MAX;
    for _ in 0..30 {
        let start = Instant::now();
        let resp = server.get("/health").expect("health request");
        fastest = fastest.min(start.elapsed());
        assert!(resp.starts_with("HTTP/1.1 200"), "{}", resp);
    }
    assert!(
        fastest < Duration::from_micros(400),
        "fastest /health took {:?}; the accept loop is polling again",
        fastest
    );
}

/// Ctrl-C (SIGINT) stops a server that is idle in a blocking `accept`, through the normal
/// shutdown path.
#[cfg(unix)]
#[test]
fn sigint_stops_idle_server() {
    let mut server = Server::spawn();
    std::thread::sleep(Duration::from_millis(200)); // idle: blocked in accept
    let status = Command::new("kill")
        .args(["-INT", &server.0.child.id().to_string()])
        .status()
        .expect("run kill");
    assert!(status.success());

    let deadline = Instant::now() + Duration::from_secs(10);
    let exit = loop {
        if let Some(exit) = server.0.child.try_wait().expect("try_wait") {
            break exit;
        }
        assert!(
            Instant::now() < deadline,
            "server still running after SIGINT"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(exit.success(), "exit status {:?}", exit);

    let stderr = server.0.stderr_after_exit();
    assert!(stderr.contains("Server shutdown complete."), "{}", stderr);
}

/// `GET /` serves the embedded documentation page, byte for byte, even when the working
/// directory has no `src/bin/http_server/documentation.html`.
#[test]
fn root_page_is_embedded() {
    let server = Server::spawn();
    let resp = server.get("/").expect("root request");
    let (head, body) = resp.split_once("\r\n\r\n").expect("header/body split");
    assert!(head.starts_with("HTTP/1.1 200"), "{}", head);
    assert!(head.contains("Content-Type: text/html"), "{}", head);
    assert_eq!(
        body,
        include_str!("../src/bin/http_server/documentation.html")
    );
}

/// `GET /docs` serves the static Scalar page (ADR-0020: no `scalar-doc`), and it points at the
/// spec the server serves at `/openapi.yml`.
#[test]
fn docs_page_points_at_openapi_spec() {
    let server = Server::spawn();
    let resp = server.get("/docs").expect("docs request");
    let (head, body) = resp.split_once("\r\n\r\n").expect("header/body split");
    assert!(head.starts_with("HTTP/1.1 200"), "{}", head);
    assert!(head.contains("Content-Type: text/html"), "{}", head);
    assert_eq!(body, include_str!("../src/bin/http_server/api_docs.html"));
    assert!(body.contains(r#"data-url="/openapi.yml""#), "{}", body);
    assert!(body.contains("@scalar/api-reference"), "{}", body);

    let spec = server.get("/openapi.yml").expect("spec request");
    assert!(spec.starts_with("HTTP/1.1 200"), "{}", spec);
    assert!(spec.contains("openapi:"), "{}", spec);
}

/// The spec's response block for `status` under `path` (text up to the next status or path).
fn spec_response(path: &str, status: &str) -> String {
    let spec = include_str!("../openapi.yml");
    let start = spec
        .find(&format!("\n  {path}:\n"))
        .unwrap_or_else(|| panic!("{path} missing"));
    let rest = &spec[start + 1..];
    let end = rest[1..].find("\n  /").map_or(rest.len(), |i| i + 1);
    let section = &rest[..end];
    let key = format!("        '{status}':\n");
    let at = section
        .find(&key)
        .unwrap_or_else(|| panic!("{path} {status} missing"));
    let block = &section[at + key.len()..];
    let stop = block.find("\n        '").map_or(block.len(), |i| i + 1);
    block[..stop].to_string()
}

/// BL-057: the plugin-free 404 of `/delete-js` has no `message` field, so the spec must not
/// describe it with `DeleteJSResponse` (which requires `message`).
#[test]
fn delete_js_404_is_documented_as_an_error_response() {
    let block = spec_response("/delete-js", "404");
    assert!(
        block.contains("$ref: '#/components/schemas/ErrorResponse'"),
        "{block}"
    );
    assert!(block.contains("not enabled in this build"), "{block}");
    let block = spec_response("/delete-js", "500");
    assert!(
        block.contains("$ref: '#/components/schemas/DeleteJSResponse'"),
        "{block}"
    );
}

/// The body the spec describes: a build without plugins answers `/delete-js` with exactly
/// `success` and `error`.
#[cfg(not(feature = "plugins"))]
#[test]
fn delete_js_without_plugins_matches_the_spec() {
    let server = Server::spawn();
    let resp = server.get("/delete-js").expect("delete-js request");
    assert!(resp.starts_with("HTTP/1.1 404"), "{resp}");
    let body = resp.split_once("\r\n\r\n").map_or("", |(_, b)| b);
    assert_eq!(
        body,
        r#"{"error":"JavaScript plugins are not enabled in this build","success":false}"#
    );
}
