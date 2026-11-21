//! PID file writing for both servers (`sk_server` includes this file with `#[path]`).

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
