//! `sk_http_server` cross-origin policy, JSON-only request bodies and the admin token from
//! the environment (ADR-0019, BL-051). Servers run on 127.0.0.1 with one worker.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::time::Duration;

const ENV_VARS: [&str; 3] = [
    "SKILLET_AUTH_TOKEN",
    "SKILLET_ADMIN_TOKEN",
    "SKILLET_CORS_ORIGIN",
];

/// `sk_http_server` on a free port, killed on drop.
struct Server {
    child: std::process::Child,
    port: u16,
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn command(port: u16, args: &[&str], env: &[(&str, &str)]) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_sk_http_server"));
    cmd.arg(port.to_string())
        .args(["--threads", "1"])
        .args(args)
        .env(
            "SKILLET_HOOKS_DIR",
            std::env::temp_dir().join("skillet-server-cors-no-hooks"),
        )
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    for var in ENV_VARS {
        cmd.env_remove(var);
    }
    for (k, v) in env {
        cmd.env(k, v);
    }
    cmd
}

impl Server {
    fn start(args: &[&str], env: &[(&str, &str)]) -> Server {
        let port = free_port();
        let child = command(port, args, env).spawn().expect("spawn server");
        for _ in 0..200 {
            if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Server { child, port }
    }

    /// Send raw `head` (request line + headers, without the blank line) and `body`.
    fn raw(&self, head: &str, body: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", self.port)).expect("connect");
        s.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
        let req = format!(
            "{head}\r\nHost: localhost\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        s.write_all(req.as_bytes()).unwrap();
        let mut resp = String::new();
        let _ = s.read_to_string(&mut resp);
        resp
    }

    fn get(&self, path: &str, extra: &str) -> String {
        self.raw(&format!("GET {path} HTTP/1.1{extra}"), "")
    }

    fn preflight(&self) -> String {
        self.raw(
            "OPTIONS /eval HTTP/1.1\r\nOrigin: https://page.example\r\n\
             Access-Control-Request-Method: POST\r\n\
             Access-Control-Request-Headers: authorization, content-type",
            "",
        )
    }

    fn post_eval(&self, content_type: Option<&str>, extra: &str) -> String {
        let ct = content_type
            .map(|c| format!("\r\nContent-Type: {c}"))
            .unwrap_or_default();
        self.raw(
            &format!("POST /eval HTTP/1.1{ct}{extra}"),
            r#"{"expression": "=1 + 2"}"#,
        )
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn status(resp: &str) -> &str {
    resp.lines().next().unwrap_or("")
}

/// The value of response header `name`, case-insensitive.
fn header<'a>(resp: &'a str, name: &str) -> Option<&'a str> {
    let head = resp.split("\r\n\r\n").next().unwrap_or("");
    head.lines().skip(1).find_map(|l| {
        let (k, v) = l.split_once(':')?;
        k.trim().eq_ignore_ascii_case(name).then_some(v.trim())
    })
}

fn acao(resp: &str) -> Option<&str> {
    header(resp, "Access-Control-Allow-Origin")
}

fn assert_preflight_allows(resp: &str, origin: &str) {
    assert!(status(resp).contains("200"), "{resp}");
    assert_eq!(acao(resp), Some(origin), "{resp}");
    let allowed = header(resp, "Access-Control-Allow-Headers")
        .unwrap_or("")
        .to_ascii_lowercase();
    assert!(allowed.contains("authorization"), "{resp}");
    assert!(allowed.contains("content-type"), "{resp}");
    let methods = header(resp, "Access-Control-Allow-Methods").unwrap_or("");
    assert!(methods.contains("POST"), "{resp}");
}

#[test]
fn bl051_development_mode_sends_no_cors_headers() {
    let server = Server::start(&[], &[]);
    let resp = server.get("/health", "\r\nOrigin: https://page.example");
    assert!(status(&resp).contains("200"), "{resp}");
    assert_eq!(acao(&resp), None, "{resp}");

    let pre = server.preflight();
    assert!(status(&pre).contains("200"), "{pre}");
    assert_eq!(acao(&pre), None, "{pre}");
    assert_eq!(header(&pre, "Access-Control-Allow-Headers"), None, "{pre}");
}

#[test]
fn bl051_a_token_without_configured_origin_allows_any_origin() {
    let server = Server::start(&["--token", "cors-eval"], &[]);
    let resp = server.get("/health", "");
    assert_eq!(acao(&resp), Some("*"), "{resp}");
    // Also on errors, so a browser can read the 401.
    let resp = server.post_eval(Some("application/json"), "");
    assert!(status(&resp).contains("401"), "{resp}");
    assert_eq!(acao(&resp), Some("*"), "{resp}");
    assert_preflight_allows(&server.preflight(), "*");

    // An admin token alone counts too (the eval token inherits it).
    let server = Server::start(&[], &[("SKILLET_ADMIN_TOKEN", "cors-admin")]);
    assert_eq!(acao(&server.get("/health", "")), Some("*"));
}

#[test]
fn bl051_configured_origin_is_sent_exactly() {
    let origin = "https://app.example:8443";
    let server = Server::start(&["--cors-origin", origin], &[]);
    let resp = server.get("/health", "");
    assert_eq!(acao(&resp), Some(origin), "{resp}");
    assert_preflight_allows(&server.preflight(), origin);

    // From the environment, with a token: the configured value still wins over `*`.
    let server = Server::start(
        &["--token", "cors-eval"],
        &[("SKILLET_CORS_ORIGIN", "https://env.example")],
    );
    assert_eq!(
        acao(&server.get("/health", "")),
        Some("https://env.example")
    );

    // `*` is allowed explicitly, even without a token.
    let server = Server::start(&["--cors-origin", "*"], &[]);
    assert_eq!(acao(&server.get("/health", "")), Some("*"));

    // The flag wins over the env var.
    let server = Server::start(
        &["--cors-origin", origin],
        &[("SKILLET_CORS_ORIGIN", "https://env.example")],
    );
    assert_eq!(acao(&server.get("/health", "")), Some(origin));
}

#[test]
fn bl051_bodies_must_be_json() {
    let server = Server::start(&[], &[]);
    for ct in [
        None,
        Some("text/plain"),
        Some("application/x-www-form-urlencoded"),
    ] {
        let resp = server.post_eval(ct, "");
        assert!(status(&resp).contains("415"), "{ct:?}: {resp}");
        assert!(resp.contains("application/json"), "{resp}");
    }
    for ct in ["application/json", "Application/JSON; charset=utf-8"] {
        let resp = server.post_eval(Some(ct), "");
        assert!(status(&resp).contains("200"), "{ct}: {resp}");
    }
    // A request without a body is not checked.
    let resp = server.raw("DELETE /cache HTTP/1.1", "");
    assert!(status(&resp).contains("200"), "{resp}");
}

#[test]
fn bl051_browser_multipart_upload_needs_an_admin_token() {
    let server = Server::start(&[], &[]);
    let body = "--b\r\nContent-Disposition: form-data; name=\"filename\"\r\n\r\nx.js\r\n--b--\r\n";
    let resp = server.raw(
        "POST /upload-js HTTP/1.1\r\nContent-Type: multipart/form-data; boundary=b\r\n\
         Origin: https://evil.example",
        body,
    );
    assert!(status(&resp).contains("415"), "{resp}");
}

#[test]
fn bl051_admin_token_from_env_guards_admin_endpoints() {
    let server = Server::start(&[], &[("SKILLET_ADMIN_TOKEN", "env-admin")]);
    for auth in ["", "\r\nAuthorization: Bearer wrong"] {
        let resp = server.raw(&format!("DELETE /cache HTTP/1.1{auth}"), "");
        assert!(status(&resp).contains("401"), "{auth:?}: {resp}");
        let resp = server.get("/list-js", auth);
        assert!(status(&resp).contains("401"), "{auth:?}: {resp}");
    }
    let ok = "\r\nAuthorization: Bearer env-admin";
    let resp = server.raw(&format!("DELETE /cache HTTP/1.1{ok}"), "");
    assert!(status(&resp).contains("200"), "{resp}");
}

#[test]
fn bl051_admin_token_flag_wins_over_env() {
    let server = Server::start(
        &["--admin-token", "flag-admin"],
        &[("SKILLET_ADMIN_TOKEN", "env-admin")],
    );
    let resp = server.raw(
        "DELETE /cache HTTP/1.1\r\nAuthorization: Bearer env-admin",
        "",
    );
    assert!(status(&resp).contains("401"), "{resp}");
    let resp = server.raw(
        "DELETE /cache HTTP/1.1\r\nAuthorization: Bearer flag-admin",
        "",
    );
    assert!(status(&resp).contains("200"), "{resp}");
}

#[test]
fn bl051_empty_env_values_are_rejected_at_start_up() {
    for var in ["SKILLET_ADMIN_TOKEN", "SKILLET_CORS_ORIGIN"] {
        let status = command(free_port(), &[], &[(var, "")])
            .status()
            .expect("run server");
        assert!(!status.success(), "{var}=\"\" must not start the server");
    }
    let status = command(free_port(), &["--cors-origin", "a b"], &[])
        .status()
        .expect("run server");
    assert!(!status.success());
}
