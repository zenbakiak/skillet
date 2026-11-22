//! BL-012 / ADR-0021: the servers cache parsed expressions per worker thread. A repeated
//! expression must still evaluate with each request's own variables, and a parse error must be
//! reported every time. Servers run on 127.0.0.1 with one worker, so every request after the
//! first one uses the cached tree.

mod common;

use common::ServerProcess;
use std::io::{BufRead, BufReader, Read, Write};

/// A server binary on a free 127.0.0.1 port, killed on drop.
struct Server(ServerProcess);

impl Server {
    fn spawn(bin: &str, args: impl Fn(u16) -> Vec<String>) -> Server {
        Server(ServerProcess::start(|port| {
            let hooks = std::env::temp_dir().join("skillet-parse-cache-no-hooks");
            let mut cmd = std::process::Command::new(bin);
            cmd.args(args(port))
                .env("SKILLET_HOOKS_DIR", hooks)
                .env("SKILLET_BIND_HOST", "127.0.0.1")
                .env_remove("SKILLET_AUTH_TOKEN")
                .stdout(std::process::Stdio::null());
            cmd
        }))
    }

    /// One request to the server: an HTTP POST /eval for `sk_http_server`, one line for
    /// `sk_server`. Returns the JSON response body.
    fn eval(&self, http: bool, body: &str) -> serde_json::Value {
        let mut s = self.0.connect();
        let text = if http {
            let req = format!(
                "POST /eval HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            s.write_all(req.as_bytes()).unwrap();
            let mut resp = String::new();
            let _ = s.read_to_string(&mut resp);
            resp.split_once("\r\n\r\n")
                .map(|(_, b)| b.to_string())
                .unwrap_or(resp)
        } else {
            s.write_all(format!("{body}\n").as_bytes()).unwrap();
            let mut line = String::new();
            let _ = BufReader::new(s).read_line(&mut line);
            line
        };
        serde_json::from_str(&text).unwrap_or_else(|e| panic!("{e}: {text}"))
    }
}

/// Request body; the HTTP server calls the variables `arguments`, `sk_server` `variables`.
fn body(http: bool, expression: &str, vars: serde_json::Value, include_variables: bool) -> String {
    let field = if http { "arguments" } else { "variables" };
    let mut b = serde_json::json!({ "expression": expression, field: vars });
    if include_variables {
        b["include_variables"] = serde_json::json!(true);
    }
    b.to_string()
}

fn check_repeated_expressions(server: &Server, http: bool) {
    // Same text, new variables each time (and so never a result-cache hit).
    for a in 0..20 {
        let b = 100 - a;
        let r = server.eval(
            http,
            &body(
                http,
                ":a * :b + 1",
                serde_json::json!({"a": a, "b": b}),
                false,
            ),
        );
        assert_eq!(r["success"], true, "{r}");
        assert_eq!(r["result"].as_f64(), Some((a * b + 1) as f64), "{r}");
    }

    // Assignments and `;` go through the other evaluation path.
    for a in 0..5 {
        let expr = ":t := :a * 2; :t + :b";
        let r = server.eval(
            http,
            &body(http, expr, serde_json::json!({"a": a, "b": 1}), http),
        );
        assert_eq!(r["result"].as_f64(), Some((a * 2 + 1) as f64), "{r}");
        if http {
            assert_eq!(r["variables"]["t"].as_f64(), Some((a * 2) as f64), "{r}");
        }
    }

    // A parse error is reported every time, and does not disturb the cached trees.
    for _ in 0..3 {
        let r = server.eval(http, &body(http, "2 +", serde_json::json!({}), false));
        assert_eq!(r["success"], false, "{r}");
        assert_eq!(r["error"], "Unexpected token: Eof at position 2", "{r}");
    }
    let r = server.eval(
        http,
        &body(
            http,
            ":a * :b + 1",
            serde_json::json!({"a": 7, "b": 6}),
            false,
        ),
    );
    assert_eq!(r["result"].as_f64(), Some(43.0), "{r}");

    // A missing variable on a cached tree is an error for this request only.
    let r = server.eval(
        http,
        &body(http, ":a * :b + 1", serde_json::json!({"a": 7}), false),
    );
    assert_eq!(r["success"], false, "{r}");
    assert_eq!(r["error"], "Missing variable: :b", "{r}");
}

#[test]
fn bl012_sk_http_server_repeated_expressions_use_each_requests_variables() {
    let server = Server::spawn(env!("CARGO_BIN_EXE_sk_http_server"), |port| {
        vec![port.to_string(), "--threads".into(), "1".into()]
    });
    check_repeated_expressions(&server, true);
}

#[test]
fn bl012_sk_server_repeated_expressions_use_each_requests_variables() {
    let server = Server::spawn(env!("CARGO_BIN_EXE_sk_server"), |port| {
        vec![port.to_string(), "1".into()]
    });
    check_repeated_expressions(&server, false);
}
