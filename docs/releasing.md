# Rustmatch release runbook

This runbook separates preparation from irreversible publication. The current
task may execute every preparation command, but must stop before the release
ceremony.

## Candidate preparation

1. Start from a clean commit descended from the accepted production baseline.
2. Set matching versions in `rustmatch` and `rustmatch-simd`; leave every
   repository-only package unpublished.
3. Curate `CHANGELOG.md`, the compatibility matrix, limitations, and migration
   note.
4. Run the full repository gate:

   ```bash
   cargo xtask ci
   ```

5. Run the exact package rehearsal:

   ```bash
   scripts/release-readiness.sh
   ```

6. Run Miri and the designated-host performance gate recorded by the
   pre-release checklist.
7. Review package contents, embedded license texts, and checksums under
   `target/package/`.
8. Freeze the candidate commit. Do not use `--allow-dirty` or `--no-verify`
   during the release ceremony.

The rehearsal packages both crates, extracts their normalized archives, binds
the packaged `rustmatch` source to the packaged `rustmatch-simd` source through
a temporary crates.io patch, runs tests/docs/soak, and compiles a clean external
consumer. It uploads nothing.

## Host identity evidence

Release-readiness evidence is meaningful only when its execution environment is
known. The hosted workflow therefore uses reviewed, explicit runner labels
instead of rolling `*-latest` aliases and retains one
`release-host-identity-v1` JSON artifact for every lane. The same identity is
shown in the GitHub Actions job summary.

The record includes the declared runner label, execution mode, target, feature
scope, toolchain, repository revision, operating-system description, system
architecture, GitHub runner image metadata, Cargo version, and complete
`rustc -vV` identity. Native lanes fail identity collection when the declared
target differs from the observed Rust host. Cross-compilation and Miri are
marked `cross-compile` and `interpreter` respectively, so they cannot be
mistaken for native execution.

The local package rehearsal writes the same schema to
`target/release-evidence/host-identity.json` unless
`RUSTMATCH_HOST_IDENTITY_PATH` overrides the destination. Before approving a
candidate, retain the workflow URL and all host-identity artifacts with the
candidate commit and package checksums. Review the runner labels whenever the
provider deprecates an image; do not silently move a release lane to a rolling
label.

These artifacts establish portability provenance, not benchmark equivalence.
Never compare timing from unlike hardware, operating-system images, execution
modes, targets, or toolchains. Performance admission remains bound to its
designated-host receipts.

The macOS release matrix deliberately retains separate Apple Silicon and Intel
lanes. Both execute the same public-crate tests and rustdoc command, while the
identity artifact records the native target and whether AVX2 is available.
Passing on both architectures establishes semantic portability; their hosted
runner timings are not comparable performance evidence.

### Preview runner policy

The native Linux ARM64 lane uses GitHub's explicit `ubuntu-24.04-arm` hosted
runner, which is currently a public preview. A failure before host identity is
recorded is runner infrastructure evidence, not a Rustmatch regression. Check
GitHub's runner status and retry that failed job once without changing source.
If a second attempt cannot allocate or initialize the preview runner, retain
the failed run and continue to describe AArch64 Linux as compile-tested only.
Never substitute emulation or cross-compilation for native execution evidence,
and never use hosted-runner timings as release performance evidence.

The Windows ARM64 lane uses GitHub's `windows-11-arm` public-preview runner and
is deliberately non-blocking. Failure before a host-identity artifact is
written is runner or toolchain infrastructure evidence. Once identity succeeds,
a build, test, or rustdoc failure is a product portability signal and must be
retained as such even though the job does not block unrelated release work.
Retry an infrastructure failure once without source changes; do not retry a
product failure merely to seek a green result.

Windows ARM64 can move into the supported matrix only after GitHub makes the
runner generally available, the lane passes on three distinct main or release
candidate commits, and the owner deliberately approves the support expansion.
Until then, it is best-effort preview evidence and its timings are diagnostic
only.

## Release ceremony: owner action

The following commands are deliberately not run during preparation:

1. Recheck both names on crates.io and authenticate using a narrowly scoped
   token or configured credential provider.
2. Publish the implementation dependency:

   ```bash
   cargo publish --package rustmatch-simd
   ```

3. Wait until `rustmatch-simd = "0.1.0"` resolves from crates.io. Then rerun:

   ```bash
   cargo publish --dry-run --package rustmatch
   cargo publish --package rustmatch
   ```

4. Verify both registry pages, the docs.rs builds, and a clean project using
   `rustmatch = "0.1.0"` without a patch.
5. Create a signed annotated `v0.1.0` tag on the exact published commit, push
   it, and create the matching GitHub release.
6. Add at least one additional crates.io owner and archive the checksums,
   registry URLs, docs.rs results, and smoke output.

## Failure recovery

- If `rustmatch-simd` publishes but `rustmatch` does not, fix the unpublished
  main package and retry only after the full gate. Do not overwrite or republish
  the SIMD version.
- If either published crate is defective, yank the affected version and
  prepare a new patch release. Published versions are never overwritten.
- A failed docs.rs build does not justify republishing identical source; fix it
  in a new version unless docs.rs can rebuild the existing package unchanged.
- Never move or recreate a release tag to disguise a publication mismatch.
