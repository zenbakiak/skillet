use skillet::{register_function, evaluate_with_custom, SqliteQueryFunction, Value};
use std::collections::HashMap;

fn main() {
    // Register the SQLite function
    register_function(Box::new(SqliteQueryFunction)).unwrap();
    
    // Test querying all users
    let vars = HashMap::new();
    let result = evaluate_with_custom("SQLITE_QUERY(\"test.db\", \"SELECT * FROM users\")", &vars).unwrap();
    
    println!("Query result: {:?}", result);
}