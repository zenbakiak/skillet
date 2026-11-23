//! Configurable evaluation deadline and budget of `sk_http_server` and `sk_server`
//! (ADR-0011 amendment 4, BL-075): `--eval-timeout` / `SKILLET_EVAL_TIMEOUT` and
//! `--max-eval-bytes` / `SKILLET_MAX_EVAL_BYTES`, the flag winning over the env var.
//! Servers run on 127.0.0.1 with one worker. The timing tests use a 1 s deadline on work that
//! takes far longer; the error text shows which deadline stopped it.

mod common;

use common::{ServerProcess, SLOW};
use std::io::{BufRead, BufReader, Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const HTTP: &str = env!("CARGO_BIN_EXE_sk_http_server");
const LINE: &str = env!("CARGO_BIN_EXE_sk_server");
const TIMEOUT_ENV: &str = "SKILLET_EVAL_TIMEOUT";
const BYTES_ENV: &str = "SKILLET_MAX_EVAL_BYTES";

/// `bin` with a clean environment plus `env`; `args` after the port and the one-worker option.
fn command(bin: &str, port: u16, args: &[&str], env: &[(&str, &str)]) -> Command {
    let hooks = std::env::temp_dir().join("skillet-server-eval-limits-no-hooks");
    let mut cmd = Command::new(bin);
    cmd.arg(port.to_string());
    if bin == HTTP {
        cmd.args(["--threads", "1"]);
    } else {
        cmd.arg("1");
    }
    cmd.args(args)
        .env("SKILLET_HOOKS_DIR", hooks)
        .env("SKILLET_BIND_HOST", "127.0.0.1")
        .env_remove("SKILLET_AUTH_TOKEN")
        .env_remove("SKILLET_ADMIN_TOKEN")
        .env_remove("SKILLET_CORS_ORIGIN")
        .env_remove("SKILLET_MAX_CONNECTIONS_PER_IP")
        .env_remove(TIMEOUT_ENV)
        .env_remove(BYTES_ENV)
        .envs(env.iter().copied())
        .stdout(Stdio::null());
    cmd
}

fn start(bin: &str, args: &[&str], env: &[(&str, &str)]) -> ServerProcess {
    ServerProcess::start(|port| command(bin, port, args, env))
}

/// Evaluate `expr` with string variables `s` and `t`; returns the raw response and its time.
fn eval(server: &ServerProcess, http: bool, expr: &str, text: &str) -> (String, Duration) {
    let started = Instant::now();
    let mut s = server.connect();
    if http {
        let body = serde_json::json!({"expression": expr, "arguments": {"s": text, "t": text}})
            .to_string();
        let req = format!(
            "POST /eval HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        s.write_all(req.as_bytes()).unwrap();
        let mut resp = String::new();
        let _ = s.read_to_string(&mut resp);
        (resp, started.elapsed())
    } else {
        let body = serde_json::json!({"expression": expr, "variables": {"s": text, "t": text}});
        s.write_all(format!("{body}\n").as_bytes()).unwrap();
        let mut line = String::new();
        let _ = BufReader::new(s).read_line(&mut line);
        (line, started.elapsed())
    }
}

fn head(resp: &str) -> &str {
    &resp[..resp.len().min(300)]
}

fn ones(n: usize) -> String {
    format!("[{}]", vec!["1"; n].join(","))
}

/// 1M iterations × 4 comparisons of two 100 KB strings: many seconds of work that the budget
/// only ticks for (comparisons borrow their operands), so only the deadline stops it.
fn heavy() -> (String, String) {
    let cmp = "((:s == :t) == (:t == :s)) == ((:s == :t) == (:t == :s))";
    let expr = format!(
        "{}.map({}.map({cmp}).length()).sum()",
        ones(1000),
        ones(1000)
    );
    (expr, "x".repeat(100_000))
}

/// The configured 1 s deadline stops the evaluation, the error shows it, and the single
/// worker serves the next request.
fn times_out_after_one_second(server: &ServerProcess, http: bool) {
    let (expr, text) = heavy();
    let (resp, took) = eval(server, http, &expr, &text);
    if http {
        assert!(resp.starts_with("HTTP/1.1 400"), "{}", head(&resp));
    } else {
        assert!(resp.contains("\"success\":false"), "{}", head(&resp));
    }
    assert!(
        resp.contains("Evaluation timed out (max 1 s)"),
        "{}",
        head(&resp)
    );
    // At least the deadline. `max 1 s` above already proves the configured value is used, so
    // the upper bound only catches a hang and stays loose for loaded machines.
    assert!(
        took >= Duration::from_secs(1) && took < Duration::from_secs(45),
        "{took:?}"
    );
    let (resp, _) = eval(server, http, "2 + 3", "");
    assert!(resp.contains("5"), "{}", head(&resp));
    assert!(!resp.contains("\"success\":false"), "{}", head(&resp));
}

#[test]
fn sk_http_server_uses_the_eval_timeout_flag() {
    let server = start(HTTP, &["--eval-timeout", "1"], &[]);
    times_out_after_one_second(&server, true);
}

/// The flag wins over the env var end to end: the env var asks for 120 s.
#[test]
fn sk_server_uses_the_eval_timeout_flag_over_the_env_var() {
    let server = start(LINE, &["--eval-timeout", "1"], &[(TIMEOUT_ENV, "120")]);
    times_out_after_one_second(&server, false);
}

/// 2 MB pre-charged by `SUBSTITUTE` is over a 1 MiB budget and well within the default.
fn budget_expr() -> String {
    format!(
        "SUBSTITUTE(\"{}\", \"a\", \"{}\").length()",
        "a".repeat(2000),
        "b".repeat(1000)
    )
}

#[test]
fn both_servers_use_the_configured_budget() {
    let expr = budget_expr();
    for (bin, http, args, env) in [
        (HTTP, true, vec!["--max-eval-bytes", "1M"], vec![]),
        (LINE, false, vec![], vec![(BYTES_ENV, "1M")]),
    ] {
        let server = start(bin, &args, &env);
        let (resp, _) = eval(&server, http, &expr, "");
        assert!(
            resp.contains("Evaluation budget exceeded (max 1 MB)"),
            "{bin}: {}",
            head(&resp)
        );
        drop(server);
        let server = start(bin, &[], &[]);
        let (resp, _) = eval(&server, http, &expr, "");
        assert!(resp.contains("2000000"), "{bin}: {}", head(&resp));
    }
}

/// Send `expr` with the variables `vars` (a JSON object) and return the raw response.
fn eval_vars(server: &ServerProcess, http: bool, expr: &str, vars: serde_json::Value) -> String {
    let mut s = server.connect();
    if http {
        let body = serde_json::json!({"expression": expr, "arguments": vars}).to_string();
        let req = format!(
            "POST /eval HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        s.write_all(req.as_bytes()).unwrap();
        let mut resp = String::new();
        let _ = s.read_to_string(&mut resp);
        resp
    } else {
        let body = serde_json::json!({"expression": expr, "variables": vars});
        s.write_all(format!("{body}\n").as_bytes()).unwrap();
        let mut line = String::new();
        let _ = BufReader::new(s).read_line(&mut line);
        line
    }
}

fn get(server: &ServerProcess, path_and_query: &str) -> String {
    let mut s = server.connect();
    let req =
        format!("GET {path_and_query} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n");
    s.write_all(req.as_bytes()).unwrap();
    let mut resp = String::new();
    let _ = s.read_to_string(&mut resp);
    resp
}

/// The response body (HTTP) or line (`sk_server`) up to and including the error text.
fn error_shape(resp: &str, http: bool) -> &str {
    let body = if http {
        &resp[resp.find("\r\n\r\n").map_or(0, |i| i + 4)..]
    } else {
        resp
    };
    &body[..body.find("\",").map_or(body.len(), |i| i + 1)]
}

/// ADR-0011 amendment 5 (BL-086): request variables are charged to the configured budget, in the
/// evaluation's scope. With a 64 KiB budget, 1900 numbers fit (HTTP: 34 × 1900 + 90 with
/// `:arguments`; `sk_server`: 32 × 1900 + 32) and 2100 do not. The error has the same status and
/// shape as a budget error from the evaluation itself.
#[test]
fn both_servers_charge_request_variables() {
    let zeros = |n: usize| serde_json::json!({ "nums": vec![0; n] });
    let too_big = "Evaluation budget exceeded (max 65536 bytes)";
    for (bin, http) in [(HTTP, true), (LINE, false)] {
        let server = start(bin, &["--max-eval-bytes", "64K"], &[]);
        let resp = eval_vars(&server, http, ":nums.length()", zeros(1900));
        assert!(resp.contains("1900"), "{bin}: {}", head(&resp));
        assert!(
            !resp.contains("\"success\":false"),
            "{bin}: {}",
            head(&resp)
        );

        let over = eval_vars(&server, http, ":nums.length()", zeros(2100));
        assert!(over.contains(too_big), "{bin}: {}", head(&over));
        // An evaluation that goes over the budget by itself, for comparison.
        let by_eval = eval_vars(&server, http, &ones(3000), serde_json::json!({}));
        assert!(by_eval.contains(too_big), "{bin}: {}", head(&by_eval));
        assert_eq!(
            error_shape(&over, http),
            error_shape(&by_eval, http),
            "{bin}"
        );
        if http {
            assert!(over.starts_with("HTTP/1.1 400"), "{}", head(&over));
            assert!(by_eval.starts_with("HTTP/1.1 400"), "{}", head(&by_eval));
        }
        drop(server);

        // The default budget takes the same request as before.
        let server = start(bin, &[], &[]);
        let resp = eval_vars(&server, http, ":nums.length()", zeros(2100));
        assert!(resp.contains("2100"), "{bin}: {}", head(&resp));
    }
}

/// `GET /eval` query variables are charged too: a 20,000-character string fits 64 KiB
/// (`:q` 48 + 20000, `:arguments` 48 + 20008), a 40,000-character one does not.
#[test]
fn sk_http_server_charges_query_variables() {
    let server = start(HTTP, &["--max-eval-bytes", "64K"], &[]);
    let resp = get(
        &server,
        &format!("/eval?expr=:q.length()&q={}", "x".repeat(20_000)),
    );
    assert!(resp.starts_with("HTTP/1.1 200"), "{}", head(&resp));
    assert!(resp.contains("20000"), "{}", head(&resp));
    let resp = get(
        &server,
        &format!("/eval?expr=:q.length()&q={}", "x".repeat(40_000)),
    );
    assert!(resp.starts_with("HTTP/1.1 400"), "{}", head(&resp));
    assert!(
        resp.contains("Evaluation budget exceeded (max 65536 bytes)"),
        "{}",
        head(&resp)
    );
}

/// Start, then stop, and return what the server printed at start-up.
fn banner(bin: &str, args: &[&str], env: &[(&str, &str)]) -> String {
    let mut server = start(bin, args, env);
    // The limits are printed after the start-up line, before the accept loop: once a request
    // is answered they are in the log.
    let (resp, _) = eval(&server, bin == HTTP, "1 + 1", "");
    assert!(resp.contains('2'), "{bin}: {}", head(&resp));
    let _ = server.child.kill();
    let _ = server.child.wait();
    server.stderr_after_exit()
}

#[test]
fn banner_shows_both_limits_and_the_flag_wins() {
    for bin in [HTTP, LINE] {
        let text = banner(bin, &[], &[]);
        assert!(text.contains("Eval timeout: 20 s"), "{bin}: {text}");
        assert!(
            text.contains("Eval budget: 256 MiB per evaluation, request variables included"),
            "{bin}: {text}"
        );
        // ADR-0011 amendment 5: 2 × 256 + 160 (1 MiB request) + 64 (JS) + 10 (parse cache)
        // per thread, plus the HTTP result cache (1000 × 64 KiB).
        let total = if bin == HTTP { "809 MiB" } else { "746 MiB" };
        assert!(
            text.contains(&format!(
                "Worst-case memory: about {total} with 1 threads (per thread: 2 x 256 MiB evaluation, 160 MiB to read a 1 MiB request"
            )),
            "{bin}: {text}"
        );

        let env = [(TIMEOUT_ENV, "7"), (BYTES_ENV, "512M")];
        let text = banner(bin, &[], &env);
        assert!(text.contains("Eval timeout: 7 s"), "{bin}: {text}");
        assert!(text.contains("Eval budget: 512 MiB"), "{bin}: {text}");

        let flags = ["--eval-timeout", "90", "--max-eval-bytes", "2G"];
        let text = banner(bin, &flags, &env);
        assert!(text.contains("Eval timeout: 90 s"), "{bin}: {text}");
        assert!(text.contains("Eval budget: 2 GiB"), "{bin}: {text}");
    }
}

/// Run `bin` with an invalid setting: it must exit with code 1 before serving, naming the
/// problem on stderr.
fn startup_error(bin: &str, args: &[&str], env: &[(&str, &str)]) -> String {
    let mut child = command(bin, common::free_port(), args, env)
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn server");
    let started = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if started.elapsed() > SLOW {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{bin} {args:?} {env:?} did not exit");
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert_eq!(status.code(), Some(1), "{bin} {args:?} {env:?}: {stderr}");
    stderr
}

/// Flags, env vars, and the text the error must contain.
type Case = (
    &'static [&'static str],
    &'static [(&'static str, &'static str)],
    &'static str,
);

#[test]
fn invalid_values_stop_start_up() {
    let cases: [Case; 9] = [
        (
            &["--eval-timeout", "0"],
            &[],
            "--eval-timeout must be a whole number of seconds from 1 to 86400 (got \"0\")",
        ),
        (
            &["--eval-timeout", "1.5"],
            &[],
            "--eval-timeout must be a whole number",
        ),
        (
            &["--eval-timeout", ""],
            &[],
            "--eval-timeout must not be empty",
        ),
        (
            &[],
            &[(TIMEOUT_ENV, "")],
            "SKILLET_EVAL_TIMEOUT must not be empty",
        ),
        (
            &[],
            &[(TIMEOUT_ENV, "ten")],
            "SKILLET_EVAL_TIMEOUT must be a whole number",
        ),
        (
            &["--max-eval-bytes", "0"],
            &[],
            "--max-eval-bytes must be a whole number of bytes greater than 0",
        ),
        (
            &["--max-eval-bytes", "1GB"],
            &[],
            "--max-eval-bytes must be a whole number",
        ),
        (
            &[],
            &[(BYTES_ENV, "")],
            "SKILLET_MAX_EVAL_BYTES must not be empty",
        ),
        (&["--max-eval-bytes"], &[], "--max-eval-bytes requires"),
    ];
    for bin in [HTTP, LINE] {
        for (args, env, want) in cases {
            let stderr = startup_error(bin, args, env);
            assert!(stderr.contains(want), "{bin} {args:?} {env:?}: {stderr}");
        }
        // A valid flag does not hide an invalid value of the other limit.
        let stderr = startup_error(bin, &["--eval-timeout", "5"], &[(BYTES_ENV, "x")]);
        assert!(
            stderr.contains("SKILLET_MAX_EVAL_BYTES must be"),
            "{bin}: {stderr}"
        );
    }
}
