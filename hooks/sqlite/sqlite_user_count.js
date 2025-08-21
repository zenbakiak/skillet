// @name: USER_COUNT
// @min_args: 1
// @max_args: 1
// @description: Get user count from SQLite database
// @example: USER_COUNT("database.db") returns number of users

function execute(args) {
    const dbPath = args[0];
    
    try {
        // Query the database for user count
        const result = sqliteQuery(dbPath, "SELECT COUNT(*) as count FROM users");
        
        // Parse the JSON result
        const data = JSON.parse(result);
        
        if (data.length > 0 && data[0].count !== undefined) {
            return data[0].count;
        } else {
            return "No count result found";
        }
        
    } catch (e) {
        return "Error: " + e.message;
    }
}