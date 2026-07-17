# I6 lazy deterministic-state cache experiment

## Status

Active. The benchmark foundation was merged before cache implementation. The
exact baseline revision is frozen below; candidate code may not alter the
fixtures or admission thresholds.

## Mechanism under test

The candidate prototype interns epsilon-closed NFA state sets as sorted boxed
state-ID slices, using a hash bucket followed by complete equality. Transitions
are materialized on demand. Materialized states have a direct 128-entry `u32`
ASCII transition table and a sparse non-ASCII path. A state-count budget bounds
cache growth. When the budget cannot admit a new state, scanning continues
through the exact NFA path. Assertion-bearing sets bypass this context-free
cache.

This is a Rust hypothesis, not a porting entitlement. Java rmatch motivates the
experiment but supplies no admission evidence.

## Baseline and candidate

- Baseline revision: **`9ef1b17cc6a865f44920c7c4f74f4a44b0b2c8f4`**.
- Candidate revision: the final I6 implementation commit.
- Build: `cargo build --locked --release --package rustmatch-bench` for both
  revisions.
- Runner: the same identified physical runner and allocation for both builds.
- Focused comparison command: `scripts/i6-focused-campaign.sh`.
- Scale command: `scripts/i6-campaign.sh`.

## Workloads

All fixtures are deterministic 7-bit ASCII, validate their exact event set
before timing, and retain compilation and scan time separately.

| Scenario | Patterns | Corpus | Gate | Why it exists |
|---|---:|---:|---|---|
| `literal-sparse` | 64 | 1 MiB | at least 5% faster | Existing representative sparse-literal workload |
| `literal-sparse` | 256 | 2 MiB | at least 5% faster | Larger shared start closure and transition reuse |
| `mixed-sparse` | 64 | 1 MiB | at least 5% faster | Alternation and repetition create nontrivial subsets |
| `literal-dense` | 64 | 1 MiB | no more than 3% slower | Event delivery can dominate transition work |
| `assertion-bypass` | 64 | 1 MiB | no more than 3% slower | Context-sensitive patterns must retain the exact path |

The retained receipt records revision, runner, scenario, fixture dimensions,
event count and digest, warm-up and measured iteration counts, compilation
time, and median scan time.

The focused admission fixtures are accompanied by the cache-pressured
[`I6-B1` Wuthering Heights scale campaign](../benchmarking/wuthering-scale.md).
That lane records 1,000, 5,000, and 10,000 literal patterns against the full
legacy corpus. It scrubs a dedicated 256 MiB buffer before every scan and
publishes both raw JSON receipts and an HTML table. The scale lane diagnoses
pattern-count behavior; it does not dilute the focused 5% improvement gate or
stand in for cross-engine benchmarking.

## Noise and admission rule

- Three warm-up scans and seven measured scans are run per scenario.
- The median measured scan time is compared.
- A target workload must improve by at least 5%, not merely avoid regression.
- A non-target workload may not regress by more than 3%.
- A failed comparison is repeated once in reverse base/candidate order before
  it is accepted as a failure.
- The complete event digest must be identical before timing can be admitted.

The cache is rejected or redesigned if either target workload misses its
positive threshold, any guard workload crosses its regression ceiling, event
output differs, or the bounded-cache fallback cannot be proved exact.

## Resource tradeoffs

The first production candidate may use at most 4 MiB of scan-local cache table
storage at its default budget, excluding the NFA state-set payload itself. The
8,192-state default uses exactly that table allowance; in a preliminary
10,000-pattern Wuthering run, a 4,096-state variant used 2 MiB but scanned
14.7% slower. The cache allocates lazily and exposes internal
hit/miss/state/fallback counters to tests and receipts through a non-default
benchmark feature, not the supported application API.
Compilation time may not regress by more than 3% in a repeatable result. Peak
memory and cache-state count must be retained during the stable-machine run;
if they cannot yet be measured automatically, the gap blocks I6 completion,
not the benchmark-foundation merge.

## Correctness evidence required

- Optimized and NFA-baseline event multisets agree for generated pattern/input
  sets.
- Budgets of zero, one, and a small finite value exercise disabled, immediate
  pressure, and partial-cache behavior.
- A budget miss falls back without losing, duplicating, or reordering the
  normalized event multiset.
- Assertion-sensitive pattern sets bypass or use explicitly context-keyed
  states; context-free keys must never be reused across incompatible context.
- The complete repository semantic and Java differential gate remains green.
