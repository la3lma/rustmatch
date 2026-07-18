# I9 evidence: ordinary benchmark-harness integration

## Outcome

I9 is complete for rustmatch commit
`fdd5efa674d34221b0dc3b13520dcbe5b903f6bc`. The ordinary private performance
harness gained the Rust-native engine in
[`rmatch-performance-measurements` PR #1](https://github.com/la3lma/rmatch-performance-measurements/pull/1),
merged as `04eb7ae`.

The harness now:

- exports only an exact 40-character commit through `git archive`;
- excludes dirty files, untracked files, repository credentials, and `.git`;
- builds the locked workspace with the pinned Rust `1.97.0` toolchain in a
  multi-stage container and runs a small unprivileged runtime image offline;
- records exact source, image, host, input, timing, partition, event, and
  implementation-diagnostic evidence;
- rejects incomplete or inconsistent output even when the engine exits zero;
  and
- plots retained Rust receipts through the ordinary plot generator.

The retained receipts and plot live in the private harness campaign
[`rainstick-2026-07-18-rustmatch-i9-smoke`](https://github.com/la3lma/rmatch-performance-measurements/tree/main/receipts/rainstick-2026-07-18-rustmatch-i9-smoke).

## Correctness matrix

| Scenario | NFA | Optimized 1 worker | Optimized 8 partitions | Event identity |
| --- | --- | --- | --- | --- |
| 1,000 diverse literals / 8 MiB | 4,000 | 4,000 | 4,000 | identical digest |
| 1,000 mixed regex / 8 MiB | 2,000 | 2,000 | 2,000 | identical digest |

All six outer receipts passed manifest counts, exact source identity, toolchain,
mode, partition, timing-array, event-digest, and diagnostic validation.

## Critical performance reading

| Scenario | Mode | Median scan | Throughput |
| --- | --- | ---: | ---: |
| Diverse literals | NFA, 1 worker | 53.541 s | 1.25 Mbit/s |
| Diverse literals | Optimized, 1 worker | 23.931 ms | 2,804.24 Mbit/s |
| Diverse literals | Optimized, 8 partitions | 12.244 ms | 5,481.16 Mbit/s |
| Mixed regex | NFA, 1 worker | 52.319 s | 1.28 Mbit/s |
| Mixed regex | Optimized, 1 worker | 23.086 ms | 2,906.92 Mbit/s |
| Mixed regex | Optimized, 8 partitions | 10.579 ms | 6,343.52 Mbit/s |

The optimized single-worker engine is roughly 2,200 times faster than the NFA
control on these fixtures. Eight partitions reduce median scan time by about
49% for diverse literals and 54% for mixed regex. Exact event parity makes
those differences admissible, but the local Docker host and short campaign do
not support cross-engine or universal worker-count claims.

The NFA control also changes future campaign design. One 8 MiB control scan
takes more than 52 seconds, so repeating it across every B1 point would consume
large amounts of machine time without refining the central conclusion. B1 may
retain sparse NFA controls while concentrating repetitions and thread sweeps on
the viable optimized modes, provided that choice is declared before results
are interpreted.

## CI boundary

Wuthering Heights remains useful in GitHub Actions as a realistic build-breaking
tripwire. The active C2 lane uses 5,000 patterns and an 8 MiB deterministic
corpus, verifies exact event evidence, retries a possible failure in reverse
order, and fails only beyond both 100% slowdown and 50 ms. That broad policy is
appropriate for catching severe regressions on noisy hosted runners.

It is not an authoritative benchmark. NFA controls, cross-engine comparisons,
complete thread sweeps, cache-sensitive scaling, and critical profile analysis
remain outside CI as retained stable-host evidence.

## Verification

- Rust runner unit tests: 24 passed before source pinning.
- Rust workspace Clippy: all targets and features, warnings denied.
- Harness orchestration tests: 6 passed.
- Harness GitHub Actions `python-tests`: passed on PR #1.
- Pinned container build: passed with Rust `1.97.0` and the locked dependency
  graph.
- Six container receipts: passed strict validation and scenario-local event
  parity.
- Generated SVG: passed XML validation and visual inspection.

## Remaining work

B1 is active. It must add fair cross-engine semantics and receipts, the full
declared pattern/corpus matrix, density controls, worker sweeps, stable Linux
host identity, and critical profiling interpretation. I9 establishes the lane;
it does not pre-approve B1's eventual comparative claims.
