use skillet::{register_function, evaluate_with_custom, JavaScriptFunction, Value};
use std::collections::HashMap;
use tempfile::TempDir;
use std::fs;
use std::sync::Mutex;

// Global test mutex to prevent concurrent access to the global function registry
static TEST_MUTEX: Mutex<()> = Mutex::new(());

#[test]
fn test_udi_price_today_caching_behavior() {
    let _lock = TEST_MUTEX.lock().unwrap();
    
    // Create temporary directory for test database
    let temp_dir = TempDir::new().unwrap();
    let temp_db_path = temp_dir.path().join("test_udi.db");
    let db_path_str = temp_db_path.to_string_lossy().to_string();
    
    // Clean up any existing function
    skillet::unregister_function("UDI_PRICE_TODAY");
    
    // Load and modify the UDI_PRICE_TODAY function to use our test database
    let mut udi_function_js = fs::read_to_string("hooks/udi_price_today.js").unwrap();
    
    // Replace the database path for testing
    udi_function_js = udi_function_js.replace("skillet.db", &db_path_str);
    
    // Parse and register the modified function
    let udi_func = JavaScriptFunction::parse_js_function(&udi_function_js).unwrap();
    register_function(Box::new(udi_func)).unwrap();
    
    let vars = HashMap::new();
    
    // Test 1: First call should create table and try API (will fail with dummy token)
    let first_result = evaluate_with_custom("UDI_PRICE_TODAY(\"dummy_token\")", &vars).unwrap();
    
    match first_result {
        Value::String(s) => {
            assert!(s.contains("Unable to get UDI price from API"));
        },
        _ => panic!("Expected error message for first call with dummy token"),
    }
    
    // Test 2: Manually insert a recent record (within 2 days)
    let yesterday = chrono::Utc::now().date_naive() - chrono::Duration::days(1);
    let yesterday_str = yesterday.format("%Y-%m-%d").to_string();
    
    // Use SQLite functions to insert test data
    skillet::unregister_function("SQLITE_QUERY");
    register_function(Box::new(skillet::SqliteQueryFunction)).unwrap();
    
    let insert_cmd = format!("SQLITE_QUERY(\"{}\", \"INSERT INTO udi_prices (fecha, dato) VALUES ('{}', 7.654321)\")", db_path_str, yesterday_str);
    let _insert_result = evaluate_with_custom(&insert_cmd, &vars).unwrap();
    
    // Test 3: Call UDI_PRICE_TODAY again, should return the cached value
    let cached_result = evaluate_with_custom("UDI_PRICE_TODAY(\"dummy_token\")", &vars).unwrap();
    
    match cached_result {
        Value::Number(n) => {
            assert!((n - 7.654321).abs() < 0.000001);
        },
        _ => panic!("Expected cached number result, got: {:?}", cached_result),
    }
    
    // Test 4: Insert today's record and test immediate cache hit
    let today = chrono::Utc::now().date_naive();
    let today_str = today.format("%Y-%m-%d").to_string();
    
    let insert_today_cmd = format!("SQLITE_QUERY(\"{}\", \"INSERT INTO udi_prices (fecha, dato) VALUES ('{}', 8.987654)\")", db_path_str, today_str);
    let _insert_today_result = evaluate_with_custom(&insert_today_cmd, &vars).unwrap();
    
    // This should return today's value immediately (fastest path)
    let today_result = evaluate_with_custom("UDI_PRICE_TODAY(\"dummy_token\")", &vars).unwrap();
    
    match today_result {
        Value::Number(n) => {
            assert!((n - 8.987654).abs() < 0.000001);
        },
        _ => panic!("Expected today's cached number result, got: {:?}", today_result),
    }
    
    // Test 5: Verify table structure
    let table_info_cmd = format!("SQLITE_QUERY(\"{}\", \"PRAGMA table_info(udi_prices)\")", db_path_str);
    let table_info_result = evaluate_with_custom(&table_info_cmd, &vars).unwrap();
    
    match table_info_result {
        Value::Array(rows) => {
            assert!(rows.len() >= 4); // Should have at least 4 columns
        },
        _ => panic!("Expected array result for table info"),
    }
    
    // Clean up
    skillet::unregister_function("UDI_PRICE_TODAY");
    skillet::unregister_function("SQLITE_QUERY");
}

#[test]
fn test_udi_price_date_fallback_logic() {
    let _lock = TEST_MUTEX.lock().unwrap();
    
    // Create temporary directory for test database
    let temp_dir = TempDir::new().unwrap();
    let temp_db_path = temp_dir.path().join("test_fallback.db");
    let db_path_str = temp_db_path.to_string_lossy().to_string();
    
    // Clean up any existing functions
    skillet::unregister_function("UDI_PRICE_TODAY");
    skillet::unregister_function("SQLITE_QUERY");
    
    // Register required functions
    let mut udi_function_js = fs::read_to_string("hooks/udi_price_today.js").unwrap();
    udi_function_js = udi_function_js.replace("skillet.db", &db_path_str);
    let udi_func = JavaScriptFunction::parse_js_function(&udi_function_js).unwrap();
    register_function(Box::new(udi_func)).unwrap();
    register_function(Box::new(skillet::SqliteQueryFunction)).unwrap();
    
    let vars = HashMap::new();
    
    // Create the table first
    let _first_call = evaluate_with_custom("UDI_PRICE_TODAY(\"dummy_token\")", &vars);
    
    // Test edge case: Insert a record that's exactly 2 days old (should be accepted)
    let two_days_ago = chrono::Utc::now().date_naive() - chrono::Duration::days(2);
    let two_days_ago_str = two_days_ago.format("%Y-%m-%d").to_string();
    
    let insert_cmd = format!("SQLITE_QUERY(\"{}\", \"INSERT INTO udi_prices (fecha, dato) VALUES ('{}', 9.111111)\")", db_path_str, two_days_ago_str);
    let _insert_result = evaluate_with_custom(&insert_cmd, &vars).unwrap();
    
    let fallback_result = evaluate_with_custom("UDI_PRICE_TODAY(\"dummy_token\")", &vars).unwrap();
    
    match fallback_result {
        Value::Number(n) => {
            assert!((n - 9.111111).abs() < 0.000001);
        },
        _ => panic!("Expected fallback number result for 2-day old record"),
    }
    
    // Test edge case: Insert a record that's 3 days old (should NOT be accepted)
    let three_days_ago = chrono::Utc::now().date_naive() - chrono::Duration::days(3);
    let three_days_ago_str = three_days_ago.format("%Y-%m-%d").to_string();
    
    // Clear the current database and create a new one with only the 3-day old record
    let clear_cmd = format!("SQLITE_QUERY(\"{}\", \"DELETE FROM udi_prices\")", db_path_str);
    let _clear_result = evaluate_with_custom(&clear_cmd, &vars).unwrap();
    
    let insert_old_cmd = format!("SQLITE_QUERY(\"{}\", \"INSERT INTO udi_prices (fecha, dato) VALUES ('{}', 6.666666)\")", db_path_str, three_days_ago_str);
    let _insert_old_result = evaluate_with_custom(&insert_old_cmd, &vars).unwrap();
    
    let no_fallback_result = evaluate_with_custom("UDI_PRICE_TODAY(\"dummy_token\")", &vars).unwrap();
    
    match no_fallback_result {
        Value::String(s) => {
            assert!(s.contains("Unable to get UDI price from API"));
        },
        _ => panic!("Expected error message for too old cached data"),
    }
    
    // Clean up
    skillet::unregister_function("UDI_PRICE_TODAY");
    skillet::unregister_function("SQLITE_QUERY");
}