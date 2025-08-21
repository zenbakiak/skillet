use skillet::{register_function, evaluate_with_custom, JavaScriptFunction, SqliteQueryFunction, Value};
use std::collections::HashMap;
use tempfile::TempDir;
use std::fs;
use std::sync::Mutex;

// Global test mutex to prevent concurrent access to the global function registry
static TEST_MUTEX: Mutex<()> = Mutex::new(());

#[test]
fn test_javascript_sqlite_functions() {
    let _lock = TEST_MUTEX.lock().unwrap();
    
    // Create temporary directory for test database
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test.db");
    let db_path_str = db_path.to_string_lossy().to_string();
    
    // Clean up any existing functions
    skillet::unregister_function("CREATE_USERS_TABLE");
    skillet::unregister_function("INSERT_USER");
    skillet::unregister_function("USER_COUNT");
    
    // Load JavaScript functions from hooks/sqlite directory
    let create_table_js = fs::read_to_string("hooks/sqlite/sqlite_create_table.js").unwrap();
    let insert_user_js = fs::read_to_string("hooks/sqlite/sqlite_insert_user.js").unwrap();
    let user_count_js = fs::read_to_string("hooks/sqlite/sqlite_user_count.js").unwrap();
    
    // Parse and register JavaScript functions
    let create_table_func = JavaScriptFunction::parse_js_function(&create_table_js).unwrap();
    let insert_user_func = JavaScriptFunction::parse_js_function(&insert_user_js).unwrap();
    let user_count_func = JavaScriptFunction::parse_js_function(&user_count_js).unwrap();
    
    register_function(Box::new(create_table_func)).unwrap();
    register_function(Box::new(insert_user_func)).unwrap();
    register_function(Box::new(user_count_func)).unwrap();
    
    let vars = HashMap::new();
    
    // Test 1: Create table
    let create_result = evaluate_with_custom(&format!("CREATE_USERS_TABLE(\"{}\")", db_path_str), &vars).unwrap();
    match create_result {
        Value::String(s) => assert!(s.contains("OK")),
        _ => panic!("Expected string result for table creation"),
    }
    
    // Test 2: Insert users
    let insert_result1 = evaluate_with_custom(&format!("INSERT_USER(\"{}\", \"Alice Smith\", \"alice@test.com\")", db_path_str), &vars).unwrap();
    match insert_result1 {
        Value::String(s) => assert!(s.contains("1 rows affected")),
        _ => panic!("Expected string result for user insertion"),
    }
    
    let insert_result2 = evaluate_with_custom(&format!("INSERT_USER(\"{}\", \"Bob Johnson\", \"bob@test.com\")", db_path_str), &vars).unwrap();
    match insert_result2 {
        Value::String(s) => assert!(s.contains("1 rows affected")),
        _ => panic!("Expected string result for user insertion"),
    }
    
    // Test 3: Count users
    let count_result = evaluate_with_custom(&format!("USER_COUNT(\"{}\")", db_path_str), &vars).unwrap();
    match count_result {
        Value::Number(n) => assert_eq!(n, 2.0),
        _ => panic!("Expected number result for user count: {:?}", count_result),
    }
    
    // Clean up
    skillet::unregister_function("CREATE_USERS_TABLE");
    skillet::unregister_function("INSERT_USER");
    skillet::unregister_function("USER_COUNT");
}

#[test]
fn test_rust_sqlite_query_function() {
    let _lock = TEST_MUTEX.lock().unwrap();
    
    // Clean up any existing function
    skillet::unregister_function("SQLITE_QUERY");
    
    // Register the Rust SQLite function
    register_function(Box::new(SqliteQueryFunction)).unwrap();
    
    // Create temporary database
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("rust_test.db");
    let db_path_str = db_path.to_string_lossy().to_string();
    
    // Setup test database
    {
        use rusqlite::Connection;
        let conn = Connection::open(&db_path).unwrap();
        conn.execute(
            "CREATE TABLE products (id INTEGER PRIMARY KEY, name TEXT, price REAL)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO products (name, price) VALUES ('Widget A', 19.99)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO products (name, price) VALUES ('Widget B', 29.99)",
            [],
        ).unwrap();
    }
    
    let vars = HashMap::new();
    
    // Test successful query
    let query_result = evaluate_with_custom(&format!("SQLITE_QUERY(\"{}\", \"SELECT name, price FROM products ORDER BY id\")", db_path_str), &vars).unwrap();
    
    match query_result {
        Value::Array(rows) => {
            assert_eq!(rows.len(), 2);
            
            // Check first row
            if let Value::Array(first_row) = &rows[0] {
                assert_eq!(first_row.len(), 2);
                assert!(matches!(first_row[0], Value::String(ref s) if s == "Widget A"));
                assert!(matches!(first_row[1], Value::Number(n) if (n - 19.99).abs() < 0.01));
            }
            
            // Check second row
            if let Value::Array(second_row) = &rows[1] {
                assert_eq!(second_row.len(), 2);
                assert!(matches!(second_row[0], Value::String(ref s) if s == "Widget B"));
                assert!(matches!(second_row[1], Value::Number(n) if (n - 29.99).abs() < 0.01));
            }
        }
        _ => panic!("Expected array result from SQLITE_QUERY"),
    }
    
    // Test error handling - invalid SQL
    let error_result = evaluate_with_custom(&format!("SQLITE_QUERY(\"{}\", \"INVALID SQL\")", db_path_str), &vars);
    assert!(error_result.is_err());
    
    // Clean up
    skillet::unregister_function("SQLITE_QUERY");
}

#[test]
fn test_sqlite_error_handling() {
    let _lock = TEST_MUTEX.lock().unwrap();
    
    // Clean up any existing functions
    skillet::unregister_function("CREATE_USERS_TABLE");
    skillet::unregister_function("SQLITE_QUERY");
    
    // Load and register functions
    let create_table_js = fs::read_to_string("hooks/sqlite/sqlite_create_table.js").unwrap();
    let create_table_func = JavaScriptFunction::parse_js_function(&create_table_js).unwrap();
    register_function(Box::new(create_table_func)).unwrap();
    register_function(Box::new(SqliteQueryFunction)).unwrap();
    
    let vars = HashMap::new();
    
    // Test 1: Invalid database path
    let invalid_path_result = evaluate_with_custom("CREATE_USERS_TABLE(\"/invalid/path/db.sqlite\")", &vars).unwrap();
    match invalid_path_result {
        Value::String(s) => assert!(s.to_lowercase().contains("error")),
        _ => panic!("Expected error message for invalid path"),
    }
    
    // Test 2: Invalid SQL in Rust function
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("error_test.db");
    let db_path_str = db_path.to_string_lossy().to_string();
    
    // Create empty database
    {
        use rusqlite::Connection;
        let _conn = Connection::open(&db_path).unwrap();
    }
    
    let invalid_sql_result = evaluate_with_custom(&format!("SQLITE_QUERY(\"{}\", \"SELECT FROM WHERE\")", db_path_str), &vars);
    assert!(invalid_sql_result.is_err());
    
    // Clean up
    skillet::unregister_function("CREATE_USERS_TABLE");
    skillet::unregister_function("SQLITE_QUERY");
}

#[test]
fn test_sqlite_data_types() {
    let _lock = TEST_MUTEX.lock().unwrap();
    
    // Test different SQLite data types
    skillet::unregister_function("SQLITE_QUERY");
    register_function(Box::new(SqliteQueryFunction)).unwrap();
    
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("types_test.db");
    let db_path_str = db_path.to_string_lossy().to_string();
    
    // Setup database with different data types
    {
        use rusqlite::Connection;
        let conn = Connection::open(&db_path).unwrap();
        conn.execute(
            "CREATE TABLE test_types (
                id INTEGER,
                name TEXT,
                price REAL,
                is_active BOOLEAN,
                data BLOB,
                nullable_field TEXT
            )",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO test_types VALUES (1, 'Test Item', 99.99, 1, X'DEADBEEF', NULL)",
            [],
        ).unwrap();
    }
    
    let vars = HashMap::new();
    let result = evaluate_with_custom(&format!("SQLITE_QUERY(\"{}\", \"SELECT * FROM test_types\")", db_path_str), &vars).unwrap();
    
    match result {
        Value::Array(rows) => {
            assert_eq!(rows.len(), 1);
            if let Value::Array(row) = &rows[0] {
                assert_eq!(row.len(), 6);
                assert!(matches!(row[0], Value::Number(n) if n == 1.0)); // INTEGER
                assert!(matches!(row[1], Value::String(ref s) if s == "Test Item")); // TEXT
                assert!(matches!(row[2], Value::Number(n) if (n - 99.99).abs() < 0.01)); // REAL
                assert!(matches!(row[3], Value::Number(n) if n == 1.0)); // BOOLEAN (stored as INTEGER)
                assert!(matches!(row[4], Value::String(ref s) if s == "[BLOB]")); // BLOB
                assert!(matches!(row[5], Value::Null)); // NULL
            }
        }
        _ => panic!("Expected array result"),
    }
    
    skillet::unregister_function("SQLITE_QUERY");
}