//! Repository maintenance commands for rustmatch.

use std::env;
use std::ffi::OsString;
use std::fs;
use std::process::{Command, ExitCode};

const ROADMAP_MARKDOWN: &str = "docs/roadmap.md";
const ROADMAP_SOURCE: &str = "/tmp/rustmatch-roadmap.mmd";
const ROADMAP_SVG: &str = "/tmp/rustmatch-roadmap.svg";

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
        Some("roadmap") => render_roadmap(),
        Some(other) => Err(format!("unknown command `{other}`; run `cargo xtask help`")),
    }
}

fn print_usage() {
    println!("rustmatch repository tasks\n\nUSAGE:\n    cargo xtask ci\n    cargo xtask roadmap");
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

    Ok(())
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
    use super::{QUALITY_STEPS, QualityStep, extract_mermaid};

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
}
