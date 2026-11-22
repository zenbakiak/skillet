//! BL-077 (threat model 2.2): `sk_http_server` exposes the whole request variables object as
//! `:arguments` for `JQ`. A variable named `arguments` must not replace it; its value stays
//! reachable inside the object. `sk_server` builds no `:arguments`, so there the key is an
//! ordinary variable. Servers run on 127.0.0.1 with one worker.

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
        let mut s = self.0.connect();
        let req = format!(
            "{request_line} HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        s.write_all(req.as_bytes()).unwrap();
        let mut resp = String::new();
        let _ = s.read_to_string(&mut resp);
        let text = resp.split_once("\r\n\r\n").map_or(&*resp, |(_, b)| b);
        serde_json::from_str(text).unwrap_or_else(|e| panic!("{e}: {resp}"))
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

#[test]
fn sk_server_arguments_is_an_ordinary_variable() {
    let server = Server::spawn(env!("CARGO_BIN_EXE_sk_server"), |port| {
        vec![port.to_string(), "1".into()]
    });
    let mut s = server.0.connect();
    let body =
        serde_json::json!({ "expression": "=:arguments + 1", "variables": { "arguments": 41 } });
    s.write_all(format!("{body}\n").as_bytes()).unwrap();
    let mut line = String::new();
    let _ = BufReader::new(s).read_line(&mut line);
    let r: serde_json::Value =
        serde_json::from_str(&line).unwrap_or_else(|e| panic!("{e}: {line}"));
    assert_eq!(r["success"], true, "{r}");
    assert_eq!(r["result"].as_f64(), Some(42.0), "{r}");
}
