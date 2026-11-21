//! Request size and time limits of `sk_server` and `sk_http_server` (BL-033).
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
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let hooks = std::env::temp_dir().join("skillet-server-limits-no-hooks");
        let child = std::process::Command::new(bin)
            .args(args(port))
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

// ---------------------------------------------------------------- total read deadline

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
