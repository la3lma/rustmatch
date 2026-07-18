# I8 experiment: explicit parallel pattern partitions

## Status

Protocol frozen before production parallel-partition implementation. The
semantics, fixtures, thresholds, resource accounting, and baseline below may
not be relaxed after candidate results are known.

## Baseline and comparison design

- Frozen pre-I8 revision:
  **`f7a5e95588b96653f8cf3cb2445c6c17f571c4fe`**.
- Semantic comparison: one final I8 release binary built with one requested
  worker and with every worker count in the declared sweep.
- Default-path regression comparison: the frozen pre-I8 revision against the
  final candidate with one worker on the same identified runner.
- Every timing pair uses the same physical allocation, validates complete
  event evidence before timing is admitted, scrubs an identified cache working
  set, and retains raw JSON receipts.
- Java rmatch's parallelism and worker heuristic are hypotheses only. They do
  not count as Rust admission evidence.

## Proposed execution contract

- `MatcherBuilder` exposes an explicit requested worker count. The default is
  one, and zero is rejected at build time.
- The actual partition count is the smaller of the requested count and the
  number of registered patterns. No arbitrary upper cap is imposed.
- Pattern assignment is deterministic from registration order and balances
  partition pattern counts to within one.
- Each partition owns an immutable compiled database and conservative
  prefilter. A scan may read those partitions concurrently.
- The configured deterministic-state budget remains a total per-scan budget,
  divided deterministically among partitions. Parallelism may not silently
  multiply the documented cache-table allowance.
- One-partition scans use the established direct path without spawning a
  thread or buffering match events.
- Multi-partition scans use scoped threads. The caller thread performs one
  partition; additional partitions use named scoped workers that are joined
  before `scan` returns.
- The existing infallible `FnMut(Match)` sink is invoked serially on the caller
  thread after every partition scan succeeds. Callback order remains
  unspecified.
- A sink panic propagates only after workers have been joined. A worker-spawn
  failure is returned before any callback is invoked. An internal worker panic
  is joined and resumed on the caller thread rather than detached or hidden.
- Partition-local result collection is therefore proportional to emitted
  events in explicit multi-worker mode. Buffered event count and bytes must be
  reported. A concurrent or fallible sink API requires a separate public-API
  decision and is not smuggled into I8.
- No persistent thread pool is introduced in I8. Repeated scans pay scoped
  worker startup and teardown, which must be measured honestly.

## Correctness and lifecycle gate

- Complete normalized event multisets must be identical for requested worker
  counts 1, 2, 3, 4, 8, 12, 24, and an oversubscribed value larger than the
  pattern count.
- The worker-count matrix covers:
  - literals with overlapping starts and duplicate text under distinct IDs;
  - alternation, groups, greedy repetition, predicates, and case folding;
  - UTF-16 supplementary characters and isolated surrogates;
  - line anchors and word boundaries;
  - empty input and input ending at a match;
  - filterable, dense, mixed, unfilterable, and assertion-bearing sets.
- Deterministic generated pattern families compare every worker count with the
  one-worker reference path.
- Repeated build, scan, and drop cycles must complete without retained workers
  or changing event evidence.
- A controlled worker panic must be joined and propagated without deadlock.
- A controlled spawn failure must return a typed error before callback
  delivery. Test-only injection may be used to prove this otherwise rare path.
- A callback panic must propagate on the caller thread after all workers have
  stopped; the matcher remains reusable when the caller catches that panic.
- The public matcher remains safe to share across caller-managed threads. Two
  simultaneous scans of one matcher must preserve each scan's event multiset.

Any event difference, leaked worker, detached panic, callback before a scan
error, or deadlock rejects I8 regardless of speed.

## Native performance campaign

The retained Wuthering Heights campaign uses 1,000, 5,000, and 10,000 literal
patterns over the 8 MiB deterministic corpus expansion. The native worker
sweep is 1, 2, 3, 4, 6, 8, 12, 18, and 24 requested workers. Every point uses at
least three warmups and seven measured scans and records the actual partition
count.

The 10,000-pattern workload is the frozen positive target. Its best repeatable
multi-worker result must improve over the candidate's one-worker median by at
least 20% and 20 ms; both conditions apply. A threshold crossing is repeated
in reverse order. The selected winner must also repeat within 5% on a second
campaign before admission.

The 1,000- and 5,000-pattern sweeps diagnose startup cost, cache pressure, and
saturation. They are not required to improve because parallel execution is
explicit, but every result and any loss must be reported. A short-input guard
must demonstrate why callers should retain one worker for small scans.

The frozen pre-I8 revision and final one-worker candidate are compared on the
same 1,000/5,000/10,000 Wuthering line. A default scan regression fails only
when it exceeds both 3% and 1 ms after reverse-order confirmation. This protects
the existing default without turning sub-millisecond build or scan noise into
a false failure.

## Resource and build gate

- No unsafe Rust and no runtime dependency are added for worker management.
- Actual workers never exceed the smaller of requested workers and patterns.
- The sum of deterministic direct-cache tables may not exceed the bytes
  implied by the configured total state budget.
- Retained databases, retained prefilters, scan-local candidate bitmaps,
  worker stacks where observable, buffered events, and peak resident memory
  are reported separately.
- One-worker scans allocate no partition-result buffer and spawn no worker.
- Median one-worker build time may regress from the frozen baseline only when
  both 3% and 1 ms are exceeded after reverse-order confirmation.
- Multi-partition build time and retained memory are reported for every worker
  count. They may grow because patterns are compiled into separate databases,
  but the growth must be explained and cannot be hidden in scan throughput.

## Critical analysis requirement

Passing the positive target is necessary but insufficient. The final evidence
must explain absolute scan and build time, speedup and efficiency per worker,
where scaling saturates, event-buffer cost, candidate density per partition,
cache states and fallback pressure, CPU utilization, memory-bandwidth evidence
where available, scheduler/startup cost, peak memory, and every losing point.
Profiles must distinguish partition scanning, worker lifecycle, result
collection, and serialized callback delivery.

The final default remains one worker unless Rust evidence supports an automatic
policy in a separate decision. A green 12-core result is not permission to
encode one machine's optimum as a universal heuristic.
