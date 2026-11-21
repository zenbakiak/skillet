//! Size and time limits for reading one request, shared by `sk_http_server` and `sk_server`
//! (included there with `#[path]`).

use std::io::{self, Read};
use std::net::TcpStream;
use std::time::{Duration, Instant};

/// Largest request accepted: an HTTP request, or one `sk_server` request line.
pub const MAX_REQUEST_SIZE: usize = 1024 * 1024;

/// Wall-clock limit for reading one request, from the first read (BL-037). Applies to the
/// whole request, so a client trickling bytes cannot hold a worker longer than this.
pub const REQUEST_READ_DEADLINE: Duration = Duration::from_secs(30);

/// Limit for one write of a response.
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

fn timed_out() -> io::Error {
    io::Error::new(io::ErrorKind::TimedOut, "Request read timed out (max 30 s)")
}
