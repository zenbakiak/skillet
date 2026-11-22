//! Starting a server binary for an integration test (BL-061).
//!
//! A free port from `bind(127.0.0.1:0)` is free only until the listener is dropped; another
//! test's server or any client socket can take it before ours binds. Ours then exits, and a
//! connect probe succeeds against whatever holds the port, often another test's server that
//! is killed mid-request when that test ends (an empty response or a reset). So readiness is
//! the server's own start-up line, printed after it has bound, never a connect probe.
#![allow(dead_code)] // each test binary uses part of this module

use std::io::{BufRead, BufReader};
use std::net::TcpStream;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// Limit for everything except the timing under test. At load average > 20 a debug server can
/// stall for many seconds.
pub const SLOW: Duration = Duration::from_secs(120);

/// A 127.0.0.1 port that was free a moment ago. Use it only through `ServerProcess::start`,
/// which retries when someone else takes it first.
pub fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

/// A started server on its own port, killed on drop.
pub struct ServerProcess {
    pub child: Child,
    pub port: u16,
    stderr: Arc<Mutex<String>>,
    drain: Option<JoinHandle<()>>,
}

impl ServerProcess {
    /// Runs `command(port)` on free ports until the server prints its start-up line. A server
    /// that exits first lost its port to another process; it is retried on a new one.
    pub fn start(mut command: impl FnMut(u16) -> Command) -> ServerProcess {
        for _ in 0..5 {
            let port = free_port();
            let mut child = command(port)
                .stderr(Stdio::piped())
                .spawn()
                .expect("spawn server");
            let pipe = child.stderr.take().unwrap();
            let stderr = Arc::new(Mutex::new(String::new()));
            let (started, ready) = mpsc::channel();
            let log = Arc::clone(&stderr);
            // Drains stderr for the server's whole life, so it never blocks writing to it.
            let drain = std::thread::spawn(move || {
                for line in BufReader::new(pipe).split(b'\n').map_while(Result::ok) {
                    let line = String::from_utf8_lossy(&line);
                    if line.contains(" started on ") {
                        let _ = started.send(());
                    }
                    let mut log = log.lock().unwrap();
                    log.push_str(&line);
                    log.push('\n');
                }
            });
            match ready.recv_timeout(SLOW) {
                Ok(()) => {
                    return ServerProcess {
                        child,
                        port,
                        stderr,
                        drain: Some(drain),
                    }
                }
                Err(RecvTimeoutError::Disconnected) => {
                    let _ = child.wait();
                    let _ = drain.join();
                    let log = stderr.lock().unwrap();
                    // Only a lost port is retried; any other start-up failure is a real one.
                    assert!(log.contains("Failed to bind"), "server exited: {log}");
                    eprintln!("port {port} was taken before the server bound it; retrying");
                }
                Err(RecvTimeoutError::Timeout) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("server did not start within {SLOW:?}");
                }
            }
        }
        panic!("server could not bind a free port in 5 tries");
    }

    /// Connects, retrying a failed connect (never a request). Reads time out after `SLOW`.
    pub fn connect(&self) -> TcpStream {
        let start = Instant::now();
        let s = loop {
            match TcpStream::connect(("127.0.0.1", self.port)) {
                Ok(s) => break s,
                Err(e) if start.elapsed() > SLOW => panic!("connect: {e}"),
                Err(_) => std::thread::sleep(Duration::from_millis(50)),
            }
        };
        s.set_read_timeout(Some(SLOW)).unwrap();
        s
    }

    /// Everything the server wrote to stderr, once it has exited.
    pub fn stderr_after_exit(&mut self) -> String {
        if let Some(drain) = self.drain.take() {
            let _ = drain.join();
        }
        self.stderr.lock().unwrap().clone()
    }
}

impl Drop for ServerProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
