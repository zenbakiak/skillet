// @name: ARRAY_SORT
// @min_args: 1
// @max_args: 2
// @description: Sort an array. Optional second argument: "asc" (default), "desc", or "numeric"
// @example: ARRAY_SORT([3, 1, 4, 1, 5]) returns [1, 1, 3, 4, 5]

function execute(args) {
    const array = args[0];
    const sortMode = args.length > 1 ? args[1] : "asc";
    
    if (!Array.isArray(array)) {
        throw new Error("ARRAY_SORT expects an array as first argument");
    }
    
    // Create a copy to avoid modifying the original
    const sortedArray = [...array];
    
    switch (sortMode.toLowerCase()) {
        case "desc":
            return sortedArray.sort().reverse();
            
        case "numeric":
            return sortedArray.sort((a, b) => {
                const numA = Number(a);
                const numB = Number(b);
                if (isNaN(numA) || isNaN(numB)) {
                    // Fall back to string comparison if not numbers
                    return String(a).localeCompare(String(b));
                }
                return numA - numB;
            });
            
        case "numeric_desc":
            return sortedArray.sort((a, b) => {
                const numA = Number(a);
                const numB = Number(b);
                if (isNaN(numA) || isNaN(numB)) {
                    return String(b).localeCompare(String(a));
                }
                return numB - numA;
            });
            
        case "asc":
        default:
            return sortedArray.sort();
    }
}