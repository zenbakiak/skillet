// @name: INSERT_USER
// @min_args: 3
// @max_args: 3
// @description: Insert user into SQLite database
// @example: INSERT_USER("database.db", "John Doe", "john@example.com") inserts user

function execute(args) {
    const dbPath = args[0];
    const name = args[1];
    const email = args[2];
    
    try {
        // Insert user into database
        const result = sqliteExec(dbPath, `
            INSERT INTO users (name, email) VALUES ('${name.replace(/'/g, "''")}', '${email.replace(/'/g, "''")}')
        `);
        
        return result;
        
    } catch (e) {
        return "Error inserting user: " + e.message;
    }
}