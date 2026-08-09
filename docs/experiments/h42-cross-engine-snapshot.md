# H42 historical production cross-engine snapshot

**Status:** historical; superseded by H43 owner-authorized production merge;
formal G9-v3 outcome remains `investigate`

**Rust revision:** `c6f221bda8241123281aea8659dd56eeb8a13be0`

**Production merge:** `deb227171dca1d584c11ade2573ed8c9478804fa`

## Purpose

This document preserves the H42 production view that preceded H43. H42 entered
production by explicit owner authorization despite its retained G9-v3
`investigate` result. H43 later added a non-default explicit matcher without
changing these ordinary-path values; the current snapshot is now
[H43](h43-cross-engine-snapshot.md).

H42 did not rerun the complete 35-to-39-cell cross-engine dataset. Its formal
matrix measured the same nine scenario groups used by the H11 overlap
snapshot. This comparison uses exact H42 medians for those groups and the
unchanged retained competitor measurements. It is an overlap snapshot, not a
full-dataset rerun.

## Summary

Ratios are Rust throughput divided by competitor throughput:

| Engine | Contract | Exact overlap | Rust wins | Geometric mean | Median | Range |
|---|---|---:|---:|---:|---:|---:|
| Java rmatch | Same complete event contract | 9 | 9 | 44.543x | 80.983x | 2.738x-126.566x |
| RegexSet | Different output contract | 8 | 4 | 2.288x | 1.086x | 0.194x-27.154x |
| Hyperscan | Native-reference diagnostic | 8 | 2 | 0.308x | 0.402x | 0.011x-3.637x |

The H42 overlap reaches 30.8% of Hyperscan's geometric-mean throughput, up
from H11's 21.0% on the same eight scenarios. Cross-engine semantic contracts
still differ for RegexSet and Hyperscan, and this ratio is not a fairness
claim.

## Method

- Rust values are exact candidate metric medians from H42's clean formal
  G9-v3 window. Where H42 measured two worker counts for one scenario, the
  higher H42 throughput is used.
- Competitor values are the unchanged retained midpoint throughputs used by
  the H11 snapshot.
- No H11 throughput is relabeled as H42.
- Java rmatch is a same-event-contract comparison. RegexSet reports native set
  membership, and Hyperscan remains a native-reference diagnostic.
- H42's Wuthering result is included unchanged even though its 2.111% loss
  against H11 caused the formal `investigate` disposition.

The H42 result evaluation SHA-256 is
`ceb32706bdf7f58a73a9776b0617d54b44f101ba325d3a10b62469ad05c52141`.
The H42 measurements SHA-256 is
`7ec1c590c49c7795adb3af64c365e18d61e5c60b867c35502e515a91f91b4326`.
The retained competitor table SHA-256 values remain:

- Java rmatch:
  `40787d5cb83b00bb11a7c92d461f0426b23ce3e9cebb8a0b4f2bda2ff4956be1`
- RegexSet:
  `3384831f82e827e1f578a4f9aaa7f89782d926cc2caddd09373580e4fcab9284`
- Hyperscan:
  `62e685b53c0c7af83825a785e76c2300d3bb088875320828fe522bfaac7790f3`

The complete formal H42 result is retained in the measurement repository at
[`046337d`](https://github.com/la3lma/rmatch-performance-measurements/blob/046337d460d8cbb443ee0e6f5812ebcfc4eb9ff4/docs/lab-notebook/2026-08-04-b2-h-0042-formal-g9-v3.md).

## Java rmatch

| Scenario | H42 Mbit/s | Java Mbit/s | Rust/Java |
|---|---:|---:|---:|
| Diverse literals, 10,000, 50 MiB | 59,216.494 | 518.621 | 114.181x |
| Diverse literals, 10,000, 8 MiB | 3,992.679 | 213.026 | 18.743x |
| Diverse literals, 7,500, 50 MiB | 70,760.722 | 559.081 | 126.566x |
| Dense literals, 10,000, 50 MiB | 10,705.533 | 132.195 | 80.983x |
| Dense literals, 10,000, 8 MiB | 3,397.337 | 105.485 | 32.207x |
| Dense literals, 5,000, 50 MiB | 13,501.841 | 145.779 | 92.619x |
| Zero-output literals, 10,000, 50 MiB | 5,460.938 | 1,994.547 | 2.738x |
| Mixed regex, 10,000, 50 MiB | 79,893.126 | 662.754 | 120.547x |
| Wuthering literals, 10,000, 8 MiB | 1,766.789 | 55.279 | 31.961x |

## RegexSet

| Scenario | H42 Mbit/s | RegexSet Mbit/s | Rust/RegexSet |
|---|---:|---:|---:|
| Diverse literals, 10,000, 50 MiB | 59,216.494 | 3,927.589 | 15.077x |
| Diverse literals, 7,500, 50 MiB | 70,760.722 | 4,205.039 | 16.828x |
| Dense literals, 10,000, 50 MiB | 10,705.533 | 14,606.483 | 0.733x |
| Dense literals, 10,000, 8 MiB | 3,397.337 | 2,402.802 | 1.414x |
| Dense literals, 5,000, 50 MiB | 13,501.841 | 69,753.058 | 0.194x |
| Zero-output literals, 10,000, 50 MiB | 5,460.938 | 7,202.698 | 0.758x |
| Mixed regex, 10,000, 50 MiB | 79,893.126 | 2,942.209 | 27.154x |
| Wuthering literals, 10,000, 8 MiB | 1,766.789 | 2,464.900 | 0.717x |

## Hyperscan

| Scenario | H42 Mbit/s | Hyperscan Mbit/s | Rust/Hyperscan |
|---|---:|---:|---:|
| Diverse literals, 10,000, 50 MiB | 59,216.494 | 179,401.330 | 0.330x |
| Diverse literals, 10,000, 8 MiB | 3,992.679 | 39,696.035 | 0.101x |
| Dense literals, 10,000, 50 MiB | 10,705.533 | 21,413.355 | 0.500x |
| Dense literals, 10,000, 8 MiB | 3,397.337 | 18,719.555 | 0.181x |
| Dense literals, 5,000, 50 MiB | 13,501.841 | 28,539.015 | 0.473x |
| Zero-output literals, 10,000, 50 MiB | 5,460.938 | 486,317.545 | 0.011x |
| Mixed regex, 10,000, 50 MiB | 79,893.126 | 21,967.340 | 3.637x |
| Wuthering literals, 10,000, 8 MiB | 1,766.789 | 1,267.470 | 1.394x |

## Production decision and residual risk

H42's production merge is an explicit owner-authorized exception. It does not
alter the formal G9-v3 result: machine disposition `investigate`, human outcome
`investigate`, and `optimization_admitted: false`. The known residual risk is
the measured 2.111% Wuthering regression with 13 of 15 negative crossover
cycles. Future work should attempt to remove that regression without tuning or
weakening the retained guard.

## Freshness invariant

The ledger generator requires exactly one current comparison snapshot and
requires its Rust revision to equal the newest merged optimization's measured
candidate revision. A future production merge therefore makes this snapshot
historical until a new exact overlap or full-dataset comparison is supplied.
