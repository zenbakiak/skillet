//! BL-049: `sk_http_server` answers a request over the size limit with HTTP 413 and one past the
//! read deadline with HTTP 408 (they were 400), and reads the rest of a rejected request before
//! closing, so the client receives the error instead of a connection reset.
//! The server runs with one worker, so a held worker shows as a refused or delayed next request.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// `sk_http_server` with one worker on a free 127.0.0.1 port, killed on drop.
struct Server {
    child: std::process::Child,
    port: u16,
}

impl Server {
    fn start() -> Server {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let hooks = std::env::temp_dir().join("skillet-http-status-no-hooks");
        let child = std::process::Command::new(env!("CARGO_BIN_EXE_sk_http_server"))
            .args([port.to_string().as_str(), "--threads", "1"])
            .env("SKILLET_HOOKS_DIR", hooks)
            .env_remove("SKILLET_AUTH_TOKEN")
            .env_remove("SKILLET_ADMIN_TOKEN")
            .env_remove("SKILLET_CORS_ORIGIN")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn sk_http_server");
        let server = Server { child, port }; // killed on drop, also if it never starts
        for _ in 0..200 {
            if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                return server;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("sk_http_server did not start");
    }

    fn connect(&self) -> TcpStream {
        let s = TcpStream::connect(("127.0.0.1", self.port)).expect("connect");
        s.set_read_timeout(Some(Duration::from_secs(60))).unwrap();
        s
    }

    /// Send `data` from another thread and read the whole response. Returns the response and
    /// whether every byte of `data` was written (false means the server reset the connection).
    fn exchange(&self, data: Vec<u8>) -> (String, bool) {
        let s = self.connect();
        let mut writer = s.try_clone().unwrap();
        let sent = std::thread::spawn(move || {
            let ok = writer.write_all(&data).is_ok();
            let _ = writer.shutdown(std::net::Shutdown::Write);
            ok
        });
        let mut resp = Vec::new();
        let _ = (&s).read_to_end(&mut resp);
        let sent = sent.join().unwrap();
        (String::from_utf8_lossy(&resp).into_owned(), sent)
    }

    fn eval(&self, expression: &str) -> String {
        let body = format!("{{\"expression\":\"{expression}\"}}");
        let req = format!(
            "POST /eval HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        self.exchange(req.into_bytes()).0
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

/// A `Content-Length` over 1 MB gets 413, the server reads the whole 4 MB body before closing
/// (the client's writes all succeed), and the worker then serves the next request.
#[test]
fn bl049_oversized_body_gets_413_and_is_drained() {
    let server = Server::start();
    let body_len = 4 * 1024 * 1024;
    let mut req = format!(
        "POST /eval HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {body_len}\r\n\r\n"
    )
    .into_bytes();
    req.resize(req.len() + body_len, b' ');

    let (resp, sent) = server.exchange(req);
    assert!(
        resp.starts_with("HTTP/1.1 413 Payload Too Large\r\n"),
        "{}",
        head(&resp)
    );
    assert!(
        resp.ends_with(r#"{"error":"Request body too large (max 1MB)","success":false}"#),
        "{}",
        head(&resp)
    );
    assert!(sent, "the server closed before reading the rejected body");

    assert!(server.eval("=2 + 3").starts_with("HTTP/1.1 200"));
}

/// Headers alone over 1 MB (no blank line yet) also get 413.
#[test]
fn bl049_oversized_headers_get_413() {
    let server = Server::start();
    let mut req = b"GET /health HTTP/1.1\r\nX-Pad: ".to_vec();
    req.resize(req.len() + 2 * 1024 * 1024, b'a');

    let (resp, sent) = server.exchange(req);
    assert!(
        resp.starts_with("HTTP/1.1 413 Payload Too Large\r\n"),
        "{}",
        head(&resp)
    );
    assert!(
        resp.contains(r#""error":"Request too large (max 1MB)""#),
        "{}",
        head(&resp)
    );
    assert!(
        sent,
        "the server closed before reading the rejected headers"
    );
    assert!(server.eval("=2 + 3").starts_with("HTTP/1.1 200"));
}

/// A request that is not UTF-8 is still 400.
#[test]
fn bl049_non_utf8_request_gets_400() {
    let server = Server::start();
    let mut req =
        b"POST /eval HTTP/1.1\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n"
            .to_vec();
    req.extend_from_slice(&[0xff, 0xfe]);
    let (resp, _) = server.exchange(req);
    assert!(
        resp.starts_with("HTTP/1.1 400 Bad Request\r\n"),
        "{}",
        head(&resp)
    );
    assert!(
        resp.contains(r#""error":"Invalid UTF-8""#),
        "{}",
        head(&resp)
    );
}

/// A request still incomplete after 30 s gets 408 with the timeout message.
#[test]
#[ignore = "slow: 30 s request read deadline"]
fn bl049_slow_request_gets_408() {
    let server = Server::start();
    let mut s = server.connect();
    s.write_all(b"POST /eval HTTP/1.1\r\nContent-Type: application/json\r\nContent-Length: 100\r\n\r\n{\"exp")
        .unwrap();
    let mut resp = String::new();
    let _ = s.read_to_string(&mut resp);
    assert!(
        resp.starts_with("HTTP/1.1 408 Request Timeout\r\n"),
        "{}",
        head(&resp)
    );
    assert!(
        resp.ends_with(r#"{"error":"Request read timed out (max 30 s)","success":false}"#),
        "{}",
        head(&resp)
    );
}
