//! `sk_http_server` accept loop (BL-039): no per-request polling delay, and Ctrl-C still
//! stops a server that is blocked in `accept`. Servers run on 127.0.0.1 only.

use std::io::{Read, Write};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

/// `sk_http_server` on a free 127.0.0.1 port, killed on drop.
struct Server {
    child: Child,
    port: u16,
}

impl Server {
    fn spawn() -> Server {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let hooks = std::env::temp_dir().join("skillet-http-accept-no-hooks");
        let child = Command::new(env!("CARGO_BIN_EXE_sk_http_server"))
            .args([port.to_string(), "--threads".into(), "2".into()])
            .env("SKILLET_HOOKS_DIR", hooks)
            .env("SKILLET_BIND_HOST", "127.0.0.1")
            .env_remove("SKILLET_AUTH_TOKEN")
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn sk_http_server");
        let server = Server { child, port };
        for _ in 0..200 {
            if server.get("/health").is_ok() {
                return server;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("sk_http_server did not start");
    }

    /// One `GET` with `Connection: close`; returns the raw response.
    fn get(&self, path: &str) -> std::io::Result<String> {
        let mut s = TcpStream::connect(("127.0.0.1", self.port))?;
        s.set_read_timeout(Some(Duration::from_secs(10)))?;
        write!(
            s,
            "GET {} HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n",
            path
        )?;
        let mut out = String::new();
        s.read_to_string(&mut out)?;
        Ok(out)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Sequential requests used to wait out a 1 ms sleep each (~1.1 ms floor per request).
/// The fastest of 30 is now ~0.1 ms (debug build, load average ~19); before BL-039 it was
/// ~0.77 ms. 0.4 ms sits between the two.
#[test]
fn sequential_requests_have_no_polling_floor() {
    let server = Server::spawn();
    let mut fastest = Duration::MAX;
    for _ in 0..30 {
        let start = Instant::now();
        let resp = server.get("/health").expect("health request");
        fastest = fastest.min(start.elapsed());
        assert!(resp.starts_with("HTTP/1.1 200"), "{}", resp);
    }
    assert!(
        fastest < Duration::from_micros(400),
        "fastest /health took {:?}; the accept loop is polling again",
        fastest
    );
}

/// Ctrl-C (SIGINT) stops a server that is idle in a blocking `accept`, through the normal
/// shutdown path.
#[cfg(unix)]
#[test]
fn sigint_stops_idle_server() {
    let mut server = Server::spawn();
    std::thread::sleep(Duration::from_millis(200)); // idle: blocked in accept
    let status = Command::new("kill")
        .args(["-INT", &server.child.id().to_string()])
        .status()
        .expect("run kill");
    assert!(status.success());

    let deadline = Instant::now() + Duration::from_secs(10);
    let exit = loop {
        if let Some(exit) = server.child.try_wait().expect("try_wait") {
            break exit;
        }
        assert!(
            Instant::now() < deadline,
            "server still running after SIGINT"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(exit.success(), "exit status {:?}", exit);

    let mut stderr = String::new();
    server
        .child
        .stderr
        .take()
        .expect("stderr")
        .read_to_string(&mut stderr)
        .unwrap();
    assert!(stderr.contains("Server shutdown complete."), "{}", stderr);
}
