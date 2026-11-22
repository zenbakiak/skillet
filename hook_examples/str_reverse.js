// @name: STR_REVERSE
// @min_args: 1
// @max_args: 1
// @description: Reverse a string
// @example: STR_REVERSE("hello") returns "olleh"

function execute(args) {
    const str = args[0].toString();
    return str.split('').reverse().join('');
}