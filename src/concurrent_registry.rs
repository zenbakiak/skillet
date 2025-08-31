use crate::custom::CustomFunction;
use crate::error::Error;
use crate::types::Value;
use std::sync::Arc;
use dashmap::DashMap;

/// High-performance, lock-free function registry optimized for concurrent access
/// Uses DashMap for better concurrent performance than RwLock<HashMap>
pub struct ConcurrentFunctionRegistry {
    functions: DashMap<String, Arc<dyn CustomFunction>>,
}

impl ConcurrentFunctionRegistry {
    pub fn new() -> Self {
        Self {
            functions: DashMap::new(),
        }
    }
    
    /// Register a function (thread-safe, lock-free)
    pub fn register(&self, function: Box<dyn CustomFunction>) -> Result<(), Error> {
        let name = function.name().to_uppercase();
        
        if name.is_empty() {
            return Err(Error::new("Function name cannot be empty", None));
        }
        
        if function.min_args() > function.max_args().unwrap_or(usize::MAX) {
            return Err(Error::new("min_args cannot be greater than max_args", None));
        }
        
        self.functions.insert(name, Arc::from(function));
        Ok(())
    }
    
    /// Check if function exists (lock-free read)
    pub fn has_function(&self, name: &str) -> bool {
        self.functions.contains_key(&name.to_uppercase())
    }
    
    /// Execute function (lock-free read)
    pub fn execute(&self, name: &str, args: Vec<Value>) -> Result<Value, Error> {
        let upper_name = name.to_uppercase();
        
        match self.functions.get(&upper_name) {
            Some(func_ref) => {
                let function = func_ref.value();
                
                // Validate argument count
                let arg_count = args.len();
                if arg_count < function.min_args() {
                    return Err(Error::new(
                        format!("{} expects at least {} arguments, got {}", 
                            name, function.min_args(), arg_count), 
                        None
                    ));
                }
                
                if let Some(max_args) = function.max_args() {
                    if arg_count > max_args {
                        return Err(Error::new(
                            format!("{} expects at most {} arguments, got {}", 
                                name, max_args, arg_count), 
                            None
                        ));
                    }
                }
                
                function.execute(args)
            }
            None => Err(Error::new(format!("Unknown custom function: {}", name), None)),
        }
    }
    
    /// List all function names
    pub fn list_functions(&self) -> Vec<String> {
        self.functions.iter().map(|entry| entry.key().clone()).collect()
    }
    
    /// Remove a function
    pub fn unregister(&self, name: &str) -> bool {
        self.functions.remove(&name.to_uppercase()).is_some()
    }
    
    /// Get function count
    pub fn len(&self) -> usize {
        self.functions.len()
    }
    
    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.functions.is_empty()
    }
}

impl Default for ConcurrentFunctionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-local function cache for even better performance
/// Reduces concurrent access to global registry for frequently used functions
pub struct ThreadLocalFunctionCache {
    cache: std::cell::RefCell<std::collections::HashMap<String, Arc<dyn CustomFunction>>>,
    registry: Arc<ConcurrentFunctionRegistry>,
    hits: std::cell::Cell<u64>,
    misses: std::cell::Cell<u64>,
}

impl ThreadLocalFunctionCache {
    pub fn new(registry: Arc<ConcurrentFunctionRegistry>) -> Self {
        Self {
            cache: std::cell::RefCell::new(std::collections::HashMap::new()),
            registry,
            hits: std::cell::Cell::new(0),
            misses: std::cell::Cell::new(0),
        }
    }
    
    pub fn get_function(&self, name: &str) -> Option<Arc<dyn CustomFunction>> {
        let upper_name = name.to_uppercase();
        
        // Check cache first
        if let Some(func) = self.cache.borrow().get(&upper_name) {
            self.hits.set(self.hits.get() + 1);
            return Some(Arc::clone(func));
        }
        
        // Cache miss - check global registry
        if let Some(func_ref) = self.registry.functions.get(&upper_name) {
            let func = Arc::clone(func_ref.value());
            self.cache.borrow_mut().insert(upper_name, Arc::clone(&func));
            self.misses.set(self.misses.get() + 1);
            Some(func)
        } else {
            None
        }
    }
    
    pub fn execute(&self, name: &str, args: Vec<Value>) -> Result<Value, Error> {
        match self.get_function(name) {
            Some(function) => {
                // Validate argument count
                let arg_count = args.len();
                if arg_count < function.min_args() {
                    return Err(Error::new(
                        format!("{} expects at least {} arguments, got {}", 
                            name, function.min_args(), arg_count), 
                        None
                    ));
                }
                
                if let Some(max_args) = function.max_args() {
                    if arg_count > max_args {
                        return Err(Error::new(
                            format!("{} expects at most {} arguments, got {}", 
                                name, max_args, arg_count), 
                            None
                        ));
                    }
                }
                
                function.execute(args)
            }
            None => Err(Error::new(format!("Unknown custom function: {}", name), None)),
        }
    }
    
    pub fn cache_stats(&self) -> (u64, u64, f64) {
        let hits = self.hits.get();
        let misses = self.misses.get();
        let hit_rate = if hits + misses > 0 {
            hits as f64 / (hits + misses) as f64 * 100.0
        } else {
            0.0
        };
        (hits, misses, hit_rate)
    }
    
    pub fn clear_cache(&self) {
        self.cache.borrow_mut().clear();
        self.hits.set(0);
        self.misses.set(0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    struct TestFunction {
        name: String,
    }
    
    impl TestFunction {
        fn new(name: &str) -> Self {
            Self { name: name.to_string() }
        }
    }
    
    impl CustomFunction for TestFunction {
        fn name(&self) -> &str { &self.name }
        fn min_args(&self) -> usize { 0 }
        fn max_args(&self) -> Option<usize> { Some(2) }
        
        fn execute(&self, args: Vec<Value>) -> Result<Value, Error> {
            Ok(Value::String(format!("{}({})", self.name, args.len())))
        }
    }
    
    #[test]
    fn test_concurrent_registry() {
        let registry = ConcurrentFunctionRegistry::new();
        
        // Register functions
        registry.register(Box::new(TestFunction::new("FUNC1"))).unwrap();
        registry.register(Box::new(TestFunction::new("FUNC2"))).unwrap();
        
        assert_eq!(registry.len(), 2);
        assert!(registry.has_function("FUNC1"));
        assert!(registry.has_function("func1")); // Case insensitive
        
        // Execute function
        let result = registry.execute("FUNC1", vec![]).unwrap();
        assert_eq!(result, Value::String("FUNC1(0)".to_string()));
        
        // Test concurrent access
        let registry = Arc::new(registry);
        let handles: Vec<_> = (0..10).map(|i| {
            let registry = Arc::clone(&registry);
            std::thread::spawn(move || {
                for _ in 0..100 {
                    let result = registry.execute("FUNC1", vec![]).unwrap();
                    assert_eq!(result, Value::String("FUNC1(0)".to_string()));
                }
                i
            })
        }).collect();
        
        for handle in handles {
            handle.join().unwrap();
        }
    }
    
    #[test]
    fn test_thread_local_cache() {
        let registry = Arc::new(ConcurrentFunctionRegistry::new());
        registry.register(Box::new(TestFunction::new("CACHED_FUNC"))).unwrap();
        
        let cache = ThreadLocalFunctionCache::new(Arc::clone(&registry));
        
        // First call - cache miss
        let result1 = cache.execute("CACHED_FUNC", vec![]).unwrap();
        assert_eq!(result1, Value::String("CACHED_FUNC(0)".to_string()));
        
        // Second call - cache hit
        let result2 = cache.execute("CACHED_FUNC", vec![]).unwrap();
        assert_eq!(result2, Value::String("CACHED_FUNC(0)".to_string()));
        
        let (hits, misses, hit_rate) = cache.cache_stats();
        assert_eq!(hits, 1);
        assert_eq!(misses, 1);
        assert_eq!(hit_rate, 50.0);
    }
}
