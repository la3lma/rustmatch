# ADR-0005: Scan-local lazy determinization with exact fallback

- **Status:** Proposed pending the frozen I6 performance and resource gate
- **Date:** 2026-07-17
- **Decision owners:** rustmatch maintainers

## Context

The semantic NFA engine repeatedly walks the same consuming edges and epsilon
closures for every candidate start. That path is deliberately simple and
remains the correctness baseline, but its work grows poorly with large pattern
sets and corpora. Java rmatch demonstrates that cached state sets can help; it
does not determine a suitable Rust representation or admission threshold.

An unbounded DFA is not acceptable. Pattern combinations can create many state
sets, and assertion transitions depend on surrounding input context. The
optimization therefore needs an explicit memory boundary, an exact pressure
path, and proof that disabling it does not change results.

## Decision

Assertion-free scans create a cache local to that scan. It is not stored in the
immutable matcher and needs no synchronization.

- A deterministic state is a sorted boxed slice of dense NFA state IDs.
- A compact FNV-derived hash selects a collision bucket; complete slice
  equality decides identity.
- Each admitted state has a direct 128-entry `u32` ASCII transition table.
  Reserved values represent unknown, dead, and exact-fallback transitions.
- Non-ASCII UTF-16 transitions are materialized in a sparse per-state vector.
- Epsilon closure during materialization uses generation-marked dense visits,
  avoiding a full state-count clear for every new transition.
- Terminal ends are retained only for pattern ordinals touched at the current
  start. Those ordinals are sorted before delivery, preserving the previous
  observable order even though the public contract does not promise it.
- The default budget is 8,192 states. Its direct ASCII tables occupy at most
  4 MiB. `MatcherBuilder::state_cache_budget(0)` selects the NFA engine.
- When a nonzero budget cannot admit a state, that transition is recomputed by
  the exact NFA machinery. No eviction, approximation, or match suppression is
  permitted.
- Pattern sets containing assertions bypass the context-free cache. A future
  contextual cache requires a separate decision and evidence.

Repository benchmarks enable the non-default `benchmark-internals` feature to
collect scan-local counters. That feature and its hidden diagnostics are not a
supported application API.

## Consequences

- Ordinary matching gains locality without adding mutable shared state to
  `Matcher`.
- Every scan pays cache allocation proportional to states actually admitted;
  the direct table has a hard 4 MiB default ceiling.
- Large sets may mix cached and NFA work, but the result remains exact.
- Setting a tiny budget is useful both as a caller resource control and as a
  test oracle for pressure behavior.
- Assertion-heavy workloads do not receive I6 acceleration and must not pay
  for cache construction.
- The separate NFA path stays executable, testable, and available for future
  differential and adversarial testing.

## Evidence

- Generated pattern/input cases compare cache and NFA event multisets with the
  tiny HIR interpreter.
- Budgets zero, one, two, and the default preserve exact events; tiny budgets
  record fallback pressure.
- Assertion fixtures record an explicit cache bypass.
- Focused base/candidate and Wuthering Heights scale receipts are retained by
  the I6 experiment and must pass before this increment is complete.
