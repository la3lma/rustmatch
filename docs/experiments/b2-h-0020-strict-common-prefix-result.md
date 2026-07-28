# B2-H-0020 strict common-prefix result

## Decision

Outcome: `investigate`

Exact candidate `730cec6492ff2356f53b368e214e5e8dfd78bbb9` is
machine-rejected and is not merged. Production remains exact H11 revision
`bacd5c46d934cd5526dcab78af88e375b2f13370`.

This is not a threshold exception. G9-v2 permits a human `investigate`
classification when a machine-rejected artifact contains a retained
high-discrepancy opposed-effect signal. The candidate receives no admission or
progress credit.

## Mechanism

H20 enables a private assertion candidate scanner only when an
assertion-bearing partition has at least 256 patterns and every pattern proves
the same first five ASCII symbols after leading assertions and epsilon
transitions. That prefix fits in one packed `u64`; no prefix table or filter
allocation is retained.

The scan can only propose starts. Exact NFA verification remains authoritative.
Any mixed, short, optional, non-ASCII, case-insensitive, predicate, ordinary,
or otherwise uncertain partition follows exact H11 behavior.

## Formal result

The guarded G9-v2 window completed all 11 frozen cells, 66 balanced AB/BA
pairs, and 531 retained raw files. Every functional test, event count, and
normalized event digest passed.

| Cell | Throughput effect | Candidate/baseline | Direction |
|---|---:|---:|---:|
| Boundary 1000, 1 MiB, 1 worker | +22978.782% | 230.79x | 6/6 positive |
| Boundary 1000, 1 MiB, 2 workers | +19949.365% | 200.49x | 6/6 positive |
| Boundary 1000, 2 MiB, 1 worker | +46078.729% | 461.79x | 6/6 positive |
| Boundary 500 neighbor | +22610.050% | 227.10x | 6/6 positive |
| Mixed assertion adversary | +10.622% | 1.11x | 6/6 positive |
| 512 KiB bypass | +11.695% | 1.12x | 6/6 positive |
| Dense assertion guard | +9.101% | 1.09x | 6/6 positive |
| Unfilterable assertion guard | +12.203% | 1.12x | 6/6 positive |
| Ordinary literals | +0.325% | 1.00x | inside noise |
| H11 shared path | +0.774% | 1.01x | inside noise |
| Wuthering | +4.507% | 1.05x | positive, below target floor |

No primary-throughput, process-wall, or peak-RSS metric vetoed. Three
preparation measurements crossed the unchanged order-stratified ceiling:

| Cell | Aggregate preparation | Baseline then candidate | Candidate then baseline |
|---|---:|---:|---:|
| Boundary 1000, 1 MiB, 1 worker | -6.617% | -0.115% | -13.472% |
| Boundary 1000, 2 MiB, 1 worker | -7.091% | +1.798% | -13.723% |
| Wuthering | -2.431% | -1.602% | -3.540% |

The exact result-evaluation SHA-256 is
`5722e0f3c35cdd3baba43091f9abd9442aa3dc2407844cace75002a47370a4ca`.
The canonical lab-note SHA-256 is
`04636a0320273fb556a19d2c17b6ab95100b692ad730d0f241e6cf5e171dcb56`.
The measurement manifest, also retained as the outer stage log, has SHA-256
`8220414776eed6a5affce61faa32f57cb4a5a2d6c93f46fd43ab6e49c74e4f63`.

## Interpretation

H20 validates the scan mechanism more strongly than any predecessor while
narrowing H18's failure surface from multiple construction and RSS vetoes to
three preparation-only vetoes. The target regressions are concentrated in one
launch order. Wuthering cannot execute the new assertion code, so its order-only
preparation loss implicates residual binary layout, launch, allocator,
cache-state, or measurement coupling rather than the prefix algorithm itself.

Those distinctions are valuable causal evidence, but they do not make H20
admissible. Either target preparation veto is sufficient to block the exact
artifact under the unchanged rules.

## Next discriminator

A successor should derive the packed prefix while patterns are already being
traversed, avoiding H20's second proof walk, and should preserve an observably
identical H11 construction and code-layout path for every ineligible
partition.

Before another formal run:

1. Compare exact H11, H20, and one successor with timer-free phase counters.
2. Require allocation parity and no extra traversal on ordinary and Wuthering
   fallback construction.
3. Verify relevant H11 fallback symbols, sizes, instruction streams, and
   alignment.
4. Run balanced fresh-process construction in both launch orders.
5. Stop before G9 unless no preparation loss persists and scan/event parity is
   unchanged.

The full retained machine summary and raw receipts remain in the benchmark
evidence repository at commit
`c9fd7f481386b9b5cf0458321a1b030bc008705d`.
