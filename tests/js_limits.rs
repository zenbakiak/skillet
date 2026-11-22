//! ADR-0017: JS plugins run under the evaluation deadline, a 64 MiB heap and a bounded result
//! depth, and the process survives hostile hooks. The per-call cap, memory, recursion and
//! `httpGet` cases are unit tests in `src/js_plugin.rs` (they need the test-only short cap);
//! these cover the public library path, `sk`, and `sk_http_server`.
#![cfg(feature = "plugins")]

use skillet::js_plugin::JavaScriptFunction;
use skillet::{CustomFunction, EvalLimits, Value};
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::time::{Duration, Instant};

const LOOP_HOOK: &str = "// @name: JSLIMITSLOOP\n// @min_args: 0\n\
    function execute(args) { for (;;) { try { while (true) {} } catch (e) {} } }\n";

const DEEP_HOOK: &str = "// @name: JSLIMITSDEEP\n// @min_args: 0\n\
    function execute(args) { var a = [1]; for (var i = 1; i < 200000; i++) { a = [a]; } return a; }\n";

const CYCLIC_HOOK: &str = "// @name: JSLIMITSCYCLE\n// @min_args: 0\n\
    function execute(args) { var a = [1]; a.push(a); return a; }\n";

/// Calls a looping JS hook and swallows its error, as a formula that ignores errors would.
struct SwallowsJs(JavaScriptFunction);

impl CustomFunction for SwallowsJs {
    fn name(&self) -> &str {
        "JSLIMITSSWALLOW"
    }
    fn min_args(&self) -> usize {
        0
    }
    fn max_args(&self) -> Option<usize> {
        Some(0)
    }
    fn execute(&self, _args: Vec<Value>) -> Result<Value, skillet::Error> {
        Ok(self.0.execute(vec![]).unwrap_or(Value::Number(1.0)))
    }
}

#[test]
fn deadline_stops_js_and_a_swallowed_error_still_times_out() {
    let hook = JavaScriptFunction::parse_js_function(LOOP_HOOK).unwrap();
    skillet::register_function(Box::new(SwallowsJs(hook))).unwrap();
    let mut limits = EvalLimits::default();
    limits.timeout = Some(Duration::from_millis(200));
    let started = Instant::now();
    let result = skillet::with_limits(limits, || {
        skillet::evaluate_with_custom("=JSLIMITSSWALLOW() + 1", &HashMap::new())
    });
    skillet::unregister_function("JSLIMITSSWALLOW");
    assert_eq!(
        result.unwrap_err().message,
        "Evaluation timed out (max 200 ms)"
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );
}

fn hooks_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("loop.js"), LOOP_HOOK).unwrap();
    std::fs::write(dir.path().join("deep.js"), DEEP_HOOK).unwrap();
    std::fs::write(dir.path().join("cycle.js"), CYCLIC_HOOK).unwrap();
    dir
}

#[test]
fn sk_survives_deep_and_cyclic_hook_results() {
    let hooks = hooks_dir();
    for expr in ["=JSLIMITSDEEP()", "=JSLIMITSCYCLE()"] {
        let out = std::process::Command::new(env!("CARGO_BIN_EXE_sk"))
            .arg(expr)
            .env("SKILLET_HOOKS_DIR", hooks.path())
            .output()
            .expect("run sk");
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
        // Exit 2 is an evaluation error; before ADR-0017 the stack overflow aborted (134).
        assert_eq!(out.status.code(), Some(2), "{expr}: {text}");
        assert!(
            text.contains("Expression nested too deeply (max 128)"),
            "{expr}: {text}"
        );
    }
}

const ADMIN: &str = "js-limits-admin";

/// `sk_http_server` with one worker on a free 127.0.0.1 port, killed on drop.
struct Server {
    child: std::process::Child,
    port: u16,
}

impl Server {
    fn start(hooks: &Path) -> Server {
        Server::start_with(hooks, &[])
    }

    fn start_with(hooks: &Path, extra: &[&str]) -> Server {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let child = std::process::Command::new(env!("CARGO_BIN_EXE_sk_http_server"))
            .args([&port.to_string(), "--threads", "1", "--admin-token", ADMIN])
            .args(extra)
            .env_remove("SKILLET_EVAL_TIMEOUT")
            .env("SKILLET_HOOKS_DIR", hooks)
            .env("SKILLET_BIND_HOST", "127.0.0.1")
            .env_remove("SKILLET_AUTH_TOKEN")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn server");
        for _ in 0..200 {
            if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Server { child, port }
    }

    fn request(&self, method: &str, path: &str, body: &str) -> (String, Duration) {
        let started = Instant::now();
        let mut s = TcpStream::connect(("127.0.0.1", self.port)).expect("connect");
        s.set_read_timeout(Some(Duration::from_secs(60))).unwrap();
        let req = format!(
            "{method} {path} HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {ADMIN}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        s.write_all(req.as_bytes()).unwrap();
        let mut resp = String::new();
        let _ = s.read_to_string(&mut resp);
        (resp, started.elapsed())
    }

    fn eval(&self, expr: &str) -> (String, Duration) {
        let body = serde_json::json!({ "expression": expr }).to_string();
        self.request("POST", "/eval", &body)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn status(resp: &str) -> &str {
    resp.split_whitespace().nth(1).unwrap_or("")
}

/// A server deadline shorter than the 10 s per-call cap stops the hook first (ADR-0017 §3).
/// The default deadline (60 s) is longer, so it is set to 2 s here.
#[test]
fn server_deadline_stops_a_looping_hook_and_frees_the_worker() {
    let hooks = hooks_dir();
    let server = Server::start_with(hooks.path(), &["--eval-timeout", "2"]);
    let (resp, took) = server.eval("=JSLIMITSLOOP()");
    assert_eq!(status(&resp), "400", "{resp}");
    assert!(resp.contains("Evaluation timed out (max 2 s)"), "{resp}");
    assert!(
        took >= Duration::from_secs(2) && took < Duration::from_secs(20),
        "{took:?}"
    );
    // The single worker is free again, and hostile results did not abort the process.
    for expr in ["=JSLIMITSDEEP()", "=JSLIMITSCYCLE()"] {
        let (resp, _) = server.eval(expr);
        assert_eq!(status(&resp), "400", "{expr}: {resp}");
        assert!(
            resp.contains("Expression nested too deeply (max 128)"),
            "{expr}: {resp}"
        );
    }
    let (resp, _) = server.eval("=2 + 3");
    assert_eq!(status(&resp), "200", "{resp}");
}

/// With the default 60 s deadline the 10 s per-call cap comes first, and the worker is free
/// again after it (ADR-0017 §3: the request deadline or 10 s, whichever is sooner).
#[test]
fn per_call_cap_stops_a_looping_hook_before_the_default_deadline() {
    let hooks = hooks_dir();
    let server = Server::start(hooks.path());
    let (resp, took) = server.eval("=JSLIMITSLOOP()");
    assert_eq!(status(&resp), "400", "{resp}");
    assert!(
        resp.contains("JS function JSLIMITSLOOP timed out (max 10 s)"),
        "{resp}"
    );
    assert!(
        took >= Duration::from_secs(9) && took < Duration::from_secs(40),
        "{took:?}"
    );
    let (resp, _) = server.eval("=2 + 3");
    assert_eq!(status(&resp), "200", "{resp}");
}

/// Upload validation runs the `@example` under the same limits: a looping example is stopped by
/// the 10 s per-call cap (there is no evaluation deadline during validation).
#[test]
fn upload_with_a_looping_example_answers() {
    let hooks = tempfile::tempdir().unwrap();
    let server = Server::start(hooks.path());
    let js = "// @name: JSLIMITSUPLOAD\n// @min_args: 0\n// @example: JSLIMITSUPLOAD() returns 1\n\
              function execute(args) { while (true) {} }\n";
    let body = serde_json::json!({ "filename": "upload_loop.js", "js_code": js }).to_string();
    let (resp, took) = server.request("POST", "/upload-js", &body);
    assert!(
        resp.contains("JS function JSLIMITSUPLOAD timed out (max 10 s)"),
        "{resp}"
    );
    assert!(took < Duration::from_secs(20), "{took:?}");
}
