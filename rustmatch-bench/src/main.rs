//! Correctness-gated smoke benchmarks for the private rustmatch workspace.

use std::collections::BTreeSet;
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::hint::black_box;
use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use regex::{Regex, RegexSet};
use rustmatch::{
    Matcher, MatcherBuilder, PatternDiagnostics, PatternId, ScanDiagnostics, Utf16Text,
};
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
const WUTHERING_TRIPWIRE_RELATIVE_LIMIT_PERCENT: u128 = 100;
const WUTHERING_TRIPWIRE_ABSOLUTE_LIMIT_NS: u128 = 50_000_000;
const I6_WARMUP_ITERATIONS: u32 = 3;
const I6_MEASURED_ITERATIONS: u32 = 7;
const I6_REQUIRED_IMPROVEMENT_BASIS_POINTS: u128 = 500;
const I6_MAXIMUM_REGRESSION_BASIS_POINTS: u128 = 300;
const I6_MAXIMUM_COMPILE_REGRESSION_BASIS_POINTS: u128 = 300;
const I7_WARMUP_ITERATIONS: u32 = 3;
const I7_MEASURED_ITERATIONS: u32 = 7;
const I7_REQUIRED_IMPROVEMENT_BASIS_POINTS: u128 = 1_000;
const I7_REQUIRED_IMPROVEMENT_NS: u128 = 2_000_000;
const I7_MAXIMUM_REGRESSION_BASIS_POINTS: u128 = 300;
const I7_MAXIMUM_REGRESSION_NS: u128 = 1_000_000;
const I7_MAXIMUM_COMPILE_REGRESSION_NS: u128 = 25_000_000;
const I7_MAXIMUM_PREFILTER_BYTES: usize = 8 * 1024 * 1024;
const I7_CANDIDATE_FIXED_ALLOWANCE_BYTES: usize = 64 * 1024;
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
        Some("harness-run") => harness_run_command(&mut arguments).map(CommandOutput::HarnessRun),
        Some("cohort-report") => {
            cohort_report_command(&mut arguments).map(CommandOutput::CohortReport)
        }
        Some("compare-tripwire") => {
            comparison_paths(&mut arguments, compare_tripwire_files).map(CommandOutput::Comparison)
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
            comparison_paths(&mut arguments, compare_i6_files).map(CommandOutput::I6Comparison)
        }
        Some("i7-scan") => i7_scan_command(&mut arguments).map(CommandOutput::I7Scan),
        Some("compare-i7") => {
            comparison_paths(&mut arguments, compare_i7_files).map(CommandOutput::I7Comparison)
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
        Some("compare-wuthering-tripwire") => {
            comparison_paths(&mut arguments, compare_wuthering_tripwire_files)
                .map(CommandOutput::ScaleComparison)
        }
        Some("compare-i7-wuthering") => {
            comparison_paths(&mut arguments, compare_i7_wuthering_files)
                .map(CommandOutput::I7ScaleComparison)
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

fn i7_scan_command(arguments: &mut impl Iterator<Item = String>) -> Result<I7ScanReceipt, String> {
    let scenario = arguments.next().ok_or_else(usage)?;
    let pattern_count = parse_positive_usize("pattern count", arguments.next())?;
    let corpus_bytes = parse_positive_usize("corpus bytes", arguments.next())?;
    let cache_scrub_bytes = parse_positive_usize("cache scrub bytes", arguments.next())?;
    if arguments.next().is_some() {
        return Err(usage());
    }
    i7_scan(&scenario, pattern_count, corpus_bytes, cache_scrub_bytes)
}

fn harness_run_command(
    arguments: &mut impl Iterator<Item = String>,
) -> Result<HarnessRunReceipt, String> {
    let patterns = arguments.next().ok_or_else(usage)?;
    let corpus = arguments.next().ok_or_else(usage)?;
    let repeats = parse_positive_usize("repeat count", arguments.next())?;
    let warmups = parse_positive_usize("warm-up count", arguments.next())?;
    let mode = HarnessMode::parse(&arguments.next().ok_or_else(usage)?)?;
    if arguments.next().is_some() {
        return Err(usage());
    }
    harness_run(
        Path::new(&patterns),
        Path::new(&corpus),
        repeats,
        warmups,
        mode,
    )
}

fn cohort_report_command(
    arguments: &mut impl Iterator<Item = String>,
) -> Result<CohortReportReceipt, String> {
    let patterns = arguments.next().ok_or_else(usage)?;
    if arguments.next().is_some() {
        return Err(usage());
    }
    cohort_report(Path::new(&patterns))
}

fn comparison_paths<T>(
    arguments: &mut impl Iterator<Item = String>,
    compare: impl FnOnce(&Path, &Path) -> Result<T, String>,
) -> Result<T, String> {
    let baseline = arguments.next().ok_or_else(usage)?;
    let candidate = arguments.next().ok_or_else(usage)?;
    if arguments.next().is_some() {
        return Err(usage());
    }
    compare(Path::new(&baseline), Path::new(&candidate))
}

fn usage() -> String {
    "usage: rustmatch-bench <literal-smoke|literal-tripwire|harness-run PATTERNS.tsv CORPUS REPEATS WARMUPS MODE|cohort-report PATTERNS.tsv|compare-tripwire BASE.json CANDIDATE.json|i6-scan SCENARIO PATTERN_COUNT CORPUS_BYTES|compare-i6 BASE.json CANDIDATE.json|i7-scan SCENARIO PATTERN_COUNT CORPUS_BYTES CACHE_SCRUB_BYTES|compare-i7 BASE.json CANDIDATE.json|wuthering-scan PATTERNS.txt CORPUS.txt PATTERN_COUNT CORPUS_BYTES CACHE_SCRUB_BYTES|compare-wuthering-tripwire BASE.json CANDIDATE.json|compare-i7-wuthering BASE.json CANDIDATE.json|render-table OUTPUT.html RECEIPT.json...>".to_owned()
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

fn harness_run(
    pattern_path: &Path,
    corpus_path: &Path,
    repeats: usize,
    warmups: usize,
    mode: HarnessMode,
) -> Result<HarnessRunReceipt, String> {
    let input_started = Instant::now();
    let pattern_bytes = fs::read(pattern_path).map_err(|error| {
        format!(
            "could not read harness patterns {}: {error}",
            pattern_path.display()
        )
    })?;
    let corpus_bytes = fs::read(corpus_path).map_err(|error| {
        format!(
            "could not read harness corpus {}: {error}",
            corpus_path.display()
        )
    })?;
    let fixture = HarnessFixture::from_bytes(&pattern_bytes, &corpus_bytes)?;
    let input_prepare_ns = nanos(input_started.elapsed());

    let prepare_started = Instant::now();
    let matcher = build_harness_matcher(&fixture.patterns, mode)?;
    let prepare_ns = nanos(prepare_started.elapsed());

    let measurements = measure_harness_scans(
        warmups,
        repeats,
        || {
            let (summary, diagnostics) =
                rust_event_summary_with_diagnostics(&matcher, &fixture.input)?;
            Ok(((summary, diagnostics), summary.count))
        },
        || harness_event_count(&matcher, &fixture.input),
    )?;
    let (expected, diagnostics) = measurements.evidence;
    let expected_count = expected.count;
    let event_digest = expected.digest();
    let warmup_ns = measurements.warmup_ns;
    let scan_ns = measurements.scan_ns;
    let median_scan_ns = median(&mut scan_ns.clone());
    if median_scan_ns == 0 {
        return Err("harness median scan time must be positive".to_owned());
    }
    let throughput_bytes = u32::try_from(fixture.corpus_bytes)
        .map_err(|_| "harness corpus exceeds the 4 GiB throughput-reporting limit".to_owned())?;
    let throughput_nanos = u64::try_from(median_scan_ns)
        .map_err(|_| "harness scan duration exceeds the u64 nanosecond range".to_owned())?;
    let throughput_mbit_per_second = f64::from(throughput_bytes) * 8.0
        / Duration::from_nanos(throughput_nanos).as_secs_f64()
        / 1_000_000.0;

    Ok(HarnessRunReceipt {
        schema_version: 1,
        runner_version: "rustmatch-harness-v2",
        engine: "rustmatch-rust",
        revision: benchmark_revision(),
        rust_version: env::var("RUSTMATCH_RUST_VERSION")
            .unwrap_or_else(|_| "local-unidentified".to_owned()),
        mode: mode.label(),
        requested_worker_count: diagnostics.requested_worker_count(),
        partition_count: diagnostics.partition_count(),
        expression_count: fixture.patterns.len(),
        corpus_bytes: fixture.corpus_bytes,
        input_units: fixture.input.as_units().len(),
        input_prepare_ns,
        prepare_ns,
        warmup_ns,
        scan_ns,
        median_scan_ns,
        throughput_mbit_per_second,
        matches_per_iteration: expected_count,
        event_digest,
        diagnostics: diagnostics.into(),
        correctness: "pass",
    })
}

#[derive(Debug)]
struct HarnessMeasurements<T> {
    evidence: T,
    warmup_ns: Vec<u128>,
    scan_ns: Vec<u128>,
}

fn measure_harness_scans<T>(
    warmups: usize,
    repeats: usize,
    first_scan: impl FnOnce() -> Result<(T, usize), String>,
    mut subsequent_scan: impl FnMut() -> Result<usize, String>,
) -> Result<HarnessMeasurements<T>, String> {
    if repeats == 0 {
        return Err("harness repeat count must be greater than zero".to_owned());
    }

    let first_started = Instant::now();
    let (evidence, expected_count) = first_scan()?;
    let first_scan_ns = nanos(first_started.elapsed());

    let mut warmup_ns = Vec::with_capacity(warmups);
    let mut scan_ns = Vec::with_capacity(repeats);
    if warmups == 0 {
        scan_ns.push(first_scan_ns);
    } else {
        warmup_ns.push(first_scan_ns);
    }

    while warmup_ns.len() < warmups {
        let started = Instant::now();
        let count = subsequent_scan()?;
        warmup_ns.push(nanos(started.elapsed()));
        validate_harness_count("warm-up", count, expected_count)?;
    }

    while scan_ns.len() < repeats {
        let started = Instant::now();
        let count = subsequent_scan()?;
        scan_ns.push(nanos(started.elapsed()));
        validate_harness_count("measurement", count, expected_count)?;
    }

    Ok(HarnessMeasurements {
        evidence,
        warmup_ns,
        scan_ns,
    })
}

fn validate_harness_count(
    phase: &str,
    actual_count: usize,
    expected_count: usize,
) -> Result<(), String> {
    if actual_count == expected_count {
        Ok(())
    } else {
        Err(format!(
            "harness {phase} produced {actual_count} events; expected {expected_count}"
        ))
    }
}

fn build_harness_matcher(
    patterns: &[HarnessPattern],
    mode: HarnessMode,
) -> Result<Matcher, String> {
    let mut builder = MatcherBuilder::new();
    match mode {
        HarnessMode::Nfa => {
            builder
                .state_cache_budget(0)
                .prefilter_enabled(false)
                .literal_prefilter_enabled(false);
        }
        HarnessMode::Single => {}
        HarnessMode::Parallel(worker_count) => {
            builder.worker_count(worker_count);
        }
    }
    for pattern in patterns {
        builder
            .add(pattern.id, &pattern.expression)
            .map_err(|error| format!("harness pattern {} was rejected: {error}", pattern.id))?;
    }
    builder
        .build()
        .map_err(|error| format!("harness matcher build failed: {error}"))
}

fn harness_event_count(matcher: &Matcher, input: &Utf16Text) -> Result<usize, String> {
    let mut count = 0_usize;
    matcher
        .scan(input, |_| count += 1)
        .map_err(|error| format!("harness scan failed: {error}"))?;
    Ok(count)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum HarnessMode {
    Nfa,
    Single,
    Parallel(usize),
}

impl HarnessMode {
    fn parse(source: &str) -> Result<Self, String> {
        match source {
            "nfa" => Ok(Self::Nfa),
            "single" => Ok(Self::Single),
            _ => {
                let worker_count = source.parse::<usize>().map_err(|error| {
                    format!(
                        "invalid harness mode {source:?}; expected nfa, single, or a positive worker count: {error}"
                    )
                })?;
                if worker_count == 0 {
                    Err("harness worker count must be greater than zero".to_owned())
                } else {
                    Ok(Self::Parallel(worker_count))
                }
            }
        }
    }

    fn label(self) -> String {
        match self {
            Self::Nfa => "nfa".to_owned(),
            Self::Single => "single".to_owned(),
            Self::Parallel(worker_count) => worker_count.to_string(),
        }
    }
}

#[derive(Debug)]
struct HarnessPattern {
    id: PatternId,
    expression: String,
}

#[derive(Debug)]
struct HarnessFixture {
    patterns: Vec<HarnessPattern>,
    input: Utf16Text,
    corpus_bytes: usize,
}

impl HarnessFixture {
    fn from_bytes(pattern_bytes: &[u8], corpus_bytes: &[u8]) -> Result<Self, String> {
        if !corpus_bytes.is_ascii() {
            return Err("harness corpus must contain only ASCII bytes".to_owned());
        }
        let patterns = parse_pattern_rows(pattern_bytes, "harness", true)?;
        let corpus_text = std::str::from_utf8(corpus_bytes)
            .map_err(|error| format!("harness corpus is not valid ASCII: {error}"))?;
        Ok(Self {
            patterns,
            input: Utf16Text::from(corpus_text),
            corpus_bytes: corpus_bytes.len(),
        })
    }
}

fn parse_pattern_rows(
    pattern_bytes: &[u8],
    description: &str,
    require_ascii: bool,
) -> Result<Vec<HarnessPattern>, String> {
    if require_ascii && !pattern_bytes.is_ascii() {
        return Err(format!(
            "{description} pattern file must contain only ASCII bytes"
        ));
    }
    let pattern_text = std::str::from_utf8(pattern_bytes).map_err(|error| {
        let encoding = if require_ascii { "ASCII" } else { "UTF-8" };
        format!("{description} pattern file is not valid {encoding}: {error}")
    })?;
    let mut ids = BTreeSet::new();
    let mut patterns = Vec::new();
    for (line_index, source_line) in pattern_text.lines().enumerate() {
        let line = source_line.strip_suffix('\r').unwrap_or(source_line);
        let (id_source, expression) = line.split_once('\t').ok_or_else(|| {
            format!(
                "{description} pattern row {} has no tab separator",
                line_index + 1
            )
        })?;
        if expression.is_empty() || expression.contains('\t') {
            return Err(format!(
                "{description} pattern row {} must contain one non-empty expression",
                line_index + 1
            ));
        }
        let id_value = id_source.parse::<u32>().map_err(|error| {
            format!(
                "invalid {description} pattern ID {id_source:?} on row {}: {error}",
                line_index + 1
            )
        })?;
        if id_value == 0 {
            return Err(format!(
                "{description} pattern ID on row {} must be positive",
                line_index + 1
            ));
        }
        if !ids.insert(id_value) {
            return Err(format!("duplicate {description} pattern ID {id_value}"));
        }
        patterns.push(HarnessPattern {
            id: PatternId::new(id_value),
            expression: expression.to_owned(),
        });
    }
    if patterns.is_empty() {
        return Err(format!(
            "{description} pattern file must contain at least one row"
        ));
    }
    Ok(patterns)
}

fn cohort_report(pattern_path: &Path) -> Result<CohortReportReceipt, String> {
    let pattern_bytes = fs::read(pattern_path).map_err(|error| {
        format!(
            "could not read cohort-report patterns {}: {error}",
            pattern_path.display()
        )
    })?;
    cohort_report_from_bytes(&pattern_bytes)
}

fn cohort_report_from_bytes(pattern_bytes: &[u8]) -> Result<CohortReportReceipt, String> {
    let patterns = parse_pattern_rows(pattern_bytes, "cohort-report", false)?;
    let mut builder = MatcherBuilder::new();
    for pattern in &patterns {
        builder
            .add(pattern.id, &pattern.expression)
            .map_err(|error| {
                format!("cohort-report pattern {} was rejected: {error}", pattern.id)
            })?;
    }
    let diagnostics = builder.cohort_diagnostics();
    if patterns.len() != diagnostics.patterns().len() {
        return Err("cohort-report diagnostics lost a registered pattern".to_owned());
    }

    let pattern_receipts = patterns
        .iter()
        .zip(diagnostics.patterns())
        .map(|(pattern, facts)| pattern_diagnostic_receipt(pattern, facts))
        .collect::<Result<Vec<_>, _>>()?;
    let mut cohorts = Vec::with_capacity(diagnostics.cohort_count());
    if !diagnostics.assertion_free_pattern_ids().is_empty() {
        cohorts.push(CohortSummaryReceipt {
            name: "assertion-free".to_owned(),
            proof: "normalized-hir-contains-no-assertion-v1".to_owned(),
            pattern_ids: diagnostics
                .assertion_free_pattern_ids()
                .iter()
                .map(|pattern_id| pattern_id.get())
                .collect(),
        });
    }
    if !diagnostics.assertion_bearing_pattern_ids().is_empty() {
        cohorts.push(CohortSummaryReceipt {
            name: "assertion-bearing".to_owned(),
            proof: "normalized-hir-contains-assertion-v1".to_owned(),
            pattern_ids: diagnostics
                .assertion_bearing_pattern_ids()
                .iter()
                .map(|pattern_id| pattern_id.get())
                .collect(),
        });
    }

    Ok(CohortReportReceipt {
        schema_version: 1,
        classifier_version: "h43-c1-v1".to_owned(),
        proof_model_version: "normalized-hir-conservative-v1".to_owned(),
        pattern_source_digest: byte_digest(pattern_bytes),
        pattern_count: pattern_receipts.len(),
        cohort_count: cohorts.len(),
        proof_provenance: ProofProvenanceReceipt::v1(),
        cohorts,
        patterns: pattern_receipts,
    })
}

fn pattern_diagnostic_receipt(
    pattern: &HarnessPattern,
    facts: &PatternDiagnostics,
) -> Result<PatternDiagnosticReceipt, String> {
    if pattern.id != facts.pattern_id() {
        return Err(format!(
            "cohort-report registration {} was paired with diagnostic {}",
            pattern.id,
            facts.pattern_id()
        ));
    }
    let maximum_consumed = match (facts.maximum_consumed(), facts.has_unbounded_maximum()) {
        (Some(units), false) => ConsumedMaximumReceipt {
            status: "finite".to_owned(),
            units: Some(units),
        },
        (None, true) => ConsumedMaximumReceipt {
            status: "unbounded".to_owned(),
            units: None,
        },
        (None, false) => ConsumedMaximumReceipt {
            status: "impossible".to_owned(),
            units: None,
        },
        (Some(_), true) => {
            return Err(format!(
                "cohort-report pattern {} has contradictory maximum-width facts",
                pattern.id
            ));
        }
    };
    Ok(PatternDiagnosticReceipt {
        pattern_id: facts.pattern_id().get(),
        registration_ordinal: facts.registration_ordinal(),
        expression_digest: byte_digest(pattern.expression.as_bytes()),
        cohort_candidate: facts.cohort().to_owned(),
        cohort_proof: if facts.uses_assertions() {
            "normalized-hir-contains-assertion-v1".to_owned()
        } else {
            "normalized-hir-contains-no-assertion-v1".to_owned()
        },
        assertion_kinds: assertion_kinds(facts),
        facts: PatternFactsReceipt {
            nullable: facts.nullable(),
            minimum_consumed_units: facts.minimum_consumed(),
            maximum_consumed,
            ascii_only: facts.is_ascii_only(),
            proven_exact_literal_units: facts.proven_exact_literal_units(),
            necessary_prefix_units: facts.necessary_prefix_units(),
            filterable_prefix: facts.has_filterable_prefix(),
            hir_nodes: facts.hir_nodes(),
        },
    })
}

fn assertion_kinds(facts: &PatternDiagnostics) -> Vec<String> {
    let mut kinds = Vec::with_capacity(4);
    if facts.uses_line_start() {
        kinds.push("line-start".to_owned());
    }
    if facts.uses_line_end() {
        kinds.push("line-end".to_owned());
    }
    if facts.uses_word_boundary() {
        kinds.push("word-boundary".to_owned());
    }
    if facts.uses_non_word_boundary() {
        kinds.push("non-word-boundary".to_owned());
    }
    kinds
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

    let (mut actual, diagnostics) = rust_events_with_diagnostics(&matcher, &input)?;
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
        cache_diagnostics: Some(diagnostics.into()),
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
        baseline_cache_diagnostics: baseline.cache_diagnostics.clone(),
        candidate_cache_diagnostics: candidate.cache_diagnostics.clone(),
        status: "pass",
    })
}

fn i7_scan(
    scenario: &str,
    pattern_count: usize,
    corpus_target_bytes: usize,
    cache_scrub_bytes: usize,
) -> Result<I7ScanReceipt, String> {
    let fixture = CampaignFixture::new(scenario, pattern_count, corpus_target_bytes)?;
    let contract = I7ScenarioContract::for_scenario(scenario)?;
    let input = Utf16Text::from(fixture.corpus.as_str());
    let mut expected = fixture.expected_events;
    expected.sort_unstable();
    let expected_summary = event_summary(&expected);

    black_box(build_rustmatch(&fixture.patterns)?);
    let median_compile_ns = measure(I7_MEASURED_ITERATIONS, || {
        build_rustmatch(&fixture.patterns)
    })?;
    let matcher = build_rustmatch(&fixture.patterns)?;
    let (mut actual, diagnostics) = rust_events_with_diagnostics(&matcher, &input)?;
    actual.sort_unstable();
    if actual != expected {
        return Err(format!(
            "I7 correctness mismatch for {scenario}: expected {} events, got {}",
            expected.len(),
            actual.len()
        ));
    }

    let mut scrubber = CacheScrubber::new(cache_scrub_bytes)?;
    for _ in 0..I7_WARMUP_ITERATIONS {
        black_box(scrubber.scrub());
        let actual = black_box(rust_event_summary(&matcher, &input)?);
        if actual != expected_summary {
            return Err(format!(
                "I7 warm-up produced inconsistent events for {scenario}"
            ));
        }
    }
    let mut samples = Vec::with_capacity(I7_MEASURED_ITERATIONS as usize);
    for _ in 0..I7_MEASURED_ITERATIONS {
        black_box(scrubber.scrub());
        let started = Instant::now();
        let actual = black_box(rust_event_summary(&matcher, &input)?);
        let elapsed = nanos(started.elapsed());
        if actual != expected_summary {
            return Err(format!(
                "I7 measurement produced inconsistent events for {scenario}"
            ));
        }
        samples.push(elapsed);
    }

    Ok(I7ScanReceipt {
        schema_version: 1,
        evidence_id: "I7-P1".to_owned(),
        benchmark: "safe-prefilter-campaign-v1".to_owned(),
        claim: "optimization-admission".to_owned(),
        scenario: scenario.to_owned(),
        gate: contract.gate.to_owned(),
        expected_path: contract.expected_path.to_owned(),
        prefilter_mode: benchmark_prefilter_mode()?.to_owned(),
        revision: benchmark_revision(),
        runner: benchmark_runner(),
        profile: "release".to_owned(),
        pattern_count: fixture.patterns.len(),
        corpus_bytes: fixture.corpus.len(),
        input_units: input.as_units().len(),
        cache_scrub_bytes: scrubber.bytes(),
        cache_scrub_digest: format!("fnv1a64:{:016x}", scrubber.digest()),
        event_count: expected.len(),
        event_digest: event_digest(&expected),
        warmup_iterations: I7_WARMUP_ITERATIONS,
        measured_iterations: I7_MEASURED_ITERATIONS,
        median_compile_ns,
        median_scan_ns: median(&mut samples),
        cache_diagnostics: diagnostics.into(),
        correctness: "pass".to_owned(),
    })
}

fn compare_i7_files(
    baseline_path: &Path,
    candidate_path: &Path,
) -> Result<I7ComparisonReceipt, String> {
    let baseline: I7ScanReceipt = read_receipt("I7 disabled", baseline_path)?;
    let candidate: I7ScanReceipt = read_receipt("I7 enabled", candidate_path)?;
    compare_i7(&baseline, &candidate)
}

fn compare_i7(
    baseline: &I7ScanReceipt,
    candidate: &I7ScanReceipt,
) -> Result<I7ComparisonReceipt, String> {
    baseline.validate("disabled")?;
    candidate.validate("enabled")?;
    if baseline.fixture_identity() != candidate.fixture_identity() {
        return Err("I7 disabled and enabled fixture identities differ".to_owned());
    }
    if baseline.revision != candidate.revision || baseline.runner != candidate.runner {
        return Err("I7 disabled and enabled runs must use one revision and runner".to_owned());
    }
    if baseline.prefilter_mode != "off" || candidate.prefilter_mode != "on" {
        return Err("I7 comparison requires prefilter off followed by prefilter on".to_owned());
    }
    if baseline.cache_diagnostics.prefilter_path != "all-starts"
        || baseline.cache_diagnostics.prefilter_bypass != "disabled"
    {
        return Err("I7 disabled receipt did not preserve the all-start baseline".to_owned());
    }
    candidate.validate_path()?;
    validate_i7_resources(candidate)?;

    let improvement_ns = baseline
        .median_scan_ns
        .saturating_sub(candidate.median_scan_ns);
    let improvement_basis_points =
        relative_decrease_basis_points(baseline.median_scan_ns, candidate.median_scan_ns);
    let regression_ns = candidate
        .median_scan_ns
        .saturating_sub(baseline.median_scan_ns);
    let slowdown_basis_points =
        relative_increase_basis_points(baseline.median_scan_ns, candidate.median_scan_ns);
    let compile_regression_ns = candidate
        .median_compile_ns
        .saturating_sub(baseline.median_compile_ns);

    match baseline.gate.as_str() {
        "positive-10-percent-and-2-ms"
            if improvement_basis_points < I7_REQUIRED_IMPROVEMENT_BASIS_POINTS
                || improvement_ns < I7_REQUIRED_IMPROVEMENT_NS =>
        {
            return Err(format!(
                "I7 positive gate failed for {}: baseline={}ns candidate={}ns improvement={}ns ({}.{:02}%)",
                baseline.scenario,
                baseline.median_scan_ns,
                candidate.median_scan_ns,
                improvement_ns,
                improvement_basis_points / 100,
                improvement_basis_points % 100
            ));
        }
        "guard-3-percent-and-1-ms"
            if slowdown_basis_points > I7_MAXIMUM_REGRESSION_BASIS_POINTS
                && regression_ns > I7_MAXIMUM_REGRESSION_NS =>
        {
            return Err(format!(
                "I7 guard failed for {}: baseline={}ns candidate={}ns regression={}ns ({}.{:02}%)",
                baseline.scenario,
                baseline.median_scan_ns,
                candidate.median_scan_ns,
                regression_ns,
                slowdown_basis_points / 100,
                slowdown_basis_points % 100
            ));
        }
        "positive-10-percent-and-2-ms" | "guard-3-percent-and-1-ms" => {}
        gate => return Err(format!("unknown I7 gate {gate:?}")),
    }
    if compile_regression_ns > I7_MAXIMUM_COMPILE_REGRESSION_NS {
        return Err(format!(
            "I7 compile guard failed for {}: baseline={}ns candidate={}ns regression={}ns",
            baseline.scenario,
            baseline.median_compile_ns,
            candidate.median_compile_ns,
            compile_regression_ns
        ));
    }

    Ok(I7ComparisonReceipt {
        schema_version: 1,
        evidence_id: "I7-P1",
        comparison: "safe-prefilter-campaign-v1",
        scenario: baseline.scenario.clone(),
        gate: baseline.gate.clone(),
        revision: baseline.revision.clone(),
        runner: baseline.runner.clone(),
        pattern_count: baseline.pattern_count,
        corpus_bytes: baseline.corpus_bytes,
        input_units: baseline.input_units,
        event_count: baseline.event_count,
        event_digest: baseline.event_digest.clone(),
        baseline_median_compile_ns: baseline.median_compile_ns,
        candidate_median_compile_ns: candidate.median_compile_ns,
        baseline_median_scan_ns: baseline.median_scan_ns,
        candidate_median_scan_ns: candidate.median_scan_ns,
        improvement_ns,
        improvement_basis_points,
        regression_ns,
        slowdown_basis_points,
        compile_regression_ns,
        baseline_diagnostics: baseline.cache_diagnostics.clone(),
        candidate_diagnostics: candidate.cache_diagnostics.clone(),
        status: "pass",
    })
}

fn validate_i7_resources(candidate: &I7ScanReceipt) -> Result<(), String> {
    let diagnostics = &candidate.cache_diagnostics;
    if diagnostics.prefilter_retained_bytes > I7_MAXIMUM_PREFILTER_BYTES {
        return Err(format!(
            "I7 retained prefilter exceeds 8 MiB: {} bytes",
            diagnostics.prefilter_retained_bytes
        ));
    }
    let candidate_limit = candidate
        .input_units
        .div_ceil(8)
        .saturating_add(I7_CANDIDATE_FIXED_ALLOWANCE_BYTES);
    if diagnostics.prefilter_candidate_bytes > candidate_limit {
        return Err(format!(
            "I7 candidate storage exceeds one bit per corpus byte plus 64 KiB: {} > {}",
            diagnostics.prefilter_candidate_bytes, candidate_limit
        ));
    }
    if diagnostics.prefilter_path != "literal-prefilter"
        && diagnostics.prefilter_candidate_bytes != 0
    {
        return Err("I7 non-literal path retained a full candidate bitmap".to_owned());
    }
    Ok(())
}

fn benchmark_prefilter_mode() -> Result<&'static str, String> {
    match env::var("RUSTMATCH_BENCH_PREFILTER") {
        Ok(value) => match value.as_str() {
            "on" | "true" | "1" => Ok("on"),
            "off" | "false" | "0" => Ok("off"),
            _ => Err(format!(
                "invalid prefilter control {value:?}; expected on or off"
            )),
        },
        Err(env::VarError::NotPresent) => Ok("on"),
        Err(error) => Err(format!("could not read prefilter control: {error}")),
    }
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
    let measured_at_utc = measurement_timestamp_utc()?;
    let warmup_iterations =
        benchmark_iteration_count("RUSTMATCH_BENCH_WARMUP_ITERATIONS", SCALE_WARMUP_ITERATIONS)?;
    let measured_iterations = benchmark_iteration_count(
        "RUSTMATCH_BENCH_MEASURED_ITERATIONS",
        SCALE_MEASURED_ITERATIONS,
    )?;
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
    let median_compile_ns = measure(measured_iterations, || build_rustmatch(&patterns))?;
    let matcher = build_rustmatch(&patterns)?;
    let (expected, diagnostics) = rust_event_summary_with_diagnostics(&matcher, &input)?;
    let mut scrubber = CacheScrubber::new(cache_scrub_bytes)?;

    for _ in 0..warmup_iterations {
        black_box(scrubber.scrub());
        let actual = black_box(rust_event_summary(&matcher, &input)?);
        if actual != expected {
            return Err("Wuthering warm-up produced inconsistent events".to_owned());
        }
    }

    let mut samples = Vec::with_capacity(measured_iterations as usize);
    for _ in 0..measured_iterations {
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
        measured_at_utc: Some(measured_at_utc),
        profile: "release".to_owned(),
        pattern_source: pattern_source.display().to_string(),
        pattern_source_digest: byte_digest(&pattern_bytes),
        corpus_source: corpus_source.display().to_string(),
        corpus_source_digest: byte_digest(&corpus_bytes),
        pattern_count: patterns.len(),
        worker_count: diagnostics.requested_worker_count(),
        corpus_bytes: corpus.len(),
        input_units: input.as_units().len(),
        cache_scrub_bytes: scrubber.bytes(),
        cache_scrub_digest: format!("fnv1a64:{:016x}", scrubber.digest()),
        event_count: expected.count,
        event_digest: expected.digest(),
        warmup_iterations,
        measured_iterations,
        median_compile_ns,
        median_scan_ns: median(&mut samples),
        cache_diagnostics: Some(diagnostics.into()),
        correctness: "pass".to_owned(),
    })
}

fn benchmark_iteration_count(variable: &str, default: u32) -> Result<u32, String> {
    match env::var(variable) {
        Ok(source) => {
            let value = source
                .parse::<u32>()
                .map_err(|error| format!("invalid {variable} value {source:?}: {error}"))?;
            if value == 0 {
                Err(format!("{variable} must be greater than zero"))
            } else {
                Ok(value)
            }
        }
        Err(env::VarError::NotPresent) => Ok(default),
        Err(error) => Err(format!("could not read {variable}: {error}")),
    }
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

fn rust_event_summary_with_diagnostics(
    matcher: &Matcher,
    input: &Utf16Text,
) -> Result<(EventSummary, ScanDiagnostics), String> {
    let mut summary = EventSummary::default();
    let diagnostics = matcher
        .scan_with_diagnostics(input, |event| {
            summary.add(&Event {
                pattern_id: event.pattern_id().get(),
                start: event.span().start(),
                end: event.span().end(),
            });
        })
        .map_err(|error| format!("rustmatch diagnostic scale scan failed: {error}"))?;
    Ok((summary, diagnostics))
}

fn render_scale_report(output: &Path, receipt_paths: &[&Path]) -> Result<ReportReceipt, String> {
    let mut receipts = receipt_paths
        .iter()
        .map(|path| read_receipt::<ScaleScanReceipt>("scale", path))
        .collect::<Result<Vec<_>, _>>()?;
    for receipt in &receipts {
        receipt.validate()?;
    }
    receipts.sort_by_key(|receipt| {
        (
            receipt.corpus_bytes,
            receipt.pattern_count,
            receipt.worker_count,
        )
    });

    let (measurement_window, timestamp_coverage) = report_measurement_window(&receipts);

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
        let partition_count = receipt.cache_diagnostics.as_ref().map_or_else(
            || "n/a".to_owned(),
            |value| value.partition_count.to_string(),
        );
        let spawned_workers = receipt.cache_diagnostics.as_ref().map_or_else(
            || "n/a".to_owned(),
            |value| value.spawned_workers.to_string(),
        );
        let database_mib = diagnostic_mib(receipt, |value| value.database_retained_bytes);
        let prefilter_mib = diagnostic_mib(receipt, |value| value.prefilter_retained_bytes);
        let candidate_mib = diagnostic_mib(receipt, |value| value.prefilter_candidate_bytes);
        let cache_table_mib = diagnostic_mib(receipt, |value| value.cache_table_bytes);
        let buffered_mib = diagnostic_mib(receipt, |value| value.buffered_event_bytes);
        write!(
            rows,
            "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{partition_count}</td><td>{spawned_workers}</td><td>{database_mib} MiB</td><td>{prefilter_mib} MiB</td><td>{candidate_mib} MiB</td><td>{cache_table_mib} MiB</td><td>{buffered_mib} MiB</td><td>{corpus_mib} MiB</td><td>{scrub_mib} MiB</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{}</td><td>{compile_ms} ms</td><td>{scan_ms} ms</td><td>{throughput} MiB/s</td><td><span class=\"pass\">pass</span></td></tr>",
            html_escape(&receipt.revision),
            receipt
                .measured_at_utc
                .as_deref()
                .map_or_else(|| "not recorded".to_owned(), html_escape),
            html_escape(&receipt.runner),
            receipt.pattern_count,
            receipt.worker_count,
            receipt.event_count,
            receipt
                .cache_diagnostics
                .as_ref()
                .map_or_else(|| "n/a".to_owned(), |value| value.cache_states.to_string()),
            receipt
                .cache_diagnostics
                .as_ref()
                .map_or_else(|| "n/a".to_owned(), |value| value.fallback_transitions.to_string()),
            receipt
                .cache_diagnostics
                .as_ref()
                .map_or_else(|| "n/a".to_owned(), |value| html_escape(&value.prefilter_path)),
            receipt.cache_diagnostics.as_ref().map_or_else(
                || "n/a".to_owned(),
                |value| value.prefilter_starts_scanned.to_string()
            ),
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
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><title>rustmatch benchmark results</title><style>:root{{--ink:#13211c;--muted:#5c6862;--paper:#f5f1e8;--green:#0f654b;--line:#c9c2b4}}*{{box-sizing:border-box}}body{{margin:0;background:linear-gradient(145deg,#dce8dd,#f5f1e8 42%);color:var(--ink);font:16px/1.45 ui-monospace,SFMono-Regular,Menlo,monospace}}main{{max-width:2250px;margin:4rem auto;padding:0 2rem}}h1{{font:700 clamp(2rem,5vw,4.8rem)/.95 Georgia,serif;max-width:12ch;margin:0 0 1rem}}.lede{{max-width:72ch;color:var(--muted);margin-bottom:1rem}}.measurement-window{{display:inline-block;margin:0 0 2rem;padding:.55rem .8rem;border:1px solid rgba(15,101,75,.25);background:rgba(255,255,255,.62)}}.card{{background:rgba(255,255,255,.82);border:1px solid rgba(19,33,28,.15);box-shadow:0 24px 70px rgba(20,40,30,.12);overflow:auto}}table{{border-collapse:collapse;width:100%;min-width:2250px}}th,td{{padding:.9rem 1rem;border-bottom:1px solid var(--line);text-align:right;white-space:nowrap}}th{{position:sticky;top:0;background:#173c31;color:white;font-size:.78rem;letter-spacing:.04em;text-transform:uppercase}}th:first-child,th:nth-child(2),th:nth-child(3),td:first-child,td:nth-child(2),td:nth-child(3){{text-align:left}}tbody tr:hover{{background:#eef6ed}}.pass{{background:#ccebd8;color:#084b35;padding:.2rem .55rem;border-radius:999px;font-weight:700}}.note{{color:var(--muted);margin-top:1.4rem;font-size:.86rem}}code{{overflow-wrap:anywhere}}@media(max-width:700px){{main{{margin:2rem auto;padding:0 1rem}}}}</style></head><body><main><h1>rustmatch benchmark receipts</h1><p class=\"lede\">Exploratory Wuthering Heights literal-pattern scale results. Each timed scan follows a full cache-scrub pass; compilation and scanning are reported separately. These are engineering receipts, not cross-engine claims.</p><p class=\"measurement-window\"><strong>Measurement window (UTC):</strong> {measurement_window}{timestamp_coverage}</p><div class=\"card\"><table><thead><tr><th>Revision</th><th>Measured UTC</th><th>Runner</th><th>Patterns</th><th>Requested workers</th><th>Partitions</th><th>Spawned workers</th><th>Database</th><th>Prefilter</th><th>Candidate bitmap</th><th>Cache tables</th><th>Buffered events</th><th>Corpus</th><th>Cache scrub</th><th>Events</th><th>Cache states</th><th>Fallbacks</th><th>Start path</th><th>Starts verified</th><th>Compile median</th><th>Scan median</th><th>Throughput</th><th>Correctness</th></tr></thead><tbody>{rows}</tbody></table></div><p class=\"note\">{provenance}<br>Each receipt records its own UTC measurement timestamp, warm-up count, and measured-iteration count. Literal patterns are the lexicographically first distinct non-empty lines from the source list, escaped before compilation. Memory columns report explicitly accounted bytes, not allocator overhead.</p></main></body></html>"
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

fn report_measurement_window(receipts: &[ScaleScanReceipt]) -> (String, String) {
    let mut measurement_times = receipts
        .iter()
        .filter_map(|receipt| receipt.measured_at_utc.as_deref())
        .collect::<Vec<_>>();
    measurement_times.sort_unstable();
    let window = match (measurement_times.first(), measurement_times.last()) {
        (Some(first), Some(last)) if first == last => html_escape(first),
        (Some(first), Some(last)) => {
            format!("{} to {}", html_escape(first), html_escape(last))
        }
        _ => "not recorded in retained receipts".to_owned(),
    };
    let coverage = if measurement_times.len() == receipts.len() {
        String::new()
    } else {
        format!(
            " (timestamps recorded for {} of {} receipts)",
            measurement_times.len(),
            receipts.len()
        )
    };
    (window, coverage)
}

fn format_tenths(value: u128) -> String {
    format!("{}.{:01}", value / 10, value % 10)
}

fn diagnostic_mib(
    receipt: &ScaleScanReceipt,
    bytes: impl FnOnce(&CacheDiagnosticsReceipt) -> usize,
) -> String {
    receipt
        .cache_diagnostics
        .as_ref()
        .map_or_else(|| "n/a".to_owned(), |value| format_mib_tenths(bytes(value)))
}

fn format_mib_tenths(bytes: usize) -> String {
    format_tenths(bytes as u128 * 10 / (1024 * 1024))
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

fn measurement_timestamp_utc() -> Result<String, String> {
    let seconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock predates the Unix epoch: {error}"))?
        .as_secs();
    format_unix_timestamp_utc(seconds)
}

fn format_unix_timestamp_utc(seconds: u64) -> Result<String, String> {
    let days = i64::try_from(seconds / 86_400)
        .map_err(|_| "UTC timestamp exceeds the supported year range".to_owned())?;
    let seconds_of_day = seconds % 86_400;
    let hour = seconds_of_day / 3_600;
    let minute = seconds_of_day % 3_600 / 60;
    let second = seconds_of_day % 60;
    let (year, month, day) = civil_date_from_unix_days(days);
    Ok(format!(
        "{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"
    ))
}

fn civil_date_from_unix_days(days: i64) -> (i64, i64, i64) {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month, day)
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

fn compare_wuthering_tripwire_files(
    baseline_path: &Path,
    candidate_path: &Path,
) -> Result<ScaleComparisonReceipt, String> {
    let baseline: ScaleScanReceipt = read_receipt("Wuthering baseline", baseline_path)?;
    let candidate: ScaleScanReceipt = read_receipt("Wuthering candidate", candidate_path)?;
    compare_wuthering_tripwire(&baseline, &candidate)
}

fn compare_wuthering_tripwire(
    baseline: &ScaleScanReceipt,
    candidate: &ScaleScanReceipt,
) -> Result<ScaleComparisonReceipt, String> {
    baseline.validate()?;
    candidate.validate()?;
    if baseline.fixture_identity() != candidate.fixture_identity() {
        return Err("C2 baseline and candidate fixture identities differ".to_owned());
    }
    if baseline.runner != candidate.runner {
        return Err("C2 baseline and candidate were not measured on the same runner".to_owned());
    }
    if baseline.worker_count != 1 || candidate.worker_count != 1 {
        return Err("C2 must compare the default one-worker path".to_owned());
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
        slowdown_basis_points > WUTHERING_TRIPWIRE_RELATIVE_LIMIT_PERCENT.saturating_mul(100);
    let absolute_limit_exceeded = regression_ns >= WUTHERING_TRIPWIRE_ABSOLUTE_LIMIT_NS;
    if relative_limit_exceeded && absolute_limit_exceeded {
        return Err(format!(
            "C2 Wuthering severe-regression tripwire fired: baseline={}ns candidate={}ns regression={}ns slowdown={}.{:02}%",
            baseline.median_scan_ns,
            candidate.median_scan_ns,
            regression_ns,
            slowdown_basis_points / 100,
            slowdown_basis_points % 100
        ));
    }

    Ok(ScaleComparisonReceipt {
        schema_version: 1,
        evidence_id: "C2",
        comparison: "wuthering-tripwire-v1",
        claim: "coarse-severe-regression-signal-only",
        baseline_revision: baseline.revision.clone(),
        candidate_revision: candidate.revision.clone(),
        baseline_measured_at_utc: baseline.measured_at_utc.clone(),
        candidate_measured_at_utc: candidate.measured_at_utc.clone(),
        runner: baseline.runner.clone(),
        pattern_count: baseline.pattern_count,
        corpus_bytes: baseline.corpus_bytes,
        event_count: baseline.event_count,
        event_digest: baseline.event_digest.clone(),
        baseline_median_scan_ns: baseline.median_scan_ns,
        candidate_median_scan_ns: candidate.median_scan_ns,
        regression_ns,
        slowdown_basis_points,
        relative_limit_percent: WUTHERING_TRIPWIRE_RELATIVE_LIMIT_PERCENT,
        absolute_limit_ns: WUTHERING_TRIPWIRE_ABSOLUTE_LIMIT_NS,
        status: "pass",
    })
}

fn compare_i7_wuthering_files(
    baseline_path: &Path,
    candidate_path: &Path,
) -> Result<I7ScaleComparisonReceipt, String> {
    let baseline: ScaleScanReceipt = read_receipt("I7 Wuthering disabled", baseline_path)?;
    let candidate: ScaleScanReceipt = read_receipt("I7 Wuthering enabled", candidate_path)?;
    compare_i7_wuthering(&baseline, &candidate)
}

fn validate_i7_wuthering_diagnostics<'a>(
    baseline: &ScaleScanReceipt,
    candidate: &'a ScaleScanReceipt,
) -> Result<&'a CacheDiagnosticsReceipt, String> {
    let baseline_diagnostics = baseline
        .cache_diagnostics
        .as_ref()
        .ok_or_else(|| "I7 Wuthering disabled receipt lacks diagnostics".to_owned())?;
    let candidate_diagnostics = candidate
        .cache_diagnostics
        .as_ref()
        .ok_or_else(|| "I7 Wuthering enabled receipt lacks diagnostics".to_owned())?;
    if baseline_diagnostics.prefilter_path != "all-starts"
        || baseline_diagnostics.prefilter_bypass != "disabled"
    {
        return Err("I7 Wuthering disabled receipt did not use all starts".to_owned());
    }
    if candidate_diagnostics.prefilter_path != "literal-prefilter"
        || candidate_diagnostics.prefilter_bypass != "none"
    {
        return Err("I7 Wuthering enabled receipt did not use the literal prefilter".to_owned());
    }
    if candidate_diagnostics.prefilter_retained_bytes > I7_MAXIMUM_PREFILTER_BYTES {
        return Err("I7 Wuthering retained prefilter exceeds 8 MiB".to_owned());
    }
    let candidate_limit = candidate
        .input_units
        .div_ceil(8)
        .saturating_add(I7_CANDIDATE_FIXED_ALLOWANCE_BYTES);
    if candidate_diagnostics.prefilter_candidate_bytes > candidate_limit {
        return Err("I7 Wuthering candidate bitmap exceeds the frozen memory bound".to_owned());
    }
    Ok(candidate_diagnostics)
}

fn compare_i7_wuthering(
    baseline: &ScaleScanReceipt,
    candidate: &ScaleScanReceipt,
) -> Result<I7ScaleComparisonReceipt, String> {
    baseline.validate()?;
    candidate.validate()?;
    if baseline.fixture_identity() != candidate.fixture_identity() {
        return Err("I7 Wuthering disabled and enabled fixture identities differ".to_owned());
    }
    if baseline.revision != candidate.revision || baseline.runner != candidate.runner {
        return Err(
            "I7 Wuthering disabled and enabled runs must use one revision and runner".to_owned(),
        );
    }
    if baseline.worker_count != 1 || candidate.worker_count != 1 {
        return Err("I7 Wuthering evidence must use one worker".to_owned());
    }
    if baseline.warmup_iterations < I7_WARMUP_ITERATIONS
        || baseline.measured_iterations < I7_MEASURED_ITERATIONS
    {
        return Err(
            "I7 Wuthering admission requires at least 3 warmups and 7 measurements".to_owned(),
        );
    }
    if !matches!(baseline.pattern_count, 1_000 | 5_000 | 10_000) {
        return Err("I7 Wuthering pattern count is outside the frozen campaign".to_owned());
    }

    let candidate_diagnostics = validate_i7_wuthering_diagnostics(baseline, candidate)?;
    let improvement_ns = baseline
        .median_scan_ns
        .saturating_sub(candidate.median_scan_ns);
    let improvement_basis_points =
        relative_decrease_basis_points(baseline.median_scan_ns, candidate.median_scan_ns);
    if improvement_basis_points < I7_REQUIRED_IMPROVEMENT_BASIS_POINTS
        || improvement_ns < I7_REQUIRED_IMPROVEMENT_NS
    {
        return Err(format!(
            "I7 Wuthering positive gate failed for {} patterns: baseline={}ns candidate={}ns improvement={}ns ({}.{:02}%)",
            baseline.pattern_count,
            baseline.median_scan_ns,
            candidate.median_scan_ns,
            improvement_ns,
            improvement_basis_points / 100,
            improvement_basis_points % 100
        ));
    }
    let compile_regression_ns = candidate
        .median_compile_ns
        .saturating_sub(baseline.median_compile_ns);
    if compile_regression_ns > I7_MAXIMUM_COMPILE_REGRESSION_NS {
        return Err(format!(
            "I7 Wuthering compile guard failed for {} patterns: regression={}ns",
            baseline.pattern_count, compile_regression_ns
        ));
    }

    Ok(I7ScaleComparisonReceipt {
        schema_version: 1,
        evidence_id: "I7-B1",
        comparison: "wuthering-safe-prefilter-v1",
        revision: baseline.revision.clone(),
        runner: baseline.runner.clone(),
        baseline_measured_at_utc: baseline.measured_at_utc.clone(),
        candidate_measured_at_utc: candidate.measured_at_utc.clone(),
        pattern_count: baseline.pattern_count,
        corpus_bytes: baseline.corpus_bytes,
        input_units: baseline.input_units,
        event_count: baseline.event_count,
        event_digest: baseline.event_digest.clone(),
        baseline_median_compile_ns: baseline.median_compile_ns,
        candidate_median_compile_ns: candidate.median_compile_ns,
        baseline_median_scan_ns: baseline.median_scan_ns,
        candidate_median_scan_ns: candidate.median_scan_ns,
        improvement_ns,
        improvement_basis_points,
        compile_regression_ns,
        candidate_diagnostics: candidate_diagnostics.clone(),
        status: "pass",
    })
}

fn build_rustmatch(patterns: &[String]) -> Result<Matcher, String> {
    let mut builder = MatcherBuilder::new();
    if let Ok(source) = env::var("RUSTMATCH_BENCH_WORKERS") {
        let worker_count = source
            .parse::<usize>()
            .map_err(|error| format!("invalid worker count {source:?}: {error}"))?;
        builder.worker_count(worker_count);
    }
    if let Ok(source) = env::var("RUSTMATCH_BENCH_STATE_CACHE_BUDGET") {
        let budget = source
            .parse::<usize>()
            .map_err(|error| format!("invalid state-cache budget {source:?}: {error}"))?;
        builder.state_cache_budget(budget);
    }
    if let Ok(source) = env::var("RUSTMATCH_BENCH_PREFILTER") {
        let enabled = match source.as_str() {
            "on" | "true" | "1" => true,
            "off" | "false" | "0" => false,
            _ => {
                return Err(format!(
                    "invalid prefilter control {source:?}; expected on or off"
                ));
            }
        };
        builder.prefilter_enabled(enabled);
    }
    if let Ok(source) = env::var("RUSTMATCH_BENCH_LITERAL_PREFILTER") {
        let enabled = match source.as_str() {
            "on" | "true" | "1" => true,
            "off" | "false" | "0" => false,
            _ => {
                return Err(format!(
                    "invalid literal-prefilter control {source:?}; expected on or off"
                ));
            }
        };
        builder.literal_prefilter_enabled(enabled);
    }
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

fn rust_events_with_diagnostics(
    matcher: &Matcher,
    input: &Utf16Text,
) -> Result<(Vec<Event>, ScanDiagnostics), String> {
    let mut events = Vec::new();
    let diagnostics = matcher
        .scan_with_diagnostics(input, |event| {
            events.push(Event {
                pattern_id: event.pattern_id().get(),
                start: event.span().start(),
                end: event.span().end(),
            });
        })
        .map_err(|error| format!("rustmatch diagnostic event scan failed: {error}"))?;
    Ok((events, diagnostics))
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

struct I7ScenarioContract {
    gate: &'static str,
    expected_path: &'static str,
}

impl I7ScenarioContract {
    fn for_scenario(scenario: &str) -> Result<Self, String> {
        match scenario {
            "literal-sparse" | "mixed-sparse" => Ok(Self {
                gate: "positive-10-percent-and-2-ms",
                expected_path: "literal-prefilter",
            }),
            "literal-dense" => Ok(Self {
                gate: "guard-3-percent-and-1-ms",
                expected_path: "literal-or-density-bypass",
            }),
            "assertion-bypass" => Ok(Self {
                gate: "guard-3-percent-and-1-ms",
                expected_path: "assertion-bypass",
            }),
            "mixed-unfilterable" => Ok(Self {
                gate: "guard-3-percent-and-1-ms",
                expected_path: "start-table",
            }),
            "short-literal" => Ok(Self {
                gate: "guard-3-percent-and-1-ms",
                expected_path: "below-size-bypass",
            }),
            _ => Err(format!("scenario {scenario:?} has no frozen I7 contract")),
        }
    }
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
            "mixed-unfilterable" => Self::mixed_unfilterable(pattern_count, corpus_target_bytes),
            "short-literal" => Self::short_literal(pattern_count, corpus_target_bytes),
            _ => Err(format!(
                "unknown campaign scenario {scenario:?}; expected literal-sparse, literal-dense, mixed-sparse, assertion-bypass, mixed-unfilterable, or short-literal"
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

    fn mixed_unfilterable(
        pattern_count: usize,
        corpus_target_bytes: usize,
    ) -> Result<Self, String> {
        let pairs: Vec<_> = (0..pattern_count)
            .map(|index| {
                if index % 2 == 0 {
                    (format!("(?i)case{index:04}"), format!("CASE{index:04}"))
                } else {
                    (format!("plain{index:04}"), format!("plain{index:04}"))
                }
            })
            .collect();
        Self::sparse_from_pairs(pairs, corpus_target_bytes, "non-regression-3-percent")
    }

    fn short_literal(pattern_count: usize, corpus_target_bytes: usize) -> Result<Self, String> {
        if pattern_count == 0 {
            return Err("short-literal requires at least one pattern".to_owned());
        }
        let patterns: Vec<_> = (0..pattern_count)
            .map(|index| format!("short{index:04}"))
            .collect();
        let mut corpus = vec![b'x'; corpus_target_bytes];
        let retained_matches = pattern_count.min(16);
        let mut expected_events = Vec::with_capacity(retained_matches);
        for (pattern_index, pattern) in patterns.iter().take(retained_matches).enumerate() {
            let pattern = pattern.as_bytes();
            let start = corpus_target_bytes * (pattern_index + 1) / (retained_matches + 1);
            if start + pattern.len() > corpus.len() {
                return Err("short-literal corpus is too small for its retained matches".to_owned());
            }
            corpus[start..start + pattern.len()].copy_from_slice(pattern);
            expected_events.push(event_for_literal(
                pattern_index,
                start,
                start + pattern.len(),
            )?);
        }
        Ok(Self {
            patterns,
            corpus: String::from_utf8(corpus)
                .map_err(|error| format!("short-literal corpus must be UTF-8: {error}"))?,
            expected_events,
            gate: "non-regression-3-percent",
        })
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

fn event_summary(events: &[Event]) -> EventSummary {
    let mut summary = EventSummary::default();
    for event in events {
        summary.add(event);
    }
    summary
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
    HarnessRun(HarnessRunReceipt),
    CohortReport(CohortReportReceipt),
    Comparison(ComparisonReceipt),
    I6Scan(I6ScanReceipt),
    I6Comparison(I6ComparisonReceipt),
    I7Scan(I7ScanReceipt),
    I7Comparison(I7ComparisonReceipt),
    I7ScaleComparison(I7ScaleComparisonReceipt),
    ScaleScan(ScaleScanReceipt),
    ScaleComparison(ScaleComparisonReceipt),
    Report(ReportReceipt),
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CohortReportReceipt {
    schema_version: u32,
    classifier_version: String,
    proof_model_version: String,
    pattern_source_digest: String,
    pattern_count: usize,
    cohort_count: usize,
    proof_provenance: ProofProvenanceReceipt,
    cohorts: Vec<CohortSummaryReceipt>,
    patterns: Vec<PatternDiagnosticReceipt>,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ProofProvenanceReceipt {
    cohort_membership: String,
    assertions: String,
    nullability: String,
    consumed_width: String,
    ascii_domain: String,
    necessary_prefix: String,
    hir_complexity: String,
}

impl ProofProvenanceReceipt {
    fn v1() -> Self {
        Self {
            cohort_membership: "exhaustive-normalized-hir-assertion-walk-v1".to_owned(),
            assertions: "exhaustive-normalized-hir-assertion-walk-v1".to_owned(),
            nullability: "normalized-hir-nullability-v1".to_owned(),
            consumed_width: "conservative-normalized-hir-width-v1".to_owned(),
            ascii_domain: "exhaustive-normalized-hir-consumed-domain-v1".to_owned(),
            necessary_prefix: "conservative-normalized-hir-required-prefix-v1".to_owned(),
            hir_complexity: "saturating-normalized-hir-node-count-v1".to_owned(),
        }
    }
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CohortSummaryReceipt {
    name: String,
    proof: String,
    pattern_ids: Vec<u32>,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PatternDiagnosticReceipt {
    pattern_id: u32,
    registration_ordinal: usize,
    expression_digest: String,
    cohort_candidate: String,
    cohort_proof: String,
    assertion_kinds: Vec<String>,
    facts: PatternFactsReceipt,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct PatternFactsReceipt {
    nullable: bool,
    minimum_consumed_units: Option<usize>,
    maximum_consumed: ConsumedMaximumReceipt,
    ascii_only: bool,
    proven_exact_literal_units: Option<usize>,
    necessary_prefix_units: usize,
    filterable_prefix: bool,
    hir_nodes: usize,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ConsumedMaximumReceipt {
    status: String,
    units: Option<usize>,
}

#[derive(Debug, Serialize)]
struct HarnessRunReceipt {
    schema_version: u32,
    runner_version: &'static str,
    engine: &'static str,
    revision: String,
    rust_version: String,
    mode: String,
    requested_worker_count: usize,
    partition_count: usize,
    expression_count: usize,
    corpus_bytes: usize,
    input_units: usize,
    input_prepare_ns: u128,
    prepare_ns: u128,
    warmup_ns: Vec<u128>,
    scan_ns: Vec<u128>,
    median_scan_ns: u128,
    throughput_mbit_per_second: f64,
    matches_per_iteration: usize,
    event_digest: String,
    diagnostics: CacheDiagnosticsReceipt,
    correctness: &'static str,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cache_diagnostics: Option<CacheDiagnosticsReceipt>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    baseline_cache_diagnostics: Option<CacheDiagnosticsReceipt>,
    #[serde(skip_serializing_if = "Option::is_none")]
    candidate_cache_diagnostics: Option<CacheDiagnosticsReceipt>,
    status: &'static str,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct I7ScanReceipt {
    schema_version: u32,
    evidence_id: String,
    benchmark: String,
    claim: String,
    scenario: String,
    gate: String,
    expected_path: String,
    prefilter_mode: String,
    revision: String,
    runner: String,
    profile: String,
    pattern_count: usize,
    corpus_bytes: usize,
    input_units: usize,
    cache_scrub_bytes: usize,
    cache_scrub_digest: String,
    event_count: usize,
    event_digest: String,
    warmup_iterations: u32,
    measured_iterations: u32,
    median_compile_ns: u128,
    median_scan_ns: u128,
    cache_diagnostics: CacheDiagnosticsReceipt,
    correctness: String,
}

type I7FixtureIdentity<'a> = (
    &'a str,
    &'a str,
    &'a str,
    usize,
    usize,
    usize,
    usize,
    &'a str,
    usize,
    &'a str,
    u32,
    u32,
);

impl I7ScanReceipt {
    fn validate(&self, description: &str) -> Result<(), String> {
        if self.schema_version != 1
            || self.evidence_id != "I7-P1"
            || self.benchmark != "safe-prefilter-campaign-v1"
            || self.claim != "optimization-admission"
            || self.profile != "release"
            || self.correctness != "pass"
            || self.revision.is_empty()
            || self.runner.is_empty()
            || self.pattern_count == 0
            || self.corpus_bytes == 0
            || self.input_units == 0
            || self.cache_scrub_bytes == 0
            || self.cache_scrub_digest.is_empty()
            || self.warmup_iterations < I7_WARMUP_ITERATIONS
            || self.measured_iterations < I7_MEASURED_ITERATIONS
            || self.median_compile_ns == 0
            || self.median_scan_ns == 0
        {
            return Err(format!("invalid {description} I7 receipt metadata"));
        }
        Ok(())
    }

    fn validate_path(&self) -> Result<(), String> {
        let diagnostics = &self.cache_diagnostics;
        let valid = match self.expected_path.as_str() {
            "literal-prefilter" => {
                diagnostics.prefilter_path == "literal-prefilter"
                    && diagnostics.prefilter_bypass == "none"
            }
            "literal-or-density-bypass" => {
                diagnostics.prefilter_path == "literal-prefilter"
                    || (diagnostics.prefilter_path == "start-table"
                        && diagnostics.prefilter_bypass == "dense-sample")
            }
            "assertion-bypass" => {
                diagnostics.prefilter_path == "all-starts"
                    && diagnostics.prefilter_bypass == "assertions"
            }
            "start-table" => diagnostics.prefilter_path == "start-table",
            "below-size-bypass" => {
                diagnostics.prefilter_path == "start-table"
                    && diagnostics.prefilter_bypass == "input-size"
            }
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            Err(format!(
                "I7 scenario {} expected path {}, observed {} ({})",
                self.scenario,
                self.expected_path,
                diagnostics.prefilter_path,
                diagnostics.prefilter_bypass
            ))
        }
    }

    fn fixture_identity(&self) -> I7FixtureIdentity<'_> {
        (
            &self.scenario,
            &self.gate,
            &self.expected_path,
            self.pattern_count,
            self.corpus_bytes,
            self.input_units,
            self.cache_scrub_bytes,
            &self.cache_scrub_digest,
            self.event_count,
            &self.event_digest,
            self.warmup_iterations,
            self.measured_iterations,
        )
    }
}

#[derive(Debug, Serialize)]
struct I7ComparisonReceipt {
    schema_version: u32,
    evidence_id: &'static str,
    comparison: &'static str,
    scenario: String,
    gate: String,
    revision: String,
    runner: String,
    pattern_count: usize,
    corpus_bytes: usize,
    input_units: usize,
    event_count: usize,
    event_digest: String,
    baseline_median_compile_ns: u128,
    candidate_median_compile_ns: u128,
    baseline_median_scan_ns: u128,
    candidate_median_scan_ns: u128,
    improvement_ns: u128,
    improvement_basis_points: u128,
    regression_ns: u128,
    slowdown_basis_points: u128,
    compile_regression_ns: u128,
    baseline_diagnostics: CacheDiagnosticsReceipt,
    candidate_diagnostics: CacheDiagnosticsReceipt,
    status: &'static str,
}

#[derive(Debug, Serialize)]
struct I7ScaleComparisonReceipt {
    schema_version: u32,
    evidence_id: &'static str,
    comparison: &'static str,
    revision: String,
    runner: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    baseline_measured_at_utc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    candidate_measured_at_utc: Option<String>,
    pattern_count: usize,
    corpus_bytes: usize,
    input_units: usize,
    event_count: usize,
    event_digest: String,
    baseline_median_compile_ns: u128,
    candidate_median_compile_ns: u128,
    baseline_median_scan_ns: u128,
    candidate_median_scan_ns: u128,
    improvement_ns: u128,
    improvement_basis_points: u128,
    compile_regression_ns: u128,
    candidate_diagnostics: CacheDiagnosticsReceipt,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    measured_at_utc: Option<String>,
    profile: String,
    pattern_source: String,
    pattern_source_digest: String,
    corpus_source: String,
    corpus_source_digest: String,
    pattern_count: usize,
    #[serde(default = "default_worker_count")]
    worker_count: usize,
    corpus_bytes: usize,
    #[serde(default)]
    input_units: usize,
    cache_scrub_bytes: usize,
    cache_scrub_digest: String,
    event_count: usize,
    event_digest: String,
    warmup_iterations: u32,
    measured_iterations: u32,
    median_compile_ns: u128,
    median_scan_ns: u128,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cache_diagnostics: Option<CacheDiagnosticsReceipt>,
    correctness: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
struct CacheDiagnosticsReceipt {
    requested_worker_count: usize,
    partition_count: usize,
    spawned_workers: usize,
    database_retained_bytes: usize,
    cache_budget: usize,
    cache_states: usize,
    cache_hits: u64,
    cache_misses: u64,
    fallback_transitions: u64,
    cache_table_bytes: usize,
    assertion_bypasses: u64,
    prefilter_path: String,
    prefilter_bypass: String,
    prefilter_retained_bytes: usize,
    prefilter_candidate_bytes: usize,
    prefilter_admissions: u64,
    prefilter_candidate_starts: usize,
    prefilter_starts_scanned: usize,
    prefilter_starts_skipped: usize,
    buffered_events: usize,
    buffered_event_bytes: usize,
}

impl From<ScanDiagnostics> for CacheDiagnosticsReceipt {
    fn from(value: ScanDiagnostics) -> Self {
        Self {
            requested_worker_count: value.requested_worker_count(),
            partition_count: value.partition_count(),
            spawned_workers: value.spawned_workers(),
            database_retained_bytes: value.database_retained_bytes(),
            cache_budget: value.cache_budget(),
            cache_states: value.cache_states(),
            cache_hits: value.cache_hits(),
            cache_misses: value.cache_misses(),
            fallback_transitions: value.fallback_transitions(),
            cache_table_bytes: value.cache_table_bytes(),
            assertion_bypasses: value.assertion_bypasses(),
            prefilter_path: value.prefilter_path().to_owned(),
            prefilter_bypass: value.prefilter_bypass().to_owned(),
            prefilter_retained_bytes: value.prefilter_retained_bytes(),
            prefilter_candidate_bytes: value.prefilter_candidate_bytes(),
            prefilter_admissions: value.prefilter_admissions(),
            prefilter_candidate_starts: value.prefilter_candidate_starts(),
            prefilter_starts_scanned: value.prefilter_starts_scanned(),
            prefilter_starts_skipped: value.prefilter_starts_skipped(),
            buffered_events: value.buffered_events(),
            buffered_event_bytes: value.buffered_event_bytes(),
        }
    }
}

const fn default_worker_count() -> usize {
    1
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
            || self.worker_count == 0
            || self.corpus_bytes == 0
            || self.cache_scrub_bytes == 0
            || self.median_compile_ns == 0
            || self.median_scan_ns == 0
        {
            return Err("invalid Wuthering scale receipt metadata".to_owned());
        }
        Ok(())
    }

    fn fixture_identity(&self) -> (usize, usize, usize, &str, &str, &str, usize, &str, u32, u32) {
        (
            self.pattern_count,
            self.corpus_bytes,
            self.cache_scrub_bytes,
            &self.pattern_source_digest,
            &self.corpus_source_digest,
            &self.cache_scrub_digest,
            self.event_count,
            &self.event_digest,
            self.warmup_iterations,
            self.measured_iterations,
        )
    }
}

#[derive(Debug, Serialize)]
struct ScaleComparisonReceipt {
    schema_version: u32,
    evidence_id: &'static str,
    comparison: &'static str,
    claim: &'static str,
    baseline_revision: String,
    candidate_revision: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    baseline_measured_at_utc: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    candidate_measured_at_utc: Option<String>,
    runner: String,
    pattern_count: usize,
    corpus_bytes: usize,
    event_count: usize,
    event_digest: String,
    baseline_median_scan_ns: u128,
    candidate_median_scan_ns: u128,
    regression_ns: u128,
    slowdown_basis_points: u128,
    relative_limit_percent: u128,
    absolute_limit_ns: u128,
    status: &'static str,
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
        CacheDiagnosticsReceipt, CacheScrubber, CampaignFixture, CohortReportReceipt, Event,
        HarnessFixture, HarnessMode, I6ScanReceipt, I7ScanReceipt, LiteralFixture,
        SMOKE_CORPUS_TARGET_BYTES, SMOKE_PATTERN_COUNT, ScaleScanReceipt, TripwireReceipt,
        build_harness_matcher, cohort_report_command, cohort_report_from_bytes, compare_i6,
        compare_i7, compare_i7_wuthering, compare_tripwire, compare_wuthering_tripwire,
        expand_corpus, format_unix_timestamp_utc, harness_event_count, measure_harness_scans,
        regex_events, render_scale_report, rust_event_summary_with_diagnostics,
        select_literal_patterns,
    };
    use regex::{Regex, RegexSet};
    use std::{cell::Cell, fs, process};

    const H43_C2_PATTERNS: &[u8] = include_bytes!("../fixtures/cohort/h43-c2-patterns.tsv");
    const H43_C2_EXPECTED: &str = include_str!("../fixtures/cohort/h43-c2-expected.json");

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
    fn harness_fixture_maps_ascii_bytes_to_equal_utf16_units() -> Result<(), String> {
        // Prepare
        let patterns = b"1\tcat\n2\tc.t\n";
        let corpus = b"cat cut";

        // Test
        let fixture = HarnessFixture::from_bytes(patterns, corpus)?;

        // Assert
        assert_eq!(fixture.patterns.len(), 2);
        assert_eq!(fixture.patterns[0].id.get(), 1);
        assert_eq!(fixture.patterns[1].expression, "c.t");
        assert_eq!(fixture.corpus_bytes, corpus.len());
        assert_eq!(fixture.input.as_units().len(), corpus.len());
        assert_eq!(fixture.input.as_units(), &[99, 97, 116, 32, 99, 117, 116]);
        Ok(())
    }

    #[test]
    fn harness_fixture_rejects_ambiguous_or_non_ascii_input() {
        // Prepare
        let duplicate_ids = b"1\tcat\n1\tdog\n";
        let zero_id = b"0\tcat\n";
        let extra_tab = b"1\tca\tt\n";
        let non_ascii_corpus = &[0xff];

        // Test
        let duplicate_result = HarnessFixture::from_bytes(duplicate_ids, b"cat dog");
        let zero_result = HarnessFixture::from_bytes(zero_id, b"cat");
        let tab_result = HarnessFixture::from_bytes(extra_tab, b"cat");
        let corpus_result = HarnessFixture::from_bytes(b"1\tcat\n", non_ascii_corpus);

        // Assert
        assert!(matches!(duplicate_result, Err(message) if message.contains("duplicate")));
        assert!(matches!(zero_result, Err(message) if message.contains("positive")));
        assert!(matches!(tab_result, Err(message) if message.contains("one non-empty")));
        assert!(matches!(corpus_result, Err(message) if message.contains("only ASCII")));
    }

    #[test]
    fn cohort_report_matches_versioned_golden_and_serializes_deterministically()
    -> Result<(), String> {
        // Prepare
        let expected: CohortReportReceipt = serde_json::from_str(H43_C2_EXPECTED)
            .map_err(|error| format!("invalid H43-C2 golden report: {error}"))?;

        // Test
        let first = cohort_report_from_bytes(H43_C2_PATTERNS)?;
        let second = cohort_report_from_bytes(H43_C2_PATTERNS)?;

        // Assert
        assert_eq!(first, expected);
        assert_eq!(second, expected);
        assert_eq!(
            serde_json::to_string(&first).map_err(|error| error.to_string())?,
            serde_json::to_string(&second).map_err(|error| error.to_string())?
        );
        Ok(())
    }

    #[test]
    fn cohort_report_boundary_mutations_change_only_proven_facts() -> Result<(), String> {
        // Prepare
        let patterns = concat!(
            "1\tabc\n",
            "2\t^abc\n",
            "3\tab.\n",
            "4\txxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\n",
            "5\txxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx\n",
            "6\t(?:ab){2,4}\n",
            "7\t(?:ab)+\n",
            "8\tab\n",
            "9\té\n",
            "10\t[a-z]+\n",
            "11\t()\n",
        );

        // Test
        let report = cohort_report_from_bytes(patterns.as_bytes())?;
        let facts = &report.patterns;

        // Assert
        assert_eq!(facts[0].cohort_candidate, "assertion-free");
        assert_eq!(facts[0].facts.proven_exact_literal_units, Some(3));
        assert!(facts[0].facts.filterable_prefix);
        assert_eq!(facts[1].cohort_candidate, "assertion-bearing");
        assert_eq!(facts[1].assertion_kinds, ["line-start"]);
        assert_eq!(facts[1].facts.proven_exact_literal_units, None);
        assert_eq!(facts[1].facts.necessary_prefix_units, 3);
        assert_eq!(facts[2].facts.necessary_prefix_units, 2);
        assert!(!facts[2].facts.filterable_prefix);
        assert_eq!(facts[3].facts.proven_exact_literal_units, Some(32));
        assert_eq!(facts[4].facts.proven_exact_literal_units, None);
        assert_eq!(facts[4].facts.necessary_prefix_units, 32);
        assert_eq!(facts[5].facts.maximum_consumed.status, "finite");
        assert_eq!(facts[5].facts.maximum_consumed.units, Some(8));
        assert_eq!(facts[6].facts.maximum_consumed.status, "unbounded");
        assert_eq!(facts[6].facts.maximum_consumed.units, None);
        assert!(!facts[7].facts.filterable_prefix);
        assert!(!facts[8].facts.ascii_only);
        assert!(facts[9].facts.ascii_only);
        assert_eq!(facts[9].facts.necessary_prefix_units, 0);
        assert_eq!(facts[10].facts.minimum_consumed_units, None);
        assert_eq!(facts[10].facts.maximum_consumed.status, "impossible");
        assert_eq!(facts[10].facts.maximum_consumed.units, None);
        Ok(())
    }

    #[test]
    fn cohort_report_schema_rejects_unknown_fields() -> Result<(), String> {
        // Prepare
        let report = cohort_report_from_bytes(b"1\tabc\n")?;
        let mut value = serde_json::to_value(report).map_err(|error| error.to_string())?;
        let object = value
            .as_object_mut()
            .ok_or_else(|| "cohort report did not serialize as an object".to_owned())?;
        object.insert(
            "unversioned_selector".to_owned(),
            serde_json::Value::Bool(true),
        );

        // Test
        let result = serde_json::from_value::<CohortReportReceipt>(value);

        // Assert
        assert!(matches!(result, Err(error) if error.to_string().contains("unknown field")));
        Ok(())
    }

    #[test]
    fn cohort_report_command_requires_exactly_one_pattern_path() -> Result<(), String> {
        // Prepare
        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/cohort/h43-c2-patterns.tsv");
        let fixture = fixture.to_string_lossy().into_owned();

        // Test
        let valid = cohort_report_command(&mut vec![fixture.clone()].into_iter())?;
        let missing = cohort_report_command(&mut Vec::<String>::new().into_iter());
        let extra = cohort_report_command(
            &mut vec![fixture, "unexpected-extra-argument".to_owned()].into_iter(),
        );

        // Assert
        assert_eq!(valid.schema_version, 1);
        assert!(matches!(missing, Err(message) if message.starts_with("usage:")));
        assert!(matches!(extra, Err(message) if message.starts_with("usage:")));
        Ok(())
    }

    #[test]
    fn harness_modes_preserve_events_and_report_actual_partitions() -> Result<(), String> {
        // Prepare
        let fixture = HarnessFixture::from_bytes(b"1\tcat\n2\tc.t\n", b"cat cut")?;
        let modes = [
            (HarnessMode::Nfa, 1),
            (HarnessMode::Single, 1),
            (HarnessMode::Parallel(2), 2),
        ];
        let mut reference = None;

        // Test
        for (mode, expected_partitions) in modes {
            let matcher = build_harness_matcher(&fixture.patterns, mode)?;
            let count = harness_event_count(&matcher, &fixture.input)?;
            let (summary, diagnostics) =
                rust_event_summary_with_diagnostics(&matcher, &fixture.input)?;
            let evidence = (summary.count, summary.digest());

            // Assert
            assert_eq!(count, 3);
            assert_eq!(diagnostics.partition_count(), expected_partitions);
            if let Some(expected) = &reference {
                assert_eq!(&evidence, expected);
            } else {
                reference = Some(evidence);
            }
        }
        Ok(())
    }

    #[test]
    fn harness_accounts_for_every_declared_scan() -> Result<(), String> {
        // Prepare
        let scan_calls = Cell::new(0_usize);

        // Test
        let measurements = measure_harness_scans(
            2,
            3,
            || {
                scan_calls.set(scan_calls.get() + 1);
                Ok(("first-scan-evidence", 7))
            },
            || {
                scan_calls.set(scan_calls.get() + 1);
                Ok(7)
            },
        )?;

        // Assert
        assert_eq!(scan_calls.get(), 5);
        assert_eq!(measurements.evidence, "first-scan-evidence");
        assert_eq!(measurements.warmup_ns.len(), 2);
        assert_eq!(measurements.scan_ns.len(), 3);
        Ok(())
    }

    #[test]
    fn harness_uses_first_scan_as_a_measurement_when_warmups_are_disabled() -> Result<(), String> {
        // Prepare
        let scan_calls = Cell::new(0_usize);

        // Test
        let measurements = measure_harness_scans(
            0,
            3,
            || {
                scan_calls.set(scan_calls.get() + 1);
                Ok(("first-scan-evidence", 7))
            },
            || {
                scan_calls.set(scan_calls.get() + 1);
                Ok(7)
            },
        )?;

        // Assert
        assert_eq!(scan_calls.get(), 3);
        assert!(measurements.warmup_ns.is_empty());
        assert_eq!(measurements.scan_ns.len(), 3);
        Ok(())
    }

    #[test]
    fn harness_mode_is_explicit_and_has_no_arbitrary_worker_cap() -> Result<(), String> {
        // Prepare
        let oversized_but_valid_count = "4096";

        // Test
        let nfa = HarnessMode::parse("nfa")?;
        let single = HarnessMode::parse("single")?;
        let parallel = HarnessMode::parse(oversized_but_valid_count)?;
        let zero = HarnessMode::parse("0");
        let unknown = HarnessMode::parse("automatic");

        // Assert
        assert_eq!(nfa, HarnessMode::Nfa);
        assert_eq!(single, HarnessMode::Single);
        assert_eq!(parallel, HarnessMode::Parallel(4096));
        assert!(matches!(zero, Err(message) if message.contains("greater than zero")));
        assert!(matches!(unknown, Err(message) if message.contains("expected nfa")));
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
    fn wuthering_tripwire_accepts_small_absolute_noise() -> Result<(), String> {
        // Prepare
        let baseline = scale_receipt(40_000_000, "base");
        let mut candidate = scale_receipt(85_000_000, "candidate");
        candidate.measured_at_utc = Some("2026-07-18T05:26:02Z".to_owned());

        // Test
        let comparison = compare_wuthering_tripwire(&baseline, &candidate)?;

        // Assert
        assert_eq!(comparison.status, "pass");
        assert_eq!(
            comparison.baseline_measured_at_utc.as_deref(),
            Some("2026-07-18T05:25:01Z")
        );
        assert_eq!(
            comparison.candidate_measured_at_utc.as_deref(),
            Some("2026-07-18T05:26:02Z")
        );
        Ok(())
    }

    #[test]
    fn wuthering_tripwire_rejects_a_large_absolute_and_relative_regression() {
        // Prepare
        let baseline = scale_receipt(50_000_000, "base");
        let candidate = scale_receipt(130_000_000, "candidate");

        // Test
        let result = compare_wuthering_tripwire(&baseline, &candidate);

        // Assert
        assert!(matches!(result, Err(message) if message.contains("tripwire fired")));
    }

    #[test]
    fn wuthering_tripwire_requires_identical_event_evidence() {
        // Prepare
        let baseline = scale_receipt(50_000_000, "base");
        let mut candidate = scale_receipt(55_000_000, "candidate");
        candidate.event_digest = "multiset64:different".to_owned();

        // Test
        let result = compare_wuthering_tripwire(&baseline, &candidate);

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
    fn utc_timestamp_format_handles_epoch_and_leap_day() -> Result<(), String> {
        // Prepare.
        let epoch = 0;
        let leap_day_2000 = 951_782_400;

        // Test.
        let epoch_timestamp = format_unix_timestamp_utc(epoch)?;
        let leap_day_timestamp = format_unix_timestamp_utc(leap_day_2000)?;

        // Assert.
        assert_eq!(epoch_timestamp, "1970-01-01T00:00:00Z");
        assert_eq!(leap_day_timestamp, "2000-02-29T00:00:00Z");
        Ok(())
    }

    #[test]
    fn historical_scale_receipt_without_timestamp_remains_readable() -> Result<(), String> {
        // Prepare.
        let mut encoded = serde_json::to_value(scale_receipt(50_000_000, "historical"))
            .map_err(|error| error.to_string())?;
        encoded
            .as_object_mut()
            .ok_or_else(|| "scale receipt was not an object".to_owned())?
            .remove("measured_at_utc");

        // Test.
        let decoded: ScaleScanReceipt =
            serde_json::from_value(encoded).map_err(|error| error.to_string())?;

        // Assert.
        assert_eq!(decoded.measured_at_utc, None);
        decoded.validate()
    }

    #[test]
    fn scale_report_displays_measurement_timestamp() -> Result<(), String> {
        // Prepare.
        let directory =
            std::env::temp_dir().join(format!("rustmatch-scale-timestamp-test-{}", process::id()));
        fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
        let earlier_receipt_path = directory.join("earlier-receipt.json");
        let later_receipt_path = directory.join("later-receipt.json");
        let report_path = directory.join("index.html");
        let mut later_receipt = scale_receipt(60_000_000, "revision");
        later_receipt.measured_at_utc = Some("2026-07-18T05:26:02Z".to_owned());
        fs::write(
            &earlier_receipt_path,
            serde_json::to_vec(&scale_receipt(50_000_000, "revision"))
                .map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;
        fs::write(
            &later_receipt_path,
            serde_json::to_vec(&later_receipt).map_err(|error| error.to_string())?,
        )
        .map_err(|error| error.to_string())?;

        // Test.
        render_scale_report(
            &report_path,
            &[earlier_receipt_path.as_path(), later_receipt_path.as_path()],
        )?;
        let html = fs::read_to_string(&report_path).map_err(|error| error.to_string())?;
        fs::remove_dir_all(&directory).map_err(|error| error.to_string())?;

        // Assert.
        assert!(html.contains("<th>Measured UTC</th>"));
        assert!(html.contains("2026-07-18T05:25:01Z"));
        assert!(html.contains(
            "<strong>Measurement window (UTC):</strong> 2026-07-18T05:25:01Z to \
             2026-07-18T05:26:02Z"
        ));
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
    fn every_frozen_i7_fixture_has_known_events() -> Result<(), String> {
        // Prepare
        let scenarios = [
            "literal-sparse",
            "literal-dense",
            "mixed-sparse",
            "assertion-bypass",
            "mixed-unfilterable",
            "short-literal",
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
    fn i7_comparison_requires_both_positive_thresholds() {
        // Prepare
        let baseline = i7_receipt(100_000_000, "off");
        let candidate = i7_receipt(91_000_000, "on");

        // Test
        let result = compare_i7(&baseline, &candidate);

        // Assert
        assert!(matches!(result, Err(message) if message.contains("positive gate failed")));
    }

    #[test]
    fn i7_comparison_accepts_a_measured_positive_result() -> Result<(), String> {
        // Prepare
        let baseline = i7_receipt(100_000_000, "off");
        let candidate = i7_receipt(80_000_000, "on");

        // Test
        let comparison = compare_i7(&baseline, &candidate)?;

        // Assert
        assert_eq!(comparison.status, "pass");
        assert_eq!(comparison.improvement_basis_points, 2_000);
        Ok(())
    }

    #[test]
    fn i7_wuthering_comparison_requires_the_full_measurement_protocol() {
        // Prepare
        let mut baseline = scale_receipt(100_000_000, "revision");
        let mut candidate = scale_receipt(80_000_000, "revision");
        baseline.corpus_bytes = 8 * 1024 * 1024;
        candidate.corpus_bytes = 8 * 1024 * 1024;
        baseline.input_units = 8 * 1024 * 1024;
        candidate.input_units = 8 * 1024 * 1024;
        baseline.cache_diagnostics = Some(prefilter_diagnostics("all-starts", "disabled"));
        candidate.cache_diagnostics = Some(prefilter_diagnostics("literal-prefilter", "none"));

        // Test
        let result = compare_i7_wuthering(&baseline, &candidate);

        // Assert
        assert!(matches!(result, Err(message) if message.contains("3 warmups and 7 measurements")));
    }

    #[test]
    fn i7_wuthering_comparison_accepts_exact_positive_evidence() -> Result<(), String> {
        // Prepare
        let mut baseline = scale_receipt(100_000_000, "revision");
        let mut candidate = scale_receipt(80_000_000, "revision");
        baseline.corpus_bytes = 8 * 1024 * 1024;
        candidate.corpus_bytes = 8 * 1024 * 1024;
        baseline.input_units = 8 * 1024 * 1024;
        candidate.input_units = 8 * 1024 * 1024;
        baseline.warmup_iterations = 3;
        baseline.measured_iterations = 7;
        candidate.warmup_iterations = 3;
        candidate.measured_iterations = 7;
        baseline.cache_diagnostics = Some(prefilter_diagnostics("all-starts", "disabled"));
        candidate.cache_diagnostics = Some(prefilter_diagnostics("literal-prefilter", "none"));

        // Test
        let comparison = compare_i7_wuthering(&baseline, &candidate)?;

        // Assert
        assert_eq!(comparison.status, "pass");
        assert_eq!(comparison.improvement_basis_points, 2_000);
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
            cache_diagnostics: None,
            correctness: "pass".to_owned(),
        }
    }

    fn i7_receipt(median_scan_ns: u128, mode: &str) -> I7ScanReceipt {
        let diagnostics = if mode == "off" {
            prefilter_diagnostics("all-starts", "disabled")
        } else {
            prefilter_diagnostics("literal-prefilter", "none")
        };
        I7ScanReceipt {
            schema_version: 1,
            evidence_id: "I7-P1".to_owned(),
            benchmark: "safe-prefilter-campaign-v1".to_owned(),
            claim: "optimization-admission".to_owned(),
            scenario: "literal-sparse".to_owned(),
            gate: "positive-10-percent-and-2-ms".to_owned(),
            expected_path: "literal-prefilter".to_owned(),
            prefilter_mode: mode.to_owned(),
            revision: "revision".to_owned(),
            runner: "runner".to_owned(),
            profile: "release".to_owned(),
            pattern_count: 1_000,
            corpus_bytes: 8 * 1024 * 1024,
            input_units: 8 * 1024 * 1024,
            cache_scrub_bytes: 256 * 1024 * 1024,
            cache_scrub_digest: "fnv1a64:scrub".to_owned(),
            event_count: 1_000,
            event_digest: "fnv1a64:events".to_owned(),
            warmup_iterations: 3,
            measured_iterations: 7,
            median_compile_ns: 5_000_000,
            median_scan_ns,
            cache_diagnostics: diagnostics,
            correctness: "pass".to_owned(),
        }
    }

    fn prefilter_diagnostics(path: &str, bypass: &str) -> CacheDiagnosticsReceipt {
        CacheDiagnosticsReceipt {
            prefilter_path: path.to_owned(),
            prefilter_bypass: bypass.to_owned(),
            prefilter_retained_bytes: 512 * 1024,
            prefilter_candidate_bytes: if path == "literal-prefilter" {
                64 * 1024
            } else {
                0
            },
            ..CacheDiagnosticsReceipt::default()
        }
    }

    fn scale_receipt(median_scan_ns: u128, revision: &str) -> ScaleScanReceipt {
        ScaleScanReceipt {
            schema_version: 1,
            evidence_id: "I6-B1".to_owned(),
            benchmark: "wuthering-literal-scale-v1".to_owned(),
            claim: "exploratory-scale-and-cache-pressure-evidence".to_owned(),
            revision: revision.to_owned(),
            runner: "runner".to_owned(),
            measured_at_utc: Some("2026-07-18T05:25:01Z".to_owned()),
            profile: "release".to_owned(),
            pattern_source: "patterns.txt".to_owned(),
            pattern_source_digest: "fnv1a64:patterns".to_owned(),
            corpus_source: "corpus.txt".to_owned(),
            corpus_source_digest: "fnv1a64:corpus".to_owned(),
            pattern_count: 5_000,
            worker_count: 1,
            corpus_bytes: 675_259,
            input_units: 675_259,
            cache_scrub_bytes: 64 * 1024 * 1024,
            cache_scrub_digest: "fnv1a64:scrub".to_owned(),
            event_count: 74_604,
            event_digest: "multiset64:receipt".to_owned(),
            warmup_iterations: 1,
            measured_iterations: 3,
            median_compile_ns: 5_000_000,
            median_scan_ns,
            cache_diagnostics: None,
            correctness: "pass".to_owned(),
        }
    }
}
