# B2-H-0013 assertion prefilter recovery result

**Outcome:** investigate; not admitted and not merged

**Machine disposition:** rejected under G9-v2

**Production baseline remains:** `bacd5c46d934cd5526dcab78af88e375b2f13370`

## Result at a glance

H13 retained H12's exact assertion-candidate algorithm while replacing five
sink-specialized assertion scanners with one non-generic function and reducing
private-filter construction. The recovery preserved the exceptional target
value, but failed the unchanged focused guardrails before the full matrix.

| Focused cell | Median effect | AB | BA | Direction |
| --- | ---: | ---: | ---: | --- |
| assertion boundary, 1,000 patterns, 1 MiB, one worker | +22,567.445% | +22,699.159% | +22,435.731% | 6/6 positive |
| assertion boundary, 500 patterns, 1 MiB, one worker | +21,516.882% | +21,437.331% | +21,577.964% | 6/6 positive |
| mixed assertion adversary, 256 patterns, 1 MiB, one worker | +72,625.754% | +72,742.145% | +71,932.032% | 6/6 positive |
| H11 shared, 10,000 patterns, 50 MiB, 16 workers | +0.771% | +0.465% | +2.005% | 4/6 positive |
| Wuthering, 10,000 patterns, 8 MiB, 64 workers | **-5.791%** | **-4.454%** | **-7.129%** | **6/6 negative** |

The boundary target reached about 226.67 times baseline throughput and the
adversary reached about 727.26 times baseline. Every event count and normalized
digest matched exactly.

## Why it did not merge

The Wuthering focused guard crossed the unchanged three-percent automatic veto
in both order strata. H13's Wuthering loss was also larger than H12's
`-3.038%`, so the proposed dispatch isolation did not repair the inactive-path
regression.

Repeated preparation remained slower:

| Assertion shape | Baseline median | H13 median | Paired effect |
| --- | ---: | ---: | ---: |
| boundary, 1,000 patterns | 841,338 ns | 923,183 ns | **-5.862%** |
| boundary, 500 patterns | 414,710 ns | 483,455 ns | -13.359% |
| mixed adversary, 256 patterns | 214,872 ns | 250,625 ns | **-11.382%** |

The preregistered stop condition therefore fired. The unchanged eleven-cell
matrix was not run, the candidate was not merged, and it receives no point in
the accepted-efficiency graph.

## What H13 taught us

The non-generic boundary removed H12's monomorphization problem, shrinking five
assertion-plan functions totaling 17,776 bytes to one 2,512-byte function.
That was not enough to preserve ordinary scanner shape: the
`harness_event_count` specialization changed from 0x2fd0 bytes in H11 to
0x2b41 bytes in H13, and total text grew by 25,688 bytes over H11.

The construction recovery also stopped short of the useful horizon.
`BoundedPrefix` retained the shared 32-unit maximum even though the
assertion-private filter consumes no more than five units. H13 reduced H12's
absolute extra construction work, but still copied and retained prefix state
that this filter cannot query.

## Recommended successor

A fresh H14 recovery should change only the two remaining measured mechanisms:

1. Cap assertion prefix proof and storage at the filter's actual five-unit
   horizon, and direct-fill only the tables that can be queried.
2. Put assertion dispatch behind a cold or separately compiled entry and
   require ordinary scanner and caller shape parity before focused timing.

H14 must receive a fresh review, authorization, candidate revision, and frozen
contract. It must reuse the same focused targets, preparation checks, Wuthering
guard, H11-shared guard, and numerical thresholds. H13's source and receipts
remain immutable.

## Evidence identity

- Accepted H11 baseline:
  `bacd5c46d934cd5526dcab78af88e375b2f13370`
- Exact measured H13 candidate:
  `fb836900c7ef8a872da44638c6f626a7547c7487`
- Candidate patch SHA-256:
  `ee82845d31f5cfe711e451dba6501c1f8d97bc23113e03444ad9108a3cb5e2b0`
- Baseline artifact SHA-256:
  `64c1ca1886a949f7ad42c908963e6ca0c80b2506f3bf24ed858b27dd0df03f78`
- Candidate artifact SHA-256:
  `5f4db1096f8555fcbed1fcfec18f85763a943d1d2d8cc2567be44b64b8731efd`
- Frozen contract SHA-256:
  `a7a2eded46f3e73891992e64bd010e46bf83c811f0d645c307f6d76fa23e283d`
- Authorization SHA-256:
  `342ae579b7b597b3d493a0456049904c7d998ce54f10179824f118509873d4b9`
- Focused summary SHA-256:
  `0bdcff33c058d14ff57bf33492b67e0c195e48acd88bd68d22bdc5252e58eda6`
- Window state SHA-256:
  `62dd3d791bdac7db21af8de9f6f292f52e99ac08e2cdeaf96bf9ae74433e2967`
- Host receipt SHA-256:
  `be7ac7ddfe57b6139a0ba3a4e4d161706d3f1a49b3ddb770443d2b3d1a3bb605`
- Service snapshot SHA-256:
  `fdc3a09ee1b0b81b30c4ca652e2bd36aad83455ab6c6077fe5b61e0e7b9215ab`
- Machine result SHA-256:
  `3c72f03f58f1d35f5e9e73ba26a2d6519761d593ab595423d9c8013b70279dd0`
- Lab note SHA-256:
  `dfdf5ce6f8310cd000485cf545aadddcac0273ff2540d81aa28f9f55c1576f38`
- Final measurement review commit:
  `af8f4b7`

One login-contaminated preflight was rejected before timing and retained. The
clean focused window ran for about ten minutes on exclusive, GPU-idle Agogo and
retained 180 raw timing files.
