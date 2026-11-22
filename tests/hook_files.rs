//! JS function management on `sk_http_server` only touches plain `.js` names directly inside
//! the hooks directory, and listing/loading stays inside it. Hooks may not use a builtin's
//! name (ADR-0024): loading skips them with a warning, upload and update reject them.
//! Servers run on 127.0.0.1 with a temporary hooks directory.
#![cfg(feature = "plugins")]

use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

const ADMIN: &str = "hook-files-admin";

const HOOK: &str = "// @name: HOOKFILESOK\n// @min_args: 0\nfunction execute(args) { return 1; }\n";

/// `sk_http_server` on a free 127.0.0.1 port with `hooks` as its hooks dir, killed on drop.
struct Server {
    child: std::process::Child,
    port: u16,
}

impl Server {
    fn start(hooks: &Path) -> Server {
        let port = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port();
        let child = std::process::Command::new(env!("CARGO_BIN_EXE_sk_http_server"))
            .args([&port.to_string(), "--threads", "1", "--admin-token", ADMIN])
            .env("SKILLET_HOOKS_DIR", hooks)
            .env("SKILLET_BIND_HOST", "127.0.0.1")
            .env_remove("SKILLET_AUTH_TOKEN")
            .env_remove("SKILLET_ADMIN_TOKEN")
            .env_remove("SKILLET_CORS_ORIGIN")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn server");
        for _ in 0..200 {
            if TcpStream::connect(("127.0.0.1", port)).is_ok() {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Server { child, port }
    }

    fn request(&self, method: &str, path: &str, body: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", self.port)).expect("connect");
        s.set_read_timeout(Some(Duration::from_secs(30))).unwrap();
        let req = format!(
            "{method} {path} HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {ADMIN}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        s.write_all(req.as_bytes()).unwrap();
        let mut resp = String::new();
        let _ = s.read_to_string(&mut resp);
        resp
    }

    fn with_filename(&self, method: &str, path: &str, filename: &str) -> String {
        let body = serde_json::json!({ "filename": filename, "js_code": HOOK }).to_string();
        self.request(method, path, &body)
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A temp dir holding `hooks/` (the hooks dir) and `outside.js` next to it.
fn layout() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let base = tempfile::tempdir().unwrap();
    let hooks = base.path().join("hooks");
    std::fs::create_dir(&hooks).unwrap();
    let outside = base.path().join("outside.js");
    std::fs::write(&outside, "original").unwrap();
    (base, hooks, outside)
}

fn status(resp: &str) -> &str {
    resp.split_whitespace().nth(1).unwrap_or("")
}

const NOT_PLAIN: &[&str] = &[
    "../outside.js",
    "sub/name.js",
    "sub\\name.js",
    ".hidden.js",
    ".js",
    "name.js.txt",
    "na me.js",
];

#[test]
fn upload_rejects_filenames_that_are_not_plain_names() {
    let (base, hooks, outside) = layout();
    let server = Server::start(&hooks);
    let absolute = base.path().join("absolute.js");
    let absolute = absolute.to_str().unwrap();
    for name in NOT_PLAIN.iter().copied().chain([absolute]) {
        let resp = server.with_filename("POST", "/upload-js", name);
        assert_eq!(status(&resp), "400", "{name}: {resp}");
    }
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "original");
    assert!(!base.path().join("absolute.js").exists());
    assert_eq!(std::fs::read_dir(&hooks).unwrap().count(), 0);
}

#[test]
fn update_rejects_filenames_that_are_not_plain_names() {
    let (_base, hooks, outside) = layout();
    let server = Server::start(&hooks);
    for name in NOT_PLAIN.iter().copied().chain([outside.to_str().unwrap()]) {
        let resp = server.with_filename("PUT", "/update-js", name);
        assert_eq!(status(&resp), "400", "{name}: {resp}");
    }
    assert_eq!(std::fs::read_to_string(&outside).unwrap(), "original");
}

#[test]
fn delete_rejects_filenames_that_are_not_plain_names() {
    let (_base, hooks, outside) = layout();
    let server = Server::start(&hooks);
    for name in NOT_PLAIN.iter().copied().chain([outside.to_str().unwrap()]) {
        let body = serde_json::json!({ "filename": name }).to_string();
        let resp = server.request("DELETE", "/delete-js", &body);
        assert_eq!(status(&resp), "400", "{name}: {resp}");
    }
    assert!(outside.exists());
}

#[test]
fn plain_filenames_upload_update_and_delete() {
    let (_base, hooks, _outside) = layout();
    let server = Server::start(&hooks);
    let name = "Hook_files-1.js";
    let resp = server.with_filename("POST", "/upload-js", name);
    assert_eq!(status(&resp), "200", "{resp}");
    assert!(hooks.join(name).exists());
    let resp = server.with_filename("PUT", "/update-js", name);
    assert_eq!(status(&resp), "200", "{resp}");
    let body = serde_json::json!({ "filename": name }).to_string();
    let resp = server.request("DELETE", "/delete-js", &body);
    assert_eq!(status(&resp), "200", "{resp}");
    assert!(!hooks.join(name).exists());
}

#[cfg(unix)]
#[test]
fn list_skips_links_that_leave_the_hooks_dir() {
    let (base, hooks, outside) = layout();
    let elsewhere = base.path().join("elsewhere");
    std::fs::create_dir(&elsewhere).unwrap();
    std::fs::write(elsewhere.join("far.js"), HOOK).unwrap();
    std::os::unix::fs::symlink(&elsewhere, hooks.join("linked_dir")).unwrap();
    std::os::unix::fs::symlink(&outside, hooks.join("linked.js")).unwrap();
    std::os::unix::fs::symlink(&hooks, hooks.join("loop")).unwrap();
    std::fs::write(hooks.join("inside.js"), HOOK).unwrap();

    let server = Server::start(&hooks);
    let resp = server.request("GET", "/list-js", "");
    assert_eq!(status(&resp), "200", "{resp}");
    assert!(resp.contains("inside.js"), "{resp}");
    assert!(!resp.contains("far.js"), "{resp}");
    assert!(!resp.contains("linked.js"), "{resp}");
}

/// A hook named `name` that returns 999.
fn named_hook(name: &str) -> String {
    format!("// @name: {name}\n// @min_args: 0\nfunction execute(args) {{ return 999; }}\n")
}

#[test]
fn sk_skips_a_hook_named_like_a_builtin_and_loads_the_others() {
    let (_base, hooks, _outside) = layout();
    std::fs::write(hooks.join("sum.js"), named_hook("SUM")).unwrap();
    std::fs::write(hooks.join("lower_if.js"), named_hook("if")).unwrap();
    std::fs::write(hooks.join("myfn.js"), named_hook("MYFN")).unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_sk"))
        .arg("=SUM(1, 2) + IF(TRUE, 10, 0) + MYFN()")
        .env("SKILLET_HOOKS_DIR", &hooks)
        .output()
        .expect("run sk");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(out.status.success(), "{stdout}\n{stderr}");
    // Builtin SUM (3) and IF (10), plus the hook MYFN (999).
    assert_eq!(stdout.trim(), "Number(1012.0)", "{stderr}");
    let warnings: Vec<_> = stderr.lines().filter(|l| l.contains("reserved")).collect();
    assert_eq!(warnings.len(), 2, "{stderr}");
    assert!(
        warnings
            .iter()
            .any(|l| l.contains("sum.js")
                && l.contains("Function name SUM is reserved by a builtin")),
        "{stderr}"
    );
    assert!(
        warnings.iter().any(|l| l.contains("lower_if.js")
            && l.contains("Function name IF is reserved by a builtin")),
        "{stderr}"
    );
    assert!(
        stderr.contains("Loaded 1 custom JavaScript function(s)"),
        "{stderr}"
    );
}

#[test]
fn reload_hooks_skips_a_hook_named_like_a_builtin() {
    let (_base, hooks, _outside) = layout();
    let server = Server::start(&hooks);
    std::fs::write(hooks.join("sum.js"), named_hook("SUM")).unwrap();
    std::fs::write(hooks.join("myfn.js"), named_hook("MYFN")).unwrap();
    let resp = server.request("POST", "/reload-hooks", "");
    assert_eq!(status(&resp), "200", "{resp}");
    assert!(resp.contains("\"functions_loaded\":1"), "{resp}");
    let body = serde_json::json!({ "expression": "=SUM(1, 2) + MYFN()" }).to_string();
    let resp = server.request("POST", "/eval", &body);
    assert_eq!(status(&resp), "200", "{resp}");
    assert!(resp.contains("1002"), "{resp}");
}

fn reserved_body(filename: &str, name: &str) -> String {
    serde_json::json!({ "filename": filename, "js_code": named_hook(name) }).to_string()
}

#[test]
fn upload_rejects_a_builtin_name_in_any_case() {
    let (_base, hooks, _outside) = layout();
    let server = Server::start(&hooks);
    for (name, shown) in [("SUM", "SUM"), ("sum", "SUM"), ("Stdev.P", "STDEV.P")] {
        let resp = server.request("POST", "/upload-js", &reserved_body("sum.js", name));
        assert_eq!(status(&resp), "400", "{name}: {resp}");
        let want = format!("Function name {shown} is reserved by a builtin");
        assert!(resp.contains(&want), "{name}: {resp}");
    }
    assert_eq!(std::fs::read_dir(&hooks).unwrap().count(), 0);
}

#[test]
fn update_rejects_a_builtin_name_in_any_case() {
    let (_base, hooks, _outside) = layout();
    let server = Server::start(&hooks);
    let resp = server.with_filename("POST", "/upload-js", "mine.js");
    assert_eq!(status(&resp), "200", "{resp}");
    for name in ["SUM", "sum"] {
        let resp = server.request("PUT", "/update-js", &reserved_body("mine.js", name));
        assert_eq!(status(&resp), "400", "{name}: {resp}");
        assert!(
            resp.contains("Function name SUM is reserved by a builtin"),
            "{name}: {resp}"
        );
    }
    assert_eq!(
        std::fs::read_to_string(hooks.join("mine.js")).unwrap(),
        HOOK
    );
}
