#![no_main]

use libfuzzer_sys::fuzz_target;
use rustmatch::{MatcherBuilder, PatternId};

fuzz_target!(|bytes: &[u8]| {
    let mut builder = MatcherBuilder::new();
    let mut next_id = 0_u32;

    for candidate in bytes.split(|&byte| byte == 0).take(32) {
        let Ok(pattern) = std::str::from_utf8(candidate) else {
            continue;
        };
        if pattern.len() > 1_024 {
            continue;
        }
        if builder.add(PatternId::new(next_id), pattern).is_ok() {
            next_id += 1;
        }
    }

    if next_id > 0 {
        let _ = builder.build();
    }
});
