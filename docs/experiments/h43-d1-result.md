# H43-D1 cohort-only cost and code-layout result

**Decision:** rework the semantically inert cohort spine; do not authorize
specialization yet

**Production baseline:** pre-K1
`05992bf1f20e0f9384fc52fcb4a9863500e7a4da`

**Cohort-only candidate:** E1
`1356197d1581ad28f6cba822d91e2d6bd5aff04d`

**Review date:** 2026-08-06

## Question

H43-D1 asked whether the exact, semantically inert assertion-free/assertion-
bearing cohort spine is cheap and stable enough to carry a future specialized
backend. It measured preparation and scan time separately, retained allocation
and RSS proxies, compared compile and artifact sizes, and distinguished the
unchanged ordinary path, one-cohort controls, and true two-cohort inputs.

This was a diagnostic gate. It did not implement or authorize specialization.

## Formal window

The hash-bound Agogo window completed 720 timing receipts and 108 resource
receipts over four fixtures, workers `1/4/16`, fifteen alternating timing
cycles, three resource cycles, and five compile cycles. Every receipt preserved
the expected event count and digest. The fixture activation reports proved
one cohort for assertion-free and assertion-only controls and two cohorts for
both mixed fixtures.

The window ran with an empty Docker runtime and idle GPU. Its contamination
record and Docker-event log are both empty. The three managed containers were
restored exited with restart policy `no`.

## Results

### Ordinary and one-cohort guards

The candidate's ordinary mode did not produce a stable scan blocker against
the pre-K1 source. One assertion-free 16-worker aggregate was 3.51% slower,
but only 8 of 15 pairs were slower; it did not meet the frozen 13-of-15
directional burden. All other ordinary scan aggregates remained below 3%.

One-cohort scan execution stayed within one percent of ordinary execution:

| Control | 1 worker | 4 workers | 16 workers |
|---|---:|---:|---:|
| Assertion-free | -0.36% | +0.73% | -0.64% |
| Assertion-only | +0.22% | -0.77% | +0.96% |

Positive percentages in this report are gains; negative percentages are
regressions. These controls show that the direct one-cohort scan path itself
is sound and that adding the cohort source did not establish a stable inactive
ordinary scan regression.

### Mixed-cohort signal

| Fixture | 1 worker | 4 workers | 16 workers |
|---|---:|---:|---:|
| Mixed balanced | **+57.58%** | -0.48% | -2.26% |
| Mixed duplicate output | **+90.46% (10.48x)** | **-53.85%** | **-37.46%** |

The one-worker gains are real and repeat in all 15 pairs. Splitting assertion-
free expressions lets their existing necessary-literal prefilter activate
instead of being disabled by the assertion-bearing registrations. The
duplicate-output fixture reduced the profiled instruction count from 118.93
billion to 12.59 billion over 100 scans, an 89.42% reduction.

The multi-worker losses are equally real and repeat in all 15 pairs. The
current implementation gives each cohort part of the requested partitions,
then scans the two partition groups in separate sequential thread scopes. At
four workers it therefore runs two workers at a time; at sixteen it runs eight
at a time.

Root `perf stat` confirmation over seven independent 100-scan processes showed:

| Cell | Ordinary task / wall | Cohort task / wall | Average CPUs | Instructions | Context switches |
|---|---:|---:|---:|---:|---:|
| 4 workers, ordinary | 493 / 166 ms | - | 2.97 | 15.04B | 110 |
| 4 workers, cohort | - | 489 / 255 ms | 1.92 | 15.34B | 249 |
| 16 workers, ordinary | 1,230 / 132 ms | - | 9.30 | 32.55B | 184 |
| 16 workers, cohort | - | 1,244 / 182 ms | 6.83 | 32.72B | 503 |

Task time and instructions are nearly unchanged at 4 and 16 workers, while
wall time rises as average CPU occupancy falls. The 2.3x-2.7x context-switch
increase is consistent with constructing two thread scopes. This establishes
lost parallel occupancy, not an inherently slower matching algorithm, as the
multi-worker cause.

### Preparation and resources

Cohort preparation crossed the unchanged 3% veto in every stable control:

- assertion-free controls regressed 10.61%-15.32%, or about 9-13 microseconds;
- assertion-only controls regressed 20.78%-27.34%, or about 19-22 microseconds;
- mixed-balanced controls regressed 10.25%-46.00%, or about 15-30 microseconds.

The cause is the build path calling the complete diagnostic classifier. It
walks each HIR repeatedly to derive prefix, range, ASCII, assertion, and node-
count facts even though cohort execution needs only one assertion-bearing bit.
The one-cohort resource cells recorded 646 extra allocations and 8,196 extra
requested bytes for assertion-free inputs, and 838 allocations plus 10,500
bytes for assertion-only inputs. Peak live growth stayed around 5.4 KiB.

No RSS comparison crossed the frozen 1 MiB materiality floor. The largest
positive median delta was 421,888 bytes. Compile time changed from 0.61 to
0.62 seconds (+1.32%). The default rlib grew 2,938 bytes (+0.13%); the
feature-enabled diagnostic rlib grew 105,292 bytes (+4.34%). The complete
benchmark binary grew 14,344 bytes (+0.36%), while its text section grew
10,372 bytes (+0.36%).

## Decision

H43-D1 returns **rework**. H43-A1 remains blocked.

The unchanged policy correctly prevents authorization: stable preparation
regressions exceed 3%, and the true mixed-output guards regress 53.85% and
37.46% at 4 and 16 workers. Those failures cannot be waived by the 10.48x
one-worker gain.

The result is not a rejection of cohorting. It exposes a high-value exact
mechanism and two narrow plumbing defects with direct evidence. H43-D1-R1 may
attempt only these causal recoveries:

1. Replace the full diagnostic classifier in the execution build path with a
   minimal, allocation-bounded assertion split. Diagnostic reporting remains
   separate and complete.
2. Keep sequential cohort execution when the caller requests one worker, but
   for `workers >= 2` prepare per-cohort candidate plans and execute all cohort
   partitions in one thread scope so total active partitions can reach the
   requested worker count.
3. Preserve global IDs, exact event multiplicity, complete buffering before
   callback delivery, worker-panic/error atomicity, cache-budget conservation,
   and deterministic cohort assignment.
4. Rerun E1 and the unchanged D1 matrix. Only a clean D1-R1 result may
   authorize H43-A1.

No backend specialization, eligibility tuning, threshold movement, fixture
change, or guardrail exception is authorized by this result.

## Retained evidence

- Formal plan SHA-256:
  `2aeac836d20314a9f347b067a546bf8832f4f09c1d2d21f8cc1ea27b6dd37558`
- Formal summary SHA-256:
  `bf7733c62a504e64f867873c3a0f75169632c7cc5aa09565ef3b512810c731f1`
- Window-state SHA-256:
  `1ed9cf9b7074e912bb56a75fa1824ae7961551f6447a68079ee1e811fb490444`
- Fixture manifest SHA-256:
  `6dc876ca75a1bfea582364357cfd4289076b9866c6a21341f8143aaa8401b625`
- Service snapshot SHA-256:
  `eef0fdc5d42f27d60549167068ca4d4fe1a28f718c0b1cb83bdb50365a6c25ab`
- Complete raw evidence archive SHA-256:
  `643aa43c0556582d13e39602b6ede4c07c87211da5c45e8881aed6fdb12d8082`
- External archive:
  `/Users/rmz/.codex/handoffs/rmatch-performance-measurements/h43-d1/h43-d1-formal-evidence.tar.gz`
- Compact retained summary and profiles:
  `docs/experiments/h43-d1/`
