//! Per-worker-thread cache of parsed expressions (BL-012, ADR-0021), used by `sk_http_server`
//! and `sk_server`.
//!
//! The AST uses `Rc` (ADR-0002), so it cannot be shared between threads: each worker thread keeps
//! its own LRU cache, keyed by the exact expression text. Only successful parses are stored; a
//! parse error is returned and parsed again next time.
//!
//! Memory bound, per worker thread: at most [`MAX_ENTRIES`] expressions and [`MAX_TOTAL_BYTES`]
//! of expression text; an expression longer than [`MAX_ENTRY_BYTES`] is parsed but not stored.
//! A parsed tree takes 31-76 times the bytes of its text (measured with a counting allocator on
//! the shapes with the most nodes per byte, such as `[{a:0},{a:0},…]`; the 2.6 KB `parse_heavy`
//! bench expression is 33 times), so one worker holds at most ~10 MiB of trees, typically ~4 MiB.

use lru::LruCache;
use skillet::{Error, Expr};
use std::cell::RefCell;
use std::num::NonZeroUsize;
use std::rc::Rc;

/// Most expressions one worker thread keeps.
pub const MAX_ENTRIES: usize = 256;
/// Longest expression text that is stored; longer ones are parsed on every request.
pub const MAX_ENTRY_BYTES: usize = 32 * 1024;
/// Most expression text, summed over the entries, that one worker thread keeps.
pub const MAX_TOTAL_BYTES: usize = 128 * 1024;

const CAPACITY: NonZeroUsize = match NonZeroUsize::new(MAX_ENTRIES) {
    Some(n) => n,
    None => panic!("MAX_ENTRIES must not be 0"),
};

/// LRU of parsed trees with a bound on the summed key length.
struct ParseCache {
    map: LruCache<String, Rc<Expr>>,
    text_bytes: usize,
}

impl ParseCache {
    fn new() -> Self {
        Self {
            map: LruCache::new(CAPACITY),
            text_bytes: 0,
        }
    }

    fn get(&mut self, expression: &str) -> Option<Rc<Expr>> {
        self.map.get(expression).cloned()
    }

    /// Store `expr` under `expression`, evicting least recently used entries until both bounds
    /// hold. Text longer than [`MAX_ENTRY_BYTES`] is not stored.
    fn insert(&mut self, expression: &str, expr: Rc<Expr>) {
        let len = expression.len();
        if len > MAX_ENTRY_BYTES {
            return;
        }
        while self.map.len() >= MAX_ENTRIES || self.text_bytes + len > MAX_TOTAL_BYTES {
            match self.map.pop_lru() {
                Some((key, _)) => self.text_bytes -= key.len(),
                None => break,
            }
        }
        self.text_bytes += len;
        self.map.put(expression.to_string(), expr);
    }
}

thread_local! {
    static PARSE_CACHE: RefCell<ParseCache> = RefCell::new(ParseCache::new());
}

/// The parsed tree of `expression`, from this thread's cache or from [`skillet::parse`].
///
/// Same result and same errors as `skillet::parse(expression)`, including the nesting limit.
pub fn parsed(expression: &str) -> Result<Rc<Expr>, Error> {
    if let Some(expr) = PARSE_CACHE.with(|c| c.borrow_mut().get(expression)) {
        return Ok(expr);
    }
    let expr = Rc::new(skillet::parse(expression)?);
    PARSE_CACHE.with(|c| c.borrow_mut().insert(expression, Rc::clone(&expr)));
    Ok(expr)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cached_len() -> usize {
        PARSE_CACHE.with(|c| c.borrow().map.len())
    }

    // Each #[test] runs on its own thread, so each starts with an empty cache.

    #[test]
    fn repeated_expression_reuses_the_tree() {
        let a = parsed("1 + :x").unwrap();
        let b = parsed("1 + :x").unwrap();
        assert!(Rc::ptr_eq(&a, &b));
        assert_eq!(*a, skillet::parse("1 + :x").unwrap());
        assert_eq!(cached_len(), 1);
    }

    #[test]
    fn parse_errors_are_returned_and_not_cached() {
        let deep = format!("{}1{}", "(".repeat(129), ")".repeat(129));
        for text in ["2 +", deep.as_str()] {
            let want = skillet::parse(text).unwrap_err();
            for _ in 0..2 {
                let got = parsed(text).unwrap_err();
                assert_eq!(
                    (got.message, got.position),
                    (want.message.clone(), want.position)
                );
            }
        }
        assert_eq!(cached_len(), 0);
    }

    #[test]
    fn long_expressions_are_not_stored() {
        let long = format!("[{}0]", "0,".repeat(MAX_ENTRY_BYTES / 2));
        assert!(long.len() > MAX_ENTRY_BYTES);
        let a = parsed(&long).unwrap();
        let b = parsed(&long).unwrap();
        assert!(!Rc::ptr_eq(&a, &b));
        assert_eq!(cached_len(), 0);
    }

    #[test]
    fn entry_count_is_bounded() {
        for i in 0..MAX_ENTRIES + 10 {
            parsed(&format!("{i} + 1")).unwrap();
        }
        assert_eq!(cached_len(), MAX_ENTRIES);
        // The oldest entries went first.
        let newest = parsed(&format!("{} + 1", MAX_ENTRIES + 9)).unwrap();
        assert!(Rc::ptr_eq(
            &newest,
            &parsed(&format!("{} + 1", MAX_ENTRIES + 9)).unwrap()
        ));
        assert!(PARSE_CACHE.with(|c| !c.borrow().map.contains("0 + 1")));
    }

    #[test]
    fn total_text_is_bounded() {
        // Entries of just under MAX_ENTRY_BYTES: only MAX_TOTAL_BYTES / MAX_ENTRY_BYTES fit.
        for i in 0..10 {
            let text = format!("{i} + [{}0]", "0,".repeat(MAX_ENTRY_BYTES / 2 - 10));
            assert!(text.len() <= MAX_ENTRY_BYTES);
            parsed(&text).unwrap();
            let total = PARSE_CACHE.with(|c| c.borrow().text_bytes);
            assert!(total <= MAX_TOTAL_BYTES, "{total}");
        }
        assert_eq!(cached_len(), MAX_TOTAL_BYTES / MAX_ENTRY_BYTES);
    }
}
