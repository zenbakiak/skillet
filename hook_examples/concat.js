// @name: CONCAT_ALL
// @min_args: 1
// @max_args: unlimited
// @description: Concatenate all arguments as strings
// @example: CONCAT_ALL("Hello", " ", "World") returns "Hello World"

function execute(args) {
    return args.map(arg => String(arg)).join('');
}