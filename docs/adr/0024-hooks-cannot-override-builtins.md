# 0024. JS hooks cannot override builtins

- Status: Accepted
- Date: 2026-11-21
- Deciders: Maintainer (BL-066, threat model §2.6)
- Related: [ADR-0016](0016-custom-function-visibility.md), [ADR-0017](0017-quickjs-resource-limits.md),
  `src/runtime/builtin_table.rs`, `src/js_plugin.rs`, `src/bin/http_server/js_management.rs`

## Context

Custom functions are looked up before builtins. JS hooks are custom functions, so a
`hooks/sum.js` with `// @name: SUM` replaced `SUM`. The default hooks directory is the relative
path `hooks`, so running `sk "=SUM(1, 2)"` in a directory that holds such a file returned the
hook's result (threat model §2.6, Medium). The same applied to `sk_server`, `sk_http_server`,
`/reload-hooks` and hooks uploaded through `/upload-js` or `/update-js`.

## Decision

1. A **reserved name** is any name or alias in the builtin table, dotted names included
   (`STDEV.P`), plus the internal operator names (`__TERNARY__`, `__CONCAT__`, ...). The table
   rows already include the names the evaluator runs itself (`IF`, `IFS`, `FILTER`, `MAP`,
   `REDUCE`, `FIND`, `SUMIF`, `COUNTIF`, `AVERAGEIF`, `JQ`, ...). Matching is case-insensitive,
   with the same `to_uppercase` the custom registry uses. One crate-private function,
   `is_reserved_function_name`, decides; a unit test runs it over the whole table, so new
   builtins are reserved automatically.
2. Loading a hooks directory (`JSPluginLoader`, used by all three binaries and `/reload-hooks`)
   skips a hook with a reserved `@name`, prints one warning on stderr with the file and the
   name, and loads the other hooks.
3. `POST /upload-js` and `PUT /update-js` answer HTTP 400 with
   `"message": "Function name SUM is reserved by a builtin"` (the name upper-cased) and
   `"error": "Reserved function name"`, and write nothing.
4. The binaries reach the check through a new public method,
   `JavaScriptFunction::check_name_not_reserved`. `JavaScriptFunction::parse_js_function` itself
   is unchanged.
5. Rust `CustomFunction`s registered through `register_function` are **not** affected. They are
   embedder code, compiled into the program, and may still shadow a builtin
   (`tests/custom_functions.rs::test_custom_function_priority_over_builtin`, ADR-0016).

## Consequences

- The relative default `hooks` directory stays as it is. With this rule a stray hooks folder can
  only **add** functions; it can no longer change what an existing expression returns.
- A deployment that relied on a hook replacing a builtin must rename the hook (and the calls) or
  register a Rust `CustomFunction` instead. The skip is visible on stderr, not silent.
- New builtins become reserved with no extra code. A builtin added later can make an existing
  hook of the same name stop loading; the warning names it.
