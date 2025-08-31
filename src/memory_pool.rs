use crate::types::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Memory pool for reusing HashMap allocations to reduce GC pressure
/// Particularly useful for variable contexts in higher-order functions
pub struct VariableContextPool {
    pool: Mutex<Vec<HashMap<String, Value>>>,
    max_size: usize,
    created_count: std::sync::atomic::AtomicUsize,
    reused_count: std::sync::atomic::AtomicUsize,
}

impl VariableContextPool {
    pub fn new(max_size: usize) -> Self {
        Self {
            pool: Mutex::new(Vec::with_capacity(max_size)),
            max_size,
            created_count: std::sync::atomic::AtomicUsize::new(0),
            reused_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }
    
    /// Get a HashMap from the pool, or create a new one
    pub fn acquire(self: &Arc<Self>) -> PooledContext {
        let context = if let Ok(mut pool) = self.pool.lock() {
            if let Some(mut ctx) = pool.pop() {
                ctx.clear(); // Clear previous contents but keep capacity
                self.reused_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                ctx
            } else {
                self.created_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                HashMap::new()
            }
        } else {
            self.created_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            HashMap::new()
        };
        
        PooledContext::new(context, Arc::clone(self))
    }
    
    /// Return a HashMap to the pool for reuse
    fn release(&self, mut context: HashMap<String, Value>) {
        if let Ok(mut pool) = self.pool.lock() {
            if pool.len() < self.max_size {
                context.clear(); // Clear contents but preserve capacity
                pool.push(context);
            }
            // If pool is full, just drop the context
        }
        // If lock fails, just drop the context
    }
    
    pub fn stats(&self) -> PoolStats {
        let pool_size = self.pool.lock().map(|p| p.len()).unwrap_or(0);
        let created = self.created_count.load(std::sync::atomic::Ordering::Relaxed);
        let reused = self.reused_count.load(std::sync::atomic::Ordering::Relaxed);
        
        PoolStats {
            pool_size,
            total_created: created,
            total_reused: reused,
            reuse_rate: if created + reused > 0 {
                reused as f64 / (created + reused) as f64 * 100.0
            } else {
                0.0
            },
        }
    }
}

impl Default for VariableContextPool {
    fn default() -> Self {
        Self::new(100) // Default pool size
    }
}

pub struct PoolStats {
    pub pool_size: usize,
    pub total_created: usize,
    pub total_reused: usize,
    pub reuse_rate: f64,
}

/// RAII wrapper for pooled HashMap that automatically returns to pool on drop
pub struct PooledContext {
    context: Option<HashMap<String, Value>>,
    pool: Arc<VariableContextPool>,
}

impl PooledContext {
    fn new(context: HashMap<String, Value>, pool: Arc<VariableContextPool>) -> Self {
        Self {
            context: Some(context),
            pool,
        }
    }
    
    /// Clone the base variables into this context
    pub fn with_base(&mut self, base: &HashMap<String, Value>) {
        if let Some(ref mut ctx) = self.context {
            ctx.extend(base.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
    }
    
    /// Insert a variable
    pub fn insert(&mut self, key: String, value: Value) -> Option<Value> {
        self.context.as_mut()?.insert(key, value)
    }
    
    /// Get a variable
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.context.as_ref()?.get(key)
    }
    
    /// Get the underlying HashMap reference
    pub fn as_ref(&self) -> Option<&HashMap<String, Value>> {
        self.context.as_ref()
    }
    
    /// Get mutable reference to underlying HashMap
    pub fn as_mut(&mut self) -> Option<&mut HashMap<String, Value>> {
        self.context.as_mut()
    }
}

impl Drop for PooledContext {
    fn drop(&mut self) {
        if let Some(context) = self.context.take() {
            self.pool.release(context);
        }
    }
}

/// Copy-on-Write variable context for efficient sharing
/// Reduces memory allocations when contexts don't need modification
pub struct CowVariableContext {
    base: Arc<HashMap<String, Value>>,
    overlay: Option<HashMap<String, Value>>,
}

impl CowVariableContext {
    pub fn new(base: HashMap<String, Value>) -> Self {
        Self {
            base: Arc::new(base),
            overlay: None,
        }
    }
    
    pub fn from_shared(base: Arc<HashMap<String, Value>>) -> Self {
        Self {
            base,
            overlay: None,
        }
    }
    
    pub fn get(&self, key: &str) -> Option<&Value> {
        // Check overlay first, then base
        if let Some(ref overlay) = self.overlay {
            if let Some(val) = overlay.get(key) {
                return Some(val);
            }
        }
        self.base.get(key)
    }
    
    pub fn insert(&mut self, key: String, value: Value) {
        if self.overlay.is_none() {
            self.overlay = Some(HashMap::new());
        }
        self.overlay.as_mut().unwrap().insert(key, value);
    }
    
    /// Get all variables as a combined view (expensive - only use when needed)
    pub fn to_combined(&self) -> HashMap<String, Value> {
        let mut result = (*self.base).clone();
        if let Some(ref overlay) = self.overlay {
            result.extend(overlay.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
        result
    }
    
    /// Check if we have any modifications (overlay exists)
    pub fn is_modified(&self) -> bool {
        self.overlay.is_some()
    }
}

// Thread-local variable context pools for zero contention
thread_local! {
    static CONTEXT_POOL: Arc<VariableContextPool> = Arc::new(VariableContextPool::new(50));
}

/// Convenience function to get a pooled context from thread-local pool
pub fn get_pooled_context() -> PooledContext {
    CONTEXT_POOL.with(|pool| pool.acquire())
}

/// Get thread-local pool statistics
pub fn get_thread_pool_stats() -> PoolStats {
    CONTEXT_POOL.with(|pool| pool.stats())
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_variable_context_pool() {
        let pool = Arc::new(VariableContextPool::new(2));
        
        // Acquire contexts
        let mut ctx1 = pool.acquire();
        let mut ctx2 = pool.acquire();
        
        ctx1.insert("x".to_string(), Value::Number(1.0));
        ctx2.insert("y".to_string(), Value::Number(2.0));
        
        assert_eq!(ctx1.get("x"), Some(&Value::Number(1.0)));
        assert_eq!(ctx2.get("y"), Some(&Value::Number(2.0)));
        
        // Drop contexts (should return to pool)
        drop(ctx1);
        drop(ctx2);
        
        // Acquire again (should reuse from pool)
        let mut ctx3 = pool.acquire();
        assert!(ctx3.get("x").is_none()); // Should be cleared
        
        let stats = pool.stats();
        assert!(stats.total_reused > 0);
    }
    
    #[test]
    fn test_cow_variable_context() {
        let base = {
            let mut map = HashMap::new();
            map.insert("x".to_string(), Value::Number(1.0));
            map.insert("y".to_string(), Value::Number(2.0));
            map
        };
        
        let mut ctx = CowVariableContext::new(base);
        
        // Reading from base
        assert_eq!(ctx.get("x"), Some(&Value::Number(1.0)));
        assert!(!ctx.is_modified());
        
        // Writing creates overlay
        ctx.insert("z".to_string(), Value::Number(3.0));
        assert!(ctx.is_modified());
        assert_eq!(ctx.get("z"), Some(&Value::Number(3.0)));
        assert_eq!(ctx.get("x"), Some(&Value::Number(1.0))); // Still accessible
        
        // Overriding base value
        ctx.insert("x".to_string(), Value::Number(10.0));
        assert_eq!(ctx.get("x"), Some(&Value::Number(10.0))); // Overlay takes precedence
    }
    
    #[test]
    fn test_thread_local_pool() {
        let ctx1 = get_pooled_context();
        let ctx2 = get_pooled_context();
        
        drop(ctx1);
        drop(ctx2);
        
        let stats = get_thread_pool_stats();
        assert!(stats.total_created >= 2);
    }
}
