#![no_main]

use libfuzzer_sys::fuzz_target;
use rustmatch::{MatcherBuilder, PatternFlags, PatternId, Utf16Text};

const FREE: [&str; 12] = [
    "a", "aa", "ab|a", "a+", "[A-Z]+", "cat", "😀", ".", "x(|a)y", "a{0,2}b", "(?:ab)+", "[α-γ]",
];
const BEARING: [&str; 10] = [
    "^a",
    "a$",
    "^line$",
    r"\bword\b",
    r"\B.\B",
    "(?i)^cat$",
    "(?:^ab|cd$)",
    r"^\w+$",
    r"a\b",
    r"\Bz",
];
const WORKERS: [usize; 6] = [1, 2, 3, 4, 8, 128];
const CACHE_BUDGETS: [usize; 3] = [0, 1, 8_192];

fuzz_target!(|bytes: &[u8]| {
    if bytes.len() < 4 {
        return;
    }
    let pattern_count = 2 + usize::from(bytes[0] % 15);
    let mut patterns = Vec::with_capacity(pattern_count);
    patterns.push((FREE[usize::from(bytes[1]) % FREE.len()], PatternFlags::NONE));
    patterns.push((
        BEARING[usize::from(bytes[2]) % BEARING.len()],
        PatternFlags::NONE,
    ));
    for ordinal in 2..pattern_count {
        let selector = bytes
            .get(ordinal + 1)
            .copied()
            .unwrap_or(bytes[ordinal % 4]);
        let source = if selector % 3 == 0 {
            BEARING[usize::from(selector) % BEARING.len()]
        } else {
            FREE[usize::from(selector) % FREE.len()]
        };
        let flags = if selector % 7 == 0 {
            PatternFlags::CASE_INSENSITIVE
        } else {
            PatternFlags::NONE
        };
        patterns.push((source, flags));
    }

    let input_offset = pattern_count.saturating_add(2).min(bytes.len());
    let input = bytes[input_offset..]
        .chunks(2)
        .take(256)
        .map(|chunk| match chunk {
            [low, high] => u16::from_le_bytes([*low, *high]),
            [low] => u16::from(*low),
            _ => unreachable!("byte chunks contain one or two elements"),
        })
        .collect::<Vec<_>>();
    let input = Utf16Text::from_units(input);
    let workers = WORKERS[usize::from(bytes[bytes.len() - 2]) % WORKERS.len()];
    let cache_budget = CACHE_BUDGETS[usize::from(bytes[bytes.len() - 1]) % CACHE_BUDGETS.len()];

    let ordinary = scan(&patterns, &input, workers, cache_budget, false);
    let cohort = scan(&patterns, &input, workers, cache_budget, true);
    let repeated = scan(&patterns, &input, workers, cache_budget, true);
    assert_eq!(ordinary, cohort);
    assert_eq!(cohort, repeated);
});

fn scan(
    patterns: &[(&str, PatternFlags)],
    input: &Utf16Text,
    workers: usize,
    cache_budget: usize,
    cohort_enabled: bool,
) -> Vec<(u32, u64, u64)> {
    let mut builder = MatcherBuilder::new();
    builder
        .worker_count(workers)
        .state_cache_budget(cache_budget)
        .cohort_compilation_enabled(cohort_enabled);
    for (ordinal, (pattern, flags)) in patterns.iter().enumerate() {
        builder
            .add_with_flags(PatternId::new(ordinal as u32), pattern, *flags)
            .expect("frozen templates are supported");
    }
    let matcher = builder.build().expect("bounded matcher builds");
    if cohort_enabled {
        let layout = matcher.cohort_execution_diagnostics();
        assert!(layout.enabled());
        assert_eq!(layout.cohort_count(), 2);
        assert_eq!(layout.total_cache_budget(), cache_budget);
    }
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
