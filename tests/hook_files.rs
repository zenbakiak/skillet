//! JS function management on `sk_http_server` only touches plain `.js` names directly inside
//! the hooks directory, and listing/loading stays inside it. Hooks may not use a builtin's
//! name (ADR-0024): loading skips them with a warning, upload and update reject them. Reload
//! makes the registry match the hooks on disk (BL-077).
//! Servers run on 127.0.0.1 with a temporary hooks directory.
#![cfg(feature = "plugins")]

mod common;

use common::ServerProcess;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const ADMIN: &str = "hook-files-admin";

const HOOK: &str = "// @name: HOOKFILESOK\n// @min_args: 0\nfunction execute(args) { return 1; }\n";

/// `sk_http_server` on a free 127.0.0.1 port with `hooks` as its hooks dir, killed on drop.
struct Server(ServerProcess);

impl Server {
    fn start(hooks: &Path) -> Server {
        Server(ServerProcess::start(|port| {
            let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_sk_http_server"));
            cmd.args([&port.to_string(), "--threads", "1", "--admin-token", ADMIN])
                .env("SKILLET_HOOKS_DIR", hooks)
                .env("SKILLET_BIND_HOST", "127.0.0.1")
                .env_remove("SKILLET_AUTH_TOKEN")
                .env_remove("SKILLET_ADMIN_TOKEN")
                .env_remove("SKILLET_CORS_ORIGIN")
                .stdout(std::process::Stdio::null());
            cmd
        }))
    }

    fn request(&self, method: &str, path: &str, body: &str) -> String {
        let mut s = self.0.connect();
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

fn hook_returning(name: &str, value: u32) -> String {
    format!("// @name: {name}\n// @min_args: 0\nfunction execute(args) {{ return {value}; }}\n")
}

fn eval(server: &Server, expression: &str) -> String {
    let body = serde_json::json!({ "expression": expression }).to_string();
    server.request("POST", "/eval", &body)
}

/// BL-077: reload drops a hook whose file is gone, even one whose result is cached.
#[test]
fn reload_hooks_drops_a_hook_whose_file_was_deleted() {
    let (_base, hooks, _outside) = layout();
    std::fs::write(hooks.join("gone.js"), hook_returning("GONEFN", 7)).unwrap();
    std::fs::write(hooks.join("kept.js"), hook_returning("KEPTFN", 8)).unwrap();
    let server = Server::start(&hooks);
    let resp = eval(&server, "=GONEFN() + KEPTFN()");
    assert!(resp.contains("\"result\":15"), "{resp}");

    std::fs::remove_file(hooks.join("gone.js")).unwrap();
    let resp = server.request("POST", "/reload-hooks", "");
    assert_eq!(status(&resp), "200", "{resp}");
    assert!(resp.contains("\"functions_loaded\":1"), "{resp}");

    let resp = eval(&server, "=GONEFN() + KEPTFN()");
    assert_eq!(status(&resp), "400", "{resp}");
    assert!(resp.contains("Unknown function: GONEFN"), "{resp}");
    let resp = eval(&server, "=KEPTFN()");
    assert!(resp.contains("\"result\":8"), "{resp}");
}

/// BL-077: a hook deleted through the API is gone after the next reload.
#[test]
fn reload_hooks_after_delete_js_removes_the_function() {
    let (_base, hooks, _outside) = layout();
    let server = Server::start(&hooks);
    let resp = server.with_filename("POST", "/upload-js", "ok.js");
    assert_eq!(status(&resp), "200", "{resp}");
    let resp = server.request("POST", "/reload-hooks", "");
    assert!(resp.contains("\"functions_loaded\":1"), "{resp}");
    assert!(eval(&server, "=HOOKFILESOK()").contains("\"result\":1"));

    let body = serde_json::json!({ "filename": "ok.js" }).to_string();
    let resp = server.request("DELETE", "/delete-js", &body);
    assert_eq!(status(&resp), "200", "{resp}");
    let resp = server.request("POST", "/reload-hooks", "");
    assert!(resp.contains("\"functions_loaded\":0"), "{resp}");
    let resp = eval(&server, "=HOOKFILESOK()");
    assert!(resp.contains("Unknown function: HOOKFILESOK"), "{resp}");
}

/// A changed hook is used after reload, not a result cached from the old version.
#[test]
fn reload_hooks_replaces_a_changed_hook_and_its_cached_results() {
    let (_base, hooks, _outside) = layout();
    std::fs::write(hooks.join("mine.js"), hook_returning("MINEFN", 7)).unwrap();
    let server = Server::start(&hooks);
    assert!(eval(&server, "=MINEFN()").contains("\"result\":7"));

    std::fs::write(hooks.join("mine.js"), hook_returning("MINEFN", 9)).unwrap();
    let resp = server.request("POST", "/reload-hooks", "");
    assert_eq!(status(&resp), "200", "{resp}");
    let resp = eval(&server, "=MINEFN()");
    assert!(resp.contains("\"result\":9"), "{resp}");
}

/// A reload that cannot read the hooks directory fails and keeps every loaded hook.
#[test]
fn failed_reload_hooks_keeps_the_loaded_hooks() {
    let (_base, hooks, _outside) = layout();
    std::fs::write(hooks.join("mine.js"), hook_returning("MINEFN", 7)).unwrap();
    let server = Server::start(&hooks);
    assert!(eval(&server, "=MINEFN()").contains("\"result\":7"));

    // The hooks path is now a file, so reading it as a directory fails.
    std::fs::remove_dir_all(&hooks).unwrap();
    std::fs::write(&hooks, "not a directory").unwrap();
    let resp = server.request("POST", "/reload-hooks", "");
    assert_eq!(status(&resp), "500", "{resp}");
    assert!(resp.contains("\"success\":false"), "{resp}");
    let resp = eval(&server, "=MINEFN() + 1");
    assert!(resp.contains("\"result\":8"), "{resp}");
}
