# B2-H-0039 partition-aware candidate routing

**Decision:** rejected before full G9; not merged

**Production baseline:** H11
`bacd5c46d934cd5526dcab78af88e375b2f13370`

**Candidate:** `3bad489ad3bc3221f1cf1e269fd95a885b3f17f5`

**Review date:** 2026-07-29

## Question

H11 scans the input once to produce a union candidate bitmap, but every
semantic partition still applies its private prefix predicate to every union
candidate. H39 asked whether computing a partition mask once per candidate
could preserve exact matching while removing enough repeated routing work to
justify the additional representation.

H24 remains pinned separately. H39 was the highest-ranked independent
opportunity after H25-H38 closed the current H24 recovery route.

## Headroom gate

An instrumentation-only diagnostic measured the removable private-prefix
routing phase before production code was authorized. Sparse 10,000-pattern
cells exposed only 3.607%-4.084% routing share and failed the frozen five
percent headroom floor. Dense cells exposed:

| Cell | Routing / complete scan | Pairs above 5% |
|---|---:|---:|
| Dense, 16 workers | 8.383% | 6/6 |
| Dense, 32 workers | 11.425% | 6/6 |

All 48 diagnostic receipts preserved exact event and digest identity. The
dense result authorized one bounded candidate; it did not authorize admission.

## Candidate

The exact candidate:

- retained H11 for sparse inputs, more than 32 partitions, and unsupported
  coordinates;
- used a 64 KiB density sample to select dense eligible inputs;
- compiled compact prefix-to-`u32` partition-mask tables;
- emitted ordered `(u32 start, u32 mask)` route entries in the existing
  parallel candidate scan;
- retained the exact per-partition NFA as semantic authority.

The complete workspace tests, doctests, formatting, and warning-free Clippy
passed. The focused screen also proved exact activation: dense route storage
was 5,574,800 bytes for 696,850 starts, exactly eight bytes per start, while
both sparse artifacts retained the unchanged 6,553,600-byte bitmap.

## Frozen focused result

The guarded screen completed 72 fresh processes and 216 raw result files with
exact correctness, no host violation, and complete service restoration.

| Cell | Scan | Wall | Peak RSS | Ruling |
|---|---:|---:|---:|---|
| Dense, 16 workers | +4.894% | +5.243% | -0.914% | Missed the frozen 5% scan target by 0.106 percentage points |
| Dense, 32 workers | **+9.529%** | **+9.134%** | -0.571% | Target passed; all 6 scan pairs positive |
| Sparse, 16 workers | +0.805% | -0.633% | -0.016% | Bypass guard passed |
| Sparse, 24 workers | -0.286% | -0.002% | +0.012% | Bypass guard passed |
| Ordinary, 1 worker | +0.031% | -0.361% | +0.517% | Guard passed |
| Wuthering, 64 workers | **-3.822%** | **-3.456%** | +1.403% | Automatic regression veto |

The dense mechanism is real: both dense cells improved in all six scan pairs.
The Wuthering loss is also real enough for the unchanged policy: scan was
negative in five of six pairs and wall was negative in all six, with both
launch orders negative. Wuthering cannot activate the new route path, so the
loss is most likely an inactive-path binary-layout or code-generation effect.
That causal interpretation is useful research evidence, but it does not waive
a demonstrated guard regression.

## Decision

H39 is rejected before the full G9 matrix for two independent reasons:

1. dense-16 missed its frozen target;
2. Wuthering scan and process wall crossed the unchanged three-percent
   automatic veto.

The candidate is not merged and H11 remains production. The result should not
be described as a failed idea: compact routing retained 4.894%-9.529% dense
scan gains and reduced dense scan-local candidate storage by 14.93%. It is
instead a failed production artifact under the current gate.

A successor is not the next active experiment. Revisit routing only if a
materially different representation or stable isolation method can preserve
the dense gain while supplying independent evidence against inactive-path
regression. Do not tune the frozen density threshold or waive the guard.
SIMD shared candidate discovery is now the highest-ranked independent
candidate.

## Retained evidence

- Focused plan SHA-256:
  `bd941a78d609d9ec88e7dfb22b8b4f19523246395ccf375d9292427b25db8145`
- Focused summary SHA-256:
  `e5acb80477af4462eb4e344a3358f39987216e41d01a9472e3ff007a576244ed`
- Focused window-state SHA-256:
  `9a04f30de7b329444dd42ac9d3b11b2eb731e29844ed6908c47de390977ca5e5`
- Focused evidence archive SHA-256:
  `1832d432b644e968201e693695b04a423bf1c546dc34745e15f51be20b7a1e83`
- Headroom evidence archive SHA-256:
  `5599e7ade5d8bbbdbea9df9cbc1757872558811126f8183121fd3509eef418c3`
- Measurement decision commit:
  `a910125`
