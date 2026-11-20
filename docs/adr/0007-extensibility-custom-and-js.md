# 0007. Extensibility: `CustomFunction` trait + QuickJS plugins

- Status: Accepted (retroactive, v0.6.3)
- Date: 2026-11-20
- Related: `src/custom.rs`, `src/js_plugin.rs`, PRD-002

## Context
Users need domain functions (e.g. Mexican ISR tax in `hooks/mx_isr.js`) without forking the crate.

## Decision
- Rust: implement `custom::CustomFunction` and register in a process-global
  `lazy_static` `Arc<RwLock<FunctionRegistry>>` (`register_function`, `evaluate_with_custom`).
- JavaScript: `JSPluginLoader` loads `.js` files from a hooks dir (`SKILLET_HOOKS_DIR`, default
  `hooks/`) and runs them in embedded QuickJS (`rquickjs`). Behind the default-on `plugins` feature.
- The HTTP server can upload/update/delete/reload JS functions at runtime (admin-token protected).

## Consequences
- + Extensible from Rust and from ops (no rebuild) alike.
- − Global registry = shared mutable state; tests and multi-tenant servers can interfere.
- − Uploading JS over HTTP is code execution by design: admin token, size limits and sandboxing
  are security-critical (needs a security review).
- Core must keep building with `--no-default-features`.
