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
- ASCII dot, positive and negated classes, ranges, literal and control escapes,
  and shorthand classes, backed by interned 128-bit predicates and differential
  evidence against the pinned Java reference.
- A parser fuzz target compiled and linted by the ordinary quality gate.
- Alternation, plain and non-capturing groups, normalized recursive HIR,
  Thompson NFA branching, and pinned Java differential composition evidence.
- Greedy `?`, `*`, `+`, `{m}`, `{m,n}`, and `{m,}` repetition through parser,
  HIR, Thompson NFA compilation, longest-match scanning, and 29 pinned Java
  differential cases, including malformed counts and the 1,000-count limit.
- Complete UTF-16 code-unit input and predicates, raw-surrogate construction,
  prefix `(?i)`/`(?s)` flags, typed `PatternFlags::CASE_INSENSITIVE`, a
  reproducible 65,536-entry Java 21 case table, and 23 pinned differential
  flag/character fixtures.
- NFA-native `^`, `$`, `\b`, and `\B` assertions with pay-for-use scan
  specialization, exhaustive ASCII boundary classification, and 28 pinned
  Java differential fixtures spanning line edges, composition, flags,
  non-ASCII input, and raw surrogates.
- Lazy deterministic state caching, conservative start tables and literal
  prefilters, exact pattern-partition parallelism, and runtime-detected AVX2
  candidate discovery, each retained through correctness-gated performance
  admission evidence.
- A non-default `unstable-assertion-prefix-v1` feature with a separate explicit
  matcher type, typed eligibility/refusal reporting, exact fallback, and
  caller-selected scan policy. It entered production through a documented
  owner exception; the rejected formal result and residual construction cost
  remain retained.
- A correctness-gated cross-engine benchmark campaign and reviewed comparison
  against Java rmatch, RegexSet, and Hyperscan.
- Explicit 256-level group-nesting rejection, generated semantic properties,
  compiler/scan/oracle fuzz targets, a bounded cache/matcher soak, and a Miri
  release lane.
- Reproducible package rehearsal for `rustmatch` and `rustmatch-simd`, including
  normalized-archive tests, documentation, and an external-consumer smoke.
- A first-release compatibility matrix, public-API and versioning policy,
  dependency/license/unsafe/panic audit, and release runbook.

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
