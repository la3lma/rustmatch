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
