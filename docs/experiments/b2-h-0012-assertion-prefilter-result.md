# B2-H-0012 assertion prefilter result

**Outcome:** investigate; not admitted and not merged

**Machine disposition:** rejected under G9-v2

**Production baseline remains:** `bacd5c46d934cd5526dcab78af88e375b2f13370`

## Result at a glance

H12 proved that assertion-bearing pattern sets can safely use conservative
literal candidates. It also failed the unchanged keeper gate.

| Frozen cell | Baseline Mbit/s | H12 Mbit/s | Median effect | Direction |
| --- | ---: | ---: | ---: | --- |
| boundary 1,000, 1 MiB, one worker | 0.981 | 195.210 | +19,745.379% | 6/6 positive |
| boundary 1,000, 1 MiB, two workers | 2.128 | 393.188 | +18,421.004% | 6/6 positive |
| boundary 1,000, 2 MiB, one worker | 0.983 | 378.253 | +38,435.392% | 6/6 positive |
| boundary 500, 1 MiB neighbor | 1.969 | 380.469 | +19,223.628% | 6/6 positive |
| mixed assertion adversary | 5.599 | 4,029.000 | +72,035.393% | 6/6 positive |
| below-threshold 512 KiB guard | 0.982 | 0.981 | -0.133% | 3/6 positive |
| dense assertion guard | 1.815 | 1.824 | +0.431% | 6/6 positive |
| unfilterable assertion guard | 0.974 | 0.982 | +0.922% | 5/6 positive |
| ordinary literal guard | 4,630.570 | 4,640.926 | +0.307% | 4/6 positive |
| H11 shared guard | 21,538.240 | 21,552.446 | -0.270% | 3/6 positive |
| Wuthering guard | 1,454.076 | 1,423.118 | -3.038% | 0/6 positive |

The target ratios are approximately 199, 185, and 385 times baseline
throughput. The adversarial neighbor is approximately 720 times baseline.
Every event count and normalized digest matched exactly.

## Why it did not merge

The Wuthering guard crossed the frozen three-percent automatic veto:

- primary scan throughput: `-3.037813%`, six of six pairs negative;
- process wall time: `-3.479505%`, six of six pairs negative.

Assertion preparation also regressed `11.160835%` through `22.315753%` because
H12 proves and compiles a filter instead of immediately bypassing prefilter
construction. Two small RSS effects crossed three percent. Those are real
costs even though the scan savings dominate total elapsed time on the targets.

The decision does not relax or reinterpret the frozen thresholds. Exact
candidate `169c47c0c1360dbbf4653834bd144d9a66a14093` is not production code and
does not enter the accepted-progress graph.

## Causal assessment

Wuthering contains no assertions and cannot execute H12's new candidate path.
The candidate emits five sink-specialized `scan_with_assertion_plan`
functions and changes the size and placement of ordinary `scan_with_stats`
specializations. The leading hypothesis is therefore separable compiler or
hot-layout interference, not an inherent cost of filtering assertion starts.

The preparation loss has a different cause: H12 deliberately does additional
prefix analysis and literal-filter construction. A successor must address
both mechanisms rather than hiding preparation work in another measured
phase.

## Recommended successor

Freeze a new assertion-dispatch isolation hypothesis:

1. Preserve H12's conservative prefix proof and exact private candidates.
2. Move assertion planning behind one non-generic or separately compiled
   boundary so ordinary scanner code and placement remain stable.
3. Require byte, mnemonic, size, alignment, and focused counter evidence for
   the Wuthering ordinary path before the full timing matrix.
4. Profile filter construction and reduce its absolute preparation cost.
5. Reuse every H12 correctness, fallback, ordinary, H11, and Wuthering guard
   under a new authorization without changing thresholds.

This is a recovery hypothesis, not authorization to implement or merge.

## Evidence identity

- Design:
  [B2-H-0012 frozen design](b2-h-0012-assertion-prefilter-design.md)
- Accepted H11 baseline:
  `bacd5c46d934cd5526dcab78af88e375b2f13370`
- Exact measured candidate:
  `169c47c0c1360dbbf4653834bd144d9a66a14093`
- Candidate patch SHA-256:
  `4929818ed5c4d9aa0deabab2f3e24d7712d88f4ede471ca5540bede8c13df5d8`
- Frozen experiment revision:
  `4a26837af3d90f3238ccb021ea9a3925113f8d00`
- Authorization SHA-256:
  `2f04f2f14e0fbab65fe1772e621c4f656bd54e55df5e155ae0cd10458baae2d7`
- Frozen contract v4 SHA-256:
  `191281864d2b601b27f492e2ceaf83dd5031a299312cc5a97cb060a605e3bdde`
- Measurements SHA-256:
  `4fd7cdf4c0f3404c8dc09ae6c8ee9e9d3c4cdf45dc35b7c2dd83aaa2a478233b`
- Window state SHA-256:
  `f5581fde7dd831a56fac153b81c27608bc725190471a4917ddbebae7a31480ef`
- Host receipt SHA-256:
  `deb1fe54f15b29929f8f17c19cf4d0cd7143bc4c9f7c34982eb939e471c16d25`
- Service snapshot SHA-256:
  `e0939ad461c6f7dba32f73d75ce109d066c8135e41b835510a7e9a24f113e62c`
- Machine summary SHA-256:
  `fb809462d80d9fc7ee42df23d5f7134bca40aab36e7072c38780962737aa033f`
- Final measurement review commit:
  `3f948d3`

The full machine evaluation, lab note, raw receipts, and every superseded
window remain in the measurement repository. The successful window ran for
49 minutes 42 seconds on an exclusive, GPU-idle Agogo host and restored all
managed services cleanly.
