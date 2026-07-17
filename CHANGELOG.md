# Changelog

All notable changes to rustmatch will be documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and released versions will follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Product requirements, architecture, use-case evidence contracts, and an
  evidence-driven implementation roadmap.
- Contribution, conduct, security, support, and governance policies.
- Repository templates for bug, feature, performance, and pull-request work.
- Rust 2024 Cargo workspace, pinned primary and MSRV toolchains, GitHub Actions
  quality workflow, and a root `cargo xtask ci` command.
- Accepted ADRs for canonical UTF-16 coordinates, normative match events, and
  the minimal public API.
- Versioned semantic fixtures and a deterministic Java 21 oracle pinned to the
  published `no.rmz:rmatch:2.0.0-RC1` artifact and JAR checksum.
- A first public builder and literal matcher running through private HIR, dense
  shared NFA, immutable database, forward scan engine, and callback layers.
- ASCII dot syntax, backed by interned 128-bit predicates and differential
  evidence against the pinned Java reference.

### Changed

- Pinned the initial Java compatibility oracle to the Maven Central artifact
  `no.rmz:rmatch:2.0.0-RC1`.
- Reduced the proposed bootstrap API and made the ASCII-literal contract and
  integration-first PR sequence explicit.
- Selected Rust 2024 edition, bootstrap toolchain `1.97.0`, MSRV `1.85.0`, and
  an integration-driven PR cadence without arbitrary size limits.

### Deprecated

- Nothing yet.

### Removed

- Nothing yet.

### Fixed

- Nothing yet.

### Security

- Nothing yet.
