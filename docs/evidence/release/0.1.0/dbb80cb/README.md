# Rustmatch 0.1.0 H43 prerelease candidate evidence

This directory archives the non-publishing package verification performed
after the H43 owner-authorized production exception.

## Frozen source and packages

| Item | Value |
|---|---|
| Candidate commit | `dbb80cb8f6c9286aa6d5db59d3d744c1b412795c` |
| Owner-exception merge | `4996bcc16a9cc936789372d8184f4ef4893fd336` |
| Upstream `main` parent | `79cc8e9b4438cb4cabd482789715a16ff9a834d8` |
| Reviewed H43 parent | `75164d9a27fe8a5f744ff838e661791061347c2f` |
| Exact measured H43 source | `a5b86c2b3943aad3121e1f06944e39331f17a474` |
| `rustmatch-simd-0.1.0.crate` SHA-256 | `5cdc9fbc4027cb993fc742507aa748b59cf9bc0a969972fa5fdf1e6419ffc3cc` |
| `rustmatch-0.1.0.crate` SHA-256 | `ea9fe938233883c62f02c12dcd37b5be4af95c9657f7c7e1615cbf8397698572` |
| Package rehearsal | Passed from the clean candidate commit without publishing |

This page is intentionally retained in a later evidence-only commit. The
source and package identity under test remains the candidate above.

## Local qualification

The clean candidate passed:

- the complete `cargo xtask ci` program, including formatting, clippy, fuzz
  target builds, workspace tests, doctests, rustdoc, MSRV, generated-ledger
  freshness, Java compatibility oracles, H43 cohort parity, and B0 smoke;
- package-content and license checks for both crates;
- packaged workspace tests with all features enabled;
- the packaged optimized release soak;
- packaged rustdoc with warnings denied; and
- a clean external consumer build and execution.

The rehearsal found and fixed one pre-package integration defect before this
candidate was frozen: two `rustmatch-bench` output variants referenced
experimental-only receipt types in the default-feature build. The variants are
now gated with the same `experimental-showcase` feature as their types. This
changes no library scan path and no retained timing.

## Performance and decision evidence

No timing was rerun during packaging. The production decision remains bound to
the designated-host evidence already reviewed:

- [H43-X1.9 owner decision](../../../../experiments/h43-x1-9-owner-exception.md)
- [immutable R3 confirmation](../../../../experiments/h43-x1-7-r3-confirmation-result.md)
- [exact-source construction follow-up](../../../../experiments/h43-x1-8-conditioned-preparation-result.md)
- [curated R3 evidence](../../../h43/x1.7/a5b86c2/README.md)
- [curated X1.8 evidence](../../../h43/x1.8/8c27d23/README.md)

Formal R3 remains rejected. The 5.792673% preparation result and every adverse
receipt remain retained; the owner exception does not convert them into a
certified pass.

## Remote qualification

The cross-platform GitHub matrices are intentionally not claimed by this local
record. They must pass for the merged head before a release ceremony begins.

No crate, tag, or GitHub release was created during this verification.
