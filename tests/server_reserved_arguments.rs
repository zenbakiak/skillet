//! BL-077 (threat model 2.2): `sk_http_server` exposes the whole request variables object as
//! `:arguments` for `JQ`. A variable named `arguments` must not replace it; its value stays
//! reachable inside the object. `sk_server` builds no `:arguments`, so there the key is an
//! ordinary variable. Two keys that sanitize to the same variable (`a-b`, `a_b`) are rejected
//! by `sk_http_server` like an invalid variable; `sk_server` does not sanitize keys, so there
//! they stay two variables. Servers run on 127.0.0.1 with one worker.

mod common;

use common::ServerProcess;
use std::io::{BufRead, BufReader, Read, Write};

struct Server(ServerProcess);

impl Server {
    fn spawn(bin: &str, args: impl Fn(u16) -> Vec<String>) -> Server {
        Server(ServerProcess::start(|port| {
            let hooks = std::env::temp_dir().join("skillet-reserved-arguments-no-hooks");
            let mut cmd = std::process::Command::new(bin);
            cmd.args(args(port))
                .env("SKILLET_HOOKS_DIR", hooks)
                .env("SKILLET_BIND_HOST", "127.0.0.1")
                .env_remove("SKILLET_AUTH_TOKEN")
                .env_remove("SKILLET_ADMIN_TOKEN")
                .stdout(std::process::Stdio::null());
            cmd
        }))
    }

    fn http_server() -> Server {
        Server::spawn(env!("CARGO_BIN_EXE_sk_http_server"), |port| {
            vec![port.to_string(), "--threads".into(), "1".into()]
        })
    }

    /// One HTTP request; returns the JSON body.
    fn http(&self, request_line: &str, body: &str) -> serde_json::Value {
        self.http_status(request_line, body).1
    }

    /// One HTTP request; returns the status code and the JSON body.
    fn http_status(&self, request_line: &str, body: &str) -> (u16, serde_json::Value) {
        let mut s = self.0.connect();
        let req = format!(
            "{request_line} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        s.write_all(req.as_bytes()).unwrap();
        let mut resp = String::new();
        let _ = s.read_to_string(&mut resp);
        let text = resp.split_once("\r\n\r\n").map_or(&*resp, |(_, b)| b);
        let status = resp
            .split_whitespace()
            .nth(1)
            .and_then(|c| c.parse().ok())
            .unwrap_or_else(|| panic!("no status: {resp}"));
        let body = serde_json::from_str(text).unwrap_or_else(|e| panic!("{e}: {resp}"));
        (status, body)
    }

    fn post_eval(&self, expression: &str, arguments: serde_json::Value) -> serde_json::Value {
        let body = serde_json::json!({ "expression": expression, "arguments": arguments });
        self.http("POST /eval", &body.to_string())
    }
}

fn colliding() -> serde_json::Value {
    serde_json::json!({ "x": 1, "arguments": { "a": 2 } })
}

#[test]
fn http_post_keeps_the_reserved_arguments_object() {
    let server = Server::http_server();
    let r = server.post_eval("=:arguments", colliding());
    assert_eq!(r["success"], true, "{r}");
    assert_eq!(r["result"], colliding(), "{r}");

    let r = server.post_eval("=JQ(:arguments, \"$.x\")", colliding());
    assert_eq!(r["result"].as_f64(), Some(1.0), "{r}");
}

#[test]
fn http_post_colliding_key_is_reachable_inside_arguments() {
    let server = Server::http_server();
    for expr in ["=:arguments.arguments", "=JQ(:arguments, \"$.arguments\")"] {
        let r = server.post_eval(expr, colliding());
        assert_eq!(r["success"], true, "{expr}: {r}");
        assert_eq!(r["result"], serde_json::json!({ "a": 2 }), "{expr}: {r}");
    }
    let r = server.post_eval("=:x + :arguments.x", colliding());
    assert_eq!(r["result"].as_f64(), Some(2.0), "{r}");
}

#[test]
fn http_get_query_variable_named_arguments_does_not_replace_it() {
    let server = Server::http_server();
    let r = server.http("GET /eval?expr=%3D%3Aarguments&arguments=5&x=1", "");
    assert_eq!(r["success"], true, "{r}");
    assert_eq!(
        r["result"],
        serde_json::json!({ "arguments": 5.0, "x": 1.0 }),
        "{r}"
    );
}

fn sk_server() -> Server {
    Server::spawn(env!("CARGO_BIN_EXE_sk_server"), |port| {
        vec![port.to_string(), "1".into()]
    })
}

/// One `sk_server` request line; returns the JSON response.
fn sk_server_eval(server: &Server, body: serde_json::Value) -> serde_json::Value {
    let mut s = server.0.connect();
    s.write_all(format!("{body}\n").as_bytes()).unwrap();
    let mut line = String::new();
    let _ = BufReader::new(s).read_line(&mut line);
    serde_json::from_str(&line).unwrap_or_else(|e| panic!("{e}: {line}"))
}

#[test]
fn sk_server_arguments_is_an_ordinary_variable() {
    let server = sk_server();
    let body =
        serde_json::json!({ "expression": "=:arguments + 1", "variables": { "arguments": 41 } });
    let r = sk_server_eval(&server, body);
    assert_eq!(r["success"], true, "{r}");
    assert_eq!(r["result"].as_f64(), Some(42.0), "{r}");
}

const KEY_COLLISION: &str = r#"JSON keys "a-b" and "a_b" both map to variable :a_b"#;

/// BL-077: colliding keys answer 400 with `success: false`, like a variable that fails to
/// convert (too deep, below), instead of one key silently replacing the other.
#[test]
fn http_post_rejects_keys_that_map_to_the_same_variable() {
    let server = Server::http_server();
    let body = serde_json::json!({ "expression": "=:a_b", "arguments": { "a_b": 1, "a-b": 2 } });
    let (status, r) = server.http_status("POST /eval", &body.to_string());
    assert_eq!(status, 400, "{r}");
    assert_eq!(r["success"], false, "{r}");
    assert_eq!(r["result"], serde_json::Value::Null, "{r}");
    assert_eq!(r["error"], KEY_COLLISION, "{r}");

    let mut deep = serde_json::json!(1);
    for _ in 0..200 {
        deep = serde_json::json!([deep]);
    }
    let body = format!(r#"{{"expression": "=1", "arguments": {{"d": {deep}}}}}"#);
    let (status, r) = server.http_status("POST /eval", &body);
    assert_eq!(status, 400, "{r}");
    assert_eq!(r["success"], false, "{r}");
}

#[test]
fn http_get_rejects_query_keys_that_map_to_the_same_variable() {
    let server = Server::http_server();
    let (status, r) = server.http_status("GET /eval?expr=%3D%3Aa_b&a-b=1&a_b=2", "");
    assert_eq!(status, 400, "{r}");
    assert_eq!(r["success"], false, "{r}");
    assert_eq!(r["error"], KEY_COLLISION, "{r}");
}

#[test]
fn http_keys_that_stay_distinct_after_sanitizing_still_work() {
    let server = Server::http_server();
    let args = serde_json::json!({ "first-name": "Ada", "arguments": 1, "a_b": 2 });
    let r = server.post_eval("=:first_name & :a_b & :arguments.arguments", args);
    assert_eq!(r["success"], true, "{r}");
    assert_eq!(r["result"], "Ada21", "{r}");
}

/// `sk_server` uses keys as written, so `a-b` and `a_b` are two variables.
#[test]
fn sk_server_does_not_sanitize_keys_so_they_do_not_collide() {
    let server = sk_server();
    let body = serde_json::json!({ "expression": "=:a_b", "variables": { "a-b": 1, "a_b": 2 } });
    let r = sk_server_eval(&server, body);
    assert_eq!(r["success"], true, "{r}");
    assert_eq!(r["result"].as_f64(), Some(2.0), "{r}");
}
