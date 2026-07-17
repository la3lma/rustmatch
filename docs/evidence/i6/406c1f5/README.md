# I6 evidence: bounded lazy deterministic-state cache

This directory retains the admission evidence for candidate
`406c1f5294b8797fbc47eb0c37342244426c8a9f` against frozen baseline
`9ef1b17cc6a865f44920c7c4f74f4a44b0b2c8f4`.

## Environment correction

The first local campaign accidentally used an x86_64 Rust toolchain and
x86_64 binaries under Rosetta on an Apple Silicon host. Those timings were
discarded rather than relabelled. Every retained timing here was produced by a
verified Mach-O arm64 benchmark binary on:

- `rainstick.local`
- Apple M2 Max
- 12 logical CPUs
- macOS 26.5
- Rust 1.97.0 standard library for `aarch64-apple-darwin`
- Cargo release profile

The compiler itself ran under Rosetta while cross-building the native binary.
That affects Rust build wall time, which is not a metric in these receipts. The
reported compile metric is rustmatch's in-process pattern compilation and ran
inside the native arm64 benchmark process.

## Admission result

All focused gates passed with exact event equality. Times are medians.

| Scenario | Patterns | Baseline | Candidate | Improvement | Cache states | Fallbacks |
|---|---:|---:|---:|---:|---:|---:|
| literal sparse | 64 | 395.39 ms | 5.37 ms | 98.64% | 79 | 0 |
| literal sparse | 256 | 3,230.81 ms | 10.50 ms | 99.67% | 292 | 0 |
| mixed sparse | 64 | 389.98 ms | 5.00 ms | 98.71% | 253 | 0 |
| literal dense | 64 | 736.22 ms | 12.81 ms | 98.25% | 79 | 0 |
| assertion bypass | 64 | 614.60 ms | 574.85 ms | 6.46% | 0 | 0 |

The first native dense-literal comparison reported a 13.61% pattern-compile
regression on a roughly 50 microsecond operation. The protocol's mandatory
reverse-order repeat measured the candidate faster and passed. This is treated
as timing noise, not hidden from the record.

The 10,000-pattern Wuthering memory run produced the same 153,971 events and
the same digest on both revisions. Median scan time fell from 45.014 seconds to
32.827 milliseconds. Peak resident memory rose from 33,177,600 to 38,043,648
bytes: 4,866,048 bytes (4.64 MiB) for the 4 MiB direct transition tables plus
state-set and index metadata.

## Scale results

The standard 8,192-state budget produced:

| Patterns | Corpus | Events | Scan median | Cache states | Fallback transitions |
|---:|---:|---:|---:|---:|---:|
| 1,000 | 675,259 B | 8,809 | 5.59 ms | 3,284 | 0 |
| 5,000 | 675,259 B | 74,604 | 16.24 ms | 8,192 | 15,460 |
| 10,000 | 675,259 B | 153,971 | 30.57 ms | 8,192 | 107,407 |
| 1,000 | 8 MiB | 109,693 | 53.70 ms | 3,284 | 0 |
| 5,000 | 8 MiB | 926,975 | 148.17 ms | 8,192 | 189,826 |
| 10,000 | 8 MiB | 1,912,854 | 289.95 ms | 8,192 | 1,332,071 |

From 5,000 to 10,000 patterns, scan time is close to linear. That observation
cannot be interpreted as pure search scaling: the number of emitted events
also doubles, while the cache fills and exact fallback work grows sharply.

An 8 MiB no-match control separates those factors. It used one cached state,
two misses, no fallback, and zero events at every pattern count:

| Patterns | Scan median |
|---:|---:|
| 1,000 | 39.44 ms |
| 5,000 | 40.53 ms |
| 10,000 | 39.55 ms |

The no-match scan is effectively independent of pattern count. Wuthering's
upper-end growth is therefore dominated by result-bearing paths and cache
pressure rather than a mandatory linear pattern-set scan at every position.

## Profiling and cache tradeoff

The companion profile used a repeated 50 MiB Wuthering corpus and the same
1,000/5,000/10,000 pattern sets. `xctrace` Time Profiler sampled at 1 ms. Raw
trace bundles remain local because they are large; the correctness-bearing
companion receipts and the extracted summary are retained here.

The most useful trend is `compute_transition`: approximately 0.1% of samples
at 1,000 patterns, 4.8% at 5,000, and 22.6% at 10,000. Resetting one start held
near 266 ms in absolute sampled time across all three runs. This supports the
hypothesis that exact fallback, not fixed per-position setup, drives much of
the high-pattern incremental cost. Some optimized scan-loop samples are
reported by Instruments only as `<deduplicated_symbol>`, so the profile is
directional rather than an exact cost partition.

Increasing the cache budget on the 10,000-pattern, 8 MiB workload showed:

| Budget | Materialized states | Direct tables | Fallbacks | Scan median |
|---:|---:|---:|---:|---:|
| 8,192 | 8,192 | 4 MiB | 1,332,071 | 289.95 ms |
| 16,384 | 16,384 | 8 MiB | 370,921 | 237.58 ms |
| 32,768 | 28,100 | 13.72 MiB | 0 | 243.08 ms |

Eliminating fallback entirely was not fastest. The 16,384-state variant was
18.1% faster than the default, while 32,768 paid enough cache cost to lose part
of that gain. The 8,192-state default remains the conservative 4 MiB choice;
callers with this workload can select 16,384 explicitly. I7 should reduce
impossible start work before rustmatch spends more memory by default.

## Critical conclusion

I6 is admitted because it preserves exact semantics, clears every declared
positive and guard threshold, and exchanges a measured 4.64 MiB peak-RSS cost
for a very large native Rust scan improvement. It does not establish constant
time for result-heavy pattern growth, accelerate assertion-bearing sets, prove
the best cache representation, or replace cross-engine benchmarking. The
retained no-match control and profile narrow the next questions; they do not
turn this campaign into a general performance claim.

The Wuthering Heights corpus remains the stable development and regression
line while performance is still changing quickly. Larger and more diverse
corpora become necessary once this fixture no longer provides enough timing
resolution or representative pressure.
