use serde::Serialize;
use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

const EVIDENCE_ID: &str = "release-host-identity-v1";
const DEFAULT_OUTPUT: &str = "target/release-evidence/host-identity.json";

#[derive(Debug, Serialize)]
struct HostIdentity {
    schema_version: u32,
    evidence_id: &'static str,
    lane: String,
    execution_mode: String,
    declared_runner_label: String,
    declared_target: String,
    declared_features: String,
    declared_toolchain: String,
    repository_revision: String,
    runner: RunnerIdentity,
    rust: RustIdentity,
    cargo_version: String,
    github: Option<GitHubIdentity>,
}

#[derive(Debug, Serialize)]
struct RunnerIdentity {
    std_os: &'static str,
    std_arch: &'static str,
    system_arch: String,
    os_description: String,
    avx2_available: bool,
    runner_os: Option<String>,
    runner_arch: Option<String>,
    runner_name: Option<String>,
    image_os: Option<String>,
    image_version: Option<String>,
}

#[derive(Debug, Serialize)]
struct RustIdentity {
    verbose_version: String,
    host: String,
    release: String,
    commit_hash: String,
    llvm_version: Option<String>,
}

#[derive(Debug, Serialize)]
struct GitHubIdentity {
    workflow: Option<String>,
    job: Option<String>,
    run_id: Option<String>,
    run_attempt: Option<String>,
    sha: Option<String>,
    git_ref: Option<String>,
}

pub(crate) fn record() -> Result<(), String> {
    let rust = rust_identity()?;
    let execution_mode = environment_or("RUSTMATCH_CI_EXECUTION", "native");
    validate_execution_mode(&execution_mode)?;

    let declared_target = environment_or("RUSTMATCH_CI_TARGET", &rust.host);
    if execution_mode == "native" && declared_target != rust.host {
        return Err(format!(
            "native lane declares target {declared_target}, but rustc host is {}",
            rust.host
        ));
    }

    let identity = HostIdentity {
        schema_version: 1,
        evidence_id: EVIDENCE_ID,
        lane: environment_or("RUSTMATCH_CI_LANE", "local-release-rehearsal"),
        execution_mode,
        declared_runner_label: environment_or("RUSTMATCH_CI_RUNNER_LABEL", "local"),
        declared_target,
        declared_features: environment_or(
            "RUSTMATCH_CI_FEATURES",
            "package rehearsal; packaged workspace all features; default consumer",
        ),
        declared_toolchain: environment_or("RUSTMATCH_CI_TOOLCHAIN", &rust.release),
        repository_revision: repository_revision()?,
        runner: runner_identity(),
        cargo_version: command_line("cargo", &["--version"])
            .map_err(|error| format!("could not query cargo version: {error}"))?,
        github: github_identity(),
        rust,
    };

    let json = serde_json::to_string_pretty(&identity)
        .map_err(|error| format!("could not serialize host identity: {error}"))?;
    let output = env::var_os("RUSTMATCH_HOST_IDENTITY_PATH")
        .map_or_else(|| PathBuf::from(DEFAULT_OUTPUT), PathBuf::from);
    write_identity(&output, &json)?;
    append_github_summary(&identity, &output)?;

    println!("{json}");
    eprintln!("wrote host identity to {}", output.display());
    Ok(())
}

fn validate_execution_mode(mode: &str) -> Result<(), String> {
    if matches!(
        mode,
        "native" | "cross-compile" | "emulated" | "interpreter"
    ) {
        Ok(())
    } else {
        Err(format!(
            "RUSTMATCH_CI_EXECUTION must be native, cross-compile, emulated, or interpreter; got {mode}"
        ))
    }
}

fn rust_identity() -> Result<RustIdentity, String> {
    let verbose_version = command_line("rustc", &["-vV"])
        .map_err(|error| format!("could not query rustc identity: {error}"))?;
    parse_rustc_verbose(&verbose_version)
}

fn parse_rustc_verbose(output: &str) -> Result<RustIdentity, String> {
    let host = required_property(output, "host")?;
    let release = required_property(output, "release")?;
    let commit_hash = required_property(output, "commit-hash")?;
    let llvm_version = property(output, "LLVM version");
    Ok(RustIdentity {
        verbose_version: single_line(output),
        host,
        release,
        commit_hash,
        llvm_version,
    })
}

fn property(output: &str, name: &str) -> Option<String> {
    output.lines().find_map(|line| {
        line.split_once(':')
            .filter(|(key, _)| key.trim() == name)
            .map(|(_, value)| value.trim().to_owned())
    })
}

fn required_property(output: &str, name: &str) -> Result<String, String> {
    property(output, name)
        .ok_or_else(|| format!("rustc -vV output has no `{name}` property: {output}"))
}

fn runner_identity() -> RunnerIdentity {
    RunnerIdentity {
        std_os: env::consts::OS,
        std_arch: env::consts::ARCH,
        system_arch: system_arch(),
        os_description: os_description(),
        avx2_available: avx2_available(),
        runner_os: optional_environment("RUNNER_OS"),
        runner_arch: optional_environment("RUNNER_ARCH"),
        runner_name: optional_environment("RUNNER_NAME"),
        image_os: optional_environment("ImageOS"),
        image_version: optional_environment("ImageVersion"),
    }
}

fn github_identity() -> Option<GitHubIdentity> {
    let identity = GitHubIdentity {
        workflow: optional_environment("GITHUB_WORKFLOW"),
        job: optional_environment("GITHUB_JOB"),
        run_id: optional_environment("GITHUB_RUN_ID"),
        run_attempt: optional_environment("GITHUB_RUN_ATTEMPT"),
        sha: optional_environment("GITHUB_SHA"),
        git_ref: optional_environment("GITHUB_REF"),
    };
    if identity.workflow.is_none()
        && identity.job.is_none()
        && identity.run_id.is_none()
        && identity.sha.is_none()
    {
        None
    } else {
        Some(identity)
    }
}

fn repository_revision() -> Result<String, String> {
    command_line("git", &["rev-parse", "HEAD"])
        .map_err(|error| format!("could not resolve repository revision: {error}"))
}

fn environment_or(name: &str, fallback: &str) -> String {
    env::var(name).unwrap_or_else(|_| fallback.to_owned())
}

fn optional_environment(name: &str) -> Option<String> {
    env::var(name).ok().filter(|value| !value.is_empty())
}

#[cfg(target_arch = "x86_64")]
fn avx2_available() -> bool {
    std::arch::is_x86_feature_detected!("avx2")
}

#[cfg(not(target_arch = "x86_64"))]
const fn avx2_available() -> bool {
    false
}

#[cfg(unix)]
fn system_arch() -> String {
    command_line("uname", &["-m"]).unwrap_or_else(|_| env::consts::ARCH.to_owned())
}

#[cfg(windows)]
fn system_arch() -> String {
    environment_or("PROCESSOR_ARCHITECTURE", env::consts::ARCH)
}

#[cfg(target_os = "linux")]
fn os_description() -> String {
    fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|contents| {
            contents.lines().find_map(|line| {
                line.strip_prefix("PRETTY_NAME=")
                    .map(|value| value.trim_matches('"').to_owned())
            })
        })
        .unwrap_or_else(|| environment_or("RUNNER_OS", env::consts::OS))
}

#[cfg(target_os = "macos")]
fn os_description() -> String {
    command_line("sw_vers", &["-productVersion"])
        .map_or_else(|_| "macOS".to_owned(), |version| format!("macOS {version}"))
}

#[cfg(windows)]
fn os_description() -> String {
    command_line("cmd", &["/C", "ver"]).unwrap_or_else(|_| "Windows".to_owned())
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn os_description() -> String {
    env::consts::OS.to_owned()
}

fn command_line(program: &str, arguments: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(arguments)
        .output()
        .map_err(|error| format!("could not start {program}: {error}"))?;
    if !output.status.success() {
        return Err(format!("{program} exited with {}", output.status));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn single_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn write_identity(path: &Path, json: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("could not create {}: {error}", parent.display()))?;
    }
    fs::write(path, format!("{json}\n"))
        .map_err(|error| format!("could not write {}: {error}", path.display()))
}

fn append_github_summary(identity: &HostIdentity, output: &Path) -> Result<(), String> {
    let Some(path) = env::var_os("GITHUB_STEP_SUMMARY").map(PathBuf::from) else {
        return Ok(());
    };
    let mut summary = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("could not open {}: {error}", path.display()))?;
    writeln!(
        summary,
        "### Host identity: {}\n\n| Field | Value |\n|---|---|\n| Runner label | \
         `{}` |\n| Execution | `{}` |\n| Rust host | `{}` |\n| Target | `{}` |\n| Features | \
         `{}` |\n| OS | {} |\n| Architecture | `{}` |\n| AVX2 available | `{}` |\n| Commit | `{}` |\n| Artifact path | \
         `{}` |\n",
        identity.lane,
        identity.declared_runner_label,
        identity.execution_mode,
        identity.rust.host,
        identity.declared_target,
        identity.declared_features,
        identity.runner.os_description,
        identity.runner.system_arch,
        identity.runner.avx2_available,
        identity.repository_revision,
        output.display()
    )
    .map_err(|error| format!("could not append {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::{HostIdentity, parse_rustc_verbose, single_line, validate_execution_mode};

    #[test]
    fn parses_required_rustc_identity() {
        let output = "rustc 1.97.0 (abc 2026-01-01)\n\
                      binary: rustc\n\
                      commit-hash: abcdef012345\n\
                      commit-date: 2026-01-01\n\
                      host: aarch64-apple-darwin\n\
                      release: 1.97.0\n\
                      LLVM version: 21.1.0\n";

        let identity = parse_rustc_verbose(output).expect("identity should parse");

        assert_eq!(identity.host, "aarch64-apple-darwin");
        assert_eq!(identity.release, "1.97.0");
        assert_eq!(identity.commit_hash, "abcdef012345");
        assert_eq!(identity.llvm_version.as_deref(), Some("21.1.0"));
    }

    #[test]
    fn rejects_missing_rustc_host() {
        let error = parse_rustc_verbose("release: 1.97.0\ncommit-hash: abc\n")
            .expect_err("host is required");

        assert!(error.contains("no `host` property"));
    }

    #[test]
    fn accepts_only_declared_execution_modes() {
        for mode in ["native", "cross-compile", "emulated", "interpreter"] {
            assert_eq!(validate_execution_mode(mode), Ok(()));
        }
        assert!(validate_execution_mode("unknown").is_err());
    }

    #[test]
    fn verbose_values_are_safe_for_one_line_evidence() {
        assert_eq!(single_line("one\n  two\tthree"), "one two three");
    }

    #[test]
    fn identity_schema_remains_serializable() {
        let rust = parse_rustc_verbose(
            "rustc 1.97.0\ncommit-hash: abc\nhost: x86_64-unknown-linux-gnu\nrelease: 1.97.0\n",
        )
        .expect("identity should parse");
        let identity = HostIdentity {
            schema_version: 1,
            evidence_id: "release-host-identity-v1",
            lane: "test".to_owned(),
            execution_mode: "native".to_owned(),
            declared_runner_label: "ubuntu-24.04".to_owned(),
            declared_target: rust.host.clone(),
            declared_features: "--all-features".to_owned(),
            declared_toolchain: rust.release.clone(),
            repository_revision: "def".to_owned(),
            runner: super::RunnerIdentity {
                std_os: "linux",
                std_arch: "x86_64",
                system_arch: "x86_64".to_owned(),
                os_description: "Ubuntu 24.04".to_owned(),
                avx2_available: true,
                runner_os: Some("Linux".to_owned()),
                runner_arch: Some("X64".to_owned()),
                runner_name: None,
                image_os: Some("ubuntu24".to_owned()),
                image_version: Some("20260801.1".to_owned()),
            },
            rust,
            cargo_version: "cargo 1.97.0".to_owned(),
            github: None,
        };

        let json = serde_json::to_string(&identity).expect("identity should serialize");

        assert!(json.contains("release-host-identity-v1"));
        assert!(json.contains("x86_64-unknown-linux-gnu"));
    }

    #[test]
    fn release_workflow_pins_runners_and_retains_identity() {
        let workflow = include_str!("../../.github/workflows/release-readiness.yml");
        let rehearsal = include_str!("../../scripts/release-readiness.sh");

        assert!(
            !workflow.contains("-latest"),
            "release evidence must not use rolling runner labels"
        );
        for label in [
            "ubuntu-24.04",
            "ubuntu-24.04-arm",
            "macos-15",
            "macos-15-intel",
            "windows-2025",
            "windows-11-arm",
        ] {
            assert!(workflow.contains(label), "missing pinned runner {label}");
        }
        assert_eq!(
            workflow.matches("RUSTMATCH_HOST_IDENTITY_PATH:").count(),
            6,
            "each workflow job definition needs an identity destination"
        );
        assert_eq!(
            workflow
                .matches("uses: actions/upload-artifact@043fb46d1a93c77aae656e7c1c64a875d1fc6a0a",)
                .count(),
            6,
            "each workflow job definition needs the reviewed identity upload action"
        );
        assert!(rehearsal.contains("cargo xtask host-identity"));
    }

    #[test]
    fn big_endian_smoke_is_non_blocking_executable_and_pinned() {
        let workflow = include_str!("../../.github/workflows/release-readiness.yml");
        let cross_config = include_str!("../../Cross.toml");
        let smoke = include_str!("../../scripts/big-endian-smoke.sh");
        let image_digest =
            "sha256:0d8edc92c39158abe211fddfc9617bf5b7865c2ae0f58873f9cf0c58bb315271";

        assert!(workflow.contains("name: Big-endian s390x emulation preview"));
        assert!(workflow.contains("continue-on-error: true"));
        assert!(workflow.contains("cargo +1.97.0 install cross --version 0.2.5 --locked"));
        assert!(workflow.contains("scripts/big-endian-smoke.sh"));
        assert!(cross_config.contains("s390x-unknown-linux-gnu"));
        assert!(cross_config.contains(image_digest));
        assert!(smoke.contains("--test big_endian_smoke"));
        assert!(smoke.contains("tests::unavailable_kernel_returns_none_without_writing"));
        assert!(smoke.contains(image_digest));
    }
}
