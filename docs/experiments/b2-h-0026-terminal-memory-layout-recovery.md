# Terminal-memory and scan-layout recovery

**Status:** H22-H38 retained; no new merge; H11 remains production

This review follows the assertion-prefix recovery from its last exceptional
scan candidate through the final attempt to separate construction-memory gains
from unrelated scan regressions. All thresholds, fixtures, order rules, and
semantic requirements remained unchanged.

## B2-H-0022 and H23

H22 derived strict assertion-prefix facts during NFA construction and stored
five bytes in existing padding. Exact target scans improved about 203x, 203x,
and 407x, while primary guards remained neutral. Boundary-500 preparation
regressed 6.931% and vetoed admission.

H23 tested whether folding the bounded prefix walk into recursive NFA
construction would remove that cost. It did not: boundary-500 build time
worsened 1.175% with 10 of 12 observations slower. H23 was rejected before a
dense or formal run.

## H24 and H25

H24 stored empty and singleton pending-edge lists inline and spilled only real
branches. It retained roughly 200x-406x target scan gains and improved
preparation across the entire formal matrix. One adversarial candidate-before
peak-RSS stratum was 2.360% worse, so the frozen result remained
`investigate`.

H25 reduced temporary `PendingState` from 40 to 32 bytes through a nonzero
terminal ordinal. Deterministic probes found 10.80%-14.31% fewer allocated
bytes and 7.55%-8.45% lower peak live allocator bytes. The unrelated Wuthering
scan nevertheless regressed 4.253% against H24, blocking formal G9.

## H26 causal separation

Exact H26 revision
`e0c33572fbb9ddd386da50151954d9b4ebb09a38` replaces the heap-backed terminal
representation with an inline sentinel. Against exact H24
`6cc6da3887555345f78b557cd47efdf92568bd21`:

- allocated bytes fell 14.30% on assertion boundary/adversarial shapes;
- allocated bytes fell 10.80% on ordinary literals;
- peak live allocator bytes fell 7.55%-8.45%;
- the 96,000-build construction repeat passed;
- retained immutable bytes and event semantics remained identical.

Natural matched Wuthering was 3.269% slower. Static analysis found the same hot
instructions shifted by 48 bytes because preceding unwind metadata shrank by
48 bytes. A fixed-address, instruction-identical discriminator recovered H26
to 0.535% faster, with both order strata inside two percent.

Hardware counters reinforced that finding: natural H26 retired 0.15% fewer
instructions but used 4.78% more cycles and delivered 4.71% lower IPC.
Construction memory and scan work were therefore separated; binary placement,
not the sentinel algorithm, caused the observed scan loss.

## Recovery attempts

| Attempt | Mechanism | Decisive result |
|---|---|---|
| H28 | LLVM innermost-loop alignment | -1.781% scan aggregate; -3.492% candidate-before |
| H30-H32 | Source-level StartTable iterator reshaping | H32 -11.050% aggregate and about -11% in both strata |
| H33-H35 | Kernel, StartTable, and fallback isolation | Static gates found call/ABI traffic or unsuitable placement; no timing |
| H36 | Stable Rust PGO | V2 -25.357% scan; bounded v3 failed frozen path coverage before build |
| H37 | Matched ThinLTO | -4.358% scan aggregate; -5.960% candidate-before |
| H38 | Matched instrumented BOLT placement | +1.266% aggregate, but +4.044% first and -4.710% second |

The H32 artifact-mutation incident was caught by SHA-256 before timing; no
affected measurement was admitted.

## H38 final separation

H38 rebuilt exact H24 and H26 with relocations, instrumented each exactly once
on the same assay-disjoint 64-worker StartTable workload, and applied one
frozen BOLT 20.1.8 policy. It changed no host package, privileged profiler,
sysctl, source, fixture, or threshold.

BOLT independently produced the same production scan hot-fragment address
(`0x60108e`), size (`0x210e`), and 64-byte phase (`14`) for both variants.
`llvm-boltdiff` attributed 97.96% of execution to the production scan
specialization in each profile and showed effectively identical hot-block and
hot-edge distributions. H26's total instrumented trace differed by only
0.001642%; the only content differences with measurable profile weight were
construction functions at about 0.03%.

The clean, exact Wuthering screen favored H26 by 1.266% in aggregate, but its
scan result crossed from 4.044% faster when first to 4.710% slower when second.
Process wall likewise regressed 4.048% in the candidate-after stratum. The
frozen rule requires every aggregate and order stratum inside two percent.
H38 therefore failed, and its one-shot contract prohibited stage two or
benchmark-informed BOLT tuning.

## Decision

No H22-H38 candidate is mergeable. The evidence is strong enough to state:

1. The H26 memory gain is real.
2. H26 does not add meaningful production scan work.
3. The natural scan regression is code-placement sensitive rather than an
   intrinsic cost of the sentinel representation.
4. No tested source, PGO, ThinLTO, or post-link policy produced a reproducible
   artifact that passed every unchanged order-stratum gate.

H11 remains the production baseline. H26 should be revisited only after an
independently justified, stable production hot-code placement policy exists,
or after fresh-process screening can demonstrate order stability for that
policy. The memory win is worth retaining as research evidence, but not at the
cost of admitting a known unresolved guard failure.
