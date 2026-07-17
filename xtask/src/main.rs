//! Repository maintenance commands for rustmatch.

use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

const ROADMAP_MARKDOWN: &str = "docs/roadmap.md";
const ROADMAP_SOURCE: &str = "/tmp/rustmatch-roadmap.mmd";
const ROADMAP_SVG: &str = "/tmp/rustmatch-roadmap.svg";
const ORACLE_POM: &str = "compat/java-oracle/pom.xml";
const ORACLE_FIXTURE_SETS: &[OracleFixtureSet] = &[
    OracleFixtureSet {
        label: "ascii-literals-v1",
        fixtures: "compat/fixtures/ascii-literals-v1.jsonl",
        expected_results: "compat/expected/java-2.0.0-RC1-ascii-literals-v1.jsonl",
        expected_manifest: "compat/expected/java-2.0.0-RC1-ascii-literals-v1.manifest.json",
    },
    OracleFixtureSet {
        label: "ascii-predicates-v1",
        fixtures: "compat/fixtures/ascii-predicates-v1.jsonl",
        expected_results: "compat/expected/java-2.0.0-RC1-ascii-predicates-v1.jsonl",
        expected_manifest: "compat/expected/java-2.0.0-RC1-ascii-predicates-v1.manifest.json",
    },
];
const EVIDENCE_SUMMARY_JSON: &str = r#"{"schema_version":1,"evidence_id":"E0","scope":"implemented-slice","status":"pass","executed":["java-2.0.0-RC1-oracle","I1-E2","I2-DOT-E1","B0"],"use_cases":[{"use_case":"UC-0","status":"partial","evidence":["I1-E1","I1-E2","I2-DOT-E1","B0","A0-E5","I1-E6"]},{"use_case":"UC-1","status":"partial","evidence":["I1-E1","I1-E3","I2-DOT-E1","B0","I1-E6"]},{"use_case":"UC-2","status":"partial","evidence":["I1-E1","I1-E2","I2-DOT-E1","I1-E6"]},{"use_case":"UC-3","status":"partial","evidence":["I1-E2"]},{"use_case":"UC-4","status":"partial","evidence":["B0"]},{"use_case":"UC-5","status":"not-started","evidence":[]},{"use_case":"UC-6","status":"not-started","evidence":[]},{"use_case":"UC-7","status":"partial","evidence":["I1-E6"]},{"use_case":"UC-8","status":"partial","evidence":["I1-E2","I2-DOT-E1","I1-E6"]},{"use_case":"UC-9","status":"partial","evidence":["I1-E3"]},{"use_case":"UC-10","status":"not-started","evidence":[]},{"use_case":"UC-11","status":"not-started","evidence":[]},{"use_case":"UC-12","status":"partial","evidence":["I1-E1","I1-E2","I2-DOT-E1"]}]}"#;

struct OracleFixtureSet {
    label: &'static str,
    fixtures: &'static str,
    expected_results: &'static str,
    expected_manifest: &'static str,
}

#[derive(Debug, Eq, PartialEq)]
struct QualityStep {
    label: &'static str,
    args: &'static [&'static str],
    rustdoc_flags: Option<&'static str>,
    toolchain: Option<&'static str>,
}

const QUALITY_STEPS: &[QualityStep] = &[
    QualityStep {
        label: "format",
        args: &["fmt", "--all", "--", "--check"],
        rustdoc_flags: None,
        toolchain: None,
    },
    QualityStep {
        label: "clippy",
        args: &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ],
        rustdoc_flags: None,
        toolchain: None,
    },
    QualityStep {
        label: "tests",
        args: &["test", "--workspace", "--all-features"],
        rustdoc_flags: None,
        toolchain: None,
    },
    QualityStep {
        label: "rustdoc",
        args: &["doc", "--workspace", "--all-features", "--no-deps"],
        rustdoc_flags: Some("-D warnings"),
        toolchain: None,
    },
    QualityStep {
        label: "MSRV check",
        args: &["check", "--workspace", "--all-targets", "--all-features"],
        rustdoc_flags: None,
        toolchain: Some("1.85.0"),
    },
];

fn main() -> ExitCode {
    match run(env::args_os().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(mut args: impl Iterator<Item = OsString>) -> Result<(), String> {
    let command = args.next();
    if args.next().is_some() {
        return Err("expected exactly one command; run `cargo xtask help`".to_owned());
    }

    match command.as_deref().and_then(|value| value.to_str()) {
        None | Some("help") => {
            print_usage();
            Ok(())
        }
        Some("bench-smoke") => run_benchmark_smoke(),
        Some("ci") => run_quality_gate(),
        Some("evidence") => run_evidence_summary(),
        Some("oracle") => run_java_oracle(),
        Some("roadmap") => render_roadmap(),
        Some(other) => Err(format!("unknown command `{other}`; run `cargo xtask help`")),
    }
}

fn print_usage() {
    println!(
        "rustmatch repository tasks\n\nUSAGE:\n    cargo xtask bench-smoke\n    cargo xtask ci\n    cargo xtask evidence\n    cargo xtask oracle\n    cargo xtask roadmap"
    );
}

fn run_quality_gate() -> Result<(), String> {
    let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));

    for step in QUALITY_STEPS {
        eprintln!("==> {}", step.label);
        let mut command = if let Some(toolchain) = step.toolchain {
            let mut command = Command::new("rustup");
            command.args(["run", toolchain, "cargo"]);
            command
        } else {
            Command::new(&cargo)
        };
        command.args(step.args);
        if let Some(flags) = step.rustdoc_flags {
            command.env("RUSTDOCFLAGS", flags);
        }

        let status = command
            .status()
            .map_err(|error| format!("could not start {}: {error}", step.label))?;
        if !status.success() {
            return Err(format!("{} failed with {status}", step.label));
        }
    }

    run_evidence_summary()
}

fn run_evidence_summary() -> Result<(), String> {
    run_java_oracle()?;
    run_literal_evidence()?;
    run_predicate_evidence()?;
    run_benchmark_smoke()?;
    eprintln!("==> E0 use-case evidence summary");
    println!("{EVIDENCE_SUMMARY_JSON}");
    Ok(())
}

fn run_predicate_evidence() -> Result<(), String> {
    eprintln!("==> I2 dot-predicate differential evidence");
    let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let status = Command::new(cargo)
        .args([
            "run",
            "--quiet",
            "--package",
            "rustmatch-compat",
            "--",
            "verify-predicates",
        ])
        .status()
        .map_err(|error| format!("could not start predicate evidence adapter: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("predicate evidence adapter failed with {status}"))
    }
}

fn run_benchmark_smoke() -> Result<(), String> {
    eprintln!("==> B0 RegexSet-aware benchmark smoke");
    let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let status = Command::new(cargo)
        .args([
            "run",
            "--quiet",
            "--release",
            "--package",
            "rustmatch-bench",
            "--",
            "literal-smoke",
        ])
        .status()
        .map_err(|error| format!("could not start benchmark smoke adapter: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("benchmark smoke adapter failed with {status}"))
    }
}

fn run_literal_evidence() -> Result<(), String> {
    eprintln!("==> I1 literal differential evidence");
    let cargo = env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let status = Command::new(cargo)
        .args([
            "run",
            "--quiet",
            "--package",
            "rustmatch-compat",
            "--",
            "verify-literals",
        ])
        .status()
        .map_err(|error| format!("could not start literal evidence adapter: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("literal evidence adapter failed with {status}"))
    }
}

fn run_java_oracle() -> Result<(), String> {
    let java_home = find_java_21()?;
    eprintln!("==> Java 2.0.0-RC1 compatibility oracle");
    run_maven(&java_home, &["-q", "-f", ORACLE_POM, "compile"])?;
    for fixture_set in ORACLE_FIXTURE_SETS {
        verify_oracle_fixture_set(&java_home, fixture_set)?;
    }
    Ok(())
}

fn verify_oracle_fixture_set(
    java_home: &Path,
    fixture_set: &OracleFixtureSet,
) -> Result<(), String> {
    let temp = env::temp_dir();
    let process = std::process::id();
    let first_results = temp.join(format!(
        "rustmatch-oracle-{process}-{}-first.jsonl",
        fixture_set.label
    ));
    let first_manifest = temp.join(format!(
        "rustmatch-oracle-{process}-{}-first.manifest.json",
        fixture_set.label
    ));
    let second_results = temp.join(format!(
        "rustmatch-oracle-{process}-{}-second.jsonl",
        fixture_set.label
    ));
    let second_manifest = temp.join(format!(
        "rustmatch-oracle-{process}-{}-second.manifest.json",
        fixture_set.label
    ));

    let result = (|| {
        run_oracle_once(
            java_home,
            fixture_set.fixtures,
            &first_results,
            &first_manifest,
        )?;
        run_oracle_once(
            java_home,
            fixture_set.fixtures,
            &second_results,
            &second_manifest,
        )?;

        compare_files(&first_results, &second_results, "repeated oracle results")?;
        compare_files(
            &first_manifest,
            &second_manifest,
            "repeated oracle manifests",
        )?;
        compare_files(
            &first_results,
            Path::new(fixture_set.expected_results),
            "committed oracle results",
        )?;
        compare_files(
            &first_manifest,
            Path::new(fixture_set.expected_manifest),
            "committed oracle manifest",
        )?;
        Ok(())
    })();

    if result.is_ok() {
        for path in [
            first_results,
            first_manifest,
            second_results,
            second_manifest,
        ] {
            let _ = fs::remove_file(path);
        }
    }
    result
}

fn run_oracle_once(
    java_home: &Path,
    fixtures: &str,
    result_path: &Path,
    manifest_path: &Path,
) -> Result<(), String> {
    let fixture_property = format!("-Doracle.fixtures={}", absolute(fixtures)?.display());
    let result_property = format!("-Doracle.results={}", result_path.display());
    let manifest_property = format!("-Doracle.manifest={}", manifest_path.display());
    run_maven(
        java_home,
        &[
            "-q",
            "-f",
            ORACLE_POM,
            "exec:java",
            &fixture_property,
            &result_property,
            &manifest_property,
        ],
    )
}

fn run_maven(java_home: &Path, arguments: &[&str]) -> Result<(), String> {
    let status = Command::new("mvn")
        .args(arguments)
        .env("JAVA_HOME", java_home)
        .status()
        .map_err(|error| format!("could not start Maven: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Java oracle Maven command failed with {status}"))
    }
}

fn find_java_21() -> Result<PathBuf, String> {
    for variable in ["JAVA_HOME", "JAVA_HOME_21_X64"] {
        if let Some(home) = env::var_os(variable).map(PathBuf::from) {
            if java_home_has_feature(&home, 21) {
                return Ok(home);
            }
        }
    }

    let macos_java_home = Path::new("/usr/libexec/java_home");
    if macos_java_home.is_file() {
        let output = Command::new(macos_java_home)
            .args(["-v", "21"])
            .output()
            .map_err(|error| format!("could not query macOS Java installations: {error}"))?;
        if output.status.success() {
            let home = PathBuf::from(String::from_utf8_lossy(&output.stdout).trim());
            if java_home_has_feature(&home, 21) {
                return Ok(home);
            }
        }
    }

    Err(
        "Java 21 is required; set JAVA_HOME or JAVA_HOME_21_X64 to a Java 21 installation"
            .to_owned(),
    )
}

fn java_home_has_feature(home: &Path, feature: u32) -> bool {
    let output = Command::new(home.join("bin/java")).arg("-version").output();
    let Ok(output) = output else {
        return false;
    };
    let version = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    parse_java_feature(&version) == Some(feature)
}

fn parse_java_feature(version_output: &str) -> Option<u32> {
    let quoted = version_output.split('"').nth(1)?;
    let first = quoted.split('.').next()?;
    first.parse().ok()
}

fn absolute(path: &str) -> Result<PathBuf, String> {
    env::current_dir()
        .map(|root| root.join(path))
        .map_err(|error| format!("could not resolve {path}: {error}"))
}

fn compare_files(actual: &Path, expected: &Path, description: &str) -> Result<(), String> {
    let actual_bytes = fs::read(actual)
        .map_err(|error| format!("could not read {}: {error}", actual.display()))?;
    let expected_bytes = fs::read(expected)
        .map_err(|error| format!("could not read {}: {error}", expected.display()))?;
    if actual_bytes == expected_bytes {
        Ok(())
    } else {
        Err(format!(
            "{description} differ; inspect generated file {} against {}",
            actual.display(),
            expected.display()
        ))
    }
}

fn render_roadmap() -> Result<(), String> {
    let markdown = fs::read_to_string(ROADMAP_MARKDOWN)
        .map_err(|error| format!("could not read {ROADMAP_MARKDOWN}: {error}"))?;
    let mermaid = extract_mermaid(&markdown)?;
    fs::write(ROADMAP_SOURCE, mermaid)
        .map_err(|error| format!("could not write {ROADMAP_SOURCE}: {error}"))?;

    let status = Command::new("npx")
        .args([
            "--yes",
            "@mermaid-js/mermaid-cli",
            "-i",
            ROADMAP_SOURCE,
            "-o",
            ROADMAP_SVG,
            "-b",
            "transparent",
        ])
        .status()
        .map_err(|error| format!("could not start Mermaid renderer: {error}"))?;
    if !status.success() {
        return Err(format!("Mermaid renderer failed with {status}"));
    }

    println!("rendered {ROADMAP_SVG}");
    Ok(())
}

fn extract_mermaid(markdown: &str) -> Result<&str, String> {
    let after_start = markdown
        .split_once("```mermaid\n")
        .map(|(_, remainder)| remainder)
        .ok_or_else(|| format!("{ROADMAP_MARKDOWN} has no Mermaid block"))?;
    after_start
        .split_once("\n```")
        .map(|(diagram, _)| diagram)
        .ok_or_else(|| format!("{ROADMAP_MARKDOWN} has an unterminated Mermaid block"))
}

#[cfg(test)]
mod tests {
    use super::{
        EVIDENCE_SUMMARY_JSON, QUALITY_STEPS, QualityStep, extract_mermaid, parse_java_feature,
    };

    #[test]
    fn quality_plan_contains_the_declared_msrv_check() {
        // Prepare
        let expected = QualityStep {
            label: "MSRV check",
            args: &["check", "--workspace", "--all-targets", "--all-features"],
            rustdoc_flags: None,
            toolchain: Some("1.85.0"),
        };

        // Test
        let msrv_step = QUALITY_STEPS.last();

        // Assert
        assert_eq!(msrv_step, Some(&expected));
    }

    #[test]
    fn quality_plan_checks_format_before_compilation() {
        // Prepare
        let expected_first_label = "format";

        // Test
        let first_label = QUALITY_STEPS.first().map(|step| step.label);

        // Assert
        assert_eq!(first_label, Some(expected_first_label));
    }

    #[test]
    fn roadmap_extraction_returns_only_the_mermaid_body() {
        // Prepare
        let markdown = "before\n```mermaid\nflowchart LR\n    A --> B\n```\nafter\n";

        // Test
        let diagram = extract_mermaid(markdown);

        // Assert
        assert_eq!(diagram.as_deref(), Ok("flowchart LR\n    A --> B"));
    }

    #[test]
    fn roadmap_extraction_rejects_a_missing_diagram() {
        // Prepare
        let markdown = "# Roadmap\n\nNo diagram here.\n";

        // Test
        let error = extract_mermaid(markdown);

        // Assert
        assert_eq!(
            error,
            Err("docs/roadmap.md has no Mermaid block".to_owned())
        );
    }

    #[test]
    fn java_feature_parser_accepts_openjdk_output() {
        // Prepare
        let output = "openjdk version \"21.0.8\" 2025-07-15 LTS\n";

        // Test
        let feature = parse_java_feature(output);

        // Assert
        assert_eq!(feature, Some(21));
    }

    #[test]
    fn java_feature_parser_rejects_unrecognized_output() {
        // Prepare
        let output = "not a Java version";

        // Test
        let feature = parse_java_feature(output);

        // Assert
        assert_eq!(feature, None);
    }

    #[test]
    fn evidence_summary_reports_every_declared_use_case_once() {
        // Prepare
        let expected_use_cases = 0..=12;

        // Test
        let occurrence_counts: Vec<_> = expected_use_cases
            .map(|number| {
                let identifier = format!(r#""use_case":"UC-{number}""#);
                EVIDENCE_SUMMARY_JSON.matches(&identifier).count()
            })
            .collect();

        // Assert
        assert_eq!(occurrence_counts, vec![1; 13]);
        assert!(EVIDENCE_SUMMARY_JSON.contains(r#""evidence_id":"E0""#));
        assert!(EVIDENCE_SUMMARY_JSON.contains(r#""status":"not-started""#));
    }
}
