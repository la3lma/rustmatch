# Rustmatch 0.1.0 pre-release checklist

**Target:** prepare a reproducible `0.1.0` release candidate without publishing,
tagging, or creating a GitHub release.

**Hard stop:** no command in the preparation workflow may upload to crates.io,
create a release tag, or create a GitHub release. Those actions remain unchecked
until the owner explicitly starts the release ceremony.

**Current qualification:** H43 production merge `4996bcc` supersedes frozen
candidate `7305a24`. The earlier candidate evidence remains valid only for its
exact historical source. Do not release current `main` until the H43
requalification below is complete.

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

- [x] Add and pass non-publishing release-readiness CI for Linux, macOS,
  Windows, MSRV, AArch64 compilation, Miri, package rehearsal, and consumer
  smoke testing.
- [x] Pin every release-readiness lane to a reviewed runner image and retain
  its exact host/toolchain identity artifact. The pinned matrix passed in
  [release-readiness run 31330109699](https://github.com/la3lma/rustmatch/actions/runs/31330109699)
  with native Rust hosts `x86_64-unknown-linux-gnu`,
  `aarch64-apple-darwin`, and `x86_64-pc-windows-msvc`; cross-compilation and
  Miri remain explicitly distinguished from native evidence.
- [ ] Retain one successful native `aarch64-unknown-linux-gnu` run of the
  all-feature public crates, rustdoc, and bounded release soak before promoting
  Linux ARM64 from compile-tested to native-tested.
- [x] Pass the full repository quality and differential-evidence gate.
- [x] Pass release-profile semantic and benchmark smoke tests.
- [x] Retain an exact release-candidate performance receipt from the designated
  benchmark host in the
  [candidate evidence archive](evidence/release/0.1.0/7305a24/README.md).
- [x] Record the candidate commit and package checksums in this checklist:
  `7305a24ba0f67219ea0d3026e45db5ad22d71734`,
  `rustmatch-simd` SHA-256
  `07825e3a04dbe888c6729c42be126865d9cda6aaf60d3df38e38325b588393ad`,
  and `rustmatch` SHA-256
  `cdff31c0a0ae63ff54734c03029a553b61ea373dadfdfee9feb06596fa707ceb`.

## H43 production requalification

- [x] Retain the exact H43 R3 rejection, owner exception, and causal follow-up.
- [x] Preserve all nine passing default-safety cells and the adverse 5.792673%
  formal preparation cell without relabeling either result.
- [x] Freeze post-exception package candidate `dbb80cb` with `rustmatch-simd`
  SHA-256 `5cdc9fbc4027cb993fc742507aa748b59cf9bc0a969972fa5fdf1e6419ffc3cc`
  and `rustmatch` SHA-256
  `ea9fe938233883c62f02c12dcd37b5be4af95c9657f7c7e1615cbf8397698572`.
- [x] Pass the exact package, all-features, rustdoc, release-soak, and clean
  consumer rehearsal for that candidate.
- [x] Pass Linux, macOS, Windows, MSRV, AArch64, Miri, differential,
  ledger, and release-readiness CI for merged H43 evidence head `e1230e9`:
  [CI run 31309147209](https://github.com/la3lma/rustmatch/actions/runs/31309147209)
  and [release-readiness run 31309147230](https://github.com/la3lma/rustmatch/actions/runs/31309147230).
- [x] Bind the designated-host R3/X1.8 receipts and local package rehearsal to
  the [H43 release-candidate evidence record](evidence/release/0.1.0/dbb80cb/README.md).

## Release ceremony: intentionally not executed

- [ ] Recheck crates.io name availability and authenticate the owner account.
- [ ] Publish `rustmatch-simd 0.1.0`.
- [ ] Wait for registry availability, then publish `rustmatch 0.1.0`.
- [ ] Verify crates.io metadata and successful docs.rs builds.
- [ ] Smoke-test `rustmatch = "0.1.0"` from crates.io in a clean project.
- [ ] Create and push the signed `v0.1.0` tag.
- [ ] Create the GitHub `0.1.0` release from the curated changelog.
- [ ] Add additional crates.io owners and archive the release receipts.
