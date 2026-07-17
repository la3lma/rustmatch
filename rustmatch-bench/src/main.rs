//! Correctness-gated smoke benchmarks for the private rustmatch workspace.

use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::hint::black_box;
use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use regex::{Regex, RegexSet};
use rustmatch::{Matcher, MatcherBuilder, PatternId, Utf16Text};
use serde::{Deserialize, Serialize};

const SMOKE_PATTERN_COUNT: usize = 32;
const SMOKE_CORPUS_TARGET_BYTES: usize = 256 * 1024;
const SMOKE_WARMUP_ITERATIONS: u32 = 2;
const SMOKE_MEASURED_ITERATIONS: u32 = 5;
const TRIPWIRE_PATTERN_COUNT: usize = 64;
const TRIPWIRE_CORPUS_TARGET_BYTES: usize = 1024 * 1024;
const TRIPWIRE_WARMUP_ITERATIONS: u32 = 3;
const TRIPWIRE_MEASURED_ITERATIONS: u32 = 7;
const TRIPWIRE_RELATIVE_LIMIT_PERCENT: u128 = 50;
const TRIPWIRE_ABSOLUTE_LIMIT_NS: u128 = 100_000_000;

fn main() -> ExitCode {
    match run(env::args().skip(1)) {
        Ok(output) => match serde_json::to_string(&output) {
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

fn run(mut arguments: impl Iterator<Item = String>) -> Result<CommandOutput, String> {
    let command = arguments.next();
    if cfg!(debug_assertions) {
        return Err("benchmark commands must be compiled with --release".to_owned());
    }
    match command.as_deref() {
        Some("literal-smoke") if arguments.next().is_none() => {
            literal_smoke().map(CommandOutput::Smoke)
        }
        Some("literal-tripwire") if arguments.next().is_none() => {
            literal_tripwire().map(CommandOutput::Tripwire)
        }
        Some("compare-tripwire") => {
            let baseline = arguments.next();
            let candidate = arguments.next();
            if arguments.next().is_some() {
                return Err(usage());
            }
            let baseline = baseline.ok_or_else(usage)?;
            let candidate = candidate.ok_or_else(usage)?;
            compare_tripwire_files(Path::new(&baseline), Path::new(&candidate))
                .map(CommandOutput::Comparison)
        }
        _ => Err(usage()),
    }
}

fn usage() -> String {
    "usage: rustmatch-bench <literal-smoke|literal-tripwire|compare-tripwire BASE.json CANDIDATE.json>"
        .to_owned()
}

fn literal_smoke() -> Result<SmokeReceipt, String> {
    let fixture = LiteralFixture::new(SMOKE_PATTERN_COUNT, SMOKE_CORPUS_TARGET_BYTES)?;
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
    let event_regexes = fixture
        .patterns
        .iter()
        .map(|pattern| Regex::new(&regex::escape(pattern)))
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
    let mut regex_event_set = regex_events(&regex_set, &event_regexes, &fixture.corpus)?;
    rust_event_set.sort_unstable();
    regex_event_set.sort_unstable();
    if rust_event_set != regex_event_set {
        return Err(format!(
            "RS-EVENTS mismatch: rustmatch produced {}, RegexSet lane produced {} events",
            rust_event_set.len(),
            regex_event_set.len()
        ));
    }

    for _ in 0..SMOKE_WARMUP_ITERATIONS {
        black_box(rust_native_ids(&rust_matcher, &utf16_input)?);
        black_box(regex_set_ids(&regex_set, &fixture.corpus)?);
        black_box(rust_events(&rust_matcher, &utf16_input)?);
        black_box(regex_events(&regex_set, &event_regexes, &fixture.corpus)?);
    }

    let rust_native_ns = measure(SMOKE_MEASURED_ITERATIONS, || {
        rust_native_ids(&rust_matcher, &utf16_input)
    })?;
    let regex_native_ns = measure(SMOKE_MEASURED_ITERATIONS, || {
        regex_set_ids(&regex_set, &fixture.corpus)
    })?;
    let rust_events_ns = measure(SMOKE_MEASURED_ITERATIONS, || {
        rust_events(&rust_matcher, &utf16_input)
    })?;
    let regex_events_ns = measure(SMOKE_MEASURED_ITERATIONS, || {
        regex_events(&regex_set, &event_regexes, &fixture.corpus)
    })?;

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
        warmup_iterations: SMOKE_WARMUP_ITERATIONS,
        measured_iterations: SMOKE_MEASURED_ITERATIONS,
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

fn literal_tripwire() -> Result<TripwireReceipt, String> {
    let fixture = LiteralFixture::new(TRIPWIRE_PATTERN_COUNT, TRIPWIRE_CORPUS_TARGET_BYTES)?;
    let input = Utf16Text::from(fixture.corpus.as_str());
    let matcher = build_rustmatch(&fixture.patterns)?;
    let expected = fixture.expected_events;
    let expected_digest = event_digest(&expected);

    let mut actual = rust_events(&matcher, &input)?;
    actual.sort_unstable();
    if actual != expected {
        return Err(format!(
            "C1 correctness mismatch: expected {} events, got {}",
            expected.len(),
            actual.len()
        ));
    }

    for _ in 0..TRIPWIRE_WARMUP_ITERATIONS {
        let events = black_box(rust_events(&matcher, &input)?);
        if events.len() != expected.len() {
            return Err("C1 warm-up produced an inconsistent event count".to_owned());
        }
    }
    let median_scan_ns = measure(TRIPWIRE_MEASURED_ITERATIONS, || {
        let events = rust_events(&matcher, &input)?;
        if events.len() != expected.len() {
            return Err("C1 measurement produced an inconsistent event count".to_owned());
        }
        Ok(events)
    })?;

    Ok(TripwireReceipt {
        schema_version: 1,
        evidence_id: "C1".to_owned(),
        benchmark: "ascii-literal-tripwire-v1".to_owned(),
        claim: "coarse-severe-regression-signal-only".to_owned(),
        revision: env::var("RUSTMATCH_BENCH_REVISION")
            .unwrap_or_else(|_| "local-unidentified".to_owned()),
        runner: env::var("RUSTMATCH_BENCH_RUNNER")
            .unwrap_or_else(|_| "local-unidentified".to_owned()),
        profile: "release".to_owned(),
        pattern_count: fixture.patterns.len(),
        corpus_bytes: fixture.corpus.len(),
        event_count: expected.len(),
        event_digest: expected_digest,
        warmup_iterations: TRIPWIRE_WARMUP_ITERATIONS,
        measured_iterations: TRIPWIRE_MEASURED_ITERATIONS,
        median_scan_ns,
        correctness: "pass".to_owned(),
    })
}

fn compare_tripwire_files(
    baseline_path: &Path,
    candidate_path: &Path,
) -> Result<ComparisonReceipt, String> {
    let baseline: TripwireReceipt = read_receipt("baseline", baseline_path)?;
    let candidate: TripwireReceipt = read_receipt("candidate", candidate_path)?;
    compare_tripwire(&baseline, &candidate)
}

fn read_receipt<T>(description: &str, path: &Path) -> Result<T, String>
where
    T: for<'de> Deserialize<'de>,
{
    let bytes = fs::read(path).map_err(|error| {
        format!(
            "could not read {description} receipt {}: {error}",
            path.display()
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "could not parse {description} receipt {}: {error}",
            path.display()
        )
    })
}

fn compare_tripwire(
    baseline: &TripwireReceipt,
    candidate: &TripwireReceipt,
) -> Result<ComparisonReceipt, String> {
    baseline.validate("baseline")?;
    candidate.validate("candidate")?;
    if baseline.fixture_identity() != candidate.fixture_identity() {
        return Err("C1 baseline and candidate fixture identities differ".to_owned());
    }
    if baseline.runner != candidate.runner {
        return Err("C1 baseline and candidate were not measured on the same runner".to_owned());
    }

    let regression_ns = candidate
        .median_scan_ns
        .saturating_sub(baseline.median_scan_ns);
    let slowdown_basis_points = candidate
        .median_scan_ns
        .saturating_mul(10_000)
        .checked_div(baseline.median_scan_ns)
        .unwrap_or(u128::MAX)
        .saturating_sub(10_000);
    let relative_limit_exceeded =
        slowdown_basis_points > TRIPWIRE_RELATIVE_LIMIT_PERCENT.saturating_mul(100);
    let absolute_limit_exceeded = regression_ns >= TRIPWIRE_ABSOLUTE_LIMIT_NS;
    if relative_limit_exceeded && absolute_limit_exceeded {
        return Err(format!(
            "C1 severe-regression tripwire fired: baseline={}ns candidate={}ns regression={}ns slowdown={}.{:02}%",
            baseline.median_scan_ns,
            candidate.median_scan_ns,
            regression_ns,
            slowdown_basis_points / 100,
            slowdown_basis_points % 100
        ));
    }

    Ok(ComparisonReceipt {
        schema_version: 1,
        evidence_id: "C1",
        comparison: "ascii-literal-tripwire-v1",
        claim: "coarse-severe-regression-signal-only",
        baseline_revision: baseline.revision.clone(),
        candidate_revision: candidate.revision.clone(),
        runner: baseline.runner.clone(),
        baseline_median_scan_ns: baseline.median_scan_ns,
        candidate_median_scan_ns: candidate.median_scan_ns,
        regression_ns,
        slowdown_basis_points,
        relative_limit_percent: TRIPWIRE_RELATIVE_LIMIT_PERCENT,
        absolute_limit_ns: TRIPWIRE_ABSOLUTE_LIMIT_NS,
        status: "pass",
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
        let mut search_start = 0;
        while let Some(matched) = regex.find_at(corpus, search_start) {
            events.push(Event {
                pattern_id,
                start: u64::try_from(matched.start())
                    .map_err(|_| "smoke corpus position exceeds u64".to_owned())?,
                end: u64::try_from(matched.end())
                    .map_err(|_| "smoke corpus position exceeds u64".to_owned())?,
            });
            search_start = matched
                .start()
                .checked_add(1)
                .ok_or_else(|| "smoke event start overflowed usize".to_owned())?;
            if search_start > corpus.len() {
                break;
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

fn measure<T>(
    measured_iterations: u32,
    mut operation: impl FnMut() -> Result<T, String>,
) -> Result<u128, String> {
    if measured_iterations == 0 {
        return Err("measurement requires at least one iteration".to_owned());
    }
    let mut samples = Vec::new();
    for _ in 0..measured_iterations {
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

struct LiteralFixture {
    patterns: Vec<String>,
    corpus: String,
    expected_events: Vec<Event>,
}

impl LiteralFixture {
    fn new(pattern_count: usize, corpus_target_bytes: usize) -> Result<Self, String> {
        if pattern_count == 0 {
            return Err("literal fixture requires at least one pattern".to_owned());
        }
        let patterns: Vec<_> = (0..pattern_count)
            .map(|index| format!("token{index:04}"))
            .collect();
        let mut corpus = String::with_capacity(corpus_target_bytes + 128);
        let mut expected_events = Vec::with_capacity(patterns.len());
        for (index, pattern) in patterns.iter().enumerate() {
            let target = corpus_target_bytes * (index + 1) / patterns.len();
            while corpus.len() < target.saturating_sub(pattern.len()) {
                corpus.push_str(" ordinary ascii filler without a marker ");
            }
            let start = corpus.len();
            corpus.push_str(pattern);
            expected_events.push(Event {
                pattern_id: u32::try_from(index + 1)
                    .map_err(|_| "literal fixture pattern count exceeds u32".to_owned())?,
                start: u64::try_from(start)
                    .map_err(|_| "literal fixture corpus position exceeds u64".to_owned())?,
                end: u64::try_from(corpus.len())
                    .map_err(|_| "literal fixture corpus position exceeds u64".to_owned())?,
            });
        }
        while corpus.len() < corpus_target_bytes {
            corpus.push_str(" ordinary ascii filler without a marker ");
        }
        Ok(Self {
            patterns,
            corpus,
            expected_events,
        })
    }
}

fn event_digest(events: &[Event]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for event in events {
        for byte in event
            .pattern_id
            .to_le_bytes()
            .into_iter()
            .chain(event.start.to_le_bytes())
            .chain(event.end.to_le_bytes())
        {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    format!("fnv1a64:{hash:016x}")
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum CommandOutput {
    Smoke(SmokeReceipt),
    Tripwire(TripwireReceipt),
    Comparison(ComparisonReceipt),
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TripwireReceipt {
    schema_version: u32,
    evidence_id: String,
    benchmark: String,
    claim: String,
    revision: String,
    runner: String,
    profile: String,
    pattern_count: usize,
    corpus_bytes: usize,
    event_count: usize,
    event_digest: String,
    warmup_iterations: u32,
    measured_iterations: u32,
    median_scan_ns: u128,
    correctness: String,
}

impl TripwireReceipt {
    fn validate(&self, description: &str) -> Result<(), String> {
        if self.schema_version != 1
            || self.evidence_id != "C1"
            || self.benchmark != "ascii-literal-tripwire-v1"
            || self.claim != "coarse-severe-regression-signal-only"
            || self.profile != "release"
            || self.correctness != "pass"
            || self.revision.is_empty()
            || self.runner.is_empty()
            || self.median_scan_ns == 0
        {
            return Err(format!("invalid {description} C1 receipt metadata"));
        }
        Ok(())
    }

    fn fixture_identity(&self) -> (usize, usize, usize, &str, u32, u32) {
        (
            self.pattern_count,
            self.corpus_bytes,
            self.event_count,
            &self.event_digest,
            self.warmup_iterations,
            self.measured_iterations,
        )
    }
}

#[derive(Debug, Serialize)]
struct ComparisonReceipt {
    schema_version: u32,
    evidence_id: &'static str,
    comparison: &'static str,
    claim: &'static str,
    baseline_revision: String,
    candidate_revision: String,
    runner: String,
    baseline_median_scan_ns: u128,
    candidate_median_scan_ns: u128,
    regression_ns: u128,
    slowdown_basis_points: u128,
    relative_limit_percent: u128,
    absolute_limit_ns: u128,
    status: &'static str,
}

#[cfg(test)]
mod tests {
    use super::{
        Event, LiteralFixture, SMOKE_CORPUS_TARGET_BYTES, SMOKE_PATTERN_COUNT, TripwireReceipt,
        compare_tripwire, regex_events,
    };
    use regex::{Regex, RegexSet};

    #[test]
    fn smoke_fixture_is_deterministic_and_contains_every_pattern() -> Result<(), String> {
        // Prepare
        let first = LiteralFixture::new(SMOKE_PATTERN_COUNT, SMOKE_CORPUS_TARGET_BYTES)?;

        // Test
        let second = LiteralFixture::new(SMOKE_PATTERN_COUNT, SMOKE_CORPUS_TARGET_BYTES)?;

        // Assert
        assert_eq!(first.patterns.len(), SMOKE_PATTERN_COUNT);
        assert!(first.corpus.len() >= SMOKE_CORPUS_TARGET_BYTES);
        assert_eq!(first.patterns, second.patterns);
        assert_eq!(first.corpus, second.corpus);
        assert_eq!(first.expected_events, second.expected_events);
        assert!(
            first
                .patterns
                .iter()
                .all(|pattern| first.corpus.contains(pattern))
        );
        Ok(())
    }

    #[test]
    fn tripwire_accepts_noise_below_the_absolute_limit() -> Result<(), String> {
        // Prepare
        let baseline = tripwire_receipt(100_000_000);
        let candidate = tripwire_receipt(160_000_000);

        // Test
        let comparison = compare_tripwire(&baseline, &candidate)?;

        // Assert
        assert_eq!(comparison.status, "pass");
        Ok(())
    }

    #[test]
    fn tripwire_rejects_a_catastrophic_slowdown() {
        // Prepare
        let baseline = tripwire_receipt(200_000_000);
        let candidate = tripwire_receipt(400_000_000);

        // Test
        let result = compare_tripwire(&baseline, &candidate);

        // Assert
        assert!(matches!(result, Err(message) if message.contains("tripwire fired")));
    }

    #[test]
    fn tripwire_rejects_different_fixture_identities() {
        // Prepare
        let baseline = tripwire_receipt(200_000_000);
        let mut candidate = tripwire_receipt(210_000_000);
        candidate.event_digest = "fnv1a64:different".to_owned();

        // Test
        let result = compare_tripwire(&baseline, &candidate);

        // Assert
        assert!(matches!(result, Err(message) if message.contains("fixture identities differ")));
    }

    #[test]
    fn regex_event_lane_preserves_overlapping_literal_starts() -> Result<(), String> {
        // Prepare
        let patterns = ["aa"];
        let regex_set = RegexSet::new(patterns).map_err(|error| error.to_string())?;
        let regexes = [Regex::new("aa").map_err(|error| error.to_string())?];

        // Test
        let events = regex_events(&regex_set, &regexes, "aaa")?;

        // Assert
        assert_eq!(
            events,
            vec![
                Event {
                    pattern_id: 1,
                    start: 0,
                    end: 2,
                },
                Event {
                    pattern_id: 1,
                    start: 1,
                    end: 3,
                },
            ]
        );
        Ok(())
    }

    fn tripwire_receipt(median_scan_ns: u128) -> TripwireReceipt {
        TripwireReceipt {
            schema_version: 1,
            evidence_id: "C1".to_owned(),
            benchmark: "ascii-literal-tripwire-v1".to_owned(),
            claim: "coarse-severe-regression-signal-only".to_owned(),
            revision: "revision".to_owned(),
            runner: "runner".to_owned(),
            profile: "release".to_owned(),
            pattern_count: 64,
            corpus_bytes: 1024 * 1024,
            event_count: 64,
            event_digest: "fnv1a64:receipt".to_owned(),
            warmup_iterations: 3,
            measured_iterations: 7,
            median_scan_ns,
            correctness: "pass".to_owned(),
        }
    }
}
