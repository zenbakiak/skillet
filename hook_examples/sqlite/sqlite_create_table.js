// @name: CREATE_USERS_TABLE
// @min_args: 1
// @max_args: 1
// @description: Create users table in SQLite database
// @example: CREATE_USERS_TABLE("database.db") creates users table

function execute(args) {
    const dbPath = args[0];
    
    try {
        // Create the users table
        const result = sqliteExec(dbPath, `
            CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL,
                email TEXT UNIQUE NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )
        `);
        
        return result;
        
    } catch (e) {
        return "Error creating table: " + e.message;
    }
}