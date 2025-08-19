// @name: OBJECT_KEYS
// @min_args: 1
// @max_args: 1
// @description: Get all keys from an object as an array
// @example: OBJECT_KEYS({"name": "John", "age": 30}) returns ["name", "age"]

function execute(args) {
    const obj = args[0];
    
    // Handle different input types
    if (obj === null || obj === undefined) {
        return [];
    }
    
    // If it's a string that looks like JSON, try to parse it first
    if (typeof obj === 'string') {
        try {
            const parsed = JSON.parse(obj);
            if (typeof parsed === 'object' && !Array.isArray(parsed) && parsed !== null) {
                return Object.keys(parsed);
            }
        } catch (e) {
            // If parsing fails, treat as regular string - return empty array
            return [];
        }
    }
    
    // If it's already an object, get its keys
    if (typeof obj === 'object' && !Array.isArray(obj)) {
        return Object.keys(obj);
    }
    
    // For other types, return empty array
    return [];
}