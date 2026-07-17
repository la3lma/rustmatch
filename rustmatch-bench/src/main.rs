//! Correctness-gated smoke benchmarks for the private rustmatch workspace.

use std::collections::BTreeSet;
use std::env;
use std::hint::black_box;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use regex::{Regex, RegexSet};
use rustmatch::{Matcher, MatcherBuilder, PatternId, Utf16Text};
use serde::Serialize;

const PATTERN_COUNT: usize = 32;
const CORPUS_TARGET_BYTES: usize = 256 * 1024;
const WARMUP_ITERATIONS: u32 = 2;
const MEASURED_ITERATIONS: u32 = 5;

fn main() -> ExitCode {
    match run(env::args().skip(1)) {
        Ok(receipt) => match serde_json::to_string(&receipt) {
            Ok(json) => {
                println!("{json}");
                ExitCode::SUCCESS
            }
            Err(error) => {
                eprintln!("error: could not serialize benchmark receipt: {error}");
                ExitCode::FAILURE
            }
        },
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(mut arguments: impl Iterator<Item = String>) -> Result<SmokeReceipt, String> {
    let command = arguments.next();
    if arguments.next().is_some() || command.as_deref() != Some("literal-smoke") {
        return Err("usage: rustmatch-bench literal-smoke".to_owned());
    }
    if cfg!(debug_assertions) {
        return Err("literal-smoke must be compiled with --release".to_owned());
    }
    literal_smoke()
}

fn literal_smoke() -> Result<SmokeReceipt, String> {
    let fixture = SmokeFixture::new();
    if !fixture.corpus.is_ascii() || fixture.patterns.iter().any(|pattern| !pattern.is_ascii()) {
        return Err("literal smoke fixture must remain 7-bit ASCII".to_owned());
    }
    let utf16_input = Utf16Text::from(fixture.corpus.as_str());

    let started = Instant::now();
    let rust_matcher = build_rustmatch(&fixture.patterns)?;
    let rust_compile = started.elapsed();

    let started = Instant::now();
    let regex_set = RegexSet::new(&fixture.patterns)
        .map_err(|error| format!("RegexSet compilation failed: {error}"))?;
    let regex_set_compile = started.elapsed();

    let started = Instant::now();
    let anchored_regexes = fixture
        .patterns
        .iter()
        .map(|pattern| Regex::new(&format!(r"\A(?:{})", regex::escape(pattern))))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("event-lane Regex compilation failed: {error}"))?;
    let regex_event_compile = started.elapsed();

    let rust_ids = rust_native_ids(&rust_matcher, &utf16_input)?;
    let regex_ids = regex_set_ids(&regex_set, &fixture.corpus)?;
    if rust_ids != regex_ids {
        return Err(format!(
            "RS-NATIVE mismatch: rustmatch {rust_ids:?}, RegexSet {regex_ids:?}"
        ));
    }

    let mut rust_event_set = rust_events(&rust_matcher, &utf16_input)?;
    let mut regex_event_set = regex_events(&regex_set, &anchored_regexes, &fixture.corpus)?;
    rust_event_set.sort_unstable();
    regex_event_set.sort_unstable();
    if rust_event_set != regex_event_set {
        return Err(format!(
            "RS-EVENTS mismatch: rustmatch produced {}, RegexSet lane produced {} events",
            rust_event_set.len(),
            regex_event_set.len()
        ));
    }

    for _ in 0..WARMUP_ITERATIONS {
        black_box(rust_native_ids(&rust_matcher, &utf16_input)?);
        black_box(regex_set_ids(&regex_set, &fixture.corpus)?);
        black_box(rust_events(&rust_matcher, &utf16_input)?);
        black_box(regex_events(
            &regex_set,
            &anchored_regexes,
            &fixture.corpus,
        )?);
    }

    let rust_native_ns = measure(|| rust_native_ids(&rust_matcher, &utf16_input))?;
    let regex_native_ns = measure(|| regex_set_ids(&regex_set, &fixture.corpus))?;
    let rust_events_ns = measure(|| rust_events(&rust_matcher, &utf16_input))?;
    let regex_events_ns = measure(|| regex_events(&regex_set, &anchored_regexes, &fixture.corpus))?;

    Ok(SmokeReceipt {
        schema_version: 1,
        evidence_id: "B0",
        benchmark: "literal-smoke-v1",
        claim: "correctness-and-harness-smoke-only",
        regex_version: "1.13.1",
        profile: "release",
        pattern_count: fixture.patterns.len(),
        corpus_bytes: fixture.corpus.len(),
        matched_pattern_count: rust_ids.len(),
        event_count: rust_event_set.len(),
        warmup_iterations: WARMUP_ITERATIONS,
        measured_iterations: MEASURED_ITERATIONS,
        rustmatch_compile_ns: nanos(rust_compile),
        regex_set_compile_ns: nanos(regex_set_compile),
        regex_event_lane_compile_ns: nanos(regex_event_compile),
        rustmatch_native_median_ns: rust_native_ns,
        regex_set_native_median_ns: regex_native_ns,
        rustmatch_events_median_ns: rust_events_ns,
        regex_set_events_median_ns: regex_events_ns,
        correctness: "pass",
    })
}

fn build_rustmatch(patterns: &[String]) -> Result<Matcher, String> {
    let mut builder = MatcherBuilder::new();
    for (index, pattern) in patterns.iter().enumerate() {
        let pattern_id = u32::try_from(index + 1)
            .map(PatternId::new)
            .map_err(|_| "smoke pattern count exceeds PatternId".to_owned())?;
        builder
            .add(pattern_id, pattern)
            .map_err(|error| format!("rustmatch registration failed: {error}"))?;
    }
    builder
        .build()
        .map_err(|error| format!("rustmatch build failed: {error}"))
}

fn rust_native_ids(matcher: &Matcher, input: &Utf16Text) -> Result<Vec<u32>, String> {
    let mut ids = BTreeSet::new();
    matcher
        .scan(input, |event| {
            ids.insert(event.pattern_id().get());
        })
        .map_err(|error| format!("rustmatch native scan failed: {error}"))?;
    Ok(ids.into_iter().collect())
}

fn regex_set_ids(regex_set: &RegexSet, corpus: &str) -> Result<Vec<u32>, String> {
    regex_set
        .matches(corpus)
        .into_iter()
        .map(pattern_id_for_index)
        .collect()
}

fn rust_events(matcher: &Matcher, input: &Utf16Text) -> Result<Vec<Event>, String> {
    let mut events = Vec::new();
    matcher
        .scan(input, |event| {
            events.push(Event {
                pattern_id: event.pattern_id().get(),
                start: event.span().start(),
                end: event.span().end(),
            });
        })
        .map_err(|error| format!("rustmatch event scan failed: {error}"))?;
    Ok(events)
}

fn regex_events(
    regex_set: &RegexSet,
    regexes: &[Regex],
    corpus: &str,
) -> Result<Vec<Event>, String> {
    let mut events = Vec::new();
    for index in regex_set.matches(corpus) {
        let pattern_id = pattern_id_for_index(index)?;
        let regex = regexes
            .get(index)
            .ok_or_else(|| "RegexSet returned an out-of-range pattern index".to_owned())?;
        for start in 0..corpus.len() {
            if let Some(matched) = regex.find(&corpus[start..]) {
                let end = start
                    .checked_add(matched.end())
                    .ok_or_else(|| "smoke event end overflowed usize".to_owned())?;
                events.push(Event {
                    pattern_id,
                    start: u64::try_from(start)
                        .map_err(|_| "smoke corpus position exceeds u64".to_owned())?,
                    end: u64::try_from(end)
                        .map_err(|_| "smoke corpus position exceeds u64".to_owned())?,
                });
            }
        }
    }
    Ok(events)
}

fn pattern_id_for_index(index: usize) -> Result<u32, String> {
    index
        .checked_add(1)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| "smoke pattern index exceeds u32".to_owned())
}

fn measure<T>(mut operation: impl FnMut() -> Result<T, String>) -> Result<u128, String> {
    let mut samples = Vec::new();
    for _ in 0..MEASURED_ITERATIONS {
        let started = Instant::now();
        black_box(operation()?);
        samples.push(nanos(started.elapsed()));
    }
    Ok(median(&mut samples))
}

fn median(samples: &mut [u128]) -> u128 {
    samples.sort_unstable();
    samples[samples.len() / 2]
}

fn nanos(duration: Duration) -> u128 {
    duration.as_nanos()
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Event {
    pattern_id: u32,
    start: u64,
    end: u64,
}

struct SmokeFixture {
    patterns: Vec<String>,
    corpus: String,
}

impl SmokeFixture {
    fn new() -> Self {
        let patterns: Vec<_> = (0..PATTERN_COUNT)
            .map(|index| format!("token{index:04}"))
            .collect();
        let mut corpus = String::with_capacity(CORPUS_TARGET_BYTES + 128);
        for (index, pattern) in patterns.iter().enumerate() {
            let target = CORPUS_TARGET_BYTES * (index + 1) / patterns.len();
            while corpus.len() < target.saturating_sub(pattern.len()) {
                corpus.push_str(" ordinary ascii filler without a marker ");
            }
            corpus.push_str(pattern);
        }
        while corpus.len() < CORPUS_TARGET_BYTES {
            corpus.push_str(" ordinary ascii filler without a marker ");
        }
        Self { patterns, corpus }
    }
}

#[derive(Debug, Serialize)]
struct SmokeReceipt {
    schema_version: u32,
    evidence_id: &'static str,
    benchmark: &'static str,
    claim: &'static str,
    regex_version: &'static str,
    profile: &'static str,
    pattern_count: usize,
    corpus_bytes: usize,
    matched_pattern_count: usize,
    event_count: usize,
    warmup_iterations: u32,
    measured_iterations: u32,
    rustmatch_compile_ns: u128,
    regex_set_compile_ns: u128,
    regex_event_lane_compile_ns: u128,
    rustmatch_native_median_ns: u128,
    regex_set_native_median_ns: u128,
    rustmatch_events_median_ns: u128,
    regex_set_events_median_ns: u128,
    correctness: &'static str,
}

#[cfg(test)]
mod tests {
    use super::{CORPUS_TARGET_BYTES, PATTERN_COUNT, SmokeFixture};

    #[test]
    fn smoke_fixture_is_deterministic_and_contains_every_pattern() {
        // Prepare
        let first = SmokeFixture::new();

        // Test
        let second = SmokeFixture::new();

        // Assert
        assert_eq!(first.patterns.len(), PATTERN_COUNT);
        assert!(first.corpus.len() >= CORPUS_TARGET_BYTES);
        assert_eq!(first.patterns, second.patterns);
        assert_eq!(first.corpus, second.corpus);
        assert!(
            first
                .patterns
                .iter()
                .all(|pattern| first.corpus.contains(pattern))
        );
    }
}
