//! Limit on concurrent connections from one IP address (ADR-0025), shared by `sk_http_server`
//! and `sk_server` (included there with `#[path]`).
//!
//! The accept loop calls [`ConnLimiter::try_acquire`] before a connection is queued for a
//! worker. The returned [`ConnGuard`] travels with the connection and gives the slot back when
//! it is dropped, so a panic or an early return in the handler cannot leak a count.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{IpAddr, Shutdown, TcpStream};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

/// Command-line flag for the limit.
pub const FLAG: &str = "--max-connections-per-ip";
/// Environment variable for the limit; the flag wins over it.
pub const ENV_VAR: &str = "SKILLET_MAX_CONNECTIONS_PER_IP";

/// Write timeout for the rejection, which is written from the accept thread. The response is a
/// few hundred bytes and fits in the socket send buffer, so this is only hit by a broken peer.
const REJECT_WRITE_TIMEOUT: Duration = Duration::from_millis(500);

/// Largest amount of already-received input discarded before closing a rejected connection.
const REJECT_DRAIN_LIMIT: usize = 64 * 1024;

/// Resolve the limit from the flag value (if given) and `ENV_VAR`. `None` means no limit.
/// `0`, an empty value or non-numeric text is an error (the message names its source).
pub fn resolve(flag: Option<&str>) -> Result<Option<usize>, String> {
    if let Some(value) = flag {
        return parse(value, FLAG).map(Some);
    }
    match std::env::var(ENV_VAR) {
        Ok(value) => parse(&value, ENV_VAR).map(Some),
        Err(_) => Ok(None),
    }
}

fn parse(value: &str, source: &str) -> Result<usize, String> {
    if value.is_empty() {
        return Err(format!(
            "{source} must not be empty; give a number greater than 0 or unset it"
        ));
    }
    match value.parse::<usize>() {
        Ok(n) if n > 0 => Ok(n),
        _ => Err(format!(
            "{source} must be a whole number greater than 0 (got {value:?})"
        )),
    }
}

/// The JSON body of a rejection, the same for both servers.
pub fn rejection_json(max: usize) -> String {
    serde_json::json!({
        "success": false,
        "error": format!("Too many connections from this address (max {max})"),
    })
    .to_string()
}

/// Open connections per peer IP address (the port is ignored).
///
/// Ceiling: the map holds one entry per address that has at least one open, accepted
/// connection; entries are removed when their count reaches zero. Its size is therefore bounded
/// by the number of open sockets, which the process file-descriptor limit already caps. One
/// short lock per accept and per close; no I/O is done while it is held.
pub struct ConnLimiter {
    max: usize,
    open: Mutex<HashMap<IpAddr, usize>>,
}

impl ConnLimiter {
    pub fn new(max: usize) -> Arc<Self> {
        Arc::new(Self {
            max,
            open: Mutex::new(HashMap::new()),
        })
    }

    pub fn max(&self) -> usize {
        self.max
    }

    /// Count one more connection from `ip`, or `None` when it already has `max` open.
    pub fn try_acquire(self: &Arc<Self>, ip: IpAddr) -> Option<ConnGuard> {
        // An IPv4 client on a dual-stack socket shows up as ::ffff:a.b.c.d; count it as a.b.c.d.
        let ip = ip.to_canonical();
        let mut open = self.lock();
        let count = open.entry(ip).or_insert(0);
        if *count >= self.max {
            return None;
        }
        *count += 1;
        Some(ConnGuard {
            limiter: Arc::clone(self),
            ip,
        })
    }

    fn release(&self, ip: IpAddr) {
        let mut open = self.lock();
        if let Some(count) = open.get_mut(&ip) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                open.remove(&ip);
            }
        }
    }

    /// Lock the map. A panic while it is held cannot leave it inconsistent (each update is a
    /// single step), so a poisoned lock is used as is.
    fn lock(&self) -> MutexGuard<'_, HashMap<IpAddr, usize>> {
        self.open.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Addresses with open connections (tests only).
    #[cfg(test)]
    fn tracked(&self) -> usize {
        self.lock().len()
    }
}

/// One counted connection; dropping it gives the slot back.
pub struct ConnGuard {
    limiter: Arc<ConnLimiter>,
    ip: IpAddr,
}

impl Drop for ConnGuard {
    fn drop(&mut self) {
        self.limiter.release(self.ip);
    }
}

/// What the accept loop does with a just-accepted connection.
pub enum Admission {
    /// Queue it for a worker; the guard (none when there is no limit) moves with it.
    Accept(Option<ConnGuard>),
    /// Over the limit of this many connections: reject it from the accept thread.
    Reject(usize),
}

/// Decide on a just-accepted connection. `limiter` is `None` when no limit is set.
pub fn admit(limiter: Option<&Arc<ConnLimiter>>, stream: &TcpStream) -> Admission {
    let Some(limiter) = limiter else {
        return Admission::Accept(None);
    };
    // Without a peer address the connection cannot be attributed; let it through.
    let Ok(peer) = stream.peer_addr() else {
        return Admission::Accept(None);
    };
    match limiter.try_acquire(peer.ip()) {
        Some(guard) => Admission::Accept(Some(guard)),
        None => Admission::Reject(limiter.max()),
    }
}

/// Write `response` to a rejected connection and close it, from the accept thread. The write
/// has a short timeout so a peer that does not read cannot stall the accept loop. Input that has
/// already arrived is discarded first (without waiting), because closing a socket with unread
/// input sends a reset that can destroy the response before the client reads it.
pub fn reject(mut stream: TcpStream, response: &[u8]) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_write_timeout(Some(REJECT_WRITE_TIMEOUT));
    if stream.write_all(response).is_err() {
        return;
    }
    let _ = stream.shutdown(Shutdown::Write);
    if stream.set_nonblocking(true).is_ok() {
        let mut sink = [0u8; 4096];
        let mut drained = 0;
        while drained < REJECT_DRAIN_LIMIT {
            match stream.read(&mut sink) {
                Ok(n) if n > 0 => drained += n,
                _ => break,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn counts_per_address_and_releases_on_drop() {
        let limiter = ConnLimiter::new(2);
        let a = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let b = IpAddr::V6(Ipv6Addr::LOCALHOST);
        let a1 = limiter.try_acquire(a).unwrap();
        let _a2 = limiter.try_acquire(a).unwrap();
        assert!(limiter.try_acquire(a).is_none());
        let _b1 = limiter.try_acquire(b).unwrap();
        drop(a1);
        assert!(limiter.try_acquire(a).is_some());
        assert_eq!(limiter.tracked(), 2);
    }

    #[test]
    fn zero_entries_are_removed() {
        let limiter = ConnLimiter::new(1);
        let g = limiter
            .try_acquire(IpAddr::V4(Ipv4Addr::LOCALHOST))
            .unwrap();
        assert_eq!(limiter.tracked(), 1);
        drop(g);
        assert_eq!(limiter.tracked(), 0);
        // A rejected attempt leaves no entry behind either.
        let _g = limiter
            .try_acquire(IpAddr::V4(Ipv4Addr::LOCALHOST))
            .unwrap();
        assert!(limiter
            .try_acquire(IpAddr::V4(Ipv4Addr::LOCALHOST))
            .is_none());
        assert_eq!(limiter.tracked(), 1);
    }

    #[test]
    fn ipv4_mapped_ipv6_counts_as_ipv4() {
        let limiter = ConnLimiter::new(1);
        let _g = limiter
            .try_acquire(IpAddr::V4(Ipv4Addr::LOCALHOST))
            .unwrap();
        let mapped = IpAddr::V6(Ipv4Addr::LOCALHOST.to_ipv6_mapped());
        assert!(limiter.try_acquire(mapped).is_none());
    }

    #[test]
    fn guard_is_released_on_panic() {
        let limiter = ConnLimiter::new(1);
        let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
        let guard = limiter.try_acquire(ip).unwrap();
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _guard = guard;
            panic!("handler failed");
        }));
        assert!(limiter.try_acquire(ip).is_some());
    }

    #[test]
    fn parse_rejects_zero_empty_and_text() {
        assert_eq!(parse("3", FLAG), Ok(3));
        assert!(parse("0", FLAG).unwrap_err().contains("greater than 0"));
        assert!(parse("", ENV_VAR)
            .unwrap_err()
            .contains("must not be empty"));
        assert!(parse("abc", FLAG).unwrap_err().contains("\"abc\""));
        assert!(parse("-1", FLAG).is_err());
    }

    #[test]
    fn rejection_body() {
        assert_eq!(
            rejection_json(2),
            r#"{"error":"Too many connections from this address (max 2)","success":false}"#
        );
    }
}
