use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

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

pub fn write_pid_file(pid_file: &str) -> Result<(), Box<dyn std::error::Error>> {
    use std::fs::File;
    use std::io::Write;
    
    let pid = std::process::id();
    let mut file = File::create(pid_file)?;
    writeln!(file, "{}", pid)?;
    Ok(())
}

pub fn setup_signal_handlers() -> Arc<AtomicBool> {
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    ctrlc::set_handler(move || {
        eprintln!("Received shutdown signal, gracefully stopping...");
        r.store(false, Ordering::SeqCst);
    }).expect("Error setting signal handler");
    running
}