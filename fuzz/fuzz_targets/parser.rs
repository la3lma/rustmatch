#![no_main]

use libfuzzer_sys::fuzz_target;
use rustmatch::{MatcherBuilder, PatternId};

fuzz_target!(|bytes: &[u8]| {
    let Ok(pattern) = std::str::from_utf8(bytes) else {
        return;
    };

    let mut builder = MatcherBuilder::new();
    if builder.add(PatternId::new(0), pattern).is_ok() {
        let _ = builder.build();
    }
});
