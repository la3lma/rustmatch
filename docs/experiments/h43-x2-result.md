# H43-X2 isolated separate-type prototype result

**Date:** 2026-08-07<br>
**Baseline:** `e4a7a65efe7f06aa8dbf42ca04aa1d3e7ebc4668`<br>
**Candidate:** `ff1ac2fdd0fe1370c33709aff9451c15c017f5fa`<br>
**Candidate tree:** `efc83981432816a98662b8ec6b40a6c2465d2407`<br>
**Contract:** ADR-0009<br>
**Disposition:** local gates passed; exclusive-host utility evidence authorized<br>
**Production state:** unchanged; no publication or release-baseline merge

## Decision

H43-X2 passes its local source, semantic, policy, and default-isolation gates.
The proven H43-A1 assertion-prefix mechanism now has a real but non-default
product boundary:

- Cargo feature `unstable-assertion-prefix-v1` is disabled by default;
- callers must explicitly construct
  `experimental::assertion_prefix_v1::AssertionPrefixMatcherBuilder`;
- statically ineligible sets return typed experimental build errors;
- every scan names either `RequireSpecialized` or `AllowExactFallback`;
- required specialization refuses short or dense inputs before callback
  delivery and before an expensive generic scan;
- allowed fallback preserves exact matching and reports the reason; and
- reports expose static eligibility, activated backend, fallback reason,
  candidate count, and candidate bytes.

This is not an admission decision. H43-A1 remains rejected under its frozen
automatic-path campaign. X2 authorizes a fresh X1.5 utility plan and
exclusive-host window for the explicit API only.

## Exactness and policy evidence

Six dedicated feature-only tests prove:

1. all three static ineligibility classes are typed;
2. a sparse one-MiB input activates v1 and emits the exact expected UTF-16
   events, including isolated-surrogate boundaries;
3. short `RequireSpecialized` refuses with zero callbacks;
4. short `AllowExactFallback` matches the ordinary matcher event multiset and
   reports `InputTooShort`;
5. a dense one-MiB sample refuses before generic work or callback delivery;
6. callback panic propagates without poisoning matcher reuse.

The existing bounded candidate-bitmap, assertion semantics, generated oracle,
failure atomicity, panic/reuse, and full public lifecycle tests remain part of
the all-features suite. All 79 crate unit tests, integration/property tests,
walking-spine tests, and doctests passed. Strict all-target Clippy and rustdoc
also passed. The complete `cargo xtask ci` pipeline passed, including Java
differential oracles, H43-E1 semantic parity, and B0 benchmark smoke.

## Default isolation

The same normal lifecycle probe was compiled against three release artifacts:

| Configuration | `MatcherBuilder` | `Matcher` | `Error` | Normal events |
|---|---:|---:|---:|---|
| Pre-X2 baseline, feature off | 96 B / align 8 | 24 B / align 8 | 24 B / align 8 | 4, digest `9b418452fceef3cc` |
| X2 candidate, feature off | 96 B / align 8 | 24 B / align 8 | 24 B / align 8 | 4, digest `9b418452fceef3cc` |
| X2 candidate, feature on but unused | 96 B / align 8 | 24 B / align 8 | 24 B / align 8 | 4, digest `9b418452fceef3cc` |

Normalized instruction hashes were identical in all three configurations for
ordinary matcher construction, pattern registration, parallel dispatch,
partition scanning, transition computation, and epsilon closure. Whole archive
hashes are deliberately not treated as an isolation metric because the
feature-on artifact correctly contains additional opt-in symbols and source
metadata.

The implementation adds no field, variant, method, or scan branch to normal
`MatcherBuilder`, `Matcher`, or `Error`. Feature-only state lives in the
separate matcher, and specialization activation is not added to ordinary
`ScanStats`.

## Remaining gates

X1.5 must freeze and run a new explicit-capability utility campaign. It must
measure target, one-unit-below-boundary, sparse/dense boundary, fallback,
prefix-mismatch, mixed-cohort, preparation, allocations, RSS, and total work
for at least `N = 1` and `N = 10`. It must prove the capability envelope and
break-even count without changing H43-A1's historical result.

Publication and merge into the release baseline remain blocked until that
window, critical review, API/SemVer review, and owner admission decision all
pass. Any automatic selector remains a separate full G9 program.

## Evidence

- [Local evidence index](../evidence/h43/x2/ff1ac2f/README.md)
- [Machine-readable receipt](../evidence/h43/x2/ff1ac2f/local-certification.json)
- [ADR-0009](../adr/0009-explicit-experimental-matcher.md)
- [Explicit capability strategy](h43-explicit-risk-tier-design.md)
