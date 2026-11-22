//! BL-085 (SEC-2): the `sk_http_server` result cache never keeps the result of an expression
//! that reads the clock, and `include_variables` is part of the key, so a cached entry always has
//! the shape the request asks for. Pure expressions are still cached.

mod common;

use common::ServerProcess;
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::time::Duration;

fn spawn() -> ServerProcess {
    ServerProcess::start(|port| {
        let hooks = std::env::temp_dir().join("skillet-result-cache-no-hooks");
        let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_sk_http_server"));
        cmd.args([port.to_string(), "--threads".into(), "1".into()])
            .env("SKILLET_HOOKS_DIR", hooks)
            .env("SKILLET_BIND_HOST", "127.0.0.1")
            .env_remove("SKILLET_AUTH_TOKEN")
            .env_remove("SKILLET_ADMIN_TOKEN")
            .stdout(std::process::Stdio::null());
        cmd
    })
}

/// One HTTP request; returns the JSON body.
fn request(server: &ServerProcess, head: &str, body: &str) -> Value {
    let mut s = server.connect();
    let req = format!(
        "{head}\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    s.write_all(req.as_bytes()).unwrap();
    let mut resp = String::new();
    let _ = s.read_to_string(&mut resp);
    let text = resp.split_once("\r\n\r\n").map_or("", |(_, b)| b);
    serde_json::from_str(text).unwrap_or_else(|e| panic!("{e}: {resp}"))
}

fn eval(server: &ServerProcess, expression: &str, include_variables: Option<bool>) -> Value {
    let mut body = json!({ "expression": expression });
    if let Some(flag) = include_variables {
        body["include_variables"] = json!(flag);
    }
    request(server, "POST /eval HTTP/1.1", &body.to_string())
}

/// `(hits, misses, entries)` of the result cache.
fn cache(server: &ServerProcess) -> (u64, u64, u64) {
    let c = &request(server, "GET /health HTTP/1.1", "")["cache_stats"];
    let n = |k: &str| c[k].as_u64().unwrap_or_else(|| panic!("{k}: {c}"));
    (n("hits"), n("misses"), n("entries"))
}

#[test]
fn bl085_now_is_evaluated_on_every_request() {
    let server = spawn();
    let first = eval(&server, "=NOW()", None);
    assert_eq!(first["success"], true, "{first}");
    std::thread::sleep(Duration::from_millis(1100));
    let second = eval(&server, "=NOW()", None);
    assert_eq!(second["success"], true, "{second}");
    assert_ne!(
        first["result"], second["result"],
        "NOW() was served from the cache"
    );
    assert_eq!(cache(&server), (0, 2, 0));
}

#[test]
fn bl085_volatile_expressions_are_never_stored() {
    let server = spawn();
    for expr in [
        "=IF(TRUE, NOW(), 1)",
        "=[1, 2, 3].map(:x + DAY(TODAY())).sum()",
        "=DATE()",
        "=TIME()",
        // No RAND builtin exists: an error, and errors are never cached either.
        "=RAND()",
    ] {
        let (hits, misses, entries) = cache(&server);
        for _ in 0..2 {
            eval(&server, expr, None);
        }
        assert_eq!(cache(&server), (hits, misses + 2, entries), "{expr}");
    }
}

#[test]
fn bl085_pure_expressions_are_still_cached() {
    let server = spawn();
    for _ in 0..3 {
        let r = eval(&server, "=SUM(1, 2, 3) * 2", None);
        assert_eq!(r["result"], 12.0, "{r}");
    }
    assert_eq!(cache(&server), (2, 1, 1));
}

#[test]
fn bl085_include_variables_is_part_of_the_key() {
    let server = spawn();
    // Without first, then with, then without again: each answer has its own shape.
    let expr = "=:x := 5; :x + 1";
    for (flag, want_vars) in [
        (None, Value::Null),
        (Some(true), json!({"x": 5.0})),
        (Some(false), Value::Null),
        (Some(true), json!({"x": 5.0})),
    ] {
        let r = eval(&server, expr, flag);
        assert_eq!(r["result"], 6.0, "{r}");
        assert_eq!(r["variables"], want_vars, "include_variables {flag:?}: {r}");
    }
    // `false` and absent share an entry; `true` has its own. Both were then hit.
    assert_eq!(cache(&server), (2, 2, 2));

    // With first, then without.
    let expr = "=:y := 2; :y * 10";
    let r = eval(&server, expr, Some(true));
    assert_eq!(r["variables"], json!({"y": 2.0}), "{r}");
    let r = eval(&server, expr, None);
    assert_eq!(
        (&r["result"], &r["variables"]),
        (&json!(20.0), &Value::Null),
        "{r}"
    );
}
