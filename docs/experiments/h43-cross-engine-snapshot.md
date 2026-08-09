# H43 current-production cross-engine snapshot

**Status:** current by owner-authorized merge; formal R3 outcome remains
`rejected`

**Rust revision:** `a5b86c2b3943aad3121e1f06944e39331f17a474`

**Production merge:** `4996bcc16a9cc936789372d8184f4ef4893fd336`

## Purpose

H43 adds a deliberately selected experimental matcher without changing the
ordinary matcher. Its R3 confirmation independently passed all nine default
safety cells. The current generic/default comparison therefore retains the
exact H42 overlap medians and unchanged competitor values rather than
pretending that a competitor rerun occurred.

The two H43 assertion targets are reported in a separate product lane. They
are not folded into Java rmatch, RegexSet, or Hyperscan geometric means because
the retained dataset has no exact competitor overlap for those target fixtures.

## Summary

Ratios are default Rust throughput divided by competitor throughput:

| Engine | Contract | Exact overlap | Rust wins | Geometric mean | Median | Range |
|---|---|---:|---:|---:|---:|---:|
| Java rmatch | Same complete event contract | 9 | 9 | 44.543x | 80.983x | 2.738x-126.566x |
| RegexSet | Different output contract | 8 | 4 | 2.288x | 1.086x | 0.194x-27.154x |
| Hyperscan | Native-reference diagnostic | 8 | 2 | 0.308x | 0.402x | 0.011x-3.637x |

The default overlap reaches 30.8% of Hyperscan's geometric-mean throughput.
Cross-engine semantic contracts still differ for RegexSet and Hyperscan, and
this ratio is not a fairness claim.

## Method

- Default Rust values are exact H42 candidate medians retained unchanged
  because H43's ordinary matcher is the same production path and all nine R3
  default-safety cells passed.
- Competitor values are the unchanged retained B2 midpoint throughputs.
- Java rmatch is a same-event-contract comparison. RegexSet reports native set
  membership, and Hyperscan remains a native-reference diagnostic.
- Experimental H43 values come from exact R3 target medians and are shown only
  against the public default Rust matcher for the same fixtures.
- No H42 or competitor throughput is relabeled as a fresh H43 measurement.

## Java rmatch

| Scenario | Default Rust Mbit/s | Java Mbit/s | Rust/Java |
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

| Scenario | Default Rust Mbit/s | RegexSet Mbit/s | Rust/RegexSet |
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

| Scenario | Default Rust Mbit/s | Hyperscan Mbit/s | Rust/Hyperscan |
|---|---:|---:|---:|
| Diverse literals, 10,000, 50 MiB | 59,216.494 | 179,401.330 | 0.330x |
| Diverse literals, 10,000, 8 MiB | 3,992.679 | 39,696.035 | 0.101x |
| Dense literals, 10,000, 50 MiB | 10,705.533 | 21,413.355 | 0.500x |
| Dense literals, 10,000, 8 MiB | 3,397.337 | 18,719.555 | 0.181x |
| Dense literals, 5,000, 50 MiB | 13,501.841 | 28,539.015 | 0.473x |
| Zero-output literals, 10,000, 50 MiB | 5,460.938 | 486,317.545 | 0.011x |
| Mixed regex, 10,000, 50 MiB | 79,893.126 | 21,967.340 | 3.637x |
| Wuthering literals, 10,000, 8 MiB | 1,766.789 | 1,267.470 | 1.394x |

## Explicit assertion capability

This secondary product lane compares H43 only with the generic Rust matcher on
the exact same assertion fixtures:

| Workload | Generic Rust | Explicit H43 | Speedup | Formal outcome |
|---|---:|---:|---:|---|
| Assertion boundary, 1,000 patterns, 1 MiB | 0.946 Mbit/s | 219.172 Mbit/s | 231.62x | R3 preparation pass |
| Assertion boundary, 1,000 patterns, 2 MiB | 0.943 Mbit/s | 435.786 Mbit/s | 460.49x | R3 preparation veto |

The capability entered production by owner exception, not certification. It
requires a non-default Cargo feature, a separate matcher type, and an explicit
scan policy. The 2 MiB R3 preparation regression remains retained, while two
focused exact-source discriminators place the reproducible construction
premium near 0.8%-1.0%.

## Freshness invariant

This snapshot follows the newest merged optimization's measured revision. A
future production optimization makes it historical until a new exact overlap
or full-dataset comparison is supplied.

See the [owner decision](h43-x1-9-owner-exception.md), [R3 result](h43-x1-7-r3-confirmation-result.md),
and [H42 source snapshot](h42-cross-engine-snapshot.md).
