//! Variable names from JSON keys, shared by the library's `*_json*` entry points, `sk --json`
//! and `sk_http_server` (the binaries include this file with `#[path]`, so it is not public API).
//!
//! A key becomes a variable by replacing every character other than a letter, a digit or `_`
//! with `_`. The reserved `:arguments` variable is not a key: callers insert it after the keys,
//! so it always wins (BL-077).

fn is_name_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// The variable name for a JSON key: characters other than letters, digits and `_` become `_`.
pub fn sanitize_json_key(key: &str) -> String {
    if key.chars().all(is_name_char) {
        return key.to_string();
    }
    key.chars()
        .map(|c| if is_name_char(c) { c } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_non_name_characters() {
        assert_eq!(sanitize_json_key("first-name"), "first_name");
        assert_eq!(sanitize_json_key("a.b c"), "a_b_c");
        assert_eq!(sanitize_json_key("plain_1"), "plain_1");
    }
}
