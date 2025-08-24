// @name: FIBONACCI
// @min_args: 1
// @max_args: 1
// @description: Calculate Fibonacci number at given position
// @example: FIBONACCI(10) returns 55

function execute(args) {
    const n = args[0];
    if (n <= 1) return n;
    
    let a = 0, b = 1;
    for (let i = 2; i <= n; i++) {
        let temp = a + b;
        a = b;
        b = temp;
    }
    return b;
}