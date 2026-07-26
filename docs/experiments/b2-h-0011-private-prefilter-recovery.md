# B2-H-0011 shared candidate recovery

**Outcome:** accepted and merged

**Measured baseline:** `da755b0069c94d4da9db74a8778dd9932fc64671`

**Exact measured candidate:** `bacd5c46d934cd5526dcab78af88e375b2f13370`

**Destination merge:** `de33ea16673d179c5878045b44ec5e4eefa970f7`

## Question

Could H10 retain its 2.18x-to-8.10x shared-candidate target gains while
eliminating the 20%-to-25% regressions on private paths that never activated
the new algorithm?

The preceding symbol and profile investigation found that H10 spent 85.32% of
sampled one-worker instructions in an outlined
`LiteralPrefilter::prefix_allows` call. The baseline and H9 had kept this tiny
predicate inside `scan_starts`. H11 reconstructed H10 from the measured
baseline and added one final source repair:

```rust
#[inline(always)]
fn prefix_allows(&self, input: &[u16], start: usize) -> bool
```

That line is not the optimization by itself. The admitted artifact is H10's
three architectural commits plus this repair, measured and merged as one exact
four-commit descendant.

## Frozen evidence

The G9-v2 contract retained H9 and H10's eleven cells, fixture hashes, worker
counts, two warmups, five retained scans, six balanced AB/BA pairs, five-percent
target floor, and exact three-percent automatic veto. No threshold or workload
changed after seeing a result.

- Candidate artifact SHA-256:
  `64c1ca1886a949f7ad42c908963e6ca0c80b2506f3bf24ed858b27dd0df03f78`
- Baseline artifact SHA-256:
  `c5c1ab91fcf34a479b24da8f8416ee784e6c9a425695a0cb83044ebc19eedcab`
- Measurements SHA-256:
  `e5edcf33480b5618501bb938ed0e95c389f325a55be3fa95f4783117aa8ac0d7`
- Machine summary SHA-256:
  `41900f65fccf2008fc0c43d173cc7dace9af5d5a5fb239874b3778d30742a26d`
- Final evaluation SHA-256:
  `88fefc2df13334380812a59352c517980105ec4458cf412e2298a1283799edf0`
- Formal evidence manifest SHA-256:
  `1477cac5f5b663092899b3922e34b144320e64f36ded51f2c47a59a048b432ed`

The complete measurement-repository lab note is retained at
[`581980d`](https://github.com/la3lma/rmatch-performance-measurements/blob/581980d524e8c083d89cf468d98c7cab06ff8776/docs/lab-notebook/2026-07-26-b2-h-0011-private-prefilter-inlining.md).

## Results

Positive values favor H11:

| Frozen cell | Baseline Mbit/s | H11 Mbit/s | Median effect | Pairs |
|---|---:|---:|---:|---:|
| Sparse 50 MiB, 16 workers | 4,565.54 | 21,140.01 | +364.954% | 6/6 positive |
| Sparse 50 MiB, 24 workers | 2,607.57 | 20,751.27 | +695.703% | 6/6 positive |
| Dense 50 MiB, 16 workers | 3,280.77 | 7,077.72 | +116.181% | 6/6 positive |
| Dense 50 MiB, 32 workers | 1,933.83 | 7,918.39 | +309.131% | 6/6 positive |
| Sparse 7,500-pattern neighbor | 4,617.92 | 22,702.11 | +391.054% | 6/6 positive |
| Dense 5,000-pattern neighbor | 3,622.91 | 9,307.43 | +156.007% | 6/6 positive |
| Mixed-regex semantic guard | 4,616.90 | 23,428.28 | +405.356% | 6/6 positive |
| Sparse 8 MiB fallback | 3,949.82 | 4,002.39 | +1.399% | 6/6 positive |
| Dense 8 MiB fallback | 3,356.03 | 3,388.22 | +0.641% | 5/6 positive |
| Zero-output 50 MiB, one worker | 5,355.94 | 5,458.98 | +1.956% | 6/6 positive |
| Wuthering 8 MiB, 64 workers | 1,420.88 | 1,528.53 | +6.803% | 5/6 positive |

The strongest gate reached 7.96 times baseline throughput. H10's three vetoes
changed from -23.058%, -20.120%, and -24.928% to +1.399%, +0.641%, and
+1.956%. Every receipt preserved exact event counts and normalized digests.

Wuthering's aggregate result was positive but order-sensitive: +9.287% in
baseline-candidate order and +0.053% in candidate-baseline order. Neither
stratum regressed, but this is not evidence for a dependable portable
Wuthering gain.

## Rejected windows

Three pre-measurement windows were rejected and retained:

- two windows exposed a missing frozen-fixture namespace;
- one window detected transient external login-session helper activity above
  the frozen CPU ceiling.

None created a timing receipt. The accepted window passed its host receipt,
kept Ollama, OpenWebUI, and telemetry exited with restart policy `no`, and
restored services cleanly.

## Decision

H11 passed all targets, correctness checks, the exact three-percent veto,
G9-v2's investigation boundary, both order checks, private-path static checks,
and the full workspace suite. It had no demonstrated regression, unresolved
negative, or opposing signal.

The exact measured source was merged without modification. The six production
source files in destination merge `de33ea1` are byte-identical to the measured
candidate. H9 and H10 remain rejected historical artifacts and remain outside
the merged-improvement graph.

## Interpretation

The large shared-candidate benefit did not inherently require a private-path
penalty. Repeated corpus traversal was the target bottleneck; an unintended
compiler boundary was the guard bottleneck. G9-v2's discrepancy rule turned
that contrast into a causal question, and the answer produced an admissible
optimization without relaxing a threshold or averaging away a regression.
