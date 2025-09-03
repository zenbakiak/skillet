// @name: UPPER
// @min_args: 1
// @max_args: 1
// @description: Converts string to uppercase
// @example: UPPER("hello") returns "HELLO"

function execute(args) {
    return args[0].toUpperCase();
}