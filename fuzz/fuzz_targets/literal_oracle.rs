#![no_main]

use libfuzzer_sys::fuzz_target;
use rustmatch::{MatcherBuilder, PatternId, Utf16Text};

const ALPHABET: &[u8] = b"abcxyz012";

fuzz_target!(|bytes: &[u8]| {
    if bytes.is_empty() {
        return;
    }

    let split = usize::from(bytes[0]) % bytes.len();
    let pattern_bytes = &bytes[..split];
    let input = bytes[split..]
        .iter()
        .map(|byte| u16::from(ALPHABET[usize::from(*byte) % ALPHABET.len()]))
        .collect::<Vec<_>>();
    let patterns = pattern_bytes
        .chunks(4)
        .take(16)
        .filter(|chunk| !chunk.is_empty())
        .map(|chunk| {
            chunk
                .iter()
                .map(|byte| char::from(ALPHABET[usize::from(*byte) % ALPHABET.len()]))
                .collect::<String>()
        })
        .collect::<Vec<_>>();
    if patterns.is_empty() {
        return;
    }

    let mut builder = MatcherBuilder::new();
    for (ordinal, pattern) in patterns.iter().enumerate() {
        builder
            .add(PatternId::new(ordinal as u32), pattern)
            .expect("generated literal is supported");
    }
    let matcher = builder.build().expect("generated matcher builds");
    let text = Utf16Text::from_units(input.clone());
    let mut actual = Vec::new();
    matcher
        .scan(&text, |matched| {
            let span = matched.span();
            actual.push((matched.pattern_id().get(), span.start(), span.end()));
        })
        .expect("bounded input scans");
    actual.sort_unstable();

    let mut expected = Vec::new();
    for (ordinal, pattern) in patterns.iter().enumerate() {
        let units = pattern.encode_utf16().collect::<Vec<_>>();
        for start in 0..=input.len().saturating_sub(units.len()) {
            if input[start..].starts_with(&units) {
                expected.push((ordinal as u32, start as u64, (start + units.len()) as u64));
            }
        }
    }
    expected.sort_unstable();

    assert_eq!(actual, expected);
});
