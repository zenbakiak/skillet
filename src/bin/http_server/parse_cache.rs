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
//!
//! Hits, misses and the stored trees are counted over all workers ([`stats`], BL-067).
//! [`clear_all`] empties the calling thread's cache at once and every other worker's at its next
//! lookup: a worker cannot reach another thread's `Rc` trees, so it bumps a generation number
//! that each worker compares before using its cache.

use lru::LruCache;
use skillet::{Error, Expr};
use std::cell::RefCell;
use std::num::NonZeroUsize;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

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

/// Counters shared by every worker thread's cache.
struct Shared {
    hits: AtomicU64,
    misses: AtomicU64,
    /// Trees stored, summed over the workers; each worker adds and removes its own.
    entries: AtomicUsize,
    /// Bumped by [`clear_all`]; a worker whose cache is older empties it before a lookup.
    generation: AtomicU64,
}

impl Shared {
    const fn new() -> Self {
        Self {
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            entries: AtomicUsize::new(0),
            generation: AtomicU64::new(0),
        }
    }

    fn stats(&self) -> ParseCacheStats {
        ParseCacheStats {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
            entries: self.entries.load(Ordering::Relaxed),
        }
    }

    fn clear(&self) {
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
        self.generation.fetch_add(1, Ordering::SeqCst);
    }
}

static SHARED: Shared = Shared::new();

/// Parse-cache counters over all worker threads, for `/health`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParseCacheStats {
    pub hits: u64,
    pub misses: u64,
    /// Parsed trees stored, summed over the workers.
    pub entries: usize,
}

impl ParseCacheStats {
    pub fn hit_rate(&self) -> f64 {
        match self.hits + self.misses {
            0 => 0.0,
            total => self.hits as f64 / total as f64,
        }
    }
}

/// LRU of parsed trees with a bound on the summed key length.
struct ParseCache {
    map: LruCache<String, Rc<Expr>>,
    text_bytes: usize,
    generation: u64,
}

impl ParseCache {
    fn new() -> Self {
        Self {
            map: LruCache::new(CAPACITY),
            text_bytes: 0,
            generation: 0,
        }
    }

    /// Empty this cache if `shared` was cleared since it was last used.
    fn sync(&mut self, shared: &Shared) {
        let generation = shared.generation.load(Ordering::SeqCst);
        if generation != self.generation {
            shared.entries.fetch_sub(self.map.len(), Ordering::Relaxed);
            self.map.clear();
            self.text_bytes = 0;
            self.generation = generation;
        }
    }

    fn lookup(&mut self, shared: &Shared, expression: &str) -> Result<Rc<Expr>, Error> {
        self.sync(shared);
        if let Some(expr) = self.map.get(expression) {
            shared.hits.fetch_add(1, Ordering::Relaxed);
            return Ok(Rc::clone(expr));
        }
        shared.misses.fetch_add(1, Ordering::Relaxed);
        let expr = Rc::new(skillet::parse(expression)?);
        self.insert(shared, expression, Rc::clone(&expr));
        Ok(expr)
    }

    /// Store `expr` under `expression`, evicting least recently used entries until both bounds
    /// hold. Text longer than [`MAX_ENTRY_BYTES`] is not stored.
    fn insert(&mut self, shared: &Shared, expression: &str, expr: Rc<Expr>) {
        let len = expression.len();
        if len > MAX_ENTRY_BYTES {
            return;
        }
        while self.map.len() >= MAX_ENTRIES || self.text_bytes + len > MAX_TOTAL_BYTES {
            match self.map.pop_lru() {
                Some((key, _)) => {
                    self.text_bytes -= key.len();
                    shared.entries.fetch_sub(1, Ordering::Relaxed);
                }
                None => break,
            }
        }
        self.text_bytes += len;
        self.map.put(expression.to_string(), expr);
        shared.entries.fetch_add(1, Ordering::Relaxed);
    }
}

thread_local! {
    static PARSE_CACHE: RefCell<ParseCache> = RefCell::new(ParseCache::new());
}

/// The parsed tree of `expression`, from this thread's cache or from [`skillet::parse`].
///
/// Same result and same errors as `skillet::parse(expression)`, including the nesting limit.
pub fn parsed(expression: &str) -> Result<Rc<Expr>, Error> {
    PARSE_CACHE.with(|c| c.borrow_mut().lookup(&SHARED, expression))
}

/// Hits, misses and stored trees over all worker threads.
pub fn stats() -> ParseCacheStats {
    SHARED.stats()
}

/// Reset the counters and drop every worker's trees: this thread's now, the others' at their
/// next lookup.
pub fn clear_all() {
    SHARED.clear();
    PARSE_CACHE.with(|c| c.borrow_mut().sync(&SHARED));
}

#[cfg(test)]
mod tests {
    use super::*;

    // `clear_all` bumps the process-wide generation, which empties every thread's cache on its
    // next lookup, so the tests in this module must not run concurrently with each other.
    static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn serial() -> std::sync::MutexGuard<'static, ()> {
        SERIAL.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn cached_len() -> usize {
        PARSE_CACHE.with(|c| c.borrow().map.len())
    }

    // Each #[test] runs on its own thread, so each starts with an empty cache. The tests below
    // that check counters use their own `Shared`, since `SHARED` is used by all of them.

    #[test]
    fn counters_track_hits_misses_and_entries() {
        let _serial = serial();
        let shared = Shared::new();
        let mut cache = ParseCache::new();
        cache.lookup(&shared, "1 + 1").unwrap();
        cache.lookup(&shared, "1 + 1").unwrap();
        cache.lookup(&shared, "2 +").unwrap_err();
        let want = ParseCacheStats {
            hits: 1,
            misses: 2,
            entries: 1,
        };
        assert_eq!(shared.stats(), want);
        assert!((shared.stats().hit_rate() - 1.0 / 3.0).abs() < 1e-9);

        // Evictions take their entries off the count.
        for i in 0..MAX_ENTRIES + 5 {
            cache.lookup(&shared, &format!("{i} * 2")).unwrap();
        }
        assert_eq!(shared.stats().entries, MAX_ENTRIES);
    }

    #[test]
    fn clear_empties_every_workers_cache() {
        let _serial = serial();
        let shared = Shared::new();
        let (mut a, mut b) = (ParseCache::new(), ParseCache::new());
        let old = a.lookup(&shared, ":x + 1").unwrap();
        b.lookup(&shared, ":x + 1").unwrap();
        b.lookup(&shared, ":y + 1").unwrap();
        assert_eq!(shared.stats().entries, 3);

        shared.clear();
        a.sync(&shared); // the clearing worker, at once
        let after = shared.stats();
        assert_eq!((after.hits, after.misses, after.entries), (0, 0, 2));

        // The other worker drops its trees at its next lookup, which is then a miss.
        b.lookup(&shared, ":y + 1").unwrap();
        assert_eq!(shared.stats().entries, 1);
        assert_eq!(shared.stats().misses, 1);
        assert!(!Rc::ptr_eq(&old, &a.lookup(&shared, ":x + 1").unwrap()));
        assert_eq!(shared.stats().entries, 2);
    }

    #[test]
    fn clear_all_empties_this_threads_cache() {
        let _serial = serial();
        parsed("7 + 7").unwrap();
        assert_eq!(cached_len(), 1);
        clear_all();
        assert_eq!(cached_len(), 0);
    }

    #[test]
    fn repeated_expression_reuses_the_tree() {
        let _serial = serial();
        let a = parsed("1 + :x").unwrap();
        let b = parsed("1 + :x").unwrap();
        assert!(Rc::ptr_eq(&a, &b));
        assert_eq!(*a, skillet::parse("1 + :x").unwrap());
        assert_eq!(cached_len(), 1);
    }

    #[test]
    fn parse_errors_are_returned_and_not_cached() {
        let _serial = serial();
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
        let _serial = serial();
        let long = format!("[{}0]", "0,".repeat(MAX_ENTRY_BYTES / 2));
        assert!(long.len() > MAX_ENTRY_BYTES);
        let a = parsed(&long).unwrap();
        let b = parsed(&long).unwrap();
        assert!(!Rc::ptr_eq(&a, &b));
        assert_eq!(cached_len(), 0);
    }

    #[test]
    fn entry_count_is_bounded() {
        let _serial = serial();
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
        let _serial = serial();
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
