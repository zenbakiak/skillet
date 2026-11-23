//! Size and time limits for reading one request and writing one response, shared by
//! `sk_http_server` and `sk_server` (included there with `#[path]`).

use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

/// Largest request accepted: an HTTP request, or one `sk_server` request line.
pub const MAX_REQUEST_SIZE: usize = 1024 * 1024;

/// Wall-clock limit for reading one request, from the first read (BL-037). Applies to the
/// whole request, so a client trickling bytes cannot hold a worker longer than this.
pub const REQUEST_READ_DEADLINE: Duration = Duration::from_secs(30);

/// Wall-clock limit for writing one response, from its first write (BL-047). Applies to the
/// whole response, so a client reading slowly cannot hold a worker longer than this.
pub const WRITE_TIMEOUT: Duration = Duration::from_secs(10);

/// Reads from a socket until a deadline. Each read waits at most the time left; once the
/// deadline has passed, reads fail with `ErrorKind::TimedOut`.
pub struct DeadlineReader<'a> {
    stream: &'a TcpStream,
    deadline: Instant,
}

impl<'a> DeadlineReader<'a> {
    /// A reader whose deadline is `REQUEST_READ_DEADLINE` from now.
    pub fn new(stream: &'a TcpStream) -> Self {
        Self {
            stream,
            deadline: Instant::now() + REQUEST_READ_DEADLINE,
        }
    }
}

impl Read for DeadlineReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let left = self.deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(timed_out());
        }
        self.stream.set_read_timeout(Some(left))?;
        let mut stream = self.stream;
        match stream.read(buf) {
            // Unix reports an expired SO_RCVTIMEO as EAGAIN (WouldBlock).
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Err(timed_out()),
            other => other,
        }
    }
}

/// After rejecting a request, close so the client still receives the error: closing a socket
/// with unread input sends a reset, which can discard the response before the client reads it.
/// Send FIN, then discard input until the client closes or `reader`'s deadline passes.
pub fn close_after_reject(stream: &TcpStream, reader: &mut DeadlineReader<'_>) {
    let _ = stream.shutdown(std::net::Shutdown::Write);
    let mut sink = [0u8; 8192];
    while matches!(reader.read(&mut sink), Ok(n) if n > 0) {}
}

fn timed_out() -> io::Error {
    io::Error::new(io::ErrorKind::TimedOut, "Request read timed out (max 30 s)")
}

/// Largest single socket write: the default SO_SNDLOWAT on BSD/macOS.
const WRITE_CHUNK: usize = 2048;

/// Writes to a socket until a deadline. Each write waits at most the time left; once the
/// deadline has passed, writes fail with `ErrorKind::TimedOut`. Use one per response.
pub struct DeadlineWriter<'a> {
    stream: &'a TcpStream,
    deadline: Instant,
}

impl<'a> DeadlineWriter<'a> {
    /// A writer whose deadline is `WRITE_TIMEOUT` from now.
    pub fn new(stream: &'a TcpStream) -> Self {
        Self {
            stream,
            deadline: Instant::now() + WRITE_TIMEOUT,
        }
    }
}

impl Write for DeadlineWriter<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let left = self.deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(write_timed_out());
        }
        self.stream.set_write_timeout(Some(left))?;
        let mut stream = self.stream;
        // On BSD/macOS one `send` keeps blocking while the reader frees space within each
        // SO_SNDTIMEO wait, so a large write could outlive the deadline. A write no larger than
        // the send low-water mark waits at most once.
        let len = buf.len().min(WRITE_CHUNK);
        match stream.write(&buf[..len]) {
            // Unix reports an expired SO_SNDTIMEO as EAGAIN (WouldBlock).
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Err(write_timed_out()),
            other => other,
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut stream = self.stream;
        stream.flush()
    }
}

fn write_timed_out() -> io::Error {
    io::Error::new(
        io::ErrorKind::TimedOut,
        "Response write timed out (max 10 s)",
    )
}
