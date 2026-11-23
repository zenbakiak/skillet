//! `sk_server --daemon` must not write its PID file through a symlink (same rule as
//! `sk_http_server`, PR #63). Unix only: daemon mode is unix only.
//!
//! A daemon closes stderr, so `common::ServerProcess` (which waits for the start-up line)
//! cannot start it. A connect probe alone is not proof either: the port can be taken by another
//! test's server before the daemon binds it (tests/common/mod.rs). So each daemon gets its own
//! token, and a port counts as "our daemon" only when a request with that token succeeds and
//! one without it is refused. Another sk_server answers one of the two differently; any other
//! server answers neither (BL-087, QA-4).
#![cfg(unix)]

mod common;

use common::{command_without_skillet_env, free_port, SLOW};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

/// One request line to `port`; the response line, or `None` when nothing answers.
fn ask(port: u16, token: Option<&str>) -> Option<String> {
    let mut s = TcpStream::connect(("127.0.0.1", port)).ok()?;
    s.set_read_timeout(Some(SLOW)).unwrap();
    let body = match token {
        Some(t) => serde_json::json!({ "expression": "=2 + 3", "token": t }),
        None => serde_json::json!({ "expression": "=2 + 3" }),
    };
    s.write_all(format!("{body}\n").as_bytes()).ok()?;
    let mut line = String::new();
    BufReader::new(s).read_line(&mut line).ok()?;
    Some(line)
}

/// True when the server on `port` is the daemon started with `token`.
fn is_our_daemon(port: u16, token: &str) -> bool {
    let with = ask(port, Some(token)).unwrap_or_default();
    let without = ask(port, None).unwrap_or_default();
    with.contains("\"result\":5.0") && without.contains("Unauthorized")
}

/// A token no other test uses.
fn unique_token(tag: &str) -> String {
    format!("pid-file-{tag}-{}-{}", std::process::id(), free_port())
}

/// Runs `sk_server <port> 1 --daemon --token <token> --pid-file <pid_file>` in `dir`.
fn start_daemon(dir: &Path, port: u16, token: &str, pid_file: &Path) -> ExitStatus {
    command_without_skillet_env(env!("CARGO_BIN_EXE_sk_server"))
        .current_dir(dir)
        .args([
            &port.to_string(),
            "1",
            "--daemon",
            "--token",
            token,
            "--pid-file",
        ])
        .arg(pid_file)
        .env("SKILLET_HOOKS_DIR", dir.join("no-hooks"))
        .env("SKILLET_BIND_HOST", "127.0.0.1")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("run sk_server")
}

fn alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

fn kill(pid: u32) {
    let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
}

#[test]
fn sk_server_daemon_does_not_write_the_pid_file_through_a_symlink() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target");
    std::fs::write(&target, "original").unwrap();
    let link = dir.path().join("skillet-server.pid");
    std::os::unix::fs::symlink(&target, &link).unwrap();

    let port = free_port();
    let token = unique_token("symlink");
    // The foreground process exits once it has forked; the daemon carries on alone.
    assert!(start_daemon(dir.path(), port, &token, &link).success());

    // Give a daemon that got past the PID write time to write through the link or to serve.
    // A daemon that stops at the PID write (the expected case) does neither.
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut serving = false;
    while Instant::now() < deadline {
        serving = is_our_daemon(port, &token);
        if serving || std::fs::read_to_string(&target).unwrap() != "original" {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let contents = std::fs::read_to_string(&target).unwrap();
    // Stop a daemon that got past the PID write, so a failing run leaves nothing behind.
    if let Ok(pid) = contents.trim().parse::<u32>() {
        kill(pid);
    }
    assert_eq!(contents, "original", "the symlink target was overwritten");
    assert!(
        !serving,
        "the daemon kept running after the PID write failed"
    );
}

/// BL-064: `sk_server --daemon` (now the shared daemon.rs) detaches, writes the daemon's PID
/// and serves requests from the working directory it was started in.
#[test]
fn sk_server_daemon_writes_its_pid_and_serves() {
    let dir = tempfile::tempdir().unwrap();
    let pid_file = dir.path().join("skillet-server.pid");
    let token = unique_token("serves");
    for _ in 0..5 {
        let _ = std::fs::remove_file(&pid_file);
        let port = free_port();
        // Relative: the daemon must not chdir away.
        assert!(start_daemon(dir.path(), port, &token, Path::new("skillet-server.pid")).success());

        // The PID is written before the daemon binds; a daemon that then exits lost its port.
        let start = Instant::now();
        let mut pid = None;
        let mut ours = false;
        while start.elapsed() < SLOW {
            if pid.is_none() {
                pid = std::fs::read_to_string(&pid_file)
                    .ok()
                    .filter(|p| p.ends_with('\n'))
                    .and_then(|p| p.trim().parse::<u32>().ok());
            }
            if let Some(p) = pid {
                ours = is_our_daemon(port, &token);
                if ours || !alive(p) {
                    break;
                }
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        let Some(pid) = pid else {
            panic!("the daemon wrote no PID file within {SLOW:?}");
        };
        if !ours && !alive(pid) {
            eprintln!("port {port} was taken before the daemon bound it; retrying");
            continue;
        }
        kill(pid);
        assert!(ours, "the daemon did not start serving within {SLOW:?}");
        assert_ne!(pid, std::process::id());
        let written = std::fs::read_to_string(&pid_file).unwrap();
        assert_eq!(written, format!("{pid}\n"));
        return;
    }
    panic!("the daemon could not bind a free port in 5 tries");
}
