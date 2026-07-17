//! Correctness-gated smoke benchmarks for the private rustmatch workspace.

use std::collections::BTreeSet;
use std::env;
use std::fmt::Write as _;
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
const I6_WARMUP_ITERATIONS: u32 = 3;
const I6_MEASURED_ITERATIONS: u32 = 7;
const I6_REQUIRED_IMPROVEMENT_BASIS_POINTS: u128 = 500;
const I6_MAXIMUM_REGRESSION_BASIS_POINTS: u128 = 300;
const I6_MAXIMUM_COMPILE_REGRESSION_BASIS_POINTS: u128 = 300;
const SCALE_WARMUP_ITERATIONS: u32 = 1;
const SCALE_MEASURED_ITERATIONS: u32 = 3;

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
        Some("i6-scan") => {
            let scenario = arguments.next();
            let pattern_count = arguments.next();
            let corpus_bytes = arguments.next();
            if arguments.next().is_some() {
                return Err(usage());
            }
            let scenario = scenario.ok_or_else(usage)?;
            let pattern_count = parse_positive_usize("pattern count", pattern_count)?;
            let corpus_bytes = parse_positive_usize("corpus bytes", corpus_bytes)?;
            i6_scan(&scenario, pattern_count, corpus_bytes).map(CommandOutput::I6Scan)
        }
        Some("compare-i6") => {
            let baseline = arguments.next();
            let candidate = arguments.next();
            if arguments.next().is_some() {
                return Err(usage());
            }
            let baseline = baseline.ok_or_else(usage)?;
            let candidate = candidate.ok_or_else(usage)?;
            compare_i6_files(Path::new(&baseline), Path::new(&candidate))
                .map(CommandOutput::I6Comparison)
        }
        Some("wuthering-scan") => {
            let patterns = arguments.next();
            let corpus = arguments.next();
            let pattern_count = arguments.next();
            let corpus_bytes = arguments.next();
            let cache_scrub_bytes = arguments.next();
            if arguments.next().is_some() {
                return Err(usage());
            }
            let patterns = patterns.ok_or_else(usage)?;
            let corpus = corpus.ok_or_else(usage)?;
            let pattern_count = parse_positive_usize("pattern count", pattern_count)?;
            let corpus_bytes = parse_positive_usize("corpus bytes", corpus_bytes)?;
            let cache_scrub_bytes = parse_positive_usize("cache scrub bytes", cache_scrub_bytes)?;
            wuthering_scan(
                Path::new(&patterns),
                Path::new(&corpus),
                pattern_count,
                corpus_bytes,
                cache_scrub_bytes,
            )
            .map(CommandOutput::ScaleScan)
        }
        Some("render-table") => {
            let output = arguments.next().ok_or_else(usage)?;
            let receipts: Vec<_> = arguments.collect();
            if receipts.is_empty() {
                return Err(usage());
            }
            render_scale_report(
                Path::new(&output),
                &receipts.iter().map(Path::new).collect::<Vec<_>>(),
            )
            .map(CommandOutput::Report)
        }
        _ => Err(usage()),
    }
}

fn usage() -> String {
    "usage: rustmatch-bench <literal-smoke|literal-tripwire|compare-tripwire BASE.json CANDIDATE.json|i6-scan SCENARIO PATTERN_COUNT CORPUS_BYTES|compare-i6 BASE.json CANDIDATE.json|wuthering-scan PATTERNS.txt CORPUS.txt PATTERN_COUNT CORPUS_BYTES CACHE_SCRUB_BYTES|render-table OUTPUT.html RECEIPT.json...>".to_owned()
}

fn parse_positive_usize(description: &str, value: Option<String>) -> Result<usize, String> {
    let source = value.ok_or_else(usage)?;
    let parsed = source
        .parse::<usize>()
        .map_err(|error| format!("invalid {description} {source:?}: {error}"))?;
    if parsed == 0 {
        Err(format!("{description} must be greater than zero"))
    } else {
        Ok(parsed)
    }
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

fn i6_scan(
    scenario: &str,
    pattern_count: usize,
    corpus_target_bytes: usize,
) -> Result<I6ScanReceipt, String> {
    let fixture = CampaignFixture::new(scenario, pattern_count, corpus_target_bytes)?;
    let input = Utf16Text::from(fixture.corpus.as_str());
    let mut expected = fixture.expected_events;
    expected.sort_unstable();
    let expected_digest = event_digest(&expected);

    black_box(build_rustmatch(&fixture.patterns)?);
    let median_compile_ns = measure(I6_MEASURED_ITERATIONS, || {
        build_rustmatch(&fixture.patterns)
    })?;
    let matcher = build_rustmatch(&fixture.patterns)?;

    let mut actual = rust_events(&matcher, &input)?;
    actual.sort_unstable();
    if actual != expected {
        return Err(format!(
            "I6 correctness mismatch for {scenario}: expected {} events, got {}",
            expected.len(),
            actual.len()
        ));
    }

    for _ in 0..I6_WARMUP_ITERATIONS {
        let mut events = black_box(rust_events(&matcher, &input)?);
        events.sort_unstable();
        if events != expected {
            return Err(format!(
                "I6 warm-up produced inconsistent events for {scenario}"
            ));
        }
    }
    let median_scan_ns = measure(I6_MEASURED_ITERATIONS, || {
        let mut events = rust_events(&matcher, &input)?;
        events.sort_unstable();
        if events != expected {
            return Err(format!(
                "I6 measurement produced inconsistent events for {scenario}"
            ));
        }
        Ok(events)
    })?;

    Ok(I6ScanReceipt {
        schema_version: 1,
        evidence_id: "I6-P1".to_owned(),
        benchmark: "state-cache-campaign-v1".to_owned(),
        claim: "optimization-admission".to_owned(),
        scenario: scenario.to_owned(),
        gate: fixture.gate.to_owned(),
        revision: env::var("RUSTMATCH_BENCH_REVISION")
            .unwrap_or_else(|_| "local-unidentified".to_owned()),
        runner: env::var("RUSTMATCH_BENCH_RUNNER")
            .unwrap_or_else(|_| "local-unidentified".to_owned()),
        profile: "release".to_owned(),
        pattern_count: fixture.patterns.len(),
        corpus_bytes: fixture.corpus.len(),
        event_count: expected.len(),
        event_digest: expected_digest,
        warmup_iterations: I6_WARMUP_ITERATIONS,
        measured_iterations: I6_MEASURED_ITERATIONS,
        median_compile_ns,
        median_scan_ns,
        correctness: "pass".to_owned(),
    })
}

fn compare_i6_files(
    baseline_path: &Path,
    candidate_path: &Path,
) -> Result<I6ComparisonReceipt, String> {
    let baseline: I6ScanReceipt = read_receipt("I6 baseline", baseline_path)?;
    let candidate: I6ScanReceipt = read_receipt("I6 candidate", candidate_path)?;
    compare_i6(&baseline, &candidate)
}

fn compare_i6(
    baseline: &I6ScanReceipt,
    candidate: &I6ScanReceipt,
) -> Result<I6ComparisonReceipt, String> {
    baseline.validate("baseline")?;
    candidate.validate("candidate")?;
    if baseline.fixture_identity() != candidate.fixture_identity() {
        return Err("I6 baseline and candidate fixture identities differ".to_owned());
    }
    if baseline.runner != candidate.runner {
        return Err("I6 baseline and candidate were not measured on the same runner".to_owned());
    }

    let improvement_basis_points =
        relative_decrease_basis_points(baseline.median_scan_ns, candidate.median_scan_ns);
    let slowdown_basis_points =
        relative_increase_basis_points(baseline.median_scan_ns, candidate.median_scan_ns);
    let compile_slowdown_basis_points =
        relative_increase_basis_points(baseline.median_compile_ns, candidate.median_compile_ns);
    if compile_slowdown_basis_points > I6_MAXIMUM_COMPILE_REGRESSION_BASIS_POINTS {
        return Err(format!(
            "I6 compile regression gate failed for {}: baseline={}ns candidate={}ns slowdown={}.{:02}%",
            baseline.scenario,
            baseline.median_compile_ns,
            candidate.median_compile_ns,
            compile_slowdown_basis_points / 100,
            compile_slowdown_basis_points % 100
        ));
    }
    match baseline.gate.as_str() {
        "improvement-5-percent"
            if improvement_basis_points < I6_REQUIRED_IMPROVEMENT_BASIS_POINTS =>
        {
            return Err(format!(
                "I6 positive gate failed for {}: baseline={}ns candidate={}ns improvement={}.{:02}%",
                baseline.scenario,
                baseline.median_scan_ns,
                candidate.median_scan_ns,
                improvement_basis_points / 100,
                improvement_basis_points % 100
            ));
        }
        "non-regression-3-percent"
            if slowdown_basis_points > I6_MAXIMUM_REGRESSION_BASIS_POINTS =>
        {
            return Err(format!(
                "I6 regression gate failed for {}: baseline={}ns candidate={}ns slowdown={}.{:02}%",
                baseline.scenario,
                baseline.median_scan_ns,
                candidate.median_scan_ns,
                slowdown_basis_points / 100,
                slowdown_basis_points % 100
            ));
        }
        "improvement-5-percent" | "non-regression-3-percent" => {}
        gate => return Err(format!("unknown I6 gate {gate:?}")),
    }

    Ok(I6ComparisonReceipt {
        schema_version: 1,
        evidence_id: "I6-P1",
        comparison: "state-cache-campaign-v1",
        scenario: baseline.scenario.clone(),
        gate: baseline.gate.clone(),
        baseline_revision: baseline.revision.clone(),
        candidate_revision: candidate.revision.clone(),
        runner: baseline.runner.clone(),
        baseline_median_compile_ns: baseline.median_compile_ns,
        candidate_median_compile_ns: candidate.median_compile_ns,
        baseline_median_scan_ns: baseline.median_scan_ns,
        candidate_median_scan_ns: candidate.median_scan_ns,
        improvement_basis_points,
        slowdown_basis_points,
        compile_slowdown_basis_points,
        status: "pass",
    })
}

fn relative_decrease_basis_points(baseline: u128, candidate: u128) -> u128 {
    baseline
        .saturating_sub(candidate)
        .saturating_mul(10_000)
        .checked_div(baseline)
        .unwrap_or(u128::MAX)
}

fn relative_increase_basis_points(baseline: u128, candidate: u128) -> u128 {
    candidate
        .saturating_sub(baseline)
        .saturating_mul(10_000)
        .checked_div(baseline)
        .unwrap_or(u128::MAX)
}

fn wuthering_scan(
    pattern_source: &Path,
    corpus_source: &Path,
    pattern_count: usize,
    corpus_target_bytes: usize,
    cache_scrub_bytes: usize,
) -> Result<ScaleScanReceipt, String> {
    let pattern_bytes = fs::read(pattern_source).map_err(|error| {
        format!(
            "could not read pattern source {}: {error}",
            pattern_source.display()
        )
    })?;
    let corpus_bytes = fs::read(corpus_source).map_err(|error| {
        format!(
            "could not read corpus source {}: {error}",
            corpus_source.display()
        )
    })?;
    let pattern_text = String::from_utf8(pattern_bytes.clone())
        .map_err(|error| format!("pattern source must be UTF-8: {error}"))?;
    let corpus_text = String::from_utf8(corpus_bytes.clone())
        .map_err(|error| format!("corpus source must be UTF-8: {error}"))?;
    let patterns = select_literal_patterns(&pattern_text, pattern_count)?;
    let corpus = expand_corpus(&corpus_text, corpus_target_bytes)?;
    let input = Utf16Text::from(corpus.as_str());

    black_box(build_rustmatch(&patterns)?);
    let median_compile_ns = measure(SCALE_MEASURED_ITERATIONS, || build_rustmatch(&patterns))?;
    let matcher = build_rustmatch(&patterns)?;
    let expected = rust_event_summary(&matcher, &input)?;
    let mut scrubber = CacheScrubber::new(cache_scrub_bytes)?;

    for _ in 0..SCALE_WARMUP_ITERATIONS {
        black_box(scrubber.scrub());
        let actual = black_box(rust_event_summary(&matcher, &input)?);
        if actual != expected {
            return Err("Wuthering warm-up produced inconsistent events".to_owned());
        }
    }

    let mut samples = Vec::with_capacity(SCALE_MEASURED_ITERATIONS as usize);
    for _ in 0..SCALE_MEASURED_ITERATIONS {
        black_box(scrubber.scrub());
        let started = Instant::now();
        let actual = black_box(rust_event_summary(&matcher, &input)?);
        let elapsed = nanos(started.elapsed());
        if actual != expected {
            return Err("Wuthering measurement produced inconsistent events".to_owned());
        }
        samples.push(elapsed);
    }

    Ok(ScaleScanReceipt {
        schema_version: 1,
        evidence_id: "I6-B1".to_owned(),
        benchmark: "wuthering-literal-scale-v1".to_owned(),
        claim: "exploratory-scale-and-cache-pressure-evidence".to_owned(),
        revision: benchmark_revision(),
        runner: benchmark_runner(),
        profile: "release".to_owned(),
        pattern_source: pattern_source.display().to_string(),
        pattern_source_digest: byte_digest(&pattern_bytes),
        corpus_source: corpus_source.display().to_string(),
        corpus_source_digest: byte_digest(&corpus_bytes),
        pattern_count: patterns.len(),
        corpus_bytes: corpus.len(),
        cache_scrub_bytes: scrubber.bytes(),
        cache_scrub_digest: format!("fnv1a64:{:016x}", scrubber.digest()),
        event_count: expected.count,
        event_digest: expected.digest(),
        warmup_iterations: SCALE_WARMUP_ITERATIONS,
        measured_iterations: SCALE_MEASURED_ITERATIONS,
        median_compile_ns,
        median_scan_ns: median(&mut samples),
        correctness: "pass".to_owned(),
    })
}

fn select_literal_patterns(source: &str, pattern_count: usize) -> Result<Vec<String>, String> {
    let unique: BTreeSet<_> = source
        .lines()
        .map(str::trim)
        .filter(|pattern| !pattern.is_empty())
        .collect();
    if unique.len() < pattern_count {
        return Err(format!(
            "pattern source has only {} distinct non-empty lines; requested {pattern_count}",
            unique.len()
        ));
    }
    Ok(unique
        .into_iter()
        .take(pattern_count)
        .map(regex::escape)
        .collect())
}

fn expand_corpus(source: &str, target_bytes: usize) -> Result<String, String> {
    if source.is_empty() {
        return Err("corpus source must not be empty".to_owned());
    }
    let mut corpus = String::with_capacity(target_bytes.saturating_add(source.len()));
    while corpus.len() < target_bytes {
        let remaining = target_bytes - corpus.len();
        if remaining >= source.len() {
            corpus.push_str(source);
        } else {
            let mut boundary = remaining;
            while boundary > 0 && !source.is_char_boundary(boundary) {
                boundary -= 1;
            }
            corpus.push_str(&source[..boundary]);
            break;
        }
    }
    Ok(corpus)
}

fn rust_event_summary(matcher: &Matcher, input: &Utf16Text) -> Result<EventSummary, String> {
    let mut summary = EventSummary::default();
    matcher
        .scan(input, |event| {
            summary.add(&Event {
                pattern_id: event.pattern_id().get(),
                start: event.span().start(),
                end: event.span().end(),
            });
        })
        .map_err(|error| format!("rustmatch scale scan failed: {error}"))?;
    Ok(summary)
}

fn render_scale_report(output: &Path, receipt_paths: &[&Path]) -> Result<ReportReceipt, String> {
    let mut receipts = receipt_paths
        .iter()
        .map(|path| read_receipt::<ScaleScanReceipt>("scale", path))
        .collect::<Result<Vec<_>, _>>()?;
    for receipt in &receipts {
        receipt.validate()?;
    }
    receipts.sort_by_key(|receipt| (receipt.corpus_bytes, receipt.pattern_count));

    let mut rows = String::new();
    for receipt in &receipts {
        let scan_ms = format_hundredths(receipt.median_scan_ns / 10_000);
        let compile_ms = format_hundredths(receipt.median_compile_ns / 10_000);
        let corpus_mib = format_tenths(receipt.corpus_bytes as u128 * 10 / (1024 * 1024));
        let scrub_mib = format_tenths(receipt.cache_scrub_bytes as u128 * 10 / (1024 * 1024));
        let throughput_hundredths = (receipt.corpus_bytes as u128)
            .saturating_mul(1_000_000_000)
            .saturating_mul(100)
            / receipt.median_scan_ns
            / (1024 * 1024);
        let throughput = format_hundredths(throughput_hundredths);
        write!(
            rows,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{corpus_mib} MiB</td><td>{scrub_mib} MiB</td><td>{}</td><td>{compile_ms} ms</td><td>{scan_ms} ms</td><td>{throughput} MiB/s</td><td><span class=\"pass\">pass</span></td></tr>",
            html_escape(&receipt.revision),
            html_escape(&receipt.runner),
            receipt.pattern_count,
            receipt.event_count,
        )
        .map_err(|error| format!("could not render report row: {error}"))?;
    }
    let provenance = receipts.first().map_or_else(String::new, |receipt| {
        format!(
            "Pattern source: <code>{}</code> ({})<br>Corpus source: <code>{}</code> ({})",
            html_escape(&receipt.pattern_source),
            html_escape(&receipt.pattern_source_digest),
            html_escape(&receipt.corpus_source),
            html_escape(&receipt.corpus_source_digest),
        )
    });
    let html = format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>rustmatch benchmark results</title><style>:root{{--ink:#13211c;--muted:#5c6862;--paper:#f5f1e8;--green:#0f654b;--line:#c9c2b4}}*{{box-sizing:border-box}}body{{margin:0;background:linear-gradient(145deg,#dce8dd,#f5f1e8 42%);color:var(--ink);font:16px/1.45 ui-monospace,SFMono-Regular,Menlo,monospace}}main{{max-width:1500px;margin:4rem auto;padding:0 2rem}}h1{{font:700 clamp(2rem,5vw,4.8rem)/.95 Georgia,serif;max-width:12ch;margin:0 0 1rem}}.lede{{max-width:72ch;color:var(--muted);margin-bottom:2rem}}.card{{background:rgba(255,255,255,.82);border:1px solid rgba(19,33,28,.15);box-shadow:0 24px 70px rgba(20,40,30,.12);overflow:auto}}table{{border-collapse:collapse;width:100%;min-width:1080px}}th,td{{padding:.9rem 1rem;border-bottom:1px solid var(--line);text-align:right;white-space:nowrap}}th{{position:sticky;top:0;background:#173c31;color:white;font-size:.78rem;letter-spacing:.04em;text-transform:uppercase}}th:first-child,th:nth-child(2),td:first-child,td:nth-child(2){{text-align:left}}tbody tr:hover{{background:#eef6ed}}.pass{{background:#ccebd8;color:#084b35;padding:.2rem .55rem;border-radius:999px;font-weight:700}}.note{{color:var(--muted);margin-top:1.4rem;font-size:.86rem}}code{{overflow-wrap:anywhere}}@media(max-width:700px){{main{{margin:2rem auto;padding:0 1rem}}}}</style></head><body><main><h1>rustmatch benchmark receipts</h1><p class=\"lede\">Exploratory Wuthering Heights literal-pattern scale results. Each timed scan follows a full cache-scrub pass; compilation and scanning are reported separately. These are engineering receipts, not cross-engine claims.</p><div class=\"card\"><table><thead><tr><th>Revision</th><th>Runner</th><th>Patterns</th><th>Corpus</th><th>Cache scrub</th><th>Events</th><th>Compile median</th><th>Scan median</th><th>Throughput</th><th>Correctness</th></tr></thead><tbody>{rows}</tbody></table></div><p class=\"note\">{provenance}<br>Protocol: one warm-up and three measured scans; medians shown. Literal patterns are the lexicographically first distinct non-empty lines from the source list, escaped before compilation.</p></main></body></html>"
    );
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "could not create report directory {}: {error}",
                parent.display()
            )
        })?;
    }
    fs::write(output, html)
        .map_err(|error| format!("could not write report {}: {error}", output.display()))?;
    Ok(ReportReceipt {
        schema_version: 1,
        report: output.display().to_string(),
        rows: receipts.len(),
        status: "pass",
    })
}

fn format_tenths(value: u128) -> String {
    format!("{}.{:01}", value / 10, value % 10)
}

fn format_hundredths(value: u128) -> String {
    format!("{}.{:02}", value / 100, value % 100)
}

fn html_escape(source: &str) -> String {
    source
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn benchmark_revision() -> String {
    env::var("RUSTMATCH_BENCH_REVISION").unwrap_or_else(|_| "local-unidentified".to_owned())
}

fn benchmark_runner() -> String {
    env::var("RUSTMATCH_BENCH_RUNNER").unwrap_or_else(|_| "local-unidentified".to_owned())
}

fn byte_digest(bytes: &[u8]) -> String {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv1a64:{hash:016x}")
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

struct CampaignFixture {
    patterns: Vec<String>,
    corpus: String,
    expected_events: Vec<Event>,
    gate: &'static str,
}

impl CampaignFixture {
    fn new(
        scenario: &str,
        pattern_count: usize,
        corpus_target_bytes: usize,
    ) -> Result<Self, String> {
        match scenario {
            "literal-sparse" => {
                let fixture = LiteralFixture::new(pattern_count, corpus_target_bytes)?;
                Ok(Self {
                    patterns: fixture.patterns,
                    corpus: fixture.corpus,
                    expected_events: fixture.expected_events,
                    gate: "improvement-5-percent",
                })
            }
            "literal-dense" => Self::literal_dense(pattern_count, corpus_target_bytes),
            "mixed-sparse" => Self::mixed_sparse(pattern_count, corpus_target_bytes),
            "assertion-bypass" => Self::assertion_sparse(pattern_count, corpus_target_bytes),
            _ => Err(format!(
                "unknown I6 scenario {scenario:?}; expected literal-sparse, literal-dense, mixed-sparse, or assertion-bypass"
            )),
        }
    }

    fn literal_dense(pattern_count: usize, corpus_target_bytes: usize) -> Result<Self, String> {
        if pattern_count == 0 {
            return Err("literal-dense requires at least one pattern".to_owned());
        }
        let patterns: Vec<_> = (0..pattern_count)
            .map(|index| format!("dense{index:04}"))
            .collect();
        let mut corpus = String::with_capacity(corpus_target_bytes + 32);
        let mut expected_events = Vec::new();
        let mut index = 0;
        while corpus.len() < corpus_target_bytes {
            corpus.push(' ');
            let pattern_index = index % patterns.len();
            let start = corpus.len();
            corpus.push_str(&patterns[pattern_index]);
            expected_events.push(event_for_literal(pattern_index, start, corpus.len())?);
            index += 1;
        }
        Ok(Self {
            patterns,
            corpus,
            expected_events,
            gate: "non-regression-3-percent",
        })
    }

    fn mixed_sparse(pattern_count: usize, corpus_target_bytes: usize) -> Result<Self, String> {
        let pairs: Vec<_> = (0..pattern_count)
            .map(|index| match index % 4 {
                0 => (
                    format!("mix{index:04}(cat|dog)"),
                    format!("mix{index:04}dog"),
                ),
                1 => (
                    format!("mix{index:04}[0-9]{{2}}"),
                    format!("mix{index:04}42"),
                ),
                2 => (format!("mix{index:04}x+z"), format!("mix{index:04}xxxz")),
                _ => (format!("mix{index:04}(ab)?z"), format!("mix{index:04}abz")),
            })
            .collect();
        Self::sparse_from_pairs(pairs, corpus_target_bytes, "improvement-5-percent")
    }

    fn assertion_sparse(pattern_count: usize, corpus_target_bytes: usize) -> Result<Self, String> {
        let pairs: Vec<_> = (0..pattern_count)
            .map(|index| (format!(r"\bword{index:04}\b"), format!("word{index:04}")))
            .collect();
        Self::sparse_from_pairs(pairs, corpus_target_bytes, "non-regression-3-percent")
    }

    fn sparse_from_pairs(
        pairs: Vec<(String, String)>,
        corpus_target_bytes: usize,
        gate: &'static str,
    ) -> Result<Self, String> {
        if pairs.is_empty() {
            return Err("I6 fixture requires at least one pattern".to_owned());
        }
        let mut corpus = String::with_capacity(corpus_target_bytes + 128);
        let mut expected_events = Vec::with_capacity(pairs.len());
        for (index, (_, literal)) in pairs.iter().enumerate() {
            let target = corpus_target_bytes * (index + 1) / pairs.len();
            while corpus.len() < target.saturating_sub(literal.len()) {
                corpus.push_str(" ordinary ascii filler between sparse matches ");
            }
            corpus.push(' ');
            let start = corpus.len();
            corpus.push_str(literal);
            let end = corpus.len();
            corpus.push(' ');
            expected_events.push(event_for_literal(index, start, end)?);
        }
        while corpus.len() < corpus_target_bytes {
            corpus.push_str(" ordinary ascii filler between sparse matches ");
        }
        Ok(Self {
            patterns: pairs.into_iter().map(|(pattern, _)| pattern).collect(),
            corpus,
            expected_events,
            gate,
        })
    }
}

fn event_for_literal(pattern_index: usize, start: usize, end: usize) -> Result<Event, String> {
    Ok(Event {
        pattern_id: u32::try_from(pattern_index + 1)
            .map_err(|_| "I6 fixture pattern count exceeds u32".to_owned())?,
        start: u64::try_from(start)
            .map_err(|_| "I6 fixture corpus position exceeds u64".to_owned())?,
        end: u64::try_from(end).map_err(|_| "I6 fixture corpus position exceeds u64".to_owned())?,
    })
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct EventSummary {
    count: usize,
    sum: u64,
    xor: u64,
    sum_of_squares: u64,
}

impl EventSummary {
    fn add(&mut self, event: &Event) {
        let hash = event_hash(event);
        self.count += 1;
        self.sum = self.sum.wrapping_add(hash);
        self.xor ^= hash.rotate_left(event.pattern_id % 64);
        self.sum_of_squares = self.sum_of_squares.wrapping_add(hash.wrapping_mul(hash));
    }

    fn digest(self) -> String {
        format!(
            "multiset64:{:016x}:{:016x}:{:016x}",
            self.sum, self.xor, self.sum_of_squares
        )
    }
}

fn event_hash(event: &Event) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
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
    hash
}

struct CacheScrubber {
    words: Vec<u64>,
    round: u64,
    digest: u64,
}

impl CacheScrubber {
    fn new(target_bytes: usize) -> Result<Self, String> {
        let words = target_bytes
            .checked_add(std::mem::size_of::<u64>() - 1)
            .ok_or_else(|| "cache scrub size overflowed usize".to_owned())?
            / std::mem::size_of::<u64>();
        Ok(Self {
            words: vec![0; words.max(1)],
            round: 0,
            digest: 0,
        })
    }

    fn scrub(&mut self) -> u64 {
        self.round = self.round.wrapping_add(1);
        let mut digest = 0xcbf2_9ce4_8422_2325_u64;
        let words_per_cache_line = 64 / std::mem::size_of::<u64>();
        for index in (0..self.words.len()).step_by(words_per_cache_line) {
            let value = self.words[index]
                .wrapping_add(self.round)
                .wrapping_add(index as u64);
            self.words[index] = value;
            digest ^= value;
            digest = digest.wrapping_mul(0x0000_0100_0000_01b3);
        }
        self.digest = digest;
        digest
    }

    fn bytes(&self) -> usize {
        self.words.len() * std::mem::size_of::<u64>()
    }

    fn digest(&self) -> u64 {
        self.digest
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
    I6Scan(I6ScanReceipt),
    I6Comparison(I6ComparisonReceipt),
    ScaleScan(ScaleScanReceipt),
    Report(ReportReceipt),
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
struct I6ScanReceipt {
    schema_version: u32,
    evidence_id: String,
    benchmark: String,
    claim: String,
    scenario: String,
    gate: String,
    revision: String,
    runner: String,
    profile: String,
    pattern_count: usize,
    corpus_bytes: usize,
    event_count: usize,
    event_digest: String,
    warmup_iterations: u32,
    measured_iterations: u32,
    median_compile_ns: u128,
    median_scan_ns: u128,
    correctness: String,
}

impl I6ScanReceipt {
    fn validate(&self, description: &str) -> Result<(), String> {
        if self.schema_version != 1
            || self.evidence_id != "I6-P1"
            || self.benchmark != "state-cache-campaign-v1"
            || self.claim != "optimization-admission"
            || self.profile != "release"
            || self.correctness != "pass"
            || self.revision.is_empty()
            || self.runner.is_empty()
            || self.median_compile_ns == 0
            || self.median_scan_ns == 0
        {
            return Err(format!("invalid {description} I6 receipt metadata"));
        }
        Ok(())
    }

    fn fixture_identity(&self) -> (&str, &str, usize, usize, usize, &str, u32, u32) {
        (
            &self.scenario,
            &self.gate,
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
struct I6ComparisonReceipt {
    schema_version: u32,
    evidence_id: &'static str,
    comparison: &'static str,
    scenario: String,
    gate: String,
    baseline_revision: String,
    candidate_revision: String,
    runner: String,
    baseline_median_compile_ns: u128,
    candidate_median_compile_ns: u128,
    baseline_median_scan_ns: u128,
    candidate_median_scan_ns: u128,
    improvement_basis_points: u128,
    slowdown_basis_points: u128,
    compile_slowdown_basis_points: u128,
    status: &'static str,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScaleScanReceipt {
    schema_version: u32,
    evidence_id: String,
    benchmark: String,
    claim: String,
    revision: String,
    runner: String,
    profile: String,
    pattern_source: String,
    pattern_source_digest: String,
    corpus_source: String,
    corpus_source_digest: String,
    pattern_count: usize,
    corpus_bytes: usize,
    cache_scrub_bytes: usize,
    cache_scrub_digest: String,
    event_count: usize,
    event_digest: String,
    warmup_iterations: u32,
    measured_iterations: u32,
    median_compile_ns: u128,
    median_scan_ns: u128,
    correctness: String,
}

impl ScaleScanReceipt {
    fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1
            || self.evidence_id != "I6-B1"
            || self.benchmark != "wuthering-literal-scale-v1"
            || self.claim != "exploratory-scale-and-cache-pressure-evidence"
            || self.profile != "release"
            || self.correctness != "pass"
            || self.revision.is_empty()
            || self.runner.is_empty()
            || self.pattern_count == 0
            || self.corpus_bytes == 0
            || self.cache_scrub_bytes == 0
            || self.median_compile_ns == 0
            || self.median_scan_ns == 0
        {
            return Err("invalid Wuthering scale receipt metadata".to_owned());
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
struct ReportReceipt {
    schema_version: u32,
    report: String,
    rows: usize,
    status: &'static str,
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
        CacheScrubber, CampaignFixture, Event, I6ScanReceipt, LiteralFixture,
        SMOKE_CORPUS_TARGET_BYTES, SMOKE_PATTERN_COUNT, TripwireReceipt, compare_i6,
        compare_tripwire, expand_corpus, regex_events, select_literal_patterns,
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

    #[test]
    fn scale_pattern_selection_is_stable_literal_and_exactly_sized() -> Result<(), String> {
        // Prepare
        let source = "zeta\na.b\nalpha\na.b\n";

        // Test
        let patterns = select_literal_patterns(source, 3)?;

        // Assert
        assert_eq!(patterns, [r"a\.b", "alpha", "zeta"]);
        Ok(())
    }

    #[test]
    fn scale_corpus_expansion_preserves_utf8_boundaries() -> Result<(), String> {
        // Prepare
        let source = "abø";

        // Test
        let corpus = expand_corpus(source, 7)?;

        // Assert
        assert_eq!(corpus, "abøab");
        assert_eq!(corpus.len(), 6);
        Ok(())
    }

    #[test]
    fn cache_scrubber_touches_the_requested_working_set() -> Result<(), String> {
        // Prepare
        let mut scrubber = CacheScrubber::new(1024)?;

        // Test
        let first = scrubber.scrub();
        let second = scrubber.scrub();

        // Assert
        assert_eq!(scrubber.bytes(), 1024);
        assert_ne!(first, second);
        assert_eq!(scrubber.digest(), second);
        Ok(())
    }

    #[test]
    fn every_focused_i6_fixture_has_known_events() -> Result<(), String> {
        // Prepare
        let scenarios = [
            "literal-sparse",
            "literal-dense",
            "mixed-sparse",
            "assertion-bypass",
        ];

        // Test
        let fixtures = scenarios
            .iter()
            .map(|scenario| CampaignFixture::new(scenario, 8, 4096))
            .collect::<Result<Vec<_>, _>>()?;

        // Assert
        assert!(
            fixtures
                .iter()
                .all(|fixture| fixture.patterns.len() == 8 && !fixture.expected_events.is_empty())
        );
        Ok(())
    }

    #[test]
    fn i6_comparison_requires_a_positive_target_result() {
        // Prepare
        let baseline = i6_receipt(100_000_000, "base");
        let candidate = i6_receipt(97_000_000, "candidate");

        // Test
        let result = compare_i6(&baseline, &candidate);

        // Assert
        assert!(matches!(result, Err(message) if message.contains("positive gate failed")));
    }

    #[test]
    fn i6_comparison_accepts_an_improvement_beyond_noise() -> Result<(), String> {
        // Prepare
        let baseline = i6_receipt(100_000_000, "base");
        let candidate = i6_receipt(94_000_000, "candidate");

        // Test
        let comparison = compare_i6(&baseline, &candidate)?;

        // Assert
        assert_eq!(comparison.status, "pass");
        Ok(())
    }

    #[test]
    fn i6_comparison_rejects_a_repeatable_compile_regression() {
        // Prepare
        let baseline = i6_receipt(100_000_000, "base");
        let mut candidate = i6_receipt(90_000_000, "candidate");
        candidate.median_compile_ns = 1_040_000;

        // Test
        let result = compare_i6(&baseline, &candidate);

        // Assert
        assert!(
            matches!(result, Err(message) if message.contains("compile regression gate failed"))
        );
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

    fn i6_receipt(median_scan_ns: u128, revision: &str) -> I6ScanReceipt {
        I6ScanReceipt {
            schema_version: 1,
            evidence_id: "I6-P1".to_owned(),
            benchmark: "state-cache-campaign-v1".to_owned(),
            claim: "optimization-admission".to_owned(),
            scenario: "literal-sparse".to_owned(),
            gate: "improvement-5-percent".to_owned(),
            revision: revision.to_owned(),
            runner: "runner".to_owned(),
            profile: "release".to_owned(),
            pattern_count: 64,
            corpus_bytes: 1024 * 1024,
            event_count: 64,
            event_digest: "fnv1a64:receipt".to_owned(),
            warmup_iterations: 3,
            measured_iterations: 7,
            median_compile_ns: 1_000_000,
            median_scan_ns,
            correctness: "pass".to_owned(),
        }
    }
}
