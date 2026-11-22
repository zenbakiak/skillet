//! Inputs found by the fuzz targets in `fuzz/` (BL-019, ADR-0028). Each one used to panic or
//! abort; it must now return a value or an `Err`. Add the minimized input here with the fix.

#[test]
fn lone_equals_at_start_is_an_error_not_a_panic() {
    // `==` is a leading `=` (stripped) followed by a lone `=`, which lexes as `EqEq`. The lexer
    // computed its start as `pos - 2` = 0 - 1: a subtract overflow (panic in debug, error
    // position usize::MAX in release).
    for src in ["==", "==A", "==IN.2)", "= =", "=A"] {
        let err = skillet::parse(src).unwrap_err();
        assert!(
            err.position.is_some_and(|p| p <= src.len()),
            "{src:?}: {err:?}"
        );
    }
    let err = skillet::evaluate_with_json("==M", r#"{"a": 1}"#).unwrap_err();
    assert_eq!(err.position, Some(0), "{err:?}");
}
