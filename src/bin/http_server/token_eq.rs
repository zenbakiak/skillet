//! Token comparison for both servers (`sk_server` includes this file with `#[path]`).

/// Whether `supplied` equals `expected`, in time that depends only on the lengths, not on
/// where the first differing byte is (BL-018). Only the length can leak.
pub fn tokens_match(supplied: &str, expected: &str) -> bool {
    let (a, b) = (supplied.as_bytes(), expected.as_bytes());
    if a.len() != b.len() {
        return false;
    }
    let diff = a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y));
    std::hint::black_box(diff) == 0
}

#[cfg(test)]
mod tests {
    use super::tokens_match;

    #[test]
    fn matches_only_identical_tokens() {
        assert!(tokens_match("secret-token", "secret-token"));
        assert!(tokens_match("", ""));
        assert!(!tokens_match("secret-tokeN", "secret-token"));
        assert!(!tokens_match("Secret-token", "secret-token"));
        assert!(!tokens_match("secret", "secret-token"));
        assert!(!tokens_match("secret-token-x", "secret-token"));
        assert!(!tokens_match("", "secret-token"));
        assert!(tokens_match("tökén", "tökén"));
        assert!(!tokens_match("tökén", "tokén"));
    }
}
