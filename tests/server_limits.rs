//! Request size and time limits of `sk_server` and `sk_http_server` (BL-033, BL-037, BL-034),
//! and their token checks (BL-018).
//! Servers run on 127.0.0.1 with one worker, so a held worker shows as a delayed next request.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

/// A server binary on a free 127.0.0.1 port, killed on drop.
struct Server {
    child: std::process::Child,
    port: u16,
}

impl Server {
    fn spawn(bin: &str, args: impl Fn(u16) -> Vec<String>) -> Server {
        Server::spawn_with_env_token(bin, args, None)
    }

    /// Like `spawn`, with `SKILLET_AUTH_TOKEN` set to `env_token` (unset when `None`).
    fn spawn_with_env_token(
        bin: &str,
        args: impl Fn(u16) -> Vec<String>,
        env_token: Option<&str>,
    ) -> Server {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let hooks = std::env::temp_dir().join("skillet-server-limits-no-hooks");
        let mut cmd = std::process::Command::new(bin);
        cmd.args(args(port))
            .env("SKILLET_HOOKS_DIR", hooks)
            .env("SKILLET_BIND_HOST", "127.0.0.1")
            .env_remove("SKILLET_AUTH_TOKEN")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        if let Some(t) = env_token {
            cmd.env("SKILLET_AUTH_TOKEN", t);
        }
        let child = cmd.spawn().expect("spawn server");
        for _ in 0..200 {
            if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Server { child, port }
    }

    /// `sk_http_server` with one worker.
    fn http() -> Server {
        Server::spawn(env!("CARGO_BIN_EXE_sk_http_server"), |port| {
            vec![port.to_string(), "--threads".into(), "1".into()]
        })
    }

    /// `sk_server` with one worker.
    fn line() -> Server {
        Server::spawn(env!("CARGO_BIN_EXE_sk_server"), |port| {
            vec![port.to_string(), "1".into()]
        })
    }

    fn connect(&self) -> TcpStream {
        let s = TcpStream::connect(("127.0.0.1", self.port)).expect("connect");
        s.set_read_timeout(Some(Duration::from_secs(60))).unwrap();
        s
    }

    /// Send one request line to `sk_server`; returns the response line.
    fn send_line(&self, body: &str) -> String {
        let mut s = self.connect();
        s.write_all(format!("{body}\n").as_bytes()).unwrap();
        let mut line = String::new();
        let _ = BufReader::new(s).read_line(&mut line);
        line
    }

    /// POST `body` to `/eval`; returns the raw response.
    fn post(&self, body: &str) -> String {
        self.post_with_headers(body, "")
    }

    /// POST `body` to `/eval` with extra header lines (each ending in `\r\n`).
    fn post_with_headers(&self, body: &str, headers: &str) -> String {
        let mut s = self.connect();
        let req = format!(
            "POST /eval HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\n{headers}Content-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        s.write_all(req.as_bytes()).unwrap();
        let mut resp = String::new();
        let _ = s.read_to_string(&mut resp);
        resp
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn head(resp: &str) -> &str {
    &resp[..resp.len().min(300)]
}

/// Sends `data` from another thread (the server may stop reading part-way).
fn send_in_background(mut s: TcpStream, data: Vec<u8>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let _ = s.write_all(&data);
    })
}

/// Trickles `data` one byte per second from another thread until it is sent or the server
/// closes the connection.
fn trickle(mut s: TcpStream, data: &'static [u8]) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        for b in data {
            if s.write_all(&[*b]).is_err() {
                return;
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    })
}

// ---------------------------------------------------------------- BL-033: sk_server

/// A request line over 1 MB is answered with an error and the connection is closed; the
/// server reads no more than the limit into memory, and the worker serves the next client.
#[test]
fn bl033_sk_server_rejects_an_oversized_line() {
    let server = Server::line();
    let s = server.connect();
    let reader = s.try_clone().unwrap();
    let writer = send_in_background(s, vec![b'x'; 4 * 1024 * 1024]); // no newline
    let mut resp = String::new();
    let mut reader = BufReader::new(reader);
    let _ = reader.read_line(&mut resp);
    assert!(resp.contains("\"success\":false"), "{}", head(&resp));
    assert!(
        resp.contains("Request too large (max 1MB)"),
        "{}",
        head(&resp)
    );
    // The server closes once the client has finished sending.
    writer.join().unwrap();
    let mut rest = String::new();
    let _ = reader.read_to_string(&mut rest);
    assert_eq!(rest, "");
    drop(reader);

    let resp = server.send_line("{\"expression\":\"=2 + 3\"}");
    assert!(resp.contains("\"success\":true"), "{}", head(&resp));
}

/// A line of exactly 1 MB (plus the newline) is still accepted.
#[test]
fn bl033_sk_server_accepts_a_line_at_the_limit() {
    let server = Server::line();
    let start = "{\"expression\":\"=2 + 3\"";
    let pad = 1024 * 1024 - start.len() - 1;
    let body = format!("{start}{}}}", " ".repeat(pad));
    assert_eq!(body.len(), 1024 * 1024);
    let resp = server.send_line(&body);
    assert!(resp.contains("\"success\":true"), "{}", head(&resp));

    let resp = server.send_line(&format!("{start}{} }}", " ".repeat(pad)));
    assert!(
        resp.contains("Request too large (max 1MB)"),
        "{}",
        head(&resp)
    );
}

/// Several requests on one connection still work, each read with its own deadline.
#[test]
fn bl033_sk_server_keeps_serving_a_persistent_connection() {
    let server = Server::line();
    let mut s = server.connect();
    s.write_all(b"{\"expression\":\"=1 + 1\"}\r\n{\"expression\":\"=2 + 2\"}\n")
        .unwrap();
    let mut reader = BufReader::new(s.try_clone().unwrap());
    let mut first = String::new();
    let mut second = String::new();
    reader.read_line(&mut first).unwrap();
    reader.read_line(&mut second).unwrap();
    assert!(first.contains("\"result\":2.0"), "{first}");
    assert!(second.contains("\"result\":4.0"), "{second}");
    s.write_all(b"{\"expression\":\"=3 + 3\"}\n").unwrap();
    let mut third = String::new();
    reader.read_line(&mut third).unwrap();
    assert!(third.contains("\"result\":6.0"), "{third}");
}

// ---------------------------------------------------------------- BL-037: total read deadline

/// A client trickling a request line holds the only worker for at most 30 s, then gets a
/// timeout error; the next client is served.
#[test]
#[ignore = "slow: 30 s request read deadline"]
fn bl037_sk_server_trickling_client_times_out() {
    let server = Server::line();
    let slow = server.connect();
    let slow_reader = slow.try_clone().unwrap();
    let _t = trickle(slow, &[b'{'; 120]);
    std::thread::sleep(Duration::from_millis(300));

    let start = Instant::now();
    let resp = server.send_line("{\"expression\":\"=2 + 3\"}");
    let took = start.elapsed();
    assert!(resp.contains("\"success\":true"), "{}", head(&resp));
    assert!(
        took >= Duration::from_secs(25) && took < Duration::from_secs(40),
        "{took:?}"
    );

    let mut slow_resp = String::new();
    let _ = BufReader::new(slow_reader).read_line(&mut slow_resp);
    assert!(
        slow_resp.contains("Request read timed out (max 30 s)"),
        "{}",
        head(&slow_resp)
    );
}

/// Same for `sk_http_server`: the 30 s limit covers the whole request, not each read.
#[test]
#[ignore = "slow: 30 s request read deadline"]
fn bl037_sk_http_server_trickling_client_times_out() {
    let server = Server::http();
    let slow = server.connect();
    let mut slow_reader = slow.try_clone().unwrap();
    let _t = trickle(slow, b"POST /eval HTTP/1.1\r\nHost: localhost\r\nX-Pad: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    std::thread::sleep(Duration::from_millis(300));

    let start = Instant::now();
    let resp = server.post("{\"expression\":\"=2 + 3\"}");
    let took = start.elapsed();
    assert!(resp.starts_with("HTTP/1.1 200"), "{}", head(&resp));
    assert!(
        took >= Duration::from_secs(25) && took < Duration::from_secs(40),
        "{took:?}"
    );

    let mut slow_resp = String::new();
    let _ = slow_reader.read_to_string(&mut slow_resp);
    assert!(
        slow_resp.contains("408 Request Timeout"),
        "{}",
        head(&slow_resp)
    );
}

// ---------------------------------------------------------------- BL-034: large responses

/// A 20 MB result arrives complete (it was cut to ~327 KB before accepted sockets were made
/// blocking in 58cc5b7).
#[test]
fn bl034_sk_http_server_sends_large_responses_complete() {
    let server = Server::http();
    let expr = format!(
        "SUBSTITUTE(SUBSTITUTE(\\\"{}\\\", \\\"a\\\", \\\"{}\\\"), \\\"a\\\", \\\"{}\\\")",
        "a".repeat(10),
        "a".repeat(1000),
        "b".repeat(2000)
    );
    let resp = server.post(&format!("{{\"expression\":\"{expr}\"}}"));
    let (headers, body) = resp.split_once("\r\n\r\n").expect("headers");
    assert!(headers.starts_with("HTTP/1.1 200"), "{}", head(&resp));
    let length: usize = headers
        .lines()
        .find_map(|l| l.strip_prefix("Content-Length: "))
        .expect("Content-Length")
        .parse()
        .unwrap();
    assert!(length > 20_000_000, "{length}");
    assert_eq!(body.len(), length);
    assert!(body.contains(&"b".repeat(20_000_000)));
}

// ---------------------------------------------------------------- BL-018: token check

/// Tokens are compared in constant time; what is accepted is unchanged: only the exact token.
#[test]
fn bl018_servers_accept_only_the_exact_token() {
    let token = "s3cret-token";
    let http = Server::spawn(env!("CARGO_BIN_EXE_sk_http_server"), |port| {
        vec![
            port.to_string(),
            "--threads".into(),
            "1".into(),
            "--token".into(),
            token.into(),
        ]
    });
    let body = "{\"expression\":\"=2 + 3\"}";
    for (auth, ok) in [
        ("Authorization: Bearer s3cret-token\r\n", true),
        ("Authorization: s3cret-token\r\n", true),
        ("Authorization: Bearer s3cret-tokeN\r\n", false),
        ("Authorization: Bearer s3cret\r\n", false),
        ("Authorization: Bearer s3cret-token-and-more\r\n", false),
        ("", false),
    ] {
        let resp = http.post_with_headers(body, auth);
        let want = if ok { "HTTP/1.1 200" } else { "HTTP/1.1 401" };
        assert!(resp.starts_with(want), "{auth:?}: {}", head(&resp));
    }
    drop(http);

    let line = Server::spawn(env!("CARGO_BIN_EXE_sk_server"), |port| {
        vec![port.to_string(), "1".into(), "--token".into(), token.into()]
    });
    for (supplied, ok) in [
        (Some("s3cret-token"), true),
        (Some("s3cret-tokeN"), false),
        (Some("s3cret"), false),
        (None, false),
    ] {
        let req = match supplied {
            Some(t) => format!("{{\"expression\":\"=2 + 3\",\"token\":\"{t}\"}}"),
            None => body.to_string(),
        };
        let resp = line.send_line(&req);
        if ok {
            assert!(resp.contains("\"success\":true"), "{}", head(&resp));
        } else {
            assert!(
                resp.contains("Unauthorized: invalid token"),
                "{}",
                head(&resp)
            );
        }
    }
}

// ---------------------------------------------------------------- BL-046: non-work lines

/// After a wrong token the error is sent and the connection closed: a later line on the same
/// connection, even with the right token, gets no answer.
#[test]
fn bl046_sk_server_closes_after_an_auth_failure() {
    let server = Server::spawn(env!("CARGO_BIN_EXE_sk_server"), |port| {
        vec![port.to_string(), "1".into(), "--token".into(), "tok".into()]
    });
    let mut s = server.connect();
    s.write_all(
        b"{\"expression\":\"=1\",\"token\":\"bad\"}\n{\"expression\":\"=2\",\"token\":\"tok\"}\n",
    )
    .unwrap();
    let mut reader = BufReader::new(s);
    let mut first = String::new();
    reader.read_line(&mut first).unwrap();
    assert!(first.contains("Unauthorized: invalid token"), "{first}");
    let mut rest = String::new();
    let _ = reader.read_to_string(&mut rest);
    assert_eq!(rest, "");

    // The worker is free again.
    let resp = server.send_line("{\"expression\":\"=2 + 3\",\"token\":\"tok\"}");
    assert!(resp.contains("\"success\":true"), "{}", head(&resp));
}

/// Sends `line` every 10 s on `s` until the server closes; returns how long that took.
fn keep_sending(mut s: TcpStream, line: &'static [u8]) -> Duration {
    let start = Instant::now();
    let mut reader = BufReader::new(s.try_clone().unwrap());
    let closed = std::thread::spawn(move || {
        // Drain answers (invalid-JSON errors) until EOF.
        let mut sink = String::new();
        while matches!(reader.read_line(&mut sink), Ok(n) if n > 0) {
            sink.clear();
        }
        start.elapsed()
    });
    while !closed.is_finished() && start.elapsed() < Duration::from_secs(60) {
        if s.write_all(line).is_err() {
            break;
        }
        std::thread::sleep(Duration::from_secs(10));
    }
    closed.join().unwrap()
}

/// Blank lines do not reset the 30 s deadline: a client sending only `\n` is closed at 30 s.
#[test]
#[ignore = "slow: 30 s request read deadline"]
fn bl046_sk_server_blank_lines_do_not_keep_a_connection() {
    let server = Server::line();
    let took = keep_sending(server.connect(), b"\n");
    assert!(
        took >= Duration::from_secs(25) && took < Duration::from_secs(40),
        "{took:?}"
    );
}

/// Invalid-JSON lines are answered but do not reset the deadline either.
#[test]
#[ignore = "slow: 30 s request read deadline"]
fn bl046_sk_server_invalid_lines_do_not_keep_a_connection() {
    let server = Server::line();
    let took = keep_sending(server.connect(), b"not json\n");
    assert!(
        took >= Duration::from_secs(25) && took < Duration::from_secs(40),
        "{took:?}"
    );
}

/// Real requests still reset the deadline: a connection used every 10 s outlives 30 s.
#[test]
#[ignore = "slow: 30 s request read deadline"]
fn bl046_sk_server_real_requests_keep_a_connection() {
    let server = Server::line();
    let mut s = server.connect();
    let mut reader = BufReader::new(s.try_clone().unwrap());
    for i in 0..5 {
        s.write_all(b"{\"expression\":\"=2 + 3\"}\n").unwrap();
        let mut resp = String::new();
        reader.read_line(&mut resp).unwrap();
        assert!(resp.contains("\"success\":true"), "request {i}: {resp}");
        std::thread::sleep(Duration::from_secs(10));
    }
}

// ---------------------------------------------------------------- BL-047: slow readers

/// An expression whose result is about 20 MB, escaped for a JSON string.
fn large_result_expr() -> String {
    format!(
        "SUBSTITUTE(SUBSTITUTE(\\\"{}\\\", \\\"a\\\", \\\"{}\\\"), \\\"a\\\", \\\"{}\\\")",
        "a".repeat(10),
        "a".repeat(1000),
        "b".repeat(2000)
    )
}

/// Reads 16 KB a second from another thread until the server closes; returns bytes read.
fn read_slowly(mut s: TcpStream) -> std::thread::JoinHandle<usize> {
    std::thread::spawn(move || {
        let start = Instant::now();
        let mut total = 0;
        let mut buf = [0u8; 16 * 1024];
        while start.elapsed() < Duration::from_secs(60) {
            match s.read(&mut buf) {
                Ok(n) if n > 0 => total += n,
                _ => break,
            }
            std::thread::sleep(Duration::from_secs(1));
        }
        total
    })
}

/// A client reading a large response slowly holds the only worker for about 10 s, then the
/// response is cut off and the next client is served.
#[test]
#[ignore = "slow: 10 s response write deadline"]
fn bl047_sk_server_slow_reader_is_cut_off() {
    let server = Server::line();
    let mut slow = server.connect();
    slow.write_all(format!("{{\"expression\":\"{}\"}}\n", large_result_expr()).as_bytes())
        .unwrap();
    let slow = read_slowly(slow);
    std::thread::sleep(Duration::from_millis(300));

    let start = Instant::now();
    let resp = server.send_line("{\"expression\":\"=2 + 3\"}");
    let took = start.elapsed();
    assert!(resp.contains("\"success\":true"), "{}", head(&resp));
    assert!(took < Duration::from_secs(30), "{took:?}");
    assert!(slow.join().unwrap() < 20_000_000);
}

/// Same for `sk_http_server`.
#[test]
#[ignore = "slow: 10 s response write deadline"]
fn bl047_sk_http_server_slow_reader_is_cut_off() {
    let server = Server::http();
    let mut slow = server.connect();
    let body = format!("{{\"expression\":\"{}\"}}", large_result_expr());
    let req = format!(
        "POST /eval HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    slow.write_all(req.as_bytes()).unwrap();
    let slow = read_slowly(slow);
    std::thread::sleep(Duration::from_millis(300));

    let start = Instant::now();
    let resp = server.post("{\"expression\":\"=2 + 3\"}");
    let took = start.elapsed();
    assert!(resp.starts_with("HTTP/1.1 200"), "{}", head(&resp));
    assert!(took < Duration::from_secs(30), "{took:?}");
    assert!(slow.join().unwrap() < 20_000_000);
}

// ---------------------------------------------------------------- BL-048: empty tokens

/// Runs a server binary that should refuse to start; returns its stderr.
fn refuses_to_start(bin: &str, args: &[&str], env_token: Option<&str>) -> String {
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
        .to_string();
    let mut cmd = std::process::Command::new(bin);
    cmd.arg(&port)
        .args(args)
        .env(
            "SKILLET_HOOKS_DIR",
            std::env::temp_dir().join("skillet-server-limits-no-hooks"),
        )
        .env("SKILLET_BIND_HOST", "127.0.0.1")
        .env_remove("SKILLET_AUTH_TOKEN")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped());
    if let Some(t) = env_token {
        cmd.env("SKILLET_AUTH_TOKEN", t);
    }
    let mut child = cmd.spawn().expect("spawn server");
    let start = Instant::now();
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if start.elapsed() > Duration::from_secs(10) {
            let _ = child.kill();
            let _ = child.wait();
            panic!("{bin} {args:?} started instead of refusing");
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    let mut stderr = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut stderr)
        .unwrap();
    assert!(!status.success(), "{bin} {args:?}: {status}");
    stderr
}

#[test]
fn bl048_servers_reject_empty_tokens() {
    let line = env!("CARGO_BIN_EXE_sk_server");
    let err = refuses_to_start(line, &["1", "--token", ""], None);
    assert!(err.contains("auth token is empty"), "{err}");
    let err = refuses_to_start(line, &["1"], Some(""));
    assert!(err.contains("auth token is empty"), "{err}");

    let http = env!("CARGO_BIN_EXE_sk_http_server");
    let err = refuses_to_start(http, &["--threads", "1", "--token", ""], None);
    assert!(err.contains("--token must not be empty"), "{err}");
    let err = refuses_to_start(http, &["--threads", "1", "--admin-token", ""], None);
    assert!(err.contains("--admin-token must not be empty"), "{err}");
}

// ---------------------------------------------------------------- SKILLET_AUTH_TOKEN

/// `sk_http_server` reads `SKILLET_AUTH_TOKEN` like `sk_server` does: without `--token`, the
/// env token is required on `/eval`.
#[test]
fn sk_http_server_enforces_the_env_token() {
    let http = Server::spawn_with_env_token(
        env!("CARGO_BIN_EXE_sk_http_server"),
        |port| vec![port.to_string(), "--threads".into(), "1".into()],
        Some("env-secret"),
    );
    let body = "{\"expression\":\"=2 + 3\"}";
    for (auth, want) in [
        ("", "HTTP/1.1 401"),
        ("Authorization: Bearer wrong\r\n", "HTTP/1.1 401"),
        ("Authorization: Bearer env-secret\r\n", "HTTP/1.1 200"),
    ] {
        let resp = http.post_with_headers(body, auth);
        assert!(resp.starts_with(want), "{auth:?}: {}", head(&resp));
    }
}

/// `--token` beats `SKILLET_AUTH_TOKEN`, as in `sk_server`.
#[test]
fn sk_http_server_token_flag_beats_the_env_token() {
    let http = Server::spawn_with_env_token(
        env!("CARGO_BIN_EXE_sk_http_server"),
        |port| {
            vec![
                port.to_string(),
                "--threads".into(),
                "1".into(),
                "--token".into(),
                "flag-secret".into(),
            ]
        },
        Some("env-secret"),
    );
    let body = "{\"expression\":\"=2 + 3\"}";
    for (auth, want) in [
        ("", "HTTP/1.1 401"),
        ("Authorization: Bearer env-secret\r\n", "HTTP/1.1 401"),
        ("Authorization: Bearer flag-secret\r\n", "HTTP/1.1 200"),
    ] {
        let resp = http.post_with_headers(body, auth);
        assert!(resp.starts_with(want), "{auth:?}: {}", head(&resp));
    }
}

/// An empty `SKILLET_AUTH_TOKEN` stops `sk_http_server` at start-up (BL-048), and a
/// non-empty `--token` still wins over it.
#[test]
fn sk_http_server_rejects_an_empty_env_token() {
    let http = env!("CARGO_BIN_EXE_sk_http_server");
    let err = refuses_to_start(http, &["--threads", "1"], Some(""));
    assert!(
        err.contains("SKILLET_AUTH_TOKEN must not be empty"),
        "{err}"
    );

    let server = Server::spawn_with_env_token(
        http,
        |port| {
            vec![
                port.to_string(),
                "--threads".into(),
                "1".into(),
                "--token".into(),
                "flag-secret".into(),
            ]
        },
        Some(""),
    );
    let resp = server.post_with_headers(
        "{\"expression\":\"=2 + 3\"}",
        "Authorization: Bearer flag-secret\r\n",
    );
    assert!(resp.starts_with("HTTP/1.1 200"), "{}", head(&resp));
}
