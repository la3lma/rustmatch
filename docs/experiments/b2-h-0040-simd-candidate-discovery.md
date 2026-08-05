# B2-H-0040 exact SIMD candidate discovery

**Decision:** rejected before full G9; not merged

**Production baseline:** H11
`bacd5c46d934cd5526dcab78af88e375b2f13370`

**Initial candidate:** `58c1eb14bd16ed95d37cac9440111fe3a1136205`

**Recovery candidate:** `843ebafe947fd8749c0f8e843af4ce480b40c3b4`

**Review date:** 2026-08-04

## Question

H11 still applies a scalar five-unit prefix predicate at every input start
while constructing one shared candidate bitmap. H40 asked whether an exact
AVX2 kernel could replace that scalar pass without changing H11's conservative
candidate semantics or regressing paths that cannot activate SIMD.

The bounded kernel computes eight adjacent low-20-bit FNV-1a hashes in AVX2
lanes and gathers the unchanged H11 union bitmap. Non-ASCII lanes are admitted
conservatively. The unchanged per-partition NFA remains the semantic authority,
and audited intrinsics are confined to a private crate behind a safe API.

## Headroom gate

The frozen isolated discriminator compared scalar and SIMD candidate plans on
the four established sparse/dense 50 MiB cells. All 24 pairs passed:

| Cell | Candidate-plan speedup | Predicted complete-scan gain |
|---|---:|---:|
| Sparse, 16 workers | 4.491x | 63.553% |
| Sparse, 24 workers | 4.559x | 62.874% |
| Dense, 16 workers | 4.641x | 22.707% |
| Dense, 32 workers | 4.413x | 25.203% |

Candidate bitmaps, counts, events, and digests were exact. This authorized one
bounded production candidate, not admission.

## Initial production screen

The first production candidate vectorized density sampling and full candidate
fill. It passed the complete test and static-correctness suite, and every
target scan pair was positive:

| Target | Scan effect | Wall effect |
|---|---:|---:|
| Sparse 50 MiB, 16 workers | +63.897% | +40.464% |
| Sparse 50 MiB, 24 workers | +61.889% | +39.641% |
| Dense 50 MiB, 16 workers | +22.110% | +19.099% |
| Dense 50 MiB, 32 workers | +25.227% | +20.228% |

Both inactive guards executed no SIMD. The ordinary one-worker
candidate-before stratum nevertheless regressed 3.750% in scan, 6.747% in
preparation, and 3.325% in wall. Wuthering candidate-after wall regressed
4.202%. The exact candidate was blocked and preserved.

## Bounded recovery

Static analysis showed complete mnemonic identity in the private scanner but
different placement. A control rebuild established that accepted H11 is
byte-reproducible only under its original B2-H-0004 Cargo and Rustup roots.

The one allowed recovery therefore:

1. restored H11's scalar density sampler byte-for-byte;
2. retained SIMD only for the full 50 MiB candidate fill; and
3. used the build roots that reproduce accepted H11 exactly.

No padding, function-order tuning, benchmark-specific placement, threshold,
cell, or repetition changed. The timed harness, private scanner, and shared
scanner retained H11's exact sizes and complete mnemonic sequences. The total
load image grew by only 24 bytes, versus 4 KiB for the initial candidate under
matched provenance.

## Frozen recovery result

The guarded recovery completed 72 fresh processes with exact event identity,
no contamination, an idle GPU, and complete service restoration.

| Cell | Scan | Before | After | Wall | Ruling |
|---|---:|---:|---:|---:|---|
| Sparse 50 MiB, 16 workers | +63.445% | +64.441% | +63.339% | +41.658% | Passed, 6/6 scan pairs |
| Sparse 50 MiB, 24 workers | +62.189% | +62.215% | +62.164% | +40.596% | Passed, 6/6 scan pairs |
| Dense 50 MiB, 16 workers | +22.937% | +23.050% | +22.825% | +19.519% | Passed, 6/6 scan pairs |
| Dense 50 MiB, 32 workers | +25.241% | +25.792% | +25.203% | +20.311% | Passed, 6/6 scan pairs |
| Ordinary 1 MiB, one worker | -0.186% | -0.396% | +0.803% | -1.061% | Before-order wall -2.266%; investigation boundary |
| Wuthering 8 MiB, 64 workers | -1.895% | -0.401% | **-5.272%** | -1.423% | After-order scan and RSS automatic veto |

The recovery retained nearly all target gain despite scalar sampling. It still
failed: Wuthering candidate-after scan regressed 5.272% and peak RSS regressed
3.321%, both beyond the unchanged automatic veto. The ordinary candidate-before
wall stratum independently regressed 2.266%.

Neither failing guard can execute SIMD. That establishes an inactive-path
code-generation or process-layout cost, not an algorithmic cost. It does not
authorize a waiver: frozen guardrails apply to the production artifact, not
only to the lines executed by the intended target.

## Decision

H40 is rejected before full G9. Neither candidate is merged, H11 remains the
production baseline, and the cross-engine comparison stays unchanged. A
second layout-recovery cycle would risk the source padding, function ordering,
or benchmark-specific placement tuning explicitly excluded by the contract.

The positive signal remains valuable: exact SIMD candidate fill delivered
22.9%-63.4% complete-scan gains on the intended cells. Revisit it only after a
generally applicable stable production-layout boundary or as part of a
materially different exact literal engine. The next independent experiment is
an exact literal-only backend diagnostic.

## Retained evidence

- Discriminator summary SHA-256:
  `d4f253a5a2892206890d3f77658ae66304a42db2e2eb4e37ffdfccbb1febe6c6`
- Initial focused summary SHA-256:
  `cf18c96c07227787e4311661041cd4db71ad38e783290093bb125bee978037c8`
- Initial focused archive SHA-256:
  `922299e312b12113f070c06b3d32cd0b0bae9ecf0b2b6208f43e302d2a46c686`
- Recovery plan SHA-256:
  `2a5f1c53557ba7709779b4585d11b25d9823b0f15b5a138768ba77d0c7e91679`
- Recovery summary SHA-256:
  `d074bb222c10a61df92c28cc03de413745b698161ddd5d9509f46dc001c20b28`
- Recovery window-state SHA-256:
  `0dcc5407b4436f2c47ac19448b47bc1717e15349aa5dd7d3bd0f0a74ee004f4c`
- Recovery archive SHA-256:
  `101ef0b52f5c7c67ebd3e19b5f7fcdaffb7b2a057e2fc653fcaedd8059574b88`
- Recovery source bundle SHA-256:
  `78d03a3b6abdfda241d181527953dcddf8e98a7ebf15fa4b9a0e394ea410fdaf`
- Measurement decision commit: `8bb1e8a`
