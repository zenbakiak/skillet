//! `sk_server --daemon` must not write its PID file through a symlink (same rule as
//! `sk_http_server`, PR #63). Unix only: daemon mode is unix only.
#![cfg(unix)]

use std::net::TcpStream;
use std::time::{Duration, Instant};

#[test]
fn sk_server_daemon_does_not_write_the_pid_file_through_a_symlink() {
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("target");
    std::fs::write(&target, "original").unwrap();
    let link = dir.path().join("skillet-server.pid");
    std::os::unix::fs::symlink(&target, &link).unwrap();

    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_sk_server"))
        .args([port.to_string().as_str(), "1", "--daemon", "--pid-file"])
        .arg(&link)
        .env("SKILLET_HOOKS_DIR", dir.path().join("no-hooks"))
        .env("SKILLET_BIND_HOST", "127.0.0.1")
        .env_remove("SKILLET_AUTH_TOKEN")
        .env_remove("SKILLET_ADMIN_TOKEN")
        .env_remove("SKILLET_CORS_ORIGIN")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("run sk_server");
    // The foreground process exits once it has forked; the daemon carries on alone.
    assert!(status.success());

    // Wait for the daemon to either write through the link or start serving.
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut serving = false;
    while Instant::now() < deadline {
        serving = TcpStream::connect(("127.0.0.1", port)).is_ok();
        if serving || std::fs::read_to_string(&target).unwrap() != "original" {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let contents = std::fs::read_to_string(&target).unwrap();
    // Stop a daemon that got past the PID write, so a failing run leaves nothing behind.
    if let Ok(pid) = contents.trim().parse::<u32>() {
        let _ = std::process::Command::new("kill")
            .args(["-9", &pid.to_string()])
            .status();
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
    use std::io::{BufRead, BufReader, Write};

    let dir = tempfile::tempdir().unwrap();
    let pid_file = dir.path().join("skillet-server.pid");
    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let status = std::process::Command::new(env!("CARGO_BIN_EXE_sk_server"))
        .current_dir(dir.path())
        .args([port.to_string().as_str(), "1", "--daemon", "--pid-file"])
        .arg("skillet-server.pid") // relative: the daemon must not chdir away
        .env("SKILLET_HOOKS_DIR", dir.path().join("no-hooks"))
        .env("SKILLET_BIND_HOST", "127.0.0.1")
        .env_remove("SKILLET_AUTH_TOKEN")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .expect("run sk_server");
    assert!(status.success());

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut stream = None;
    while Instant::now() < deadline && stream.is_none() {
        stream = TcpStream::connect(("127.0.0.1", port)).ok();
        std::thread::sleep(Duration::from_millis(50));
    }
    let pid = std::fs::read_to_string(&pid_file).unwrap_or_default();
    let kill = || {
        if let Ok(pid) = pid.trim().parse::<u32>() {
            let _ = std::process::Command::new("kill")
                .args(["-9", &pid.to_string()])
                .status();
        }
    };
    let Some(mut s) = stream else {
        kill();
        panic!("the daemon did not start serving");
    };
    s.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
    s.write_all(b"{\"expression\":\"=2 + 3\"}\n").unwrap();
    let mut line = String::new();
    let _ = BufReader::new(s).read_line(&mut line);
    kill();

    assert!(pid.ends_with('\n'), "{pid:?}");
    let pid: u32 = pid.trim().parse().expect("a pid");
    assert_ne!(pid, std::process::id());
    assert!(line.contains("\"result\":5.0"), "{line}");
}
