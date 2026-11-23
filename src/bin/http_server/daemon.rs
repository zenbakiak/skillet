use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

#[cfg(unix)]
pub fn daemonize() -> Result<(), Box<dyn std::error::Error>> {
    use std::fs::OpenOptions;
    use std::os::unix::io::AsRawFd;

    // Fork the process
    match unsafe { libc::fork() } {
        -1 => return Err("Failed to fork process".into()),
        0 => {
            // Child process continues
        }
        _ => {
            // Parent process exits
            std::process::exit(0);
        }
    }

    // Create a new session
    if unsafe { libc::setsid() } == -1 {
        return Err("Failed to create new session".into());
    }

    // Fork again to ensure we're not a session leader
    match unsafe { libc::fork() } {
        -1 => return Err("Failed to fork second time".into()),
        0 => {
            // Grandchild continues
        }
        _ => {
            // Child exits
            std::process::exit(0);
        }
    }

    // DON'T change working directory to root - stay in current directory
    // This allows relative paths to work (like hooks directory)

    // Close standard file descriptors
    unsafe {
        libc::close(libc::STDIN_FILENO);
        libc::close(libc::STDOUT_FILENO);
        libc::close(libc::STDERR_FILENO);
    }

    // Redirect stdin, stdout, stderr to /dev/null
    let dev_null = OpenOptions::new()
        .read(true)
        .write(true)
        .open("/dev/null")?;

    let null_fd = dev_null.as_raw_fd();
    unsafe {
        libc::dup2(null_fd, libc::STDIN_FILENO);
        libc::dup2(null_fd, libc::STDOUT_FILENO);
        libc::dup2(null_fd, libc::STDERR_FILENO);
    }

    Ok(())
}

/// Installs the shutdown handler (Ctrl-C). The accept loop blocks in `accept` (BL-039), so the
/// handler clears the flag and then opens one connection to `wake` to unblock it; the loop sees
/// the flag and stops. If that connection cannot be made, the process exits directly.
pub fn setup_signal_handlers(wake: SocketAddr) -> Arc<AtomicBool> {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        eprintln!("Received shutdown signal, gracefully stopping...");
        r.store(false, Ordering::SeqCst);
        if TcpStream::connect_timeout(&wake, Duration::from_secs(1)).is_err() {
            eprintln!("Server shutdown complete.");
            std::process::exit(0);
        }
    })
    .expect("Error setting signal handler");
    running
}

/// Address a local client can use to reach a listener bound to `local`: a wildcard bind
/// (`0.0.0.0` / `::`) is reached through loopback.
pub fn wake_address(local: SocketAddr) -> SocketAddr {
    let ip = match local.ip() {
        IpAddr::V4(ip) if ip.is_unspecified() => IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(ip) if ip.is_unspecified() => IpAddr::V6(Ipv6Addr::LOCALHOST),
        ip => ip,
    };
    SocketAddr::new(ip, local.port())
}

#[cfg(test)]
mod tests {
    use super::wake_address;
    use std::net::SocketAddr;

    #[test]
    fn wake_address_maps_wildcard_binds_to_loopback() {
        let cases = [
            ("0.0.0.0:5074", "127.0.0.1:5074"),
            ("[::]:5074", "[::1]:5074"),
            ("127.0.0.1:5074", "127.0.0.1:5074"),
            ("192.168.1.10:80", "192.168.1.10:80"),
        ];
        for (bound, expected) in cases {
            let bound: SocketAddr = bound.parse().unwrap();
            let expected: SocketAddr = expected.parse().unwrap();
            assert_eq!(wake_address(bound), expected);
        }
    }
}
