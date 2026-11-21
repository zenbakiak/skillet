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
