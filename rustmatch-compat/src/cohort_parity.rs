//! Deterministic H43-E1 ordinary-versus-cohort semantic evidence.

use std::collections::BTreeMap;

use rustmatch::{Error, Matcher, MatcherBuilder, PatternFlags, PatternId, Utf16Text};
use serde::{Deserialize, Serialize};

use super::{
    ASSERTION_FIXTURES, ASSERTION_JAVA_RESULTS, COMPOSITION_FIXTURES, COMPOSITION_JAVA_RESULTS,
    ExpectedResult, Fixture, LITERAL_FIXTURES, LITERAL_JAVA_RESULTS, OracleEvent,
    PREDICATE_FIXTURES, PREDICATE_JAVA_RESULTS, REPETITION_FIXTURES, REPETITION_JAVA_RESULTS,
    UTF16_FLAGS_FIXTURES, UTF16_FLAGS_JAVA_RESULTS, parse_jsonl,
};

const CAMPAIGN: &str = "h43-e1-cohort-semantic-parity-v1";
const GENERATOR_SEED: u32 = 0x43e1_2026;
const GENERATED_CASE_COUNT: usize = 96;
const WORKER_COUNTS: [usize; 6] = [1, 2, 3, 4, 8, 128];
const CACHE_BUDGETS: [usize; 3] = [0, 1, 8_192];

const FIXTURE_SETS: [FixtureSetInput; 6] = [
    FixtureSetInput {
        name: "ascii-literals-v1",
        tier: "ascii-literal-v1",
        fixtures: LITERAL_FIXTURES,
        java_results: LITERAL_JAVA_RESULTS,
    },
    FixtureSetInput {
        name: "ascii-predicates-v1",
        tier: "ascii-predicate-v1",
        fixtures: PREDICATE_FIXTURES,
        java_results: PREDICATE_JAVA_RESULTS,
    },
    FixtureSetInput {
        name: "ascii-composition-v1",
        tier: "ascii-composition-v1",
        fixtures: COMPOSITION_FIXTURES,
        java_results: COMPOSITION_JAVA_RESULTS,
    },
    FixtureSetInput {
        name: "ascii-repetition-v1",
        tier: "ascii-repetition-v1",
        fixtures: REPETITION_FIXTURES,
        java_results: REPETITION_JAVA_RESULTS,
    },
    FixtureSetInput {
        name: "utf16-flags-v1",
        tier: "utf16-flags-v1",
        fixtures: UTF16_FLAGS_FIXTURES,
        java_results: UTF16_FLAGS_JAVA_RESULTS,
    },
    FixtureSetInput {
        name: "assertions-v1",
        tier: "assertions-v1",
        fixtures: ASSERTION_FIXTURES,
        java_results: ASSERTION_JAVA_RESULTS,
    },
];

#[derive(Clone, Copy)]
struct FixtureSetInput {
    name: &'static str,
    tier: &'static str,
    fixtures: &'static str,
    java_results: &'static str,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CohortParityReceipt {
    schema_version: u32,
    evidence_id: String,
    campaign: String,
    generator_seed: String,
    generated_case_count: usize,
    worker_counts: Vec<usize>,
    cache_budgets: Vec<usize>,
    source_digests: Vec<SourceDigestReceipt>,
    java_fixture_families: Vec<FamilyReceipt>,
    adversarial_family: FamilyReceipt,
    generated_family: FamilyReceipt,
    explicit_build_error_checks: usize,
    total_cases: usize,
    total_comparisons: usize,
    total_reference_events: u64,
    aggregate_event_digest: String,
    status: String,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct SourceDigestReceipt {
    fixture_set: String,
    fixture_digest: String,
    java_result_digest: String,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct FamilyReceipt {
    family: String,
    cases: usize,
    matched_cases: usize,
    rejected_cases: usize,
    mixed_cohort_cases: usize,
    comparisons: usize,
    reference_events: u64,
    event_digest: String,
}

#[derive(Clone, Debug)]
struct PatternSpec {
    id: u32,
    source: String,
    flags: PatternFlags,
}

#[derive(Clone, Debug)]
struct ParityCase {
    id: String,
    patterns: Vec<PatternSpec>,
    input: Vec<u16>,
}

type Event = (u32, u64, u64);

pub(crate) fn verify() -> Result<CohortParityReceipt, String> {
    let mut source_digests = Vec::with_capacity(FIXTURE_SETS.len());
    let mut java_fixture_families = Vec::with_capacity(FIXTURE_SETS.len());
    for fixture_set in FIXTURE_SETS {
        source_digests.push(SourceDigestReceipt {
            fixture_set: fixture_set.name.to_owned(),
            fixture_digest: stable_source_digest(fixture_set.fixtures),
            java_result_digest: stable_source_digest(fixture_set.java_results),
        });
        java_fixture_families.push(verify_fixture_set(fixture_set)?);
    }

    let adversarial_family = verify_case_family("adversarial-v1", adversarial_cases())?;
    let generated_family = verify_case_family("generated-v1", generated_cases())?;
    let explicit_build_error_checks = verify_build_error_parity()?;

    let total_cases = java_fixture_families
        .iter()
        .map(|family| family.cases)
        .sum::<usize>()
        + adversarial_family.cases
        + generated_family.cases;
    let total_comparisons = java_fixture_families
        .iter()
        .map(|family| family.comparisons)
        .sum::<usize>()
        + adversarial_family.comparisons
        + generated_family.comparisons
        + explicit_build_error_checks;
    let total_reference_events = java_fixture_families
        .iter()
        .map(|family| family.reference_events)
        .sum::<u64>()
        .saturating_add(adversarial_family.reference_events)
        .saturating_add(generated_family.reference_events);
    let mut aggregate = StableDigest::new();
    for family in java_fixture_families
        .iter()
        .chain([&adversarial_family, &generated_family])
    {
        aggregate.update(family.family.as_bytes());
        aggregate.update(family.event_digest.as_bytes());
    }

    Ok(CohortParityReceipt {
        schema_version: 1,
        evidence_id: "H43-E1".to_owned(),
        campaign: CAMPAIGN.to_owned(),
        generator_seed: format!("0x{GENERATOR_SEED:08x}"),
        generated_case_count: GENERATED_CASE_COUNT,
        worker_counts: WORKER_COUNTS.to_vec(),
        cache_budgets: CACHE_BUDGETS.to_vec(),
        source_digests,
        java_fixture_families,
        adversarial_family,
        generated_family,
        explicit_build_error_checks,
        total_cases,
        total_comparisons,
        total_reference_events,
        aggregate_event_digest: aggregate.finish(),
        status: "pass".to_owned(),
    })
}

fn verify_fixture_set(input: FixtureSetInput) -> Result<FamilyReceipt, String> {
    let fixtures: Vec<Fixture> = parse_jsonl("fixtures", input.fixtures)?;
    let expected_results: Vec<ExpectedResult> = parse_jsonl("Java results", input.java_results)?;
    let mut expected_by_case = expected_results
        .into_iter()
        .map(|expected| (expected.case_id.clone(), expected))
        .collect::<BTreeMap<_, _>>();
    if expected_by_case.len() != fixtures.len() {
        return Err(format!(
            "H43-E1 {} fixture and Java-result counts differ or IDs repeat",
            input.name
        ));
    }

    let mut evidence = FamilyAccumulator::new(input.name);
    for fixture in &fixtures {
        fixture.validate(input.tier)?;
        let expected = expected_by_case
            .remove(&fixture.case_id)
            .ok_or_else(|| format!("{} has no Java result", fixture.case_id))?;
        expected.validate_for(fixture)?;
        let case = fixture_case(input.name, fixture)?;
        match fixture.rust_expectation.as_str() {
            "matched" => {
                if expected.status != "matched" || expected.rejection.is_some() {
                    return Err(format!(
                        "{} should have matched in the Java oracle",
                        fixture.case_id
                    ));
                }
                let mut expected_events =
                    expected.events.iter().map(oracle_event).collect::<Vec<_>>();
                expected_events.sort_unstable();
                let outcome = verify_case(&case, Some(expected_events), false)?;
                evidence.record_matched(&case.id, &outcome.events, outcome.comparisons, false);
            }
            "rejected-invalid" | "rejected-unsupported" => {
                verify_rejected_case(&case)?;
                evidence.record_rejected(&case.id);
            }
            expectation => {
                return Err(format!(
                    "{} has unknown expectation {expectation}",
                    fixture.case_id
                ));
            }
        }
    }
    if !expected_by_case.is_empty() {
        return Err(format!(
            "H43-E1 {} Java results contain cases absent from fixtures",
            input.name
        ));
    }
    Ok(evidence.finish())
}

fn fixture_case(fixture_set: &str, fixture: &Fixture) -> Result<ParityCase, String> {
    let patterns = fixture
        .patterns
        .iter()
        .map(|pattern| {
            Ok(PatternSpec {
                id: pattern.pattern_id,
                source: pattern.text()?,
                flags: PatternFlags::NONE,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(ParityCase {
        id: format!("{fixture_set}/{}", fixture.case_id),
        patterns,
        input: fixture.input_utf16.clone(),
    })
}

fn oracle_event(event: &OracleEvent) -> Event {
    (event.pattern_id, event.start_utf16, event.end_utf16)
}

struct CaseOutcome {
    events: Vec<Event>,
    comparisons: usize,
}

fn verify_case(
    case: &ParityCase,
    expected: Option<Vec<Event>>,
    require_mixed: bool,
) -> Result<CaseOutcome, String> {
    let reference = if let Some(events) = expected {
        events
    } else {
        let matcher = build_matcher(case, 1, 0, false)?;
        collect_events(&matcher, &case.input)?
    };
    let mut comparisons = 0;
    for worker_count in WORKER_COUNTS {
        for cache_budget in CACHE_BUDGETS {
            let ordinary = build_matcher(case, worker_count, cache_budget, false)?;
            let cohort = build_matcher(case, worker_count, cache_budget, true)?;
            let ordinary_layout = ordinary.cohort_execution_diagnostics();
            let cohort_layout = cohort.cohort_execution_diagnostics();
            if ordinary_layout.enabled() || !cohort_layout.enabled() {
                return Err(format!(
                    "H43-E1 activation mismatch in {} workers={worker_count} cache={cache_budget}",
                    case.id
                ));
            }
            if require_mixed && cohort_layout.cohort_count() != 2 {
                return Err(format!(
                    "H43-E1 generated case {} did not activate both cohorts",
                    case.id
                ));
            }
            if cohort_layout.total_cache_budget() != cache_budget {
                return Err(format!(
                    "H43-E1 cache budget changed in {}: configured={cache_budget} actual={}",
                    case.id,
                    cohort_layout.total_cache_budget()
                ));
            }

            let ordinary_events = collect_events(&ordinary, &case.input)?;
            let cohort_events = collect_events(&cohort, &case.input)?;
            let repeated_events = collect_events(&cohort, &case.input)?;
            if ordinary_events != reference
                || cohort_events != reference
                || repeated_events != reference
            {
                return Err(parity_failure(
                    case,
                    worker_count,
                    cache_budget,
                    &reference,
                    &ordinary_events,
                    &cohort_events,
                    &repeated_events,
                ));
            }
            comparisons += 1;
        }
    }
    Ok(CaseOutcome {
        events: reference,
        comparisons,
    })
}

fn build_matcher(
    case: &ParityCase,
    worker_count: usize,
    cache_budget: usize,
    cohort_enabled: bool,
) -> Result<Matcher, String> {
    let mut builder = MatcherBuilder::new();
    builder
        .worker_count(worker_count)
        .state_cache_budget(cache_budget)
        .cohort_compilation_enabled(cohort_enabled);
    for pattern in &case.patterns {
        builder
            .add_with_flags(PatternId::new(pattern.id), &pattern.source, pattern.flags)
            .map_err(|error| {
                format!(
                    "H43-E1 {} pattern {} registration failed: {error}",
                    case.id, pattern.id
                )
            })?;
    }
    builder
        .build()
        .map_err(|error| format!("H43-E1 {} build failed: {error}", case.id))
}

fn collect_events(matcher: &Matcher, input: &[u16]) -> Result<Vec<Event>, String> {
    let text = Utf16Text::from_units(input.to_vec());
    let mut events = Vec::new();
    matcher
        .scan(&text, |matched| {
            let span = matched.span();
            assert!(span.start() <= span.end());
            assert!(span.end() <= text.as_units().len() as u64);
            events.push((matched.pattern_id().get(), span.start(), span.end()));
        })
        .map_err(|error| format!("H43-E1 bounded scan failed: {error}"))?;
    events.sort_unstable();
    Ok(events)
}

fn parity_failure(
    case: &ParityCase,
    worker_count: usize,
    cache_budget: usize,
    reference: &[Event],
    ordinary: &[Event],
    cohort: &[Event],
    repeated: &[Event],
) -> String {
    format!(
        "H43-E1 mismatch: case={} workers={worker_count} cache={cache_budget} case_digest={} patterns={:?} input_utf16={:?} reference={reference:?} ordinary={ordinary:?} cohort={cohort:?} repeated={repeated:?}",
        case.id,
        case_digest(case),
        case.patterns,
        case.input
    )
}

fn verify_rejected_case(case: &ParityCase) -> Result<(), String> {
    let ordinary = construction_result(case, 1, false);
    let cohort = construction_result(case, 1, true);
    if ordinary != cohort {
        return Err(format!(
            "H43-E1 rejection mismatch in {}: ordinary={ordinary:?} cohort={cohort:?}",
            case.id
        ));
    }
    if ordinary.is_ok() {
        return Err(format!(
            "H43-E1 {} was expected to be rejected but both modes built",
            case.id
        ));
    }
    Ok(())
}

fn construction_result(
    case: &ParityCase,
    worker_count: usize,
    cohort_enabled: bool,
) -> Result<(), Error> {
    let mut builder = MatcherBuilder::new();
    builder
        .worker_count(worker_count)
        .cohort_compilation_enabled(cohort_enabled);
    for pattern in &case.patterns {
        builder.add_with_flags(PatternId::new(pattern.id), &pattern.source, pattern.flags)?;
    }
    builder.build().map(|_| ())
}

fn verify_build_error_parity() -> Result<usize, String> {
    let cases = [
        ParityCase {
            id: "build-error/no-patterns".to_owned(),
            patterns: Vec::new(),
            input: Vec::new(),
        },
        case("build-error/empty", &[(1, "")], ""),
        case("build-error/malformed", &[(1, "(")], ""),
        case("build-error/unsupported", &[(1, "(?=a)")], ""),
        ParityCase {
            id: "build-error/duplicate-id".to_owned(),
            patterns: vec![pattern(7, "a"), pattern(7, "^a")],
            input: Vec::new(),
        },
        case("build-error/zero-workers", &[(1, "a"), (2, "^a")], "a"),
    ];
    for (index, case) in cases.iter().enumerate() {
        let worker_count = usize::from(index + 1 != cases.len());
        let ordinary = construction_result(case, worker_count, false);
        let cohort = construction_result(case, worker_count, true);
        if ordinary != cohort || ordinary.is_ok() {
            return Err(format!(
                "H43-E1 build-error mismatch in {}: ordinary={ordinary:?} cohort={cohort:?}",
                case.id
            ));
        }
    }
    Ok(cases.len())
}

fn verify_case_family(name: &str, cases: Vec<ParityCase>) -> Result<FamilyReceipt, String> {
    let mut evidence = FamilyAccumulator::new(name);
    for case in cases {
        let outcome = verify_case(&case, None, true)?;
        evidence.record_matched(&case.id, &outcome.events, outcome.comparisons, true);
    }
    Ok(evidence.finish())
}

// Keeping this declarative matrix together makes retained case review easier.
#[allow(clippy::too_many_lines)]
fn adversarial_cases() -> Vec<ParityCase> {
    let mut large_input = vec![u16::from(b'x'); 65_536];
    place(&mut large_input, 0, "header\n");
    place(&mut large_input, 16_384, "alpha123 needle\n");
    place(&mut large_input, 65_531, "tail\n");

    let mut large_patterns = Vec::new();
    for index in 0_u32..64 {
        let id = 2_000 + index;
        let source = if index % 2 == 0 {
            format!("token{index:02}")
        } else {
            format!("^token{index:02}")
        };
        large_patterns.push(PatternSpec {
            id,
            source,
            flags: PatternFlags::NONE,
        });
    }
    large_patterns.push(pattern(2_100, "needle"));
    large_patterns.push(pattern(2_101, r"\bneedle\b"));

    vec![
        case(
            "adversarial/mixed-overlap-duplicates",
            &[
                (10, "a"),
                (20, "^a"),
                (30, "aa"),
                (40, "a$"),
                (50, r"\ba+\b"),
                (60, "a"),
                (70, "(?:a|aa)+"),
                (80, r"\Ba\B"),
            ],
            "aa a\nbaaa\n",
        ),
        case(
            "adversarial/empty-input",
            &[(100, "a+"), (101, "^a"), (102, "a$"), (103, r"\ba")],
            "",
        ),
        ParityCase {
            id: "adversarial/one-unit-raw-surrogate".to_owned(),
            patterns: vec![
                pattern(110, "."),
                pattern(111, "[^a]"),
                pattern(112, "^.$"),
                pattern(113, r"\B.\B"),
            ],
            input: vec![0xd800],
        },
        case(
            "adversarial/no-match",
            &[
                (120, "needle"),
                (121, "[0-9]+"),
                (122, "^needle$"),
                (123, r"\bneedle\b"),
            ],
            "haystack only",
        ),
        case(
            "adversarial/output-heavy",
            &[
                (130, "a"),
                (131, "aa"),
                (132, "a+"),
                (133, "a{2,8}"),
                (134, "^a+"),
                (135, "a+$"),
                (136, r"\ba+\b"),
                (137, "a"),
            ],
            &"a".repeat(64),
        ),
        ParityCase {
            id: "adversarial/large-sparse".to_owned(),
            patterns: vec![
                pattern(140, "alpha[0-9]+"),
                pattern(141, "needle"),
                pattern(142, "omega"),
                pattern(143, "^header$"),
                pattern(144, "tail$"),
                pattern(145, r"\bneedle\b"),
            ],
            input: large_input,
        },
        ParityCase {
            id: "adversarial/unicode-and-surrogates".to_owned(),
            patterns: vec![
                pattern(150, "snø"),
                pattern(151, "[α-γ]"),
                pattern(152, "."),
                pattern(153, "^snø"),
                pattern(154, "😀$"),
                pattern(155, r"\B.\B"),
            ],
            input: vec![
                0x0073, 0x006e, 0x00f8, 0x0020, 0x03b2, 0x0020, 0xd800, 0x0020, 0xd83d, 0xde00,
            ],
        },
        ParityCase {
            id: "adversarial/flags-and-lines".to_owned(),
            patterns: vec![
                flagged_pattern(160, "cat"),
                pattern(161, "(?s)a.b"),
                pattern(162, "^line$"),
                pattern(163, "(?i)^cat$"),
                pattern(164, r"\bword\b"),
                pattern(165, "[A-Z]+"),
            ],
            input: "CAT\na\nb\nline\nword sword".encode_utf16().collect(),
        },
        case(
            "adversarial/nullable-composition",
            &[
                (170, "x(|a)y"),
                (171, "a{0,2}b"),
                (172, "(?:ab)+"),
                (173, "^x(|a)y$"),
                (174, r"\ba{0,2}b\b"),
            ],
            "xy xay aab abab\nxy",
        ),
        ParityCase {
            id: "adversarial/large-pattern-set".to_owned(),
            patterns: large_patterns,
            input: "token00 token17\nneedle".encode_utf16().collect(),
        },
    ]
}

fn generated_cases() -> Vec<ParityCase> {
    const FREE: [&str; 14] = [
        "a", "aa", "ab|a", "a+", "[A-Z]+", "cat", "😀", ".", "x(|a)y", "a{0,2}b", "(?:ab)+",
        "[α-γ]", r"\w+", "(?s)a.b",
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
    let mut random = XorShift32::new(GENERATOR_SEED);
    let mut cases = Vec::with_capacity(GENERATED_CASE_COUNT);
    for case_index in 0..GENERATED_CASE_COUNT {
        let pattern_count = 2 + usize::try_from(random.bounded(15)).expect("bounded count");
        let id_base = 10_000 + u32::try_from(case_index).expect("bounded case") * 100;
        let mut patterns = vec![
            pattern(id_base, FREE[random_index(&mut random, FREE.len())]),
            pattern(
                id_base + 1,
                BEARING[random_index(&mut random, BEARING.len())],
            ),
        ];
        while patterns.len() < pattern_count {
            let ordinal = u32::try_from(patterns.len()).expect("bounded ordinal");
            let choose_bearing = random.bounded(3) == 0;
            let source = if choose_bearing {
                BEARING[random_index(&mut random, BEARING.len())]
            } else {
                FREE[random_index(&mut random, FREE.len())]
            };
            let flags = if random.bounded(7) == 0 {
                PatternFlags::CASE_INSENSITIVE
            } else {
                PatternFlags::NONE
            };
            patterns.push(PatternSpec {
                id: id_base + ordinal,
                source: source.to_owned(),
                flags,
            });
        }
        if case_index % 8 == 0 {
            let ordinal = u32::try_from(patterns.len()).expect("bounded ordinal");
            let duplicate = patterns[0].source.clone();
            patterns.push(PatternSpec {
                id: id_base + ordinal,
                source: duplicate,
                flags: patterns[0].flags,
            });
        }
        let input = generated_input(&mut random, case_index);
        cases.push(ParityCase {
            id: format!("generated/seed-{GENERATOR_SEED:08x}/case-{case_index:03}"),
            patterns,
            input,
        });
    }
    cases
}

fn generated_input(random: &mut XorShift32, case_index: usize) -> Vec<u16> {
    const SYMBOLS: [u16; 15] = [
        0x0061, 0x0062, 0x0063, 0x0078, 0x0079, 0x007a, 0x0030, 0x0020, 0x000a, 0x0041, 0x0043,
        0x00f8, 0x03b2, 0xd800, 0xdc00,
    ];
    let length = match case_index % 12 {
        0 => 0,
        1 => 1,
        _ => 1 + usize::try_from(random.bounded(128)).expect("bounded input length"),
    };
    (0..length)
        .map(|_| SYMBOLS[random_index(random, SYMBOLS.len())])
        .collect()
}

fn random_index(random: &mut XorShift32, length: usize) -> usize {
    let upper = u32::try_from(length).expect("template count fits u32");
    usize::try_from(random.bounded(upper)).expect("bounded index fits usize")
}

fn case(id: &str, patterns: &[(u32, &str)], input: &str) -> ParityCase {
    ParityCase {
        id: id.to_owned(),
        patterns: patterns
            .iter()
            .map(|&(pattern_id, source)| pattern(pattern_id, source))
            .collect(),
        input: input.encode_utf16().collect(),
    }
}

fn pattern(id: u32, source: &str) -> PatternSpec {
    PatternSpec {
        id,
        source: source.to_owned(),
        flags: PatternFlags::NONE,
    }
}

fn flagged_pattern(id: u32, source: &str) -> PatternSpec {
    PatternSpec {
        id,
        source: source.to_owned(),
        flags: PatternFlags::CASE_INSENSITIVE,
    }
}

fn place(input: &mut [u16], start: usize, source: &str) {
    let units = source.encode_utf16().collect::<Vec<_>>();
    input[start..start + units.len()].copy_from_slice(&units);
}

struct FamilyAccumulator {
    family: String,
    matched_cases: usize,
    rejected_cases: usize,
    mixed_cohort_cases: usize,
    comparisons: usize,
    reference_events: u64,
    digest: StableDigest,
}

impl FamilyAccumulator {
    fn new(family: &str) -> Self {
        Self {
            family: family.to_owned(),
            matched_cases: 0,
            rejected_cases: 0,
            mixed_cohort_cases: 0,
            comparisons: 0,
            reference_events: 0,
            digest: StableDigest::new(),
        }
    }

    fn record_matched(&mut self, case_id: &str, events: &[Event], comparisons: usize, mixed: bool) {
        self.matched_cases += 1;
        self.mixed_cohort_cases += usize::from(mixed);
        self.comparisons += comparisons;
        self.reference_events = self.reference_events.saturating_add(events.len() as u64);
        self.record(case_id, events);
    }

    fn record_rejected(&mut self, case_id: &str) {
        self.rejected_cases += 1;
        self.comparisons += 1;
        self.record(case_id, &[]);
    }

    fn record(&mut self, case_id: &str, events: &[Event]) {
        self.digest.update(case_id.as_bytes());
        self.digest.update(&(events.len() as u64).to_le_bytes());
        for &(pattern_id, start, end) in events {
            self.digest.update(&pattern_id.to_le_bytes());
            self.digest.update(&start.to_le_bytes());
            self.digest.update(&end.to_le_bytes());
        }
    }

    fn finish(self) -> FamilyReceipt {
        FamilyReceipt {
            family: self.family,
            cases: self.matched_cases + self.rejected_cases,
            matched_cases: self.matched_cases,
            rejected_cases: self.rejected_cases,
            mixed_cohort_cases: self.mixed_cohort_cases,
            comparisons: self.comparisons,
            reference_events: self.reference_events,
            event_digest: self.digest.finish(),
        }
    }
}

fn stable_source_digest(source: &str) -> String {
    let mut digest = StableDigest::new();
    digest.update(source.as_bytes());
    digest.finish()
}

fn case_digest(case: &ParityCase) -> String {
    let mut digest = StableDigest::new();
    digest.update(case.id.as_bytes());
    for pattern in &case.patterns {
        digest.update(&pattern.id.to_le_bytes());
        digest.update(pattern.source.as_bytes());
        digest.update(&[u8::from(pattern.flags == PatternFlags::CASE_INSENSITIVE)]);
    }
    for unit in &case.input {
        digest.update(&unit.to_le_bytes());
    }
    digest.finish()
}

struct StableDigest(u64);

impl StableDigest {
    const fn new() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }

    fn update(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.0 ^= u64::from(*byte);
            self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn finish(self) -> String {
        format!("fnv1a64:{:016x}", self.0)
    }
}

struct XorShift32(u32);

impl XorShift32 {
    const fn new(seed: u32) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }

    fn bounded(&mut self, upper: u32) -> u32 {
        self.next() % upper
    }
}

#[cfg(test)]
mod tests {
    use super::{CohortParityReceipt, verify};

    const GOLDEN: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../compat/expected/h43-e1-cohort-parity-v1.json"
    ));

    #[test]
    fn campaign_is_deterministic_and_covers_every_required_lane() -> Result<(), String> {
        // Prepare / Test
        let first = verify()?;
        let second = verify()?;

        // Assert
        assert_eq!(first, second);
        assert_eq!(first.java_fixture_families.len(), 6);
        assert_eq!(first.generated_case_count, 96);
        assert!(first.adversarial_family.mixed_cohort_cases >= 10);
        assert_eq!(first.generated_family.mixed_cohort_cases, 96);
        assert!(first.total_comparisons > 3_000);
        assert_eq!(first.status, "pass");
        Ok(())
    }

    #[test]
    fn campaign_matches_the_strict_retained_receipt() -> Result<(), String> {
        // Prepare
        let expected: CohortParityReceipt = serde_json::from_str(GOLDEN)
            .map_err(|error| format!("invalid H43-E1 golden receipt: {error}"))?;

        // Test
        let actual = verify()?;

        // Assert
        assert_eq!(actual, expected);
        Ok(())
    }
}
