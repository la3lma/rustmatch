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
const ORACLE_FIXTURES: &str = "compat/fixtures/ascii-literals-v1.jsonl";
const ORACLE_EXPECTED_RESULTS: &str = "compat/expected/java-2.0.0-RC1-ascii-literals-v1.jsonl";
const ORACLE_EXPECTED_MANIFEST: &str =
    "compat/expected/java-2.0.0-RC1-ascii-literals-v1.manifest.json";

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
        Some("ci") => run_quality_gate(),
        Some("oracle") => run_java_oracle(),
        Some("roadmap") => render_roadmap(),
        Some(other) => Err(format!("unknown command `{other}`; run `cargo xtask help`")),
    }
}

fn print_usage() {
    println!(
        "rustmatch repository tasks\n\nUSAGE:\n    cargo xtask ci\n    cargo xtask oracle\n    cargo xtask roadmap"
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

    run_java_oracle()
}

fn run_java_oracle() -> Result<(), String> {
    let java_home = find_java_21()?;
    let temp = env::temp_dir();
    let process = std::process::id();
    let first_results = temp.join(format!("rustmatch-oracle-{process}-first.jsonl"));
    let first_manifest = temp.join(format!("rustmatch-oracle-{process}-first.manifest.json"));
    let second_results = temp.join(format!("rustmatch-oracle-{process}-second.jsonl"));
    let second_manifest = temp.join(format!("rustmatch-oracle-{process}-second.manifest.json"));

    let result = (|| {
        eprintln!("==> Java 2.0.0-RC1 compatibility oracle");
        run_maven(&java_home, &["-q", "-f", ORACLE_POM, "compile"])?;
        run_oracle_once(&java_home, &first_results, &first_manifest)?;
        run_oracle_once(&java_home, &second_results, &second_manifest)?;

        compare_files(&first_results, &second_results, "repeated oracle results")?;
        compare_files(
            &first_manifest,
            &second_manifest,
            "repeated oracle manifests",
        )?;
        compare_files(
            &first_results,
            Path::new(ORACLE_EXPECTED_RESULTS),
            "committed oracle results",
        )?;
        compare_files(
            &first_manifest,
            Path::new(ORACLE_EXPECTED_MANIFEST),
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
    result_path: &Path,
    manifest_path: &Path,
) -> Result<(), String> {
    let fixture_property = format!("-Doracle.fixtures={}", absolute(ORACLE_FIXTURES)?.display());
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
    use super::{QUALITY_STEPS, QualityStep, extract_mermaid, parse_java_feature};

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
}
