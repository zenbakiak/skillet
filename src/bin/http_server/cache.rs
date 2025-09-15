use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use lru::LruCache;
use once_cell::sync::Lazy;
use std::num::NonZeroUsize;

use skillet::{Value, evaluate_with_assignments, evaluate_with_assignments_and_context};

/// Cached expression result with optional variable context
#[derive(Clone, Debug)]
pub struct CachedResult {
    pub result: Result<Value, String>,
    pub variable_context: Option<HashMap<String, Value>>,
    pub execution_time_ms: f64,
    pub cache_hit: bool,
}

/// Expression cache entry
#[derive(Clone, Debug)]
struct CacheEntry {
    result: Value,
    variable_context: Option<HashMap<String, Value>>,
    execution_time_ms: f64,
    hit_count: u64,
    last_accessed: std::time::Instant,
}

/// Expression cache with LRU eviction
pub struct ExpressionCache {
    cache: LruCache<String, CacheEntry>,
    stats: CacheStats,
}

/// Cache statistics
#[derive(Debug, Clone, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub entries: usize,
    pub evictions: u64,
    pub total_saved_time_ms: f64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        if self.hits + self.misses == 0 {
            0.0
        } else {
            self.hits as f64 / (self.hits + self.misses) as f64
        }
    }
}

/// Global expression cache instance
static EXPRESSION_CACHE: Lazy<Arc<Mutex<ExpressionCache>>> = Lazy::new(|| {
    Arc::new(Mutex::new(ExpressionCache::new(1000))) // Cache up to 1000 expressions
});

impl ExpressionCache {
    fn new(capacity: usize) -> Self {
        Self {
            cache: LruCache::new(NonZeroUsize::new(capacity).unwrap()),
            stats: CacheStats::default(),
        }
    }

    fn get(&mut self, key: &str) -> Option<CacheEntry> {
        if let Some(entry) = self.cache.get_mut(key) {
            entry.hit_count += 1;
            entry.last_accessed = std::time::Instant::now();
            self.stats.hits += 1;
            self.stats.total_saved_time_ms += entry.execution_time_ms;
            Some(entry.clone())
        } else {
            self.stats.misses += 1;
            None
        }
    }

    fn put(&mut self, key: String, entry: CacheEntry) {
        if self.cache.put(key, entry).is_some() {
            self.stats.evictions += 1;
        }
        self.stats.entries = self.cache.len();
    }

    fn get_stats(&self) -> CacheStats {
        self.stats.clone()
    }

    fn clear(&mut self) {
        self.cache.clear();
        self.stats = CacheStats::default();
    }
}

/// Generate cache key from expression and variables
fn generate_cache_key(expression: &str, variables: &HashMap<String, Value>) -> String {
    if variables.is_empty() {
        expression.to_string()
    } else {
        // Create deterministic key including sorted variable names and values
        let mut var_parts: Vec<String> = variables
            .iter()
            .map(|(k, v)| format!("{}:{:?}", k, v))
            .collect();
        var_parts.sort();
        format!("{}|{}", expression, var_parts.join(","))
    }
}

/// Evaluate expression with caching support
pub fn evaluate_cached(
    expression: &str, 
    variables: &HashMap<String, Value>,
    include_variables: bool,
) -> CachedResult {
    let cache_key = generate_cache_key(expression, variables);
    
    // Try to get from cache first
    if let Ok(mut cache) = EXPRESSION_CACHE.lock() {
        if let Some(entry) = cache.get(&cache_key) {
            return CachedResult {
                result: Ok(entry.result.clone()),
                variable_context: entry.variable_context.clone(),
                execution_time_ms: entry.execution_time_ms,
                cache_hit: true,
            };
        }
    }

    // Cache miss - evaluate the expression
    let start_time = std::time::Instant::now();
    
    let (result, variable_context) = if expression.contains(";") || expression.contains(":=") {
        if include_variables {
            match evaluate_with_assignments_and_context(expression, variables) {
                Ok((val, ctx)) => (Ok(val), Some(ctx)),
                Err(e) => (Err(e), None),
            }
        } else {
            (evaluate_with_assignments(expression, variables), None)
        }
    } else {
        (skillet::evaluate_with_custom(expression, variables), None)
    };
    
    let execution_time_ms = start_time.elapsed().as_secs_f64() * 1000.0;

    // Store successful results in cache
    if let Ok(ref value) = result {
        let cache_entry = CacheEntry {
            result: value.clone(),
            variable_context: variable_context.clone(),
            execution_time_ms,
            hit_count: 0,
            last_accessed: std::time::Instant::now(),
        };

        if let Ok(mut cache) = EXPRESSION_CACHE.lock() {
            cache.put(cache_key, cache_entry);
        }
    }

    CachedResult {
        result: result.map_err(|e| e.to_string()),
        variable_context,
        execution_time_ms,
        cache_hit: false,
    }
}

/// Get current cache statistics
pub fn get_cache_stats() -> CacheStats {
    EXPRESSION_CACHE
        .lock()
        .map(|cache| cache.get_stats())
        .unwrap_or_default()
}

/// Clear the expression cache
pub fn clear_cache() {
    if let Ok(mut cache) = EXPRESSION_CACHE.lock() {
        cache.clear();
    }
}

/// Buffer pool for HTTP request parsing
pub struct BufferPool {
    buffers: Vec<Vec<u8>>,
    capacity: usize,
    buffer_size: usize,
}

impl BufferPool {
    fn new(capacity: usize, buffer_size: usize) -> Self {
        Self {
            buffers: Vec::with_capacity(capacity),
            capacity,
            buffer_size,
        }
    }

    fn get_buffer(&mut self) -> Vec<u8> {
        self.buffers.pop().unwrap_or_else(|| Vec::with_capacity(self.buffer_size))
    }

    fn return_buffer(&mut self, mut buffer: Vec<u8>) {
        if self.buffers.len() < self.capacity {
            buffer.clear();
            if buffer.capacity() > self.buffer_size * 2 {
                // Shrink oversized buffers
                buffer.shrink_to(self.buffer_size);
            }
            self.buffers.push(buffer);
        }
        // Drop buffer if pool is full
    }
}

// Thread-local buffer pool for HTTP parsing
thread_local! {
    static HTTP_BUFFER_POOL: std::cell::RefCell<BufferPool> = 
        std::cell::RefCell::new(BufferPool::new(4, 65536)); // Increased to 64KB for large requests
}

/// Get a buffer from the thread-local pool
pub fn get_pooled_buffer() -> Vec<u8> {
    HTTP_BUFFER_POOL.with(|pool| pool.borrow_mut().get_buffer())
}

/// Return a buffer to the thread-local pool
pub fn return_pooled_buffer(buffer: Vec<u8>) {
    HTTP_BUFFER_POOL.with(|pool| pool.borrow_mut().return_buffer(buffer));
}

/// Response object pool for JSON serialization
pub struct ResponsePool<T> {
    pool: Vec<T>,
    capacity: usize,
}

impl<T> ResponsePool<T> {
    fn new(capacity: usize) -> Self {
        Self {
            pool: Vec::with_capacity(capacity),
            capacity,
        }
    }

    fn get(&mut self) -> Option<T> {
        self.pool.pop()
    }

    fn return_item(&mut self, item: T) {
        if self.pool.len() < self.capacity {
            self.pool.push(item);
        }
        // Drop item if pool is full
    }
}

// Response pooling for EvalResponse objects
thread_local! {
    static RESPONSE_POOL: std::cell::RefCell<ResponsePool<super::types::EvalResponse>> = 
        std::cell::RefCell::new(ResponsePool::new(8));
}

/// Get a pooled response object (if available)
pub fn get_pooled_response() -> Option<super::types::EvalResponse> {
    RESPONSE_POOL.with(|pool| pool.borrow_mut().get())
}

/// Return a response object to the pool
pub fn return_pooled_response(response: super::types::EvalResponse) {
    RESPONSE_POOL.with(|pool| pool.borrow_mut().return_item(response));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expression_cache() {
        clear_cache();
        
        let vars = HashMap::new();
        
        // First evaluation should be a cache miss
        let result1 = evaluate_cached("2+2", &vars, false);
        assert!(!result1.cache_hit);
        assert!(result1.result.is_ok());
        
        // Second evaluation should be a cache hit
        let result2 = evaluate_cached("2+2", &vars, false);
        assert!(result2.cache_hit);
        assert!(result2.result.is_ok());
        
        let stats = get_cache_stats();
        assert_eq!(stats.hits, 1);
        assert_eq!(stats.misses, 1);
        assert!(stats.hit_rate() > 0.0);
    }

    #[test]
    fn test_cache_key_generation() {
        let mut vars1 = HashMap::new();
        vars1.insert("a".to_string(), Value::Number(1.0));
        vars1.insert("b".to_string(), Value::Number(2.0));
        
        let mut vars2 = HashMap::new();
        vars2.insert("b".to_string(), Value::Number(2.0));
        vars2.insert("a".to_string(), Value::Number(1.0));
        
        let key1 = generate_cache_key("test", &vars1);
        let key2 = generate_cache_key("test", &vars2);
        
        // Should generate same key regardless of variable order
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_buffer_pool() {
        let buffer = get_pooled_buffer();
        assert!(buffer.capacity() > 0);
        
        let original_capacity = buffer.capacity();
        return_pooled_buffer(buffer);
        
        // Get another buffer - might be the same one
        let buffer2 = get_pooled_buffer();
        assert!(buffer2.capacity() >= original_capacity || buffer2.capacity() > 0);
    }
}
