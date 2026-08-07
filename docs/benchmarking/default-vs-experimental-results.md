# Default and experimental benchmark results

**Status:** accepted, published README snapshot

**Measured revision:** `7dd80c00ade7624629f3e20d49fe0b6437a8c9e2`

**Campaign:** `readme-default-experimental-v1`

**Evidence:** [`c2a3e25` benchmark archive](https://github.com/la3lma/rmatch-performance-measurements/tree/c2a3e25bfcfe91371902557524a81374dd87a824/docs/benchmarking/readme-default-experimental-v1)

## Interpretation boundary

This report applies the [two-product contract](default-vs-experimental.md).
The generic and experimental results are separate products and must not be
combined into one aggregate.

- `generic-default-v1` is exactly `MatcherBuilder::new()` with no overrides, a
  feature-off release binary, and the public one-worker default.
- `experimental-oracle-v1` explicitly enables `assertion-prefix-v1` after the
  exact workload is known. It forbids fallback and demonstrates narrow
  capability, not automatic selection or robustness.

## Generic/default summary

Ratios are `Rustmatch throughput / reference throughput`. The geometric mean
uses only workloads shared with each retained reference series.

| Reference | Shared cells | Rust wins | Geometric mean | Median | Range |
|---|---:|---:|---:|---:|---:|
| Java rmatch | 9 | 9 | **5.539x** | 6.402x | 2.758x-6.977x |
| Rust `regex::RegexSet` | 8 | 1 | **0.256x (25.6%)** | 0.498x | 0.0136x-1.447x |
| Hyperscan | 8 | 0 | **0.0424x (4.24%)** | 0.0307x | 0.0113x-0.284x |

Java rmatch emits the same complete event stream. RegexSet exposes membership
semantics, and Hyperscan is a native-reference diagnostic, so those two columns
are performance context rather than equivalence claims.

## Generic/default cells

| Workload | Rustmatch Mbit/s | / Java | / RegexSet | / Hyperscan |
|---|---:|---:|---:|---:|
| Diverse literals, 10,000, 50 MiB | 3,319.959 | 6.402x | 0.845x | 0.0185x |
| Diverse literals, 10,000, 8 MiB | 1,156.190 | 5.427x | N/A | 0.0291x |
| Diverse literals, 7,500, 50 MiB | 3,900.903 | 6.977x | 0.928x | N/A |
| Dense literals, 10,000, 50 MiB | 678.467 | 5.132x | 0.0464x | 0.0317x |
| Dense literals, 10,000, 8 MiB | 555.641 | 5.267x | 0.231x | 0.0297x |
| Dense literals, 5,000, 50 MiB | 945.317 | 6.485x | 0.0136x | 0.0331x |
| Zero-output literals, 10,000, 50 MiB | 5,501.476 | 2.758x | 0.764x | 0.0113x |
| Mixed regex, 10,000, 50 MiB | 4,258.154 | 6.425x | 1.447x | 0.194x |
| Wuthering literals, 10,000, 8 MiB | 360.050 | 6.513x | 0.146x | 0.284x |

The older H42 README panel measured benchmark-selected worker counts. Its much
higher aggregate ratios remain valid for that frozen tuned series, but are not
public-default results and are no longer the primary product headline.

## Experimental capability cells

| Workload | Public default | `assertion-prefix-v1` | Speedup | Status |
|---|---:|---:|---:|---|
| Assertion boundary, 1,000 patterns, 1 MiB | 1.009 Mbit/s | 232.207 Mbit/s | **230.05x** | pass |
| Assertion boundary, 1,000 patterns, 2 MiB | 1.012 Mbit/s | 461.144 Mbit/s | **455.52x** | pass |
| Adversarial assertions, 256 patterns, 1 MiB | 5.676 Mbit/s | N/A | N/A | not applicable |
| Ordinary literals, 1,000 patterns, 1 MiB | 4,643.490 Mbit/s | N/A | N/A | not applicable |

The two `not applicable` cells are positive safety evidence. The specialized
matcher refused workloads outside its eligibility envelope instead of silently
substituting the generic path.

## Method and evidence quality

- Host: Agogo, AMD Ryzen 9 9950X3D, 32 logical CPUs, CPU set `0-15`.
- Toolchain: `rustc 1.97.0 (2d8144b78 2026-07-07)`.
- Five independent process cycles per cell, with two warmups and five retained
  scans in each process.
- Separate feature-off and feature-on release binaries, respectively bound by
  SHA-256 `55363fb8f8b72776134eae7b1788c7b11a4349f37f28ff0f55310a29b5de990e`
  and `3870fc4ec1193bf193aa5b364e123591928302cf105690c5e5f6ff89f4d5d6ce`.
- 85 process receipts: 75 `pass/pass` and ten
  `pass/not-applicable`; every event count and multiset digest matched.
- Clean formal window: `2026-08-07T11:01:52.717387Z` through
  `2026-08-07T11:18:28.680451Z`, exit zero, no contamination, no window error,
  idle GPU, and complete service restoration.
- Evidence manifest: 343 files, all rehashed after download with zero mismatch.

An earlier launch was rejected before timing because SSH-login transients
exceeded the frozen external-process CPU threshold. Its receipts remain in the
archive. The accepted recovery used the unchanged monitor after the transients
settled; no rejected timing was admitted.
