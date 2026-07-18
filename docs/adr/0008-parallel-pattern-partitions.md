# ADR-0008: Explicit scoped pattern-partition parallelism

- **Status:** Accepted
- **Date:** 2026-07-18
- **Decision owners:** rustmatch maintainers

## Context

The I6 cache and I7 start acceleration leave substantial semantic work for
candidate starts, especially as pattern and event counts rise. Patterns are
independent under the documented longest-match-per-pattern-and-start semantics,
so separate pattern subsets can scan the same immutable input concurrently.

The public `scan` callback is an infallible `FnMut`, not a concurrent sink.
Calling it from workers would require synchronization, a stronger public bound,
or observable races. Input partitioning would instead require proving overlap
and unbounded-lookahead rules for the complete regular-expression language.

## Decision

I8 partitions patterns, not input. `MatcherBuilder` receives an explicit
requested worker count, defaults to one, and rejects zero. Build assigns
patterns deterministically and compiles one immutable database and conservative
prefilter per partition. The actual partition count is bounded only by the
number of patterns, not by an arbitrary machine-size constant.

One partition retains the current direct scan path. Multiple partitions use
scoped standard-library threads, with the caller thread scanning one partition.
Workers collect partition-local matches. After every worker succeeds and is
joined, the caller invokes the existing sink serially. Delivery order remains
unspecified, as it already is in ADR-0002.

The configured deterministic-state budget is divided across partitions and
remains a total scan budget. Benchmark-only diagnostics aggregate partition
statistics and separately expose actual partitions, spawned workers, and
buffered result storage.

## Consequences

- Matching semantics can be checked as exact event-multiset equality across
  worker counts.
- The public callback remains ordinary `FnMut`; applications need no `Send`,
  `Sync`, mutex, or channel merely to request internal parallel scanning.
- Explicit multi-worker scans buffer events and may use materially more memory
  on result-heavy workloads. That cost is documented and measured.
- Scoped workers cannot outlive the scan. Spawn failures occur before callback
  delivery, and joined worker panics are resumed on the caller thread.
- Repeated scans pay thread startup because I8 does not add a persistent pool.
- No automatic worker heuristic is accepted without a separate Rust campaign.
- A later concurrent/fallible sink or persistent executor can be added without
  changing the one-worker behavior, but requires its own API and evidence.

## Rejected initial alternatives

Sharing the existing `FnMut` behind a mutex would serialize callbacks in the
worker hot path and make application callback latency part of lock contention.
Changing `scan` to require a concurrent sink would be a breaking API expansion.
Input partitioning is postponed because arbitrary repetition and assertions
make safe chunk boundaries a separate semantic project. A new runtime or
thread-pool dependency is also postponed until scoped standard threads have
been measured and shown insufficient.

## Admission evidence

The frozen protocol in
[`docs/experiments/i8-parallel-partitions.md`](../experiments/i8-parallel-partitions.md)
passed exact worker-count parity, lifecycle and failure tests, explicit memory
accounting, a protected one-worker default, and a repeatable native Rust
throughput result beyond noise. The retained
[`I8 evidence package`](../evidence/i8/ef61173/README.md) records a 60.09%
10,000-pattern improvement at eight workers, a 0.04% winner-repeat drift, the
complete thread sweep, and the associated memory and profile costs.
