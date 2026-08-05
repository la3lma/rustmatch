#![no_main]

use libfuzzer_sys::fuzz_target;
use rustmatch::{MatcherBuilder, PatternId, Utf16Text};

const ALPHABET: &[u8] = b"abcxyz012";

fuzz_target!(|bytes: &[u8]| {
    if bytes.is_empty() {
        return;
    }

    let pattern_count = 1 + usize::from(bytes[0] % 8);
    let mut patterns = Vec::with_capacity(pattern_count);
    for ordinal in 0..pattern_count {
        let source = bytes.get(ordinal + 1).copied().unwrap_or(0);
        let length = 1 + usize::from(source % 6);
        let pattern = (0..length)
            .map(|offset| {
                let byte = bytes.get(ordinal + offset + 2).copied().unwrap_or(source);
                char::from(ALPHABET[usize::from(byte) % ALPHABET.len()])
            })
            .collect::<String>();
        patterns.push(pattern);
    }

    let input = bytes
        .chunks(2)
        .map(|chunk| match chunk {
            [low, high] => u16::from_le_bytes([*low, *high]),
            [low] => u16::from(*low),
            _ => unreachable!("byte chunks contain one or two elements"),
        })
        .collect::<Vec<_>>();
    let input = Utf16Text::from_units(input);

    let single = scan(&patterns, &input, 1, 0);
    let parallel = scan(&patterns, &input, 4, 8_192);
    assert_eq!(single, parallel);
});

fn scan(
    patterns: &[String],
    input: &Utf16Text,
    workers: usize,
    cache_budget: usize,
) -> Vec<(u32, u64, u64)> {
    let mut builder = MatcherBuilder::new();
    builder
        .worker_count(workers)
        .state_cache_budget(cache_budget);
    for (ordinal, pattern) in patterns.iter().enumerate() {
        builder
            .add(PatternId::new(ordinal as u32), pattern)
            .expect("generated literal is supported");
    }
    let matcher = builder.build().expect("generated matcher builds");
    let mut events = Vec::new();
    matcher
        .scan(input, |matched| {
            let span = matched.span();
            assert!(span.start() <= span.end());
            assert!(span.end() <= input.as_units().len() as u64);
            events.push((matched.pattern_id().get(), span.start(), span.end()));
        })
        .expect("bounded input scans");
    events.sort_unstable();
    events
}
