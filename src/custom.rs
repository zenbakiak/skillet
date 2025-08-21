use crate::error::Error;
use crate::types::Value;
use std::collections::HashMap;

/// Trait for implementing custom functions in skillet
/// 
/// # Example
/// ```rust
/// use skillet::custom::CustomFunction;
/// use skillet::{Value, Error};
/// 
/// struct DoubleFunction;
/// 
/// impl CustomFunction for DoubleFunction {
///     fn name(&self) -> &str { "DOUBLE" }
///     fn min_args(&self) -> usize { 1 }
///     fn max_args(&self) -> Option<usize> { Some(1) }
///     
///     fn execute(&self, args: Vec<Value>) -> Result<Value, Error> {
///         let num = args[0].as_number()
///             .ok_or_else(|| Error::new("DOUBLE expects a number", None))?;
///         Ok(Value::Number(num * 2.0))
///     }
/// }
/// ```
pub trait CustomFunction: Send + Sync {
    /// The name of the function (case-insensitive)
    fn name(&self) -> &str;
    
    /// Minimum number of arguments required
    fn min_args(&self) -> usize;
    
    /// Maximum number of arguments allowed (None = unlimited)
    fn max_args(&self) -> Option<usize>;
    
    /// Execute the function with the given arguments
    fn execute(&self, args: Vec<Value>) -> Result<Value, Error>;
    
    /// Optional: Description of the function for documentation
    fn description(&self) -> Option<&str> { None }
    
    /// Optional: Example usage for documentation
    fn example(&self) -> Option<&str> { None }
}

/// Registry for custom functions
#[derive(Default)]
pub struct FunctionRegistry {
    functions: HashMap<String, Box<dyn CustomFunction>>,
}

impl FunctionRegistry {
    /// Create a new empty function registry
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
        }
    }
    
    /// Register a custom function
    pub fn register(&mut self, function: Box<dyn CustomFunction>) -> Result<(), Error> {
        let name = function.name().to_uppercase();
        
        // Validate function definition
        if name.is_empty() {
            return Err(Error::new("Function name cannot be empty", None));
        }
        
        if function.min_args() > function.max_args().unwrap_or(usize::MAX) {
            return Err(Error::new("min_args cannot be greater than max_args", None));
        }
        
        self.functions.insert(name, function);
        Ok(())
    }
    
    /// Get a function by name (case-insensitive)
    pub fn get(&self, name: &str) -> Option<&dyn CustomFunction> {
        self.functions.get(&name.to_uppercase()).map(|f| f.as_ref())
    }
    
    /// List all registered function names
    pub fn list_functions(&self) -> Vec<&str> {
        self.functions.keys().map(|s| s.as_str()).collect()
    }
    
    /// Remove a function by name
    pub fn unregister(&mut self, name: &str) -> bool {
        self.functions.remove(&name.to_uppercase()).is_some()
    }
    
    /// Check if a function is registered
    pub fn has_function(&self, name: &str) -> bool {
        self.functions.contains_key(&name.to_uppercase())
    }
    
    /// Validate and execute a function
    pub fn execute(&self, name: &str, args: Vec<Value>) -> Result<Value, Error> {
        let function = self.get(name)
            .ok_or_else(|| Error::new(format!("Unknown custom function: {}", name), None))?;
        
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
        
        // Execute the function
        function.execute(args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    struct TestFunction;
    
    impl CustomFunction for TestFunction {
        fn name(&self) -> &str { "TEST" }
        fn min_args(&self) -> usize { 1 }
        fn max_args(&self) -> Option<usize> { Some(2) }
        
        fn execute(&self, args: Vec<Value>) -> Result<Value, Error> {
            Ok(Value::String(format!("Called with {} args", args.len())))
        }
        
        fn description(&self) -> Option<&str> { Some("A test function") }
        fn example(&self) -> Option<&str> { Some("TEST(1, 2)") }
    }
    
    #[test]
    fn test_function_registry() {
        let mut registry = FunctionRegistry::new();
        
        // Register function
        assert!(registry.register(Box::new(TestFunction)).is_ok());
        
        // Check if registered
        assert!(registry.has_function("TEST"));
        assert!(registry.has_function("test")); // Case insensitive
        
        // Execute function
        let result = registry.execute("TEST", vec![Value::Number(1.0)]).unwrap();
        assert!(matches!(result, Value::String(_)));
        
        // Test argument validation
        assert!(registry.execute("TEST", vec![]).is_err()); // Too few args
        assert!(registry.execute("TEST", vec![Value::Number(1.0), Value::Number(2.0), Value::Number(3.0)]).is_err()); // Too many args
        
        // Unregister
        assert!(registry.unregister("TEST"));
        assert!(!registry.has_function("TEST"));
    }
    
    #[test]
    fn test_sqlite_query_function() {
        use tempfile::TempDir;
        
        // Create a temporary directory for test database
        let temp_dir = TempDir::new().unwrap();
        let db_path = temp_dir.path().join("test.db");
        
        // Create test database and table
        {
            use rusqlite::Connection;
            let conn = Connection::open(&db_path).unwrap();
            conn.execute(
                "CREATE TABLE test_users (id INTEGER PRIMARY KEY, name TEXT, email TEXT)",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO test_users (name, email) VALUES ('Alice', 'alice@example.com')",
                [],
            ).unwrap();
            conn.execute(
                "INSERT INTO test_users (name, email) VALUES ('Bob', 'bob@example.com')",
                [],
            ).unwrap();
        }
        
        let sqlite_func = SqliteQueryFunction;
        
        // Test successful query
        let result = sqlite_func.execute(vec![
            Value::String(db_path.to_string_lossy().to_string()),
            Value::String("SELECT name, email FROM test_users ORDER BY id".to_string())
        ]).unwrap();
        
        match result {
            Value::Array(rows) => {
                assert_eq!(rows.len(), 2);
                // Check first row
                if let Value::Array(first_row) = &rows[0] {
                    assert_eq!(first_row.len(), 2);
                    assert!(matches!(first_row[0], Value::String(ref s) if s == "Alice"));
                    assert!(matches!(first_row[1], Value::String(ref s) if s == "alice@example.com"));
                }
            }
            _ => panic!("Expected array result"),
        }
        
        // Test error handling - invalid SQL
        let error_result = sqlite_func.execute(vec![
            Value::String(db_path.to_string_lossy().to_string()),
            Value::String("INVALID SQL QUERY".to_string())
        ]);
        assert!(error_result.is_err());
        
        // Test error handling - non-existent database
        let error_result2 = sqlite_func.execute(vec![
            Value::String("/nonexistent/path/db.sqlite".to_string()),
            Value::String("SELECT 1".to_string())
        ]);
        assert!(error_result2.is_err());
        
        // Test wrong argument types
        let error_result3 = sqlite_func.execute(vec![
            Value::Number(123.0),
            Value::String("SELECT 1".to_string())
        ]);
        assert!(error_result3.is_err());
    }
}

/// Example SQLite function for Rust custom functions
pub struct SqliteQueryFunction;

impl CustomFunction for SqliteQueryFunction {
    fn name(&self) -> &str { "SQLITE_QUERY" }
    fn min_args(&self) -> usize { 2 }
    fn max_args(&self) -> Option<usize> { Some(2) }
    
    fn execute(&self, args: Vec<Value>) -> Result<Value, Error> {
        use rusqlite::Connection;
        
        let db_path = match &args[0] {
            Value::String(s) => s,
            _ => return Err(Error::new("SQLITE_QUERY expects database path as first argument", None)),
        };
        
        let query = match &args[1] {
            Value::String(s) => s,
            _ => return Err(Error::new("SQLITE_QUERY expects SQL query as second argument", None)),
        };
        
        // Open database connection
        let conn = Connection::open(db_path)
            .map_err(|e| Error::new(format!("Failed to open database: {}", e), None))?;
        
        // Execute query and collect results
        let mut stmt = conn.prepare(query)
            .map_err(|e| Error::new(format!("Failed to prepare statement: {}", e), None))?;
            
        let column_count = stmt.column_count();
        let _column_names: Vec<String> = (0..column_count)
            .map(|i| stmt.column_name(i).unwrap_or("").to_string())
            .collect();
        
        let rows = stmt.query_map([], |row| {
            let mut result = Vec::new();
            for i in 0..column_count {
                let value: rusqlite::types::Value = row.get(i)?;
                let skillet_value = match value {
                    rusqlite::types::Value::Null => Value::Null,
                    rusqlite::types::Value::Integer(i) => Value::Number(i as f64),
                    rusqlite::types::Value::Real(f) => Value::Number(f),
                    rusqlite::types::Value::Text(s) => Value::String(s),
                    rusqlite::types::Value::Blob(_) => Value::String("[BLOB]".to_string()),
                };
                result.push(skillet_value);
            }
            Ok(result)
        }).map_err(|e| Error::new(format!("Query execution failed: {}", e), None))?;
        
        let mut all_rows = Vec::new();
        for row_result in rows {
            let row = row_result.map_err(|e| Error::new(format!("Row processing failed: {}", e), None))?;
            all_rows.push(Value::Array(row));
        }
        
        Ok(Value::Array(all_rows))
    }
    
    fn description(&self) -> Option<&str> {
        Some("Execute SQLite query and return results as array of arrays")
    }
    
    fn example(&self) -> Option<&str> {
        Some("SQLITE_QUERY(\"database.db\", \"SELECT * FROM users LIMIT 10\")")
    }
}