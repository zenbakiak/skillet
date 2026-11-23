//! Per-IP connection limit of `sk_http_server` and `sk_server` (ADR-0025, BL-066).
//! Servers run on 127.0.0.1 with enough workers for every connection the tests open. Timeouts
//! are generous and waits poll, so the tests hold under load.

mod common;

use common::ServerProcess;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const HTTP: &str = env!("CARGO_BIN_EXE_sk_http_server");
const LINE: &str = env!("CARGO_BIN_EXE_sk_server");
const ENV_VAR: &str = "SKILLET_MAX_CONNECTIONS_PER_IP";
const REJECTED: &str = "Too many connections from this address (max 2)";

fn command(bin: &str, args: &[String], env_limit: Option<&str>) -> Command {
    let hooks = std::env::temp_dir().join("skillet-server-conn-limit-no-hooks");
    let mut cmd = Command::new(bin);
    cmd.args(args)
        .env("SKILLET_HOOKS_DIR", hooks)
        .env("SKILLET_BIND_HOST", "127.0.0.1")
        .env_remove("SKILLET_AUTH_TOKEN")
        .env_remove("SKILLET_ADMIN_TOKEN")
        .env_remove("SKILLET_CORS_ORIGIN")
        .env_remove(ENV_VAR);
    if let Some(v) = env_limit {
        cmd.env(ENV_VAR, v);
    }
    cmd
}

/// A server binary on its own 127.0.0.1 port, killed on drop.
struct Server {
    process: ServerProcess,
    http: bool,
}

impl Server {
    /// Start `bin` with 12 workers and `extra` arguments.
    fn start(bin: &str, extra: &[&str], env_limit: Option<&str>) -> Server {
        let http = bin == HTTP;
        let process = ServerProcess::start(|port| {
            let mut args = vec![port.to_string()];
            if http {
                args.extend(["--threads".to_string(), "12".to_string()]);
            } else {
                args.push("12".to_string());
            }
            args.extend(extra.iter().map(|s| s.to_string()));
            let mut cmd = command(bin, &args, env_limit);
            cmd.stdout(Stdio::null());
            cmd
        });
        Server { process, http }
    }

    fn connect(&self) -> TcpStream {
        self.process.connect()
    }

    /// Send one evaluation request on `s` and return the response, or `None` if the connection
    /// ended without one.
    fn request(&self, mut s: TcpStream) -> Option<String> {
        s.set_read_timeout(Some(Duration::from_secs(60))).ok()?;
        if self.http {
            let req = "GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
            s.write_all(req.as_bytes()).ok()?;
            let mut resp = String::new();
            let _ = s.read_to_string(&mut resp);
            (!resp.is_empty()).then_some(resp)
        } else {
            s.write_all(b"{\"expression\":\"=1+1\"}\n").ok()?;
            let mut line = String::new();
            let _ = BufReader::new(s).read_line(&mut line);
            (!line.is_empty()).then_some(line)
        }
    }

    fn served(&self, resp: &str) -> bool {
        if self.http {
            resp.starts_with("HTTP/1.1 200")
        } else {
            resp.contains("\"success\":true")
        }
    }

    /// Read what the server sends on a fresh connection without sending anything first.
    fn read_unprompted(&self) -> String {
        let mut s = self.connect();
        let mut resp = String::new();
        let _ = s.read_to_string(&mut resp);
        resp
    }

    /// Request on a new connection until one is served (a closed connection's slot is given
    /// back asynchronously, when its worker notices the close).
    fn eventually_served(&self) {
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut last = String::new();
        while Instant::now() < deadline {
            if let Some(resp) = self.request(self.connect()) {
                if self.served(&resp) {
                    return;
                }
                last = resp;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        panic!("no connection was served within 30 s; last response: {last:?}");
    }
}

fn assert_rejected(server: &Server, resp: &str) {
    if server.http {
        assert!(
            resp.starts_with("HTTP/1.1 429 Too Many Requests\r\n"),
            "{resp:?}"
        );
        assert!(resp.contains("\r\nConnection: close\r\n"), "{resp:?}");
        let body = &resp[resp.find("\r\n\r\n").expect("header end") + 4..];
        let json: serde_json::Value = serde_json::from_str(body).expect("JSON body");
        assert_eq!(json["success"], false);
        assert_eq!(json["error"], REJECTED);
    } else {
        assert!(resp.ends_with('\n'), "one line: {resp:?}");
        let json: serde_json::Value = serde_json::from_str(resp.trim_end()).expect("JSON line");
        assert_eq!(json["success"], false);
        assert_eq!(json["error"], REJECTED);
    }
}

/// Two idle connections fill the limit; a third is rejected without reaching a worker; after
/// one closes, a new connection is served.
fn limit_of_two(server: &Server) {
    // A connection closed just before (the start-up probe) is released asynchronously, so its
    // slot may still be counted for a moment; then `second` is the one rejected. Detect that
    // and try again.
    for _ in 0..10 {
        let first = server.connect();
        let mut second = server.connect();
        // Connections are accepted in order, so both are counted before the third.
        let resp = server.read_unprompted();
        assert_rejected(server, &resp);

        second
            .set_read_timeout(Some(Duration::from_millis(500)))
            .unwrap();
        let second_got_data = second.read(&mut [0u8; 1]).is_ok();
        if second_got_data {
            drop((first, second));
            std::thread::sleep(Duration::from_millis(200));
            continue;
        }

        drop(first);
        server.eventually_served();
        return;
    }
    panic!("the second idle connection was rejected in every attempt");
}

#[test]
fn http_server_limit_from_flag() {
    let server = Server::start(HTTP, &["--max-connections-per-ip", "2"], None);
    limit_of_two(&server);
}

#[test]
fn http_server_limit_from_env() {
    let server = Server::start(HTTP, &[], Some("2"));
    limit_of_two(&server);
}

#[test]
fn sk_server_limit_from_flag() {
    let server = Server::start(LINE, &["--max-connections-per-ip", "2"], None);
    limit_of_two(&server);
}

#[test]
fn sk_server_limit_from_env() {
    let server = Server::start(LINE, &[], Some("2"));
    limit_of_two(&server);
}

/// The flag wins over the env var: an invalid env value is not read when the flag is given.
#[test]
fn flag_wins_over_env() {
    for bin in [HTTP, LINE] {
        let server = Server::start(bin, &["--max-connections-per-ip", "2"], Some("abc"));
        limit_of_two(&server);
    }
}

/// Without a limit, ten connections from one address are open at once and all are served.
fn no_limit(server: &Server) {
    let conns: Vec<TcpStream> = (0..10).map(|_| server.connect()).collect();
    for s in conns {
        let resp = server.request(s).expect("a response");
        assert!(server.served(&resp), "{resp:?}");
    }
}

#[test]
fn http_server_no_limit_by_default() {
    no_limit(&Server::start(HTTP, &[], None));
}

#[test]
fn sk_server_no_limit_by_default() {
    no_limit(&Server::start(LINE, &[], None));
}

/// Start `bin` with `args` and return (exit code, stderr); it must exit on its own.
fn start_up_failure(bin: &str, args: &[&str], env_limit: Option<&str>) -> (Option<i32>, String) {
    let mut all = vec!["0".to_string()];
    all.extend(args.iter().map(|s| s.to_string()));
    let mut child = command(bin, &all, env_limit)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn server");
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            let mut err = String::new();
            child
                .stderr
                .take()
                .unwrap()
                .read_to_string(&mut err)
                .unwrap();
            return (status.code(), err);
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("{bin} {args:?} (env {env_limit:?}) did not exit");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn invalid_values_fail_at_start_up() {
    for bin in [HTTP, LINE] {
        for value in ["0", "", "abc", "-1", "2x"] {
            let (code, err) = start_up_failure(bin, &["--max-connections-per-ip", value], None);
            assert_eq!(code, Some(1), "{bin} flag {value:?}: {err}");
            assert!(err.contains("--max-connections-per-ip must"), "{err}");

            let (code, err) = start_up_failure(bin, &[], Some(value));
            assert_eq!(code, Some(1), "{bin} env {value:?}: {err}");
            assert!(err.contains("SKILLET_MAX_CONNECTIONS_PER_IP must"), "{err}");
        }
        let (code, err) = start_up_failure(bin, &["--max-connections-per-ip"], None);
        assert_eq!(code, Some(1), "{bin} missing value: {err}");
    }
}
