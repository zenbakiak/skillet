// @name: RANDOM
// @min_args: 0
// @max_args: 2
// @description: Generate random number. RANDOM() for 0-1, RANDOM(max) for 0-max, RANDOM(min, max) for min-max
// @example: RANDOM(1, 10) returns number between 1 and 10

function execute(args) {
    if (args.length === 0) {
        return Math.random();
    } else if (args.length === 1) {
        const max = args[0];
        return Math.random() * max;
    } else {
        const min = args[0];
        const max = args[1];
        return Math.random() * (max - min) + min;
    }
}