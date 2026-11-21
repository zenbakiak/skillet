# 0021. Evaluate a parsed expression; servers cache parsed expressions per worker thread

- Status: Accepted
- Date: 2026-11-21
- Deciders: Maintainer
- Related: BL-012, ADR-0002 (tree-walking interpreter, `Rc` AST), ADR-0008 (servers on std
  threads), ADR-0011 (evaluation budget and deadline), BL-017 (nesting depth)

## Context

`sk_server` and `sk_http_server` call `evaluate_with_custom` / `evaluate_with_assignments*`, which
parse the text on every request. The `sk_http_server` result cache does not help when the same
formula comes with different variables, which is the normal case for a rules or pricing service.

The public API had no supported way to evaluate an `Expr`: `skillet::parse` returned one, but only
`skillet::runtime::evaluator::eval*` (public module, undocumented) accepted it.

A prototype measured less than 2% gain on the BL-005 baseline expressions (parsing takes
0.3-1.2 µs of a ~110 µs request) and a clear gain only on the 1.8 KB `parse_heavy` expression
(~25 µs of parse). The maintainer chose to add the cache anyway, because Skillet supports and is
used with large formulas.

The AST uses `Rc` (ADR-0002), so it is neither `Send` nor `Sync` and cannot sit in a cache shared
by worker threads. Moving the AST to `Arc` would touch every node and cost atomic reference
counting on every clone, for a cache that per-thread copies serve just as well.

## Decision

1. **Public API (additive).** `src/lib.rs` gets one `evaluate_parsed*` twin for each text entry
   point that the servers and typical embedders use:
   `evaluate_parsed(&Expr)`, `evaluate_parsed_with(&Expr, &vars)`,
   `evaluate_parsed_with_custom(&Expr, &vars)`, `evaluate_parsed_with_assignments(&Expr, &vars)`,
   `evaluate_parsed_with_assignments_and_context(&Expr, &vars)`. The text functions now call them:
   `evaluate_x(text, …)` is `evaluate_parsed_x(&parse(text)?, …)`, so results and errors are the
   same by construction. The JSON-variable functions get no twin (they only build the map).
2. **Limits.** The nesting limit (`MAX_NESTING_DEPTH`) stays a parse-time check: `parse` enforces
   it, and the evaluator applies it to arrays it builds, as before. The budget and deadline
   (ADR-0011) are applied by each `evaluate_parsed*` call exactly as by its text twin (a fresh
   default scope per call, or the enclosing `with_limits` scope). A hand-built `Expr` is not
   depth-checked; the rustdoc says to pass trees from `parse`. This adds no new exposure:
   `runtime::evaluator::eval` already accepted any `Expr`.
3. **Server cache.** Both servers keep a `thread_local!` `lru::LruCache<String, Rc<Expr>>` per
   worker thread (`src/bin/http_server/parse_cache.rs`), keyed by the exact expression text.
   Bounds per thread: 256 entries, 128 KiB of key text in total, and text over 32 KiB is parsed but
   not stored. Parse errors are not cached. Parsing still happens inside the request's
   `with_limits` scope. The `sk_http_server` result cache is unchanged; it is checked first.
4. **Memory bound.** Measured with a counting allocator (release build), a parsed tree takes 31-76
   times the bytes of its text; the worst shapes are many tiny nodes (`[{a:0},{a:0},…]` 76×,
   `[0+0,…]` 66×), and the `parse_heavy` bench formula is 33×. So one worker thread holds at most
   ~10 MiB of trees (128 KiB × 76), ~4 MiB typically, and a server with N threads N times that.
   The bound is on text bytes instead of measured tree bytes because it is O(1). The 76× is the
   worst shape measured, not a proven maximum; every node needs at least one byte of text, so the
   ratio cannot grow without limit.

## Consequences

- Repeated expressions skip the parse: over HTTP, one connection at a time, 1.8 KB `parse_heavy`
  190 → 140 µs per request (+41% req/s); short formulas unchanged within noise
  (`benches/BASELINE.md`, "BL-012").
- The public API grows by five functions; they must keep the same semantics as their text twins
  (pinned by `tests/evaluate_parsed.rs`).
- Server memory grows by up to ~10 MiB per worker thread in the worst case. The bounds are
  constants, not flags; changing them is a code change.
- Identical formulas that differ only in whitespace or a leading `=` are separate entries.
- There are no cache statistics and no way to clear the parse cache; it holds no results, so it
  never returns stale data (custom functions are looked up at evaluation time).

## Alternatives considered

- **`Arc<Expr>` and one shared cache.** Needs an AST change across the parser and evaluator,
  atomic counts on every node clone, and a lock on every request. Rejected: per-thread caches give
  the same hit rate once each worker has seen a formula.
- **Cache inside the library** (`parse` memoizing). Hides memory use from embedders and makes
  `parse` stateful. Rejected; embedders can cache `Expr` themselves with the new API.
- **No cache, document the API only.** Leaves the servers re-parsing large formulas; rejected by
  the maintainer.
