# I7 evidence: conservative start acceleration

This directory retains the admission evidence for implementation
`37f69819d9231fb99d747d4ae48034204657c1f2` against the frozen protocol in
[`docs/experiments/i7-safe-prefilter.md`](../../../experiments/i7-safe-prefilter.md).
The build-cost baseline is
`284571996b343041d374f2620905f32d63dbafdc`, the pre-I7 merge revision.

## Environment and evidence identity

Every admission timing was produced by a verified Mach-O arm64 release binary
on:

- `rainstick.local`
- Apple M2 Max
- 12 logical CPUs
- macOS 26.5
- Rust 1.97.0 standard library for `aarch64-apple-darwin`
- three warmups and seven measured scans
- a 256 MiB cache-scrub working set before every scan

The compiler process was an x86_64 Rust 1.97.0 toolchain cross-building native
arm64 binaries. Rustmatch's reported pattern-build and scan timings ran inside
the native binary. The raw JSON in `focused/` and `wuthering-8m/` is
authoritative; this document is its critical interpretation.

## Correctness result

The complete workspace suite passes, including deterministic generated
filterable pattern families, structural-prefix proofs, optional and repeated
terms, alternation, predicates, case-insensitive syntax, Unicode UTF-16 units,
assertions, input-end matches, mixed filterable and unfilterable sets, and
candidate-bitmap ordering. Every focused and Wuthering on/off pair produced an
identical event count and digest.

The prefilter never creates match events. It admits possible start positions;
the I6 semantic engine verifies them and remains the only source of observable
events. Assertions bypass start acceleration, and uncertain prefix analysis
admits work rather than risking a false negative.

## Focused admission result

All frozen focused gates passed. Times are medians.

| Scenario | Patterns | Disabled | Enabled | Change | Selected path |
|---|---:|---:|---:|---:|---|
| literal sparse | 1,000 | 45.157 ms | 18.548 ms | 58.92% faster | literal filter |
| literal sparse | 5,000 | 48.118 ms | 20.653 ms | 57.07% faster | literal filter |
| literal dense | 1,000 | 89.432 ms | 64.705 ms | 27.64% faster | literal filter |
| mixed sparse | 1,000 | 44.622 ms | 18.791 ms | 57.88% faster | literal filter |
| assertion bypass | 1,000 | 76.122 s | 76.224 s | 0.13% slower | all starts |
| mixed unfilterable | 1,000 | 46.570 ms | 6.434 ms | 86.18% faster | start table |
| short literal | 5,000 | 0.992 ms | 0.720 ms | 27.47% faster | start table |

The assertion result is a guard, not a positive claim. Both paths perform the
same all-start semantic scan; 0.13% on a 76-second run is below the frozen 3%
and 1 ms conjunctive failure threshold. It demonstrates bypass neutrality, but
also exposes assertion-heavy scanning as an expensive unresolved path.

The short-input result is only 0.273 ms in absolute terms. It confirms that the
full input-sized candidate bitmap is not allocated below the activation
threshold; it is not presented as a meaningful application-speed claim.

## Wuthering Heights scale result

The retained 8 MiB campaign uses a deterministic expansion of the historical
Wuthering Heights corpus. Exact event evidence passed at every pattern count.

| Patterns | Events | Disabled | Enabled | Saved | Improvement |
|---:|---:|---:|---:|---:|---:|
| 1,000 | 109,693 | 56.066 ms | 35.995 ms | 20.071 ms | 35.79% |
| 5,000 | 926,975 | 148.888 ms | 108.565 ms | 40.323 ms | 27.08% |
| 10,000 | 1,912,854 | 296.844 ms | 251.244 ms | 45.601 ms | 15.36% |

Relative improvement declines as the pattern set and emitted event set grow,
although absolute time saved still increases. This is not constant-time
pattern scaling. At 10,000 patterns the filter removes most impossible starts,
but the remaining candidate starts drive cache pressure, semantic transition
construction, and delivery of almost two million events.

| Patterns | Candidate starts | Starts skipped | Candidate density | Starts skipped |
|---:|---:|---:|---:|---:|
| 1,000 | 100,411 | 8,288,197 | 1.20% | 98.80% |
| 5,000 | 791,212 | 7,597,396 | 9.43% | 90.57% |
| 10,000 | 1,598,884 | 6,789,724 | 19.06% | 80.94% |

Candidate density rises with pattern count because more prefixes are admitted
and because hash collisions are intentionally false-positive only. The trend
explains part of the diminishing relative return. Improving selectivity could
remove more semantic work; merely making the current filter loop cheaper has a
much smaller ceiling.

## Resource and build result

The 10,000-pattern filter retains 620,896 bytes, well below the frozen 8 MiB
limit. The scan-local candidate bitmap uses 1,048,576 bytes for 8,388,608 input
starts, exactly one bit per start. The start-table path allocates no candidate
bitmap. I7 adds no runtime dependency and uses no unsafe Rust.

Median 10,000-pattern build time changed from 7.673 ms on the frozen pre-I7
revision to 17.261 ms for the candidate, a 9.587 ms absolute regression. The
relative increase is large because the baseline operation is short, but the
measured cost remains below the precommitted 25 ms reusable-matcher allowance.
It is a real setup cost and should not be hidden in scan-only claims.

## Profile interpretation

A companion native `xctrace` Time Profiler run used 10,000 patterns and a
50 MiB expanded Wuthering corpus. Its 8,083 samples are summarized in
`profiles/native-arm64-summary.tsv`:

| Category | Samples |
|---|---:|
| semantic scan and event aggregation | 36.1% |
| semantic transition construction | 29.7% |
| literal filter scan | 17.6% |
| deterministic cache lookup | 12.6% |
| terminal recording, candidate iteration, build, scrub, and unresolved | 4.0% |

The raw Instruments trace remains local because it is large. This profile used
a production-equivalent build immediately before the evidence commit; later
changes only renamed a diagnostic field and added documentation. It is
directional hotspot evidence, not a substitute for the revision-exact JSON
admission timings.

Even deleting the complete measured filter cost could recover at most about
17.6% in that profile. The larger remaining opportunity is reducing or
parallelizing semantic work for admitted starts while preserving exact event
ordering and multiplicity. I8 should test that hypothesis rather than assuming
that the Java implementation's partitioning choices transfer to Rust.

## Rejected prototypes

The first implementation used a sparse Aho-Corasick-style trie with failure
links. It retained about 1.83 MiB at 5,000 Wuthering patterns, but candidate
generation became a dominant native-profile cost. Small-node linear lookup and
an anchored-trie variant did not recover enough time. Those variants were
removed rather than retained as speculative complexity.

The accepted compact filter uses a direct table for exact three-unit ASCII
prefixes and one-hash bitsets for four-unit and first-five-unit prefixes. Hash
collisions add verification work but cannot remove a match. This result is
specific to the current engine and workloads; it is not a general claim that
trie or Aho-Corasick prefilters are poor designs.

## Critical conclusion

I7 is admitted because exact semantics, every frozen positive and guard gate,
resource bounds, and native positive evidence all pass. The implementation
removes substantial impossible-start work on both synthetic fixtures and the
retained realistic corpus without exposing prefilter machinery in the public
API.

I7 does not solve assertion-heavy scans, prove the final activation policy,
establish cross-engine superiority, or prevent result-heavy scaling from
approaching linear growth over this range. Wuthering Heights remains an
appropriate stable regression corpus at the current performance level. Larger
and more diverse corpora should supplement it after I8 and later optimizations
make this line too fast or too narrow to expose the next bottleneck.
