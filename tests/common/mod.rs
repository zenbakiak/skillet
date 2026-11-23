//! Starting a server binary for an integration test (BL-061).
//!
//! A free port from `bind(127.0.0.1:0)` is free only until the listener is dropped; another
//! test's server or any client socket can take it before ours binds. Ours then exits, and a
//! connect probe succeeds against whatever holds the port, often another test's server that
//! is killed mid-request when that test ends (an empty response or a reset). So readiness is
//! the server's own start-up line, printed after it has bound, never a connect probe.
//!
//! A server started here never inherits a `SKILLET_*` variable from the shell running the
//! tests (BL-087, QA-4): an operator who exported `SKILLET_MAX_CONNECTIONS_PER_IP=1` or
//! `SKILLET_MAX_EVAL_BYTES=1M` must not get red tests. A test that wants one sets it on the
//! command with `.env(...)`.
#![allow(dead_code)] // each test binary uses part of this module

use std::ffi::OsString;
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

/// Removes every `SKILLET_*` variable the test process has from `command`'s environment,
/// except those the command sets (or removes) itself.
pub fn clear_inherited_skillet_env(command: &mut Command) -> &mut Command {
    let own: Vec<OsString> = command.get_envs().map(|(k, _)| k.to_os_string()).collect();
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("SKILLET_") && !own.contains(&key) {
            command.env_remove(&key);
        }
    }
    command
}

/// `Command::new(bin)` without any inherited `SKILLET_*` variable, for a binary run outside
/// [`ServerProcess`] (a daemon, `sk`).
pub fn command_without_skillet_env(bin: &str) -> Command {
    let mut command = Command::new(bin);
    clear_inherited_skillet_env(&mut command);
    command
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
    /// Inherited `SKILLET_*` variables are cleared (see the module docs).
    pub fn start(mut command: impl FnMut(u16) -> Command) -> ServerProcess {
        for _ in 0..5 {
            let port = free_port();
            let mut command = command(port);
            let mut child = clear_inherited_skillet_env(&mut command)
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
