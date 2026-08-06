//! Differential fixture adapter for the private rustmatch development workspace.

mod cohort_parity;

use std::collections::BTreeMap;
use std::env;
use std::process::ExitCode;

use rustmatch::{MatcherBuilder, PatternId, Utf16Text};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const LITERAL_FIXTURES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../compat/fixtures/ascii-literals-v1.jsonl"
));
const LITERAL_JAVA_RESULTS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../compat/expected/java-2.0.0-RC1-ascii-literals-v1.jsonl"
));
const PREDICATE_FIXTURES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../compat/fixtures/ascii-predicates-v1.jsonl"
));
const PREDICATE_JAVA_RESULTS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../compat/expected/java-2.0.0-RC1-ascii-predicates-v1.jsonl"
));
const COMPOSITION_FIXTURES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../compat/fixtures/ascii-composition-v1.jsonl"
));
const COMPOSITION_JAVA_RESULTS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../compat/expected/java-2.0.0-RC1-ascii-composition-v1.jsonl"
));
const REPETITION_FIXTURES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../compat/fixtures/ascii-repetition-v1.jsonl"
));
const REPETITION_JAVA_RESULTS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../compat/expected/java-2.0.0-RC1-ascii-repetition-v1.jsonl"
));
const UTF16_FLAGS_FIXTURES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../compat/fixtures/utf16-flags-v1.jsonl"
));
const UTF16_FLAGS_JAVA_RESULTS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../compat/expected/java-2.0.0-RC1-utf16-flags-v1.jsonl"
));
const ASSERTION_FIXTURES: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../compat/fixtures/assertions-v1.jsonl"
));
const ASSERTION_JAVA_RESULTS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../compat/expected/java-2.0.0-RC1-assertions-v1.jsonl"
));

fn main() -> ExitCode {
    match execute(env::args().skip(1).collect()) {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn execute(arguments: Vec<String>) -> Result<String, String> {
    if arguments == ["verify-cohort-parity"] {
        return serde_json::to_string(&cohort_parity::verify()?)
            .map_err(|error| format!("could not serialize H43-E1 evidence: {error}"));
    }
    serde_json::to_string(&run(arguments.into_iter()))
        .map_err(|error| format!("could not serialize evidence summary: {error}"))
}

fn usage() -> String {
    "usage: rustmatch-compat <verify-literals|verify-predicates|verify-composition|verify-repetition|verify-utf16-flags|verify-assertions|verify-cohort-parity>"
        .to_owned()
}

fn run(mut arguments: impl Iterator<Item = String>) -> Result<EvidenceSummary, String> {
    let command = arguments.next();
    if arguments.next().is_some() {
        return Err(usage());
    }
    match command.as_deref() {
        Some("verify-literals") => verify_fixture_set(
            LITERAL_FIXTURES,
            LITERAL_JAVA_RESULTS,
            "ascii-literal-v1",
            "ascii-literals-v1",
            "I1-E2",
        ),
        Some("verify-predicates") => verify_fixture_set(
            PREDICATE_FIXTURES,
            PREDICATE_JAVA_RESULTS,
            "ascii-predicate-v1",
            "ascii-predicates-v1",
            "I2-E1",
        ),
        Some("verify-composition") => verify_fixture_set(
            COMPOSITION_FIXTURES,
            COMPOSITION_JAVA_RESULTS,
            "ascii-composition-v1",
            "ascii-composition-v1",
            "I3-E1",
        ),
        Some("verify-repetition") => verify_fixture_set(
            REPETITION_FIXTURES,
            REPETITION_JAVA_RESULTS,
            "ascii-repetition-v1",
            "ascii-repetition-v1",
            "I4-E1",
        ),
        Some("verify-utf16-flags") => verify_fixture_set(
            UTF16_FLAGS_FIXTURES,
            UTF16_FLAGS_JAVA_RESULTS,
            "utf16-flags-v1",
            "utf16-flags-v1",
            "I5-E1",
        ),
        Some("verify-assertions") => verify_fixture_set(
            ASSERTION_FIXTURES,
            ASSERTION_JAVA_RESULTS,
            "assertions-v1",
            "assertions-v1",
            "I5-E2",
        ),
        _ => Err(usage()),
    }
}

fn verify_fixture_set(
    fixture_jsonl: &str,
    java_result_jsonl: &str,
    expected_tier: &str,
    fixture_set: &'static str,
    evidence_id: &'static str,
) -> Result<EvidenceSummary, String> {
    let fixtures: Vec<Fixture> = parse_jsonl("fixtures", fixture_jsonl)?;
    let expected_results: Vec<ExpectedResult> = parse_jsonl("Java results", java_result_jsonl)?;
    let mut expected_by_case: BTreeMap<String, ExpectedResult> = expected_results
        .into_iter()
        .map(|result| (result.case_id.clone(), result))
        .collect();
    if expected_by_case.len() != fixtures.len() {
        return Err("fixture and Java-result case counts differ or IDs repeat".to_owned());
    }

    let mut matched_cases = 0;
    let mut rejected_cases = 0;
    for fixture in &fixtures {
        fixture.validate(expected_tier)?;
        let expected = expected_by_case
            .remove(&fixture.case_id)
            .ok_or_else(|| format!("{} has no Java result", fixture.case_id))?;
        expected.validate_for(fixture)?;

        match fixture.rust_expectation.as_str() {
            "matched" => {
                verify_matched_fixture(fixture, &expected)?;
                matched_cases += 1;
            }
            "rejected-invalid" | "rejected-unsupported" => {
                verify_rejected_fixture(fixture)?;
                rejected_cases += 1;
            }
            other => {
                return Err(format!(
                    "{} has unknown expectation {other}",
                    fixture.case_id
                ));
            }
        }
    }
    if !expected_by_case.is_empty() {
        return Err("Java results contain cases absent from the fixture set".to_owned());
    }

    Ok(EvidenceSummary {
        schema_version: 1,
        evidence_id,
        fixture_set,
        matched_cases,
        rejected_cases,
        status: "pass",
    })
}

fn verify_matched_fixture(fixture: &Fixture, expected: &ExpectedResult) -> Result<(), String> {
    if expected.status != "matched" || expected.rejection.is_some() {
        return Err(format!(
            "{} should have matched in the Java oracle",
            fixture.case_id
        ));
    }
    let mut builder = MatcherBuilder::new();
    for pattern in &fixture.patterns {
        builder
            .add(PatternId::new(pattern.pattern_id), &pattern.text()?)
            .map_err(|error| format!("{} registration failed: {error}", fixture.case_id))?;
    }
    let matcher = builder
        .build()
        .map_err(|error| format!("{} build failed: {error}", fixture.case_id))?;
    let input = Utf16Text::from_units(fixture.input_utf16.clone());
    let mut actual_events = Vec::new();
    matcher
        .scan(&input, |event| {
            actual_events.push(OracleEvent {
                pattern_id: event.pattern_id().get(),
                start_utf16: event.span().start(),
                end_utf16: event.span().end(),
            });
        })
        .map_err(|error| format!("{} scan failed: {error}", fixture.case_id))?;
    actual_events.sort_unstable();

    if actual_events == expected.events {
        Ok(())
    } else {
        Err(format!(
            "{} differs from Java: actual {actual_events:?}, expected {:?}",
            fixture.case_id, expected.events
        ))
    }
}

fn verify_rejected_fixture(fixture: &Fixture) -> Result<(), String> {
    let mut builder = MatcherBuilder::new();
    for pattern in &fixture.patterns {
        let text = pattern.text()?;
        if builder
            .add(PatternId::new(pattern.pattern_id), &text)
            .is_err()
        {
            return Ok(());
        }
    }
    Err(format!(
        "{} was expected to be rejected by the first Rust slice",
        fixture.case_id
    ))
}

fn parse_jsonl<T>(description: &str, input: &str) -> Result<Vec<T>, String>
where
    T: for<'de> Deserialize<'de>,
{
    input
        .lines()
        .enumerate()
        .filter(|(_, line)| !line.is_empty())
        .map(|(index, line)| {
            serde_json::from_str(line).map_err(|error| {
                format!("invalid {description} JSONL at line {}: {error}", index + 1)
            })
        })
        .collect()
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    schema_version: u32,
    case_id: String,
    compatibility_tier: String,
    rust_expectation: String,
    patterns: Vec<FixturePattern>,
    input_utf16: Vec<u16>,
}

impl Fixture {
    fn validate(&self, expected_tier: &str) -> Result<(), String> {
        if self.schema_version != 1 || self.compatibility_tier != expected_tier {
            return Err(format!(
                "{} uses an unsupported schema or tier",
                self.case_id
            ));
        }
        if self.patterns.is_empty() {
            return Err(format!("{} contains no patterns", self.case_id));
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct FixturePattern {
    pattern_id: u32,
    utf16: Vec<u16>,
}

impl FixturePattern {
    fn text(&self) -> Result<String, String> {
        String::from_utf16(&self.utf16).map_err(|error| format!("invalid UTF-16 pattern: {error}"))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedResult {
    schema_version: u32,
    case_id: String,
    rust_expectation: String,
    status: String,
    #[serde(default)]
    events: Vec<OracleEvent>,
    rejection: Option<Value>,
}

impl ExpectedResult {
    fn validate_for(&self, fixture: &Fixture) -> Result<(), String> {
        if self.schema_version != 1 || self.rust_expectation != fixture.rust_expectation {
            return Err(format!(
                "{} has inconsistent fixture and Java-result metadata",
                fixture.case_id
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(deny_unknown_fields)]
struct OracleEvent {
    pattern_id: u32,
    start_utf16: u64,
    end_utf16: u64,
}

#[derive(Debug, Eq, PartialEq, Serialize)]
struct EvidenceSummary {
    schema_version: u32,
    evidence_id: &'static str,
    fixture_set: &'static str,
    matched_cases: u32,
    rejected_cases: u32,
    status: &'static str,
}

#[cfg(test)]
mod tests {
    use super::{
        ASSERTION_FIXTURES, ASSERTION_JAVA_RESULTS, COMPOSITION_FIXTURES, COMPOSITION_JAVA_RESULTS,
        EvidenceSummary, LITERAL_FIXTURES, LITERAL_JAVA_RESULTS, PREDICATE_FIXTURES,
        PREDICATE_JAVA_RESULTS, REPETITION_FIXTURES, REPETITION_JAVA_RESULTS, UTF16_FLAGS_FIXTURES,
        UTF16_FLAGS_JAVA_RESULTS, run, verify_fixture_set,
    };

    #[test]
    fn literal_evidence_agrees_with_the_pinned_java_results() -> Result<(), String> {
        // Prepare
        let arguments = ["verify-literals".to_owned()];

        // Test
        let summary = run(arguments.into_iter())?;

        // Assert
        assert_eq!(
            summary,
            EvidenceSummary {
                schema_version: 1,
                evidence_id: "I1-E2",
                fixture_set: "ascii-literals-v1",
                matched_cases: 6,
                rejected_cases: 1,
                status: "pass",
            }
        );
        Ok(())
    }

    #[test]
    fn literal_evidence_rejects_a_corrupted_expected_event() {
        // Prepare
        let corrupted = LITERAL_JAVA_RESULTS.replacen("\"end_utf16\":1", "\"end_utf16\":9", 1);

        // Test
        let result = verify_fixture_set(
            LITERAL_FIXTURES,
            &corrupted,
            "ascii-literal-v1",
            "ascii-literals-v1",
            "I1-E2",
        );

        // Assert
        assert!(matches!(result, Err(message) if message.contains("differs from Java")));
    }

    #[test]
    fn predicate_evidence_agrees_with_the_pinned_java_results() -> Result<(), String> {
        // Prepare
        let arguments = ["verify-predicates".to_owned()];

        // Test
        let summary = run(arguments.into_iter())?;

        // Assert
        assert_eq!(
            summary,
            EvidenceSummary {
                schema_version: 1,
                evidence_id: "I2-E1",
                fixture_set: "ascii-predicates-v1",
                matched_cases: 9,
                rejected_cases: 3,
                status: "pass",
            }
        );
        Ok(())
    }

    #[test]
    fn predicate_fixture_metadata_is_checked() {
        // Prepare
        let wrong_tier = PREDICATE_FIXTURES.replace("ascii-predicate-v1", "ascii-literal-v1");

        // Test
        let result = verify_fixture_set(
            &wrong_tier,
            PREDICATE_JAVA_RESULTS,
            "ascii-predicate-v1",
            "ascii-predicates-v1",
            "I2-E1",
        );

        // Assert
        assert!(matches!(result, Err(message) if message.contains("unsupported schema or tier")));
    }

    #[test]
    fn composition_evidence_agrees_with_the_pinned_java_results() -> Result<(), String> {
        // Prepare
        let arguments = ["verify-composition".to_owned()];

        // Test
        let summary = run(arguments.into_iter())?;

        // Assert
        assert_eq!(
            summary,
            EvidenceSummary {
                schema_version: 1,
                evidence_id: "I3-E1",
                fixture_set: "ascii-composition-v1",
                matched_cases: 12,
                rejected_cases: 5,
                status: "pass",
            }
        );
        Ok(())
    }

    #[test]
    fn pure_zero_width_divergence_is_explicit_in_java_evidence() {
        // Prepare / Test
        let java_result = COMPOSITION_JAVA_RESULTS
            .lines()
            .find(|line| line.contains("\"case_id\":\"pure-zero-width-alternation\""));

        // Assert
        assert!(matches!(
            java_result,
            Some(line) if line.contains("\"rust_expectation\":\"rejected-invalid\"")
                && line.contains("\"status\":\"matched\"")
        ));
        assert!(COMPOSITION_FIXTURES.contains("pure-zero-width-alternation"));
    }

    #[test]
    fn repetition_evidence_agrees_with_the_pinned_java_results() -> Result<(), String> {
        // Prepare
        let arguments = ["verify-repetition".to_owned()];

        // Test
        let summary = run(arguments.into_iter())?;

        // Assert
        assert_eq!(
            summary,
            EvidenceSummary {
                schema_version: 1,
                evidence_id: "I4-E1",
                fixture_set: "ascii-repetition-v1",
                matched_cases: 20,
                rejected_cases: 9,
                status: "pass",
            }
        );
        Ok(())
    }

    #[test]
    fn zero_width_repetition_divergence_is_explicit_in_java_evidence() {
        // Prepare / Test
        let java_result = REPETITION_JAVA_RESULTS
            .lines()
            .find(|line| line.contains("\"case_id\":\"pure-zero-width-repetition\""));

        // Assert
        assert!(matches!(
            java_result,
            Some(line) if line.contains("\"rust_expectation\":\"rejected-invalid\"")
                && line.contains("\"status\":\"matched\"")
        ));
        assert!(REPETITION_FIXTURES.contains("pure-zero-width-repetition"));
    }

    #[test]
    fn utf16_and_flag_evidence_agrees_with_the_pinned_java_results() -> Result<(), String> {
        // Prepare
        let arguments = ["verify-utf16-flags".to_owned()];

        // Test
        let summary = run(arguments.into_iter())?;

        // Assert
        assert_eq!(
            summary,
            EvidenceSummary {
                schema_version: 1,
                evidence_id: "I5-E1",
                fixture_set: "utf16-flags-v1",
                matched_cases: 21,
                rejected_cases: 2,
                status: "pass",
            }
        );
        Ok(())
    }

    #[test]
    fn raw_surrogate_fixture_is_not_decoded_through_rust_string() {
        // Prepare / Test
        let fixture = UTF16_FLAGS_FIXTURES
            .lines()
            .find(|line| line.contains("dot-matches-isolated-surrogates"));

        // Assert
        assert!(matches!(fixture, Some(line) if line.contains("55296,56320")));
        assert!(UTF16_FLAGS_JAVA_RESULTS.contains("dot-matches-isolated-surrogates"));
    }

    #[test]
    fn assertion_evidence_agrees_with_the_pinned_java_results() -> Result<(), String> {
        // Prepare
        let arguments = ["verify-assertions".to_owned()];

        // Test
        let summary = run(arguments.into_iter())?;

        // Assert
        assert_eq!(
            summary,
            EvidenceSummary {
                schema_version: 1,
                evidence_id: "I5-E2",
                fixture_set: "assertions-v1",
                matched_cases: 26,
                rejected_cases: 2,
                status: "pass",
            }
        );
        Ok(())
    }

    #[test]
    fn assertion_fixtures_cover_phase_and_raw_utf16_adversaries() {
        // Prepare / Test / Assert
        assert!(ASSERTION_FIXTURES.contains("line-start-after-consumed-newline-works"));
        assert!(ASSERTION_FIXTURES.contains("line-end-before-consuming-newline-fails"));
        assert!(ASSERTION_FIXTURES.contains("raw-surrogate-has-non-boundaries"));
        assert!(ASSERTION_JAVA_RESULTS.contains("pure-line-assertion-rejected"));
    }
}
