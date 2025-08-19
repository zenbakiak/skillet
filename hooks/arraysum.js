// @name: ARRAYSUM
// @min_args: 1
// @max_args: 1
// @description: Sum all numbers in an array
// @example: ARRAYSUM([1, 2, 3, 4, 5]) returns 15

function execute(args) {
    const array = args[0];
    if (!Array.isArray(array)) {
        throw new Error("ARRAYSUM expects an array as argument");
    }
    
    return array.reduce((sum, item) => {
        if (typeof item === 'number') {
            return sum + item;
        }
        return sum;
    }, 0);
}