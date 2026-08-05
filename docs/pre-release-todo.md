# Rustmatch 0.1.0 pre-release checklist

**Target:** prepare a reproducible `0.1.0` release candidate without publishing,
tagging, or creating a GitHub release.

**Hard stop:** no command in the preparation workflow may upload to crates.io,
create a release tag, or create a GitHub release. Those actions remain unchecked
until the owner explicitly starts the release ceremony.

## Release definition

- [x] Use `0.1.0` for the first public, pre-1.0 API.
- [x] Publish one application crate, `rustmatch`, with `rustmatch-simd` as a
  separately versioned implementation dependency.
- [x] Keep benchmark, compatibility, fuzz, and repository automation crates
  unpublished.
- [x] Retain Rust `1.85.0` as the minimum supported Rust version (MSRV).
- [x] Retain Apache-2.0 as the package license.
- [x] Confirm that crates.io registry lookups do not currently find
  `rustmatch@0.1.0` or `rustmatch-simd@0.1.0`. Names cannot be reserved without
  publication, so this must be checked again during the release ceremony.

## Hardening

- [x] Add property coverage beyond parser examples.
- [x] Compile parser, compiler, scan, and independent-oracle fuzz targets in
  the ordinary quality gate.
- [x] Add and pass a Miri lane for the core safe semantic path.
- [x] Add and pass a bounded matcher-reuse/cache-budget soak test.
- [x] Add an explicit parser nesting resource limit and adversarial tests.
- [x] Audit public panic behavior and internal invariant panics.
- [x] Record the runtime dependency and license audit.
- [x] Build complete public API documentation with warnings denied.

## Package preparation

- [x] Make `rustmatch-simd` independently packageable as version `0.1.0`.
- [x] Give `rustmatch` a registry version requirement for `rustmatch-simd`.
- [x] Enable publication only for the two intended crates and only to
  crates.io.
- [x] Complete package descriptions, READMEs, documentation links, keywords,
  categories, repository links, license metadata, and MSRV metadata.
- [x] Review exact package contents and exclude repository-only evidence.
- [x] Build and test the exact packaged sources together without publishing
  either crate.
- [x] Compile and run a clean external consumer against the packaged sources.
- [x] Verify package documentation and doctests from the packaged sources.

## Public contract

- [x] Record the supported platform, MSRV, syntax, coordinate, and Java
  compatibility matrix.
- [x] Complete the public-surface and stability review.
- [x] Record the pre-1.0 SemVer, deprecation, and MSRV policies.
- [x] Curate the `0.1.0` changelog and state that no migration is required for
  the first public release.
- [x] Document known limitations as prominently as performance strengths.
- [x] Write the release runbook with immutable-artifact recovery instructions.

## Automation and evidence

- [x] Add non-publishing release-readiness CI for Linux, macOS, Windows, MSRV,
  AArch64 compilation, Miri, package rehearsal, and consumer smoke testing.
- [x] Pass the full repository quality and differential-evidence gate.
- [x] Pass release-profile semantic and benchmark smoke tests.
- [ ] Retain an exact release-candidate performance receipt from the designated
  benchmark host.
- [ ] Record the candidate commit and package checksums in this checklist.

## Release ceremony: intentionally not executed

- [ ] Recheck crates.io name availability and authenticate the owner account.
- [ ] Publish `rustmatch-simd 0.1.0`.
- [ ] Wait for registry availability, then publish `rustmatch 0.1.0`.
- [ ] Verify crates.io metadata and successful docs.rs builds.
- [ ] Smoke-test `rustmatch = "0.1.0"` from crates.io in a clean project.
- [ ] Create and push the signed `v0.1.0` tag.
- [ ] Create the GitHub `0.1.0` release from the curated changelog.
- [ ] Add additional crates.io owners and archive the release receipts.
