# B1 experiment: correctness-gated cross-engine campaign

## Status

Active. This protocol is frozen before the stable-host B1 results are run. A
change to the matrix, engine semantics, timing boundary, worker sweep, or
acceptance rule requires an explicit protocol revision before affected results
are interpreted.

The designated host is `agogo.local`, previously recorded as a 16-core,
32-thread AMD Ryzen 9 9950X3D Linux machine. Execution cannot begin until the
host is reachable and its current CPU, memory, kernel, Docker, governor, load,
and isolation metadata have been captured again. Old host descriptions are not
silently reused as current evidence.

## Questions

B1 must answer, without collapsing unlike execution models:

1. How does Rust rustmatch throughput scale with pattern count, corpus size,
   match density, and explicit pattern partitions?
2. Where does Rust rustmatch sit relative to Java rmatch, RE2/J, Java regex,
   and the one-thread native Hyperscan reference on a shared event-enumeration
   task?
3. How does Rust `RegexSet` compare on its native set-membership task, and what
   changes when event enumeration is required?
4. Which worker counts maximize retained whole-task throughput for each engine
   and workload, and how stable are those winners?
5. Where do scaling discontinuities appear, and what do profiles and engine
   diagnostics say about them?

## Exact engine identities

- Rust rustmatch uses commit
  `da755b0069c94d4da9db74a8778dd9932fc64671`, built with the committed lockfile
  and Rust `1.97.0`. This is the frozen B1 candidate whose runner accounts for
  every requested warm-up and measurement scan. Any later candidate receives a
  separate exact identity and base/candidate receipts.
- Java rmatch uses the published Maven Central artifact
  `no.rmz:rmatch:2.0.0-RC1`, including its recorded artifact checksum.
- RE2/J uses `1.8`.
- Java regex uses OpenJDK `21` and records the full runtime build.
- Rust `RegexSet` uses the workspace-pinned `regex 1.13.1` until an explicit
  dependency revision is admitted.
- Hyperscan uses release `5.4.2` through its general regex API. Its pure-literal
  API, if measured, is a separately labeled best-case lane.

Every retained receipt records the container image ID in addition to the human
version. Moving tags, branch names, dirty worktrees, and unrecorded local
patches are rejected.

## Semantic lanes

### Event-enumeration task

Rust rustmatch, Java rmatch, Hyperscan, RE2/J, and Java regex must enumerate the
same generated non-overlapping events. Fixtures avoid self-overlap and
cross-pattern ambiguity that would expose known differences between
leftmost/non-overlapping and overlap-preserving APIs. A timing is rejected
unless its manifest count agrees exactly.

RE2/J and Java regex compile and retain every pattern, reuse matcher objects,
and scan each pattern to exhaustion. Their physical traffic is pattern count
times corpus size, but their reported task throughput remains one logical
corpus divided by the time to apply the complete pattern set. Inflating their
throughput by physical rescans would be unfair.

Hyperscan's `hsbench -T` mode gives each thread an independent copy of the full
corpus. It measures aggregate independent-stream throughput, not parallel
completion of one shared task. B1 therefore retains Hyperscan at one thread as
a native reference. Multi-thread Hyperscan may be explored separately, but it
may not appear as a peer in the maximum-throughput task chart.

### RegexSet-native task

`RegexSet` answers which patterns match, not where or how often they match. Its
native set-membership lane is useful and must be measured, but is labeled as a
different task. An event-preserving adapter may also be measured; it is not
presented as the native `RegexSet` result. Neither lane can satisfy the event
count gate by substituting matched-pattern count for event count.

## Scenario matrix

The reproducible generated intersection uses both `diverse_literals` and
`mixed_regex` families.

The core sparse matrix is the Cartesian product of:

- 1,000, 2,500, 5,000, 7,500, and 10,000 patterns;
- 8 MiB and 50 MiB corpora; and
- both families.

Sparse fixtures retain four matches per literal pattern and two matches per
mixed-regex pattern. At 1,000, 5,000, and 10,000 patterns, both sizes and both
families also receive:

- a zero-match fixture; and
- a controlled dense fixture with 100,000 total events at 8 MiB or 500,000 at
  50 MiB.

This yields 44 ordinary scenarios: 20 sparse core points and 24 additional
density controls. A 256 MiB cache-pressure checkpoint at 1,000 patterns in both
families is run after ordinary calibration. It is intentionally large enough
to exceed the host's shared last-level cache. Multi-pass engines are included
at one worker and their selected 50 MiB worker count, even if the result is
slow; no engine is omitted merely because the checkpoint is unfavorable.

The established Wuthering Heights 1,000/5,000/10,000 line remains the realistic
language-corpus scaling control. Its exact ASCII source files and hashes come
from Rust commit `65453779f2560014fba020161b262ca660aeb58b`. Because the
repository is private, the harness extracts the blobs from that exact commit
through a local or SSH-accessible Git clone and then verifies SHA-256; it does
not retain a temporary authenticated download URL. Wuthering results complement
generated fixtures; they do not replace the controlled family/density matrix.
Together with the 44 ordinary generated scenarios and two 256 MiB checkpoints,
these controls produce 49 frozen scenarios. The preregistered initial worker
sweeps contain 1,891 runs before adaptive winner confirmation and profiling.
The executable campaign is frozen in the private benchmark repository at commit
`91466be8fbc0f117433bf7ba6d9168aef477538e`. That revision includes the exact
initial matrix, fresh-host resumable execution, deterministic winner
confirmation, confirmed cache-pressure and profile plans, a self-contained
hash-verified report archive, and the reversible clean-host window used for
authoritative execution. It also preserves each run's UTC start and finish
time, reports the campaign measurement window, requests untruncated Docker
identities, and rejects the earlier RegexSet and Rustmatch runners whose
consistency scans occurred outside their declared warm-ups. Both corrected
runners prove that two requested warm-ups plus three measurements execute
exactly five full scans. The frozen scenario dimensions and other engine inputs
are unchanged.

The initial timing window remains attributed to harness revision
`91466be8fbc0f117433bf7ba6d9168aef477538e`. Before the separate profile window,
benchmark revision `221b51d3c781939625100de2cf7eeca6f075f3f4` strengthened the
profile and report path without changing any of the 1,891 initial timing runs.
It validates and independently re-parses raw hardware counters; reports IPC,
miss rates, scheduler activity, counter running time, and process RSS; expands
the core profile line described below; and exposes adjacent pattern scaling,
host-relative worker retention, and exact anomalies for critical review. The
report also preserves whole-process wall effort separately from preparation
and scan throughput. The final report retains both window revisions rather
than attributing earlier timings to the later profiler.

The Wuthering event expectations are 109,693, 926,975, and 1,912,854. The
separate RegexSet membership expectations are 901, 4,693, and 9,466. The new
engine-neutral fixture was locally checked against Rust rustmatch at all three
event points; authoritative timings still await the stable host.

## Declared worker sweeps

The full sparse-core sweeps are:

- Rust rustmatch and Java rmatch: `1, 2, 3, 4, 6, 8, 12, 16, 24, 32, 48, 64`;
- RE2/J and Java regex: `1, 2, 4, 8, 16, 32, 64, 128, 256`;
- RegexSet application-level task lanes: `1, 2, 4, 8, 16, 24, 32, 48, 64`;
  and
- Hyperscan native reference: `1` only for the shared-task chart.

Oversubscribed values are intentional. Requested and actual workers are both
recorded. Density controls use the abbreviated preregistered sets
`1, 4, 8, 16, 32, 64` for Rust/Java rmatch and
`1, 8, 32, 64, 128, 256` for RE2/J/Java regex. A measured winner is not inferred
from adjacent points that were never run.

The Wuthering language controls use the same abbreviated worker grids as the
density controls. This retains one realistic pattern-count line without
duplicating the entire synthetic calibration matrix.

The unoptimized Rust NFA control is limited to the two 1,000-pattern / 8 MiB
sparse scenarios with one warm-up and one retained measurement. I9 measured
more than 52 seconds per NFA scan. Repeating that settled architectural contrast
at every B1 point would consume machine time without improving the comparative
answer. This exception was declared before B1 results.

## Timing and run order

- Fixture generation, file reads, encoding conversion, compilation, matcher
  preparation, warm-up, scan, callback, and process wall time remain distinct.
- Task throughput is logical corpus bytes divided by median complete-pattern-set
  scan time. It is never multiplied by patterns or workers.
- Sweep points use two warm-ups and three measurements.
- Hyperscan is invoked with `--per-scan`; its declared warm-up scans are
  excluded and the median of the remaining scan throughputs is reported.
- The apparent winner and its nearest declared neighbors are rerun with three
  warm-ups and seven measurements.
- Winner confirmation is repeated in reverse order. A winner with more than 3%
  median drift or contradictory ordering remains inconclusive.
- Scenario/engine/worker execution order is deterministically shuffled from a
  recorded campaign seed. Failed or interrupted points resume without changing
  the original order.
- Every executor invocation requires a passing host receipt no more than 15
  minutes old and an independent live host check. Resumed invocations create a
  new host session in campaign state, and every run records its session ID;
  stale idle-host observations cannot authorize later work.
- The host must be otherwise idle. CPU affinity, frequency policy, thermal
  state, and co-located load are recorded; unexplained interference invalidates
  the affected block.

The 8 MiB and 50 MiB scans are steady-state throughput lanes. The separate
256 MiB checkpoint supplies the explicit beyond-last-level-cache pressure; B1
does not falsely describe every ordinary point as a cold-cache measurement.

## Correctness and admission

A retained run requires:

- exact scenario, input hashes, engine artifact, container image, host, mode,
  requested workers, actual workers, warm-ups, repetitions, and timing arrays;
- successful compile and scan;
- the manifest's exact event count for event-enumeration lanes;
- stable count across every warm-up and measured scan;
- complete and internally coherent engine-specific diagnostics where exposed;
  and
- no unsupported syntax silently dropped from the pattern set.

Cross-engine event digests are compared only where event identities are known
to have the same semantics. Count equality is mandatory everywhere in the
event lane. RegexSet-native membership uses its own manifest expectation and
cannot be relabeled as event parity.

## Critical analysis and profiling

Green validation is the beginning of analysis, not its conclusion. Reports
must discuss pattern-count slopes, corpus-size effects, density sensitivity,
worker optima, oversubscription, compile/prepare cost, memory and callback work,
outliers, crossovers, and discontinuities such as the historical 1,000-to-10,000
throughput drop.

Linux `perf stat` and sampled profiles are retained for Rust rustmatch at 1,000,
2,500, 5,000, 7,500, and 10,000 patterns in single-worker and
confirmed-winner modes for both 50 MiB core families. These 20 evidence roles
exceed the original endpoint minimum and locate a scaling discontinuity rather
than asking endpoint profiles to explain an unknown transition. Identical
single-worker and winner commands collapse into one physical point with both
roles. Any unexplained cliff or engine-order reversal outside this complete
core line adds a profile point before a claim is accepted. Profiles record
symbols, exact image, kernel restrictions, command, validated counters, raw
output, and whole-process resource use.

## Post-B1 optimization handoff

B1 measures and explains the engines; it does not admit optimizations. After
all required points, confirmations, and profiles are complete, the
[B2/G9 protocol](b2-competitor-win-optimization.md) ranks the confirmed cells
where RegexSet beats Rustmatch and profiles Rustmatch on those exact weak
fixtures. Native set-membership and complete-event results remain separate.

Those competitor wins nominate Rust-native hypotheses. They do not become the
baseline for accepting a change. Each candidate is measured against the exact
existing Rustmatch production revision frozen before implementation. A
candidate is retained only when it preserves results, improves that Rustmatch
baseline beyond the declared noise threshold on its target set, and avoids an
unaccepted broader regression. It may also improve other workloads; such gains
are welcome and reported. RegexSet parity is neither required nor sufficient.

This handoff adds no point to the frozen 1,891-run initial B1 plan and does not
change B1's engine identities, measurement boundaries, or exit criteria.

## CI boundary

GitHub Actions keeps the optimized Wuthering Heights C2 tripwire: 5,000
patterns, an 8 MiB deterministic corpus, exact event evidence, same-runner
base/candidate comparison, reverse-order retry, and a failure only beyond both
100% slowdown and 50 ms. It is a useful build-breaking detector for severe
regressions.

Hosted CI is not B1 evidence. It does not replace stable-host worker sweeps,
cross-engine receipts, cache-pressure checkpoints, or profiles. The NFA control
is far too expensive for routine CI and is not added to that lane.

## Exit criteria

B1 completes only when:

1. every required point is either a validated receipt or an explicitly reviewed
   protocol exception;
2. complete declared thread sweeps and confirmation runs are retained;
3. ordinary and cache-pressure plots can be regenerated from receipts;
4. critical analysis and required profiles are published;
5. no chart equates unlike semantics or unlike parallel tasks; and
6. the campaign is rerunnable from exact versions without access to an
   uncommitted worktree.
