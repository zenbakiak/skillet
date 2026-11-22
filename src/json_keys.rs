//! Variable names from JSON keys, shared by the library's `*_json*` entry points, `sk --json`,
//! `sk_http_server` and `sk_server` (the binaries include this file with `#[path]`, so it is not
//! public API).
//!
//! A key becomes a variable by replacing every character other than a letter, a digit or `_`
//! with `_`. Two different keys that give the same name (`a-b` and `a_b`) are an error instead
//! of one silently replacing the other (BL-077, threat model 2.2). The reserved `:arguments`
//! variable is not a key: callers insert it after the keys, so it always wins.

use std::borrow::Cow;
use std::collections::HashMap;

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

/// Checks that no two keys map to the same variable name. Keys that need no sanitizing cannot
/// collide (JSON object keys are unique), so that case allocates nothing. The error names both
/// keys, in sorted order, and the variable.
pub fn check_json_keys<'a>(keys: impl IntoIterator<Item = &'a str> + Clone) -> Result<(), String> {
    if keys
        .clone()
        .into_iter()
        .all(|k| k.chars().all(is_name_char))
    {
        return Ok(());
    }
    let mut seen: HashMap<Cow<'a, str>, &'a str> = HashMap::new();
    for key in keys {
        let name: Cow<'a, str> = if key.chars().all(is_name_char) {
            Cow::Borrowed(key)
        } else {
            Cow::Owned(sanitize_json_key(key))
        };
        if let Some(other) = seen.get(&name) {
            let (first, second) = if *other <= key {
                (*other, key)
            } else {
                (key, *other)
            };
            return Err(format!(
                "JSON keys {first:?} and {second:?} both map to variable :{name}"
            ));
        }
        seen.insert(name, key);
    }
    Ok(())
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

    #[test]
    fn distinct_names_pass() {
        assert_eq!(check_json_keys(["a", "b_c", "first-name"]), Ok(()));
        assert_eq!(check_json_keys(["arguments", "x"]), Ok(()));
        assert_eq!(check_json_keys(Vec::<&str>::new()), Ok(()));
    }

    #[test]
    fn colliding_keys_name_both_keys_in_sorted_order() {
        let expected = Err(r#"JSON keys "a-b" and "a_b" both map to variable :a_b"#.to_string());
        assert_eq!(check_json_keys(["a_b", "a-b"]), expected);
        assert_eq!(check_json_keys(["a-b", "a_b"]), expected);
        assert_eq!(
            check_json_keys(["a.b", "x", "a-b"]),
            Err(r#"JSON keys "a-b" and "a.b" both map to variable :a_b"#.to_string())
        );
    }
}
