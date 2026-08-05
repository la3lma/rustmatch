# H11 historical production cross-engine snapshot

**Status:** historical; superseded by the owner-authorized H42 production merge

**Rust revision:** `bacd5c46d934cd5526dcab78af88e375b2f13370`

**Production merge:** `de33ea16673d179c5878045b44ec5e4eefa970f7`

## Purpose

The optimization ledger's leading Java rmatch, RegexSet, and Hyperscan summary
followed H11 until the owner-authorized H42 merge. This document now preserves
the exact H11 historical overlap; the H42 snapshot is the current view.

H11 did not rerun the complete 35-to-39-cell cross-engine dataset. Its frozen
admission matrix did provide current Rust measurements for nine exact scenario
groups. This snapshot compares those H11 medians with retained competitor
measurements for the same scenarios. It is the newest valid comparison, but it
is intentionally labeled as an **admission-overlap snapshot**, not a
full-dataset rerun.

## Summary

Ratios are Rust throughput divided by competitor throughput:

| Engine | Contract | Exact overlap | Rust wins | Geometric mean | Median | Range |
|---|---|---:|---:|---:|---:|---:|
| Java rmatch | Same complete event contract | 9 | 9 | 27.893x | 35.350x | 2.737x-63.846x |
| RegexSet | Different output contract | 8 | 4 | 1.351x | 1.084x | 0.133x-7.963x |
| Hyperscan | Native-reference diagnostic | 8 | 2 | 0.210x | 0.254x | 0.011x-1.206x |

This is materially newer but narrower than the retained full B2 snapshot.
Cross-snapshot geometric means must not be treated as a longitudinal
apples-to-apples speedup because their scenario sets differ.

## Method

- Rust values are exact candidate metric medians from the accepted H11
  evaluation. Where H11 measured two worker counts for one scenario, this
  summary uses the higher measured H11 throughput.
- Competitor values are the retained midpoint throughputs from the unchanged
  B2-v12 comparison tables. Competitor artifacts, fixtures, and contracts did
  not change during H11.
- No pre-H11 Rust throughput is relabeled as H11.
- Missing competitor scenarios are omitted rather than estimated.
- Java rmatch is a same-event-contract comparison. RegexSet reports native set
  membership, and Hyperscan remains a native-reference diagnostic.

The accepted H11 evaluation SHA-256 is
`88fefc2df13334380812a59352c517980105ec4458cf412e2298a1283799edf0`.
The retained competitor table SHA-256 values are:

- Java rmatch:
  `40787d5cb83b00bb11a7c92d461f0426b23ce3e9cebb8a0b4f2bda2ff4956be1`
- RegexSet:
  `3384831f82e827e1f578a4f9aaa7f89782d926cc2caddd09373580e4fcab9284`
- Hyperscan:
  `62e685b53c0c7af83825a785e76c2300d3bb088875320828fe522bfaac7790f3`

The full H11 measurement note is retained in the measurement repository at
[`581980d`](https://github.com/la3lma/rmatch-performance-measurements/blob/581980d524e8c083d89cf468d98c7cab06ff8776/docs/lab-notebook/2026-07-26-b2-h-0011-private-prefilter-inlining.md).
The pre-H11 full-dataset review remains frozen at measurement revision
`d2a92174e4629aa33d04b636e469c7e41505c240`.

## Java rmatch

| Scenario | H11 Mbit/s | Java Mbit/s | Rust/Java |
|---|---:|---:|---:|
| Diverse literals, 10,000, 50 MiB | 21,140.011 | 518.621 | 40.762x |
| Diverse literals, 10,000, 8 MiB | 4,002.389 | 213.026 | 18.788x |
| Diverse literals, 7,500, 50 MiB | 22,702.110 | 559.081 | 40.606x |
| Dense literals, 10,000, 50 MiB | 7,918.385 | 132.195 | 59.899x |
| Dense literals, 10,000, 8 MiB | 3,388.217 | 105.485 | 32.120x |
| Dense literals, 5,000, 50 MiB | 9,307.434 | 145.779 | 63.846x |
| Zero-output literals, 10,000, 50 MiB | 5,458.975 | 1,994.547 | 2.737x |
| Mixed regex, 10,000, 50 MiB | 23,428.283 | 662.754 | 35.350x |
| Wuthering literals, 10,000, 8 MiB | 1,528.531 | 55.279 | 27.651x |

## RegexSet

| Scenario | H11 Mbit/s | RegexSet Mbit/s | Rust/RegexSet |
|---|---:|---:|---:|
| Diverse literals, 10,000, 50 MiB | 21,140.011 | 3,927.589 | 5.382x |
| Diverse literals, 7,500, 50 MiB | 22,702.110 | 4,205.039 | 5.399x |
| Dense literals, 10,000, 50 MiB | 7,918.385 | 14,606.483 | 0.542x |
| Dense literals, 10,000, 8 MiB | 3,388.217 | 2,402.802 | 1.410x |
| Dense literals, 5,000, 50 MiB | 9,307.434 | 69,753.058 | 0.133x |
| Zero-output literals, 10,000, 50 MiB | 5,458.975 | 7,202.698 | 0.758x |
| Mixed regex, 10,000, 50 MiB | 23,428.283 | 2,942.209 | 7.963x |
| Wuthering literals, 10,000, 8 MiB | 1,528.531 | 2,464.900 | 0.620x |

## Hyperscan

| Scenario | H11 Mbit/s | Hyperscan Mbit/s | Rust/Hyperscan |
|---|---:|---:|---:|
| Diverse literals, 10,000, 50 MiB | 21,140.011 | 179,401.330 | 0.118x |
| Diverse literals, 10,000, 8 MiB | 4,002.389 | 39,696.035 | 0.101x |
| Dense literals, 10,000, 50 MiB | 7,918.385 | 21,413.355 | 0.370x |
| Dense literals, 10,000, 8 MiB | 3,388.217 | 18,719.555 | 0.181x |
| Dense literals, 5,000, 50 MiB | 9,307.434 | 28,539.015 | 0.326x |
| Zero-output literals, 10,000, 50 MiB | 5,458.975 | 486,317.545 | 0.011x |
| Mixed regex, 10,000, 50 MiB | 23,428.283 | 21,967.340 | 1.067x |
| Wuthering literals, 10,000, 8 MiB | 1,528.531 | 1,267.470 | 1.206x |

## Freshness invariant

The ledger generator requires exactly one current comparison snapshot, requires
that snapshot to appear first, and requires its Rust revision to equal the
latest merged optimization's measured candidate revision. A future accepted
optimization therefore makes the ledger stale until a new exact overlap or
full-dataset comparison is supplied. Historical snapshots remain visible
below the current one.
