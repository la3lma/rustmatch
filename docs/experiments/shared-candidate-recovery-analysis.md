# Shared candidate recovery analysis

**Status:** `investigate`; no successor implementation is authorized

**Predecessor:** `B2-H-0009`

**Production baseline:** `da755b0069c94d4da9db74a8778dd9932fc64671`

## Executive conclusion

The shared-candidate line should continue. H9 preserved exact event semantics
and improved every primary target by 122.143% to 734.672%. Its strongest target
ran 8.35 times as fast and used substantially less peak memory. The algorithm
removed the intended bottleneck: every pattern partition no longer traversed
the complete corpus to rediscover the same possible starts.

The rejected candidate mixed that algorithm with candidate-only measurement
instrumentation in the timed executable. `main` and all four direct
one-partition scanner specializations moved by exactly `0x1cf0` bytes even
though every scanner retained its baseline size. Static control builds show
that removing the H9-only harness and library diagnostics recovers `0x1540`,
about 73% of that displacement. The rejected one-worker guard never executed
the shared planner.

This is a strong instrumentation and code-layout confound. It does not prove
that removing the extra fields will eliminate the 2.00249% regression. The
exact candidate remains correctly rejected and unmerged under its frozen
`G9-v1` ruling. Under the later
[`G9-v2` decision policy](../optimization-decision-policy.md), the mechanism is
`investigate`: the contrast obligates a substantially narrower and more
principled successor than padding or a wholesale algorithm rewrite.

## What H9 established

H9 used one union literal filter and one candidate bitmap. Eight scoped workers
filled disjoint bitmap word ranges without locks. The semantic workers then
retained only candidates allowed by their own partition's necessary prefixes
before invoking the unchanged verifier.

The canonical six-pair results were:

| Frozen cell | Median throughput effect |
|---|---:|
| Sparse 50 MiB, 16 workers | +395.281% |
| Sparse 50 MiB, 24 workers | +734.672% |
| Dense 50 MiB, 16 workers | +122.143% |
| Dense 50 MiB, 32 workers | +322.926% |
| Sparse 7,500-pattern neighbor | +423.523% |
| Dense 5,000-pattern neighbor | +166.256% |
| Mixed-regex semantic guard | +438.742% |

The sparse 24-worker baseline traversed approximately 1.258 billion aggregate
partition starts. The candidate traversed approximately 52.5 million input
starts once, then filtered the 55,748 union candidates inside the relevant
semantic partitions. Target peak RSS improved by 47% to 98%.

This is not a marginal result that depends on a favorable summary statistic.
All six pairs favored the candidate on all four primary targets.

## Why H9 was not admitted

The zero-output 50 MiB, one-worker guard regressed 2.00249%, with all six pairs
negative. That crossed the frozen two-percent demonstrated-regression boundary.
The separate absolute three-percent veto did not fire, but the `G9-v1`
no-regression rule rejected the exact artifact. `G9-v2` preserves that
non-admission and classifies the evidence as an investigation rather than a
terminal algorithmic rejection.

This guard has one partition. H9 cannot compile a shared union for fewer than
16 partitions and cannot plan shared candidates for inputs below 32 MiB. The
guard therefore executed the private path, not the new algorithm.

The mitigation sequence is informative:

| Candidate step | Mean zero-guard time change in retained smoke evidence |
|---|---:|
| Private scan isolation | +39.103% |
| Private diagnostics isolation | +41.327% |
| Original private literal loop restored | +34.506% |
| Shared predicate code generation separated | +2.117% |
| Inactive allocation removed | +2.055% |
| Final guarded matrix | +2.00249% throughput regression |

Separating shared-only predicate code generation removed most of the inactive
regression. The remaining failure was much smaller but still real under the
frozen rule.

## Timed-harness layout finding

The final candidate changed `rustmatch-bench/src/main.rs` only to add four
shared-plan fields to `CacheDiagnosticsReceipt` and populate them from
`ScanDiagnostics`. The receipt derives `Clone`, `Deserialize`, and `Serialize`,
so those source lines generated substantially more executable code. The
library also added corresponding `ScanDiagnostics` fields and conversion
plumbing.

The baseline and candidate symbol inventories show one uniform insertion before
the direct scanner block:

| Symbol | Baseline address | Candidate address | Delta | Size unchanged |
|---|---:|---:|---:|---:|
| `main` | `0x140910` | `0x142600` | `0x1cf0` | yes |
| `scan_with_stats<rust_events>` | `0x140a30` | `0x142720` | `0x1cf0` | `0x3340` |
| `scan_with_stats<rust_native_ids>` | `0x143d70` | `0x145a60` | `0x1cf0` | `0x3527` |
| `scan_with_stats<rust_event_summary>` | `0x1472a0` | `0x148f90` | `0x1cf0` | `0x3850` |
| `scan_with_stats<harness_event_count>` | `0x14aaf0` | `0x14c7e0` | `0x1cf0` | `0x2fd0` |

`harness-run`, including the rejected guard, times
`harness_event_count`. Shared diagnostics are collected outside the timed
iterations. They nevertheless changed the timed function's placement.

Binary comparison also found the final private partition specialization had
the same `0x55e4` size and the same 4,341-instruction mnemonic sequence as the
baseline. Its differing bytes were relocation operands.

Two Rust 1.97 Linux static control builds removed diagnostics without running
timing:

| Build | `main` address | Delta from baseline |
|---|---:|---:|
| Baseline | `0x140910` | `0x0` |
| Full H9 | `0x142600` | `0x1cf0` |
| H9 without four harness receipt fields | `0x141100` | `0x7f0` |
| H9 without harness or library shared diagnostics | `0x1410c0` | `0x7b0` |

The diagnostic pipeline therefore accounts for `0x1540` of the `0x1cf0`
uniform shift. The shared implementation still contributes a residual
`0x7b0`, so instrumentation neutrality is necessary but may not be sufficient.
The direct scanner sizes remained identical in both controls.

These facts establish a measurement confound, not a causal acquittal. A fresh
candidate must remove the confound and pass the same guard. H9 itself remains
rejected and unmerged; the shared-candidate mechanism remains under
investigation.

## Recommended successor

The first successor should be an **instrumentation-neutral, pipeline-isolated
shared-candidate candidate**, not an immediate crate split.

### 1. Preserve the timed harness

`rustmatch-bench/src/main.rs` must be byte-for-byte identical to the production
baseline for the timed artifact. Candidate-specific receipt fields, Serde
schema changes, commands, and conversion code are forbidden in that artifact.

Shared-phase evidence should come from a separately identified diagnostic
artifact or external profiling:

- a diagnostic-only feature or executable may expose shared-plan timing;
- the diagnostic artifact must never supply admission timing;
- Linux `perf`, thread names, symbol inventories, and internal tests can prove
  parallel construction without changing the timed harness;
- the diagnostic and timed artifact hashes must both be retained.

### 2. Separate whole private and shared pipelines

The inactive path should not branch inside a modified common
`scan_partition`. Preserve the baseline private helper's signature and body,
and add a distinct shared helper:

- private eligibility failure calls the original private orchestration;
- shared eligibility success calls separate shared orchestration;
- `scan_partition` remains the baseline private function;
- `scan_partition_shared` owns shared candidate filtering;
- shared planner, bitmap, predicates, and diagnostics live in separate modules.

The decision between whole pipelines happens before semantic workers start.
This is source-level isolation and is independently reviewable.

### 3. Escalate only if necessary

If the instrumentation-neutral candidate still moves or regresses the private
path, terminate that experiment. A later, separately reviewed hypothesis may
move the union filter, bitmap, and sharding implementation into an unpublished
internal workspace crate. That crate should accept primitive slices and return
an owned bitmap; it should not receive callbacks or semantic-engine types.

If a crate boundary is still insufficient, a monomorphic private scan core
behind a small generic adapter is the next structural option. That is a larger
change because dynamic callback dispatch can affect dense-output workloads, so
it should not be bundled into the first successor.

## Frozen-gate proposal

A future B2 review may authorize exactly one successor with these stages:

1. **Static preflight**
   - baseline remains the measured production revision;
   - the timed `rustmatch-bench/src/main.rs` hash matches baseline;
   - no candidate-specific diagnostics exist in the timed artifact;
   - private scanner sizes and mnemonic sequences are compared;
   - both timed and diagnostic artifact identities are retained.
2. **Correctness**
   - locked workspace tests and doctests pass;
   - shared/private equality tests cover sparse, dense, mixed-regex, Unicode,
     assertion, and fallback activation boundaries;
   - all timed receipts retain exact event counts and normalized digests.
3. **Discriminating preflight**
   - run the frozen zero-output guard with balanced AB/BA order;
   - do not edit or retune after seeing that result;
   - stop admission timing and investigate any demonstrated regression above
     two percent.
4. **Complete admission matrix**
   - rerun all H9 primary targets, neighbors, semantic guards, and fallbacks;
   - retain the five-percent target floor, two-percent demonstrated-regression
     boundary, and absolute three-percent veto;
   - merge only if every target and guard passes; classify a repeatable
     two-to-three-percent regression as `investigate`, never as admission.

The preflight is a cost-saving stage, not a weaker gate. A passing preflight
does not admit code; the complete matrix remains mandatory.

## Approaches explicitly rejected

- Do not average away a regression because the target gain is large.
- Do not pad or reorder functions until one benchmark happens to pass.
- Do not omit the zero-output or small-input guards.
- Do not time a candidate-only receipt schema against the baseline schema.
- Do not merge the optimization disabled by default and claim the feature-off
  binary as evidence for the feature-on path.
- Do not combine planner pooling, pipelining, shard-count tuning, or a new
  literal algorithm with the recovery candidate.

Those ideas either hide the failure, overfit the measured executable, or make
the next result impossible to attribute.

## Recommendation

Review and freeze the instrumentation-neutral successor first. It is the
smallest change that directly tests the newly identified confound while
preserving H9's demonstrated algorithm. Keep the internal-crate boundary as a
ranked fallback, not as speculative complexity in the first retry.

Until that review occurs, production remains unchanged and H9 remains absent
from the merged-improvement graph.
