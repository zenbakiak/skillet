use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

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

/// Write this process's PID, followed by a newline, to `pid_file`.
///
/// On unix the file is opened with `O_NOFOLLOW`: if `pid_file` is a symlink the write fails
/// instead of going to the link's target.
pub fn write_pid_file(pid_file: &str) -> Result<(), Box<dyn std::error::Error>> {
    use std::fs::OpenOptions;
    use std::io::Write;

    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW);
    }

    let pid = std::process::id();
    let mut file = options.open(pid_file)?;
    writeln!(file, "{}", pid)?;
    Ok(())
}

pub fn setup_signal_handlers() -> Arc<AtomicBool> {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        eprintln!("Received shutdown signal, gracefully stopping...");
        r.store(false, Ordering::SeqCst);
    })
    .expect("Error setting signal handler");
    running
}

#[cfg(test)]
mod tests {
    use super::write_pid_file;

    #[test]
    fn pid_file_holds_the_pid_and_a_newline() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("server.pid");
        std::fs::write(&path, "stale contents that are longer than a pid\n").unwrap();
        write_pid_file(path.to_str().unwrap()).unwrap();
        let expected = format!("{}\n", std::process::id());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), expected);
    }

    #[cfg(unix)]
    #[test]
    fn pid_file_is_not_written_through_a_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        std::fs::write(&target, "original").unwrap();
        let link = dir.path().join("server.pid");
        std::os::unix::fs::symlink(&target, &link).unwrap();

        assert!(write_pid_file(link.to_str().unwrap()).is_err());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "original");
    }
}
