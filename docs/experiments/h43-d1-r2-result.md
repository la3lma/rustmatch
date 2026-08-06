# H43-D1-R2 final cohort-spine recovery result

**Decision:** reject the semantically inert two-database cohort spine  
**Measured candidate:** `9bb4ad8f1b50db701ec1ecea66d9fdb73f2fae07`  
**Baseline:** `05992bf1f20e0f9384fc52fcb4a9863500e7a4da`  
**Policy:** unchanged G9-v3 thresholds  
**Consequence:** H43-A1 remains blocked; no formal admission window or merge

## Executive result

R2 implemented both bounded recoveries authorized by R1. Cohort-only build,
diagnostic, and mixed-scheduler code moved into a feature-gated module with an
out-of-line scan boundary, while the ordinary scheduler returned to its
pre-H43 source structure. Start-table construction now reuses closure and
transition buffers, and a direct old/new table comparison covers structural
and generated assertion-free databases.

Correctness held. The unchanged H43-E1 campaign reproduced 222 cases, 3,628
ordinary/cohort comparisons, 5,394 reference events, and aggregate digest
`fnv1a64:359bc776d9cd1aba`. Complete repository CI, strict Clippy, fuzz-target
builds, MSRV, rustdoc, Java compatibility evidence, and generated-document
freshness passed.

The predeclared three-cycle early screen nevertheless reproduced both causal
vetoes strongly enough to stop before a formal window. Assertion-free ordinary
short scans were 13.16% to 28.50% slower, with the candidate slower in every
pair. One-worker cohort preparation was 16.73% slower on `mixed-balanced` and
23.19% slower on `mixed-output`, also in every pair. These results exceed the
unchanged three-percent veto by large margins.

## Early-screen integrity

| Item | Value |
|---|---|
| Window | `2026-08-06T13:49:02Z` to `2026-08-06T13:49:46Z` |
| Plan SHA-256 | `3a263c8df1fe8c1d7faaad82fc06bc8a3f820593c5da6a553283e134ea449c53` |
| Window state SHA-256 | `474f56f8fd463a28be202356d40239d4979088e7361e97caeda1661b2f417bc3` |
| Summary SHA-256 | `4d9d6467c656469ab80b74497cd34aac49491d7c2da3afeb4f12700f47181d04` |
| Receipts | 144 timing; 36 resource |
| Correctness | zero errors; all four activation counts exact |
| Environment | zero contamination records; zero Docker events; GPU idle |
| Managed services | all three exited with restart policy `no` |
| Raw archive SHA-256 | `fc69919b1b393bd525209f9103fb06ace6246f12e5059dc890d933f3e91098a2` |

The analyzer reports `authorize` only because the deliberately short screen
retained the formal 13-of-15 directional threshold while executing three
cycles. That mechanical label is not an admission decision. R2 step 4
explicitly requires early rejection when either causal signal remains clearly
above three percent, and every decisive pair points in the adverse direction.

## Decisive screen

Negative deltas are improvements. Each value is the geometric paired delta
across three AB/BA cycles.

| Lane | Fixture | Workers | Metric | Delta | Slower pairs |
|---|---|---:|---|---:|---:|
| inactive | assertion-free | 1 | scan | +13.78% | 3/3 |
| inactive | assertion-free | 4 | scan | +28.50% | 3/3 |
| inactive | assertion-free | 16 | scan | +13.16% | 3/3 |
| cohort | mixed-balanced | 1 | prepare | +16.73% | 3/3 |
| cohort | mixed-output | 1 | prepare | +23.19% | 3/3 |

The original target signal remains real: one-worker cohort scan improved
57.17% on `mixed-balanced` and 90.16% on `mixed-output`. Four- and
sixteen-worker mixed scan lanes stayed near neutral. The rejection therefore
closes this carrier design, not the underlying specialization opportunity.

## Corrected causal model

Scratch reuse removed only eight allocation calls from the one-worker
`mixed-balanced` cohort build: the ordinary/cohort difference fell from 1,603
calls in R1 to 1,595 in R2. `mixed-output` fell from 418 extra calls to 416.
The dominant cost is not temporary closure vectors. It is constructing a
second complete pattern database, including duplicated NFA, prefilter, and
database-owned structures. The two-database design makes that work intrinsic.

Module extraction also did not recover the inactive scan guard. The cohort
scheduler is emitted out of line, but the feature binary still grows by about
28.9 KiB of text relative to baseline, and the same cold short-cell effect
persists. Further source reshuffling would cross into prohibited placement
tuning rather than supply a stable architectural boundary.

## Decision and future boundary

Do not run the fifteen-cycle formal matrix, authorize H43-A1, merge R2, relax
the thresholds, add warmups, or tune link placement. The predeclared early
falsification gate has already failed decisively.

A future cohort program requires a materially different carrier before
specialization can resume. The most plausible boundary is one compiled
database with lightweight cohort roots/views and shared database-owned
storage, rather than independently compiling one complete database per
cohort. That is a new representation hypothesis with its own design and
semantic proof; it is not another recovery of H43-D1-R2.

