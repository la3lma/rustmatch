# H43-D1-R1 cohort-spine recovery result

**Decision:** rework  
**Measured candidate:** `2b650e980cafaed608ce8c7e8c7e9bde02e27a5e`  
**Baseline:** `05992bf1f20e0f9384fc52fcb4a9863500e7a4da`  
**Controller:** `123585bf72e2a4f2f6531c357d9c0c4d708a5b5c`  
**Policy:** unchanged G9-v3 thresholds  
**Consequence:** H43-A1 remains blocked; H43-D1-R2 is authorized

## Executive result

R1 repaired the original sequential-scope occupancy failure. The four- and
sixteen-worker mixed lanes moved from severe losses to results within 1.56% of
ordinary execution, while the one-worker target retained gains of 55.00% and
90.27%. Exact events, failure behavior, worker bounds, and environmental
guards all passed.

The recovery is still not admissible. Three assertion-free ordinary scan
guards regressed by 10.13% to 22.35%, and mixed-balanced one-worker cohort
preparation regressed 24.97%. These are formal vetoes under the unchanged
policy, not thresholds to reinterpret.

Post-window diagnostics separated the failures. The ordinary hot scan kernel
has identical 21,988-byte machine text in baseline and candidate, but the
added cohort code moved its linked address by about 4.3 KiB. Paired hardware
counters found nearly unchanged instructions but higher cycles and cache
activity during short early scans. The cohort preparation cost comes from
building the newly eligible assertion-free start table through a transition
compiler that repeatedly allocates temporary vectors. R2 may address only
those two mechanisms.

## Evidence integrity

| Item | Value |
|---|---|
| Formal window | `2026-08-06T13:18:58Z` to `2026-08-06T13:21:46Z` |
| Plan SHA-256 | `36da2a69703d48a76c7571b74ca59fa22f9a5e57f20a8e1d59cb1eeb788bfc61` |
| Window state SHA-256 | `c04cae1a1609055ef1c8f4dcca92b28794f2968d62eb872d9b0d9a7570d1c541` |
| Summary SHA-256 | `b84408f642f612e05fdb3ecd9ad08b7c6d07e152ac5d1b2bb3f67f4bc4b707d4` |
| Receipts | 720 timing; 108 resource |
| Correctness | zero errors; all four activation counts exact |
| Environment | zero contamination records; zero Docker events; GPU idle |
| Service restoration | all three managed containers exited with restart policy `no` |
| Raw archive SHA-256 | `74c34a997953aba689f5cb36d33f32234303763e5683d1cb3b9259167356fa66` |

The compact summary, window state, fixture manifest, service snapshot, and
review manifest are retained in `docs/experiments/h43-d1-r1/`. The complete
raw archive is retained outside the repository at the path in
`evidence-manifest.json`.

## Target recovery

Negative deltas are improvements. Each value is the geometric paired delta
across fifteen cycles.

| Fixture | Workers | Cohort scan delta | Interpretation |
|---|---:|---:|---|
| mixed-balanced | 1 | -55.00% | large target gain retained |
| mixed-balanced | 4 | +0.40% | recovered to neutral |
| mixed-balanced | 16 | -1.56% | recovered to neutral/slightly faster |
| mixed-output | 1 | -90.27% | about 10.3x faster |
| mixed-output | 4 | +0.32% | recovered to neutral |
| mixed-output | 16 | -0.76% | recovered to neutral/slightly faster |

## Formal blockers

| Lane | Fixture | Workers | Metric | Delta | Direction |
|---|---|---:|---|---:|---:|
| inactive | assertion-free | 1 | scan | +15.23% | slower in 13/15 |
| inactive | assertion-free | 4 | scan | +22.35% | slower in 15/15 |
| inactive | assertion-free | 16 | scan | +10.13% | slower in 13/15 |
| cohort | mixed-balanced | 1 | prepare | +24.97% | slower in 15/15 |
| inactive | mixed-balanced | 4 | prepare | +5.51% | slower in 13/15 |

No peak-RSS comparison exceeded the one-MiB materiality boundary. Feature-mode
rlib size increased 5.52%, default rlib size increased 0.14%, and measured
compile time increased 3.67%; these remain costs for R2 to reduce even though
they were not among the analyzer's five automatic timing/RSS signals.

## Causal diagnostics

The baseline and candidate ordinary scan instantiations are both 21,988 bytes
and have the same SHA-256 over emitted text:
`a01640720a27c65befa3acdf6b1bec5c6acbfd94668784b26ecdb612ab0effe6`.
Only relocation identities and linked placement differ. The corresponding
linked hot symbol moved from `0x14efc0` to `0x1500b0`.

Fifteen additional AB/BA `perf stat` cycles used 100 retained scans per
process. Their median scan ratios converged to +0.39%, +0.01%, and +0.83% at
workers 1, 4, and 16, while one-worker instructions changed -0.20% and cycles
changed +8.21%. At four workers, instructions changed -0.81%, cycles +3.15%,
and cache misses +8.49%. This supports a cold code-layout/cache mechanism
rather than added semantic work. It does not invalidate the formal short-cell
guard.

For preparation, splitting the mixed database makes the assertion-free cohort
eligible for `StartTable::compile`. That compiler explores ASCII symbol pairs
while allocating fresh transition/closure vectors. The extra table is useful
at scan time, but its construction algorithm is not an immutable cost.

## Decision and bounded recovery

Do not authorize H43-A1 and do not alter warmups, fixtures, cycle counts, or
thresholds. H43-D1-R2 may:

1. move cohort-only build and scheduler code out of the ordinary API
   compilation unit while preserving the direct ordinary source path;
2. make start-table closure and transition construction reuse bounded scratch
   vectors without changing the resulting table;
3. rerun exact table-equivalence tests, H43-E1, repository CI, and the complete
   unchanged D1 matrix.

If either ordinary short-scan guards or cohort preparation remains above the
policy boundary after R2, the semantically inert execution spine is rejected
and cohort classification remains diagnostics-only.
