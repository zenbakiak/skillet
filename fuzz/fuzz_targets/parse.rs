//! Any UTF-8 text into `skillet::parse`. An `Err` is fine; a panic or abort is a bug.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(src) = std::str::from_utf8(data) {
        let _ = skillet::parse(src);
    }
});
