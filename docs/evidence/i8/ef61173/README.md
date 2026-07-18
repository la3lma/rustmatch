# I8 evidence: explicit parallel pattern partitions

This directory retains the admission evidence for implementation
`ef6117368cd70b23fcfede1b26525c5f5366f53a` against the protocol frozen in
[`docs/experiments/i8-parallel-partitions.md`](../../../experiments/i8-parallel-partitions.md).
The default-path baseline is
`f7a5e95588b96653f8cf3cb2445c6c17f571c4fe`, the pre-I8 merge revision.

## Environment and evidence identity

All admission timings were produced by a verified Mach-O arm64 release binary
on:

- `rainstick.local`
- Apple M2 Max
- 12 physical/logical CPUs
- macOS 26.5
- Rust 1.97.0 standard library for `aarch64-apple-darwin`
- three warmups and seven measured scans
- a 256 MiB cache-scrub working set before every admission scan

The installed Rust compiler is an x86_64 executable, so the benchmark was
explicitly built with `--target aarch64-apple-darwin`; `file` verified the
result before it was timed. The runner was not idle: the retained system-state
files record WebKit, Mail, Spotlight, and a virtual machine competing for CPU.
Reverse-order pairs and repeated winner runs are therefore the admission
evidence. The complete sweep is useful for shape and resource analysis, not
for publishing a machine-maximum claim.

## Correctness and lifecycle result

The complete workspace suite passes exact normalized event-multiset equality
for requested worker counts 1, 2, 3, 4, 8, 12, 24, and 128. It covers literals,
duplicate text under distinct IDs, predicates, alternation, groups, repetition,
flags, UTF-16 supplementary and isolated-surrogate units, anchors, boundaries,
empty input, end-of-input matches, generated pattern families, and simultaneous
scans of one matcher.

Focused tests also prove the non-happy paths: zero workers are rejected, a
spawn failure returns a typed error before callback delivery, an internal
worker panic is joined and resumed, and a callback panic leaves the matcher
reusable after all scoped workers have stopped. Repeated build/scan/drop cycles
complete without retained workers. No unsafe Rust or runtime dependency was
introduced.

Every one of the 27 Wuthering Heights sweep receipts reports the same event
count and digest at a given pattern count. The 10,000-pattern line always emits
1,912,854 events with digest
`multiset64:e2645ce33f92320c:682cc0a867ada85d:c4a89edd4c72902e`.

## Positive admission result

The frozen positive gate is 10,000 patterns over the deterministic 8 MiB
Wuthering Heights expansion. Both a 20% relative improvement and 20 ms absolute
improvement were required. Eight workers won the final sweep and were then run in
winner/baseline/winner order:

| Mode | Median scan |
|---|---:|
| one worker | 249.801 ms |
| eight workers, first run | 99.680 ms |
| eight workers, repeated | 99.635 ms |

The first winner run is 150.121 ms and 60.09% faster than one worker. The two
winner medians differ by 0.04%, below the frozen 5% repeatability limit. Exact
event evidence is identical. I8 therefore clears its positive Rust gate without
using Java results as admission evidence.

## Complete thread sweep

The final 10,000-pattern sweep shows useful but strongly sublinear scaling:

| Workers | Median | Speedup | Parallel efficiency |
|---:|---:|---:|---:|
| 1 | 249.021 ms | 1.00x | 100.0% |
| 2 | 162.337 ms | 1.53x | 76.7% |
| 3 | 130.570 ms | 1.91x | 63.6% |
| 4 | 113.870 ms | 2.19x | 54.7% |
| 6 | 105.610 ms | 2.36x | 39.3% |
| 8 | 104.488 ms | 2.38x | 29.8% |
| 12 | 105.936 ms | 2.35x | 19.6% |
| 18 | 123.161 ms | 2.02x | 11.2% |
| 24 | 153.936 ms | 1.62x | 6.7% |

The curve plateaus from six through twelve workers, with its lowest median at
eight, and declines after twelve. More partitions duplicate corpus and
prefilter traversal, increase scheduling and join work, and contend for shared
caches and memory bandwidth. The current
implementation also starts scoped workers for every scan and serializes event
delivery after the scans complete. There is no evidence for making eight the
default on other machines or workloads, so the public default remains one.

The 1,000- and 5,000-pattern curves contain deliberate activation-threshold
discontinuities. At 1,000 patterns, four partitions contain only 250 patterns;
at 5,000 patterns, 24 partitions contain about 208. Both fall below I7's
256-pattern full-prefilter threshold and switch to the smaller start-table
path. Their apparent improvement is partly a different algorithmic path, not
clean evidence that those worker counts scale better. The 10,000-pattern
positive line keeps every tested partition above that threshold and is the
sounder parallelism comparison.

## Short-input guard

Thread startup dominates when both the input and pattern set are small. On a
1 KiB Wuthering prefix, one worker took 0.006 ms, 0.020 ms, and 0.084 ms for
10, 100, and 500 patterns. Eight workers took 0.134 ms, 0.130 ms, and 0.152 ms:
roughly 23.6x, 6.5x, and 1.8x slower. At 1,000 patterns the two paths were
within 0.013 ms and one worker still won; at 10,000 patterns the partitioned
path won. The crossover
depends on both dimensions and the machine, which is why callers must choose
parallelism from measurements rather than a built-in core-count heuristic.

## Default and build guard

The final one-worker candidate was measured beside the frozen pre-I8 binary on
the same runner. Times are medians:

| Patterns | Pre-I8 | I8 one worker | Change |
|---:|---:|---:|---:|
| 1,000 | 35.955 ms | 37.025 ms | 2.97% / 1.070 ms slower |
| 5,000 | 104.775 ms | 107.462 ms | 2.56% / 2.686 ms slower |
| 10,000 | 247.715 ms | 247.801 ms | 0.03% / 0.086 ms slower |

Every first-pair loss remains below the 3% relative half of the conjunctive
failure threshold. Because the 1,000-pattern result was only 0.03 percentage
points below that limit, the series was repeated in candidate/base order. That
confirmation was neutral at 1,000, faster at 5,000, and produced one 4.94%
10,000-pattern outlier. Four additional order-alternating 10,000-pattern pairs
measured candidate changes of -0.07%, +1.27%, +2.40%, and -0.47%. The outlier
is retained, but it did not repeat. The evidence supports no material default
regression claim more precise than the frozen 3% guard.

Median build time was lower at 5,000 and 10,000 patterns. The 1,000-pattern
build difference was 0.38% and 0.006 ms, so the 3% plus 1 ms build guard also
passes. The direct one-partition path spawns no worker and buffers no events.

## Resource accounting

At 10,000 patterns, the configured cache remains one total 8,192-state budget.
Its table storage is 4,194,304 bytes for every measured worker count; I8 does
not multiply it by the partition count. Retained database storage changes only
from 3,740,660 bytes at one worker to 3,743,420 bytes at 24 workers.

Other storage does scale with partitions:

| Workers | Prefilter retained | Candidate bitmaps | Event buffer | Peak footprint |
|---:|---:|---:|---:|---:|
| 1 | 0.59 MiB | 1.00 MiB | 0 | 62.6 MiB |
| 6 | 2.79 MiB | 6.00 MiB | 43.78 MiB | 149.1 MiB |
| 8 | 3.67 MiB | 8.00 MiB | 43.78 MiB | 162.1 MiB |
| 12 | 5.43 MiB | 12.00 MiB | 43.78 MiB | 157.1 MiB |
| 24 | 10.70 MiB | 24.00 MiB | 43.78 MiB | 155.2 MiB |

Peak-process measurements vary with allocator reuse and concurrent scheduling,
which explains the nearly flat multi-worker peak observations. The structural
counts are more reliable: each active full prefilter adds one input-sized
candidate bitmap, and every multi-worker scan buffers all 1,912,854 events.
The 43.78 MiB event buffer is the largest explicit I8 cost and is paid even
when more workers no longer improve throughput.

## Profile interpretation

Companion native `sample` profiles used 10,000 patterns and a 50 MiB Wuthering
expansion so the steady scan dominated startup. One worker took 1.605 s; eight
workers took 1.057 s while sampling was active. These are profile companions,
not the 8 MiB admission gate.

The one-worker aggregate is led by semantic transition construction, literal
prefilter traversal, and deterministic-cache transition/lookup. With eight
workers, aggregate samples are led by duplicated literal-prefilter traversal,
then partition scanning and transition construction. Join waits and serialized
`scan_parallel_and_deliver` work are visible as separate costs. Aggregate
multi-thread sample counts cannot be read as wall-time percentages, but they
do identify the scaling limits: every partition scans the corpus, transition
work remains substantial, and event collection/delivery has not disappeared.

Thread creation is intentionally difficult to see in the long 50 MiB profile
but matters on short scans. A persistent executor, shared prefilter candidate
stream, or concurrent/fallible event sink could change that balance. Each is a
separate design with semantic and resource consequences and is not smuggled
into I8.

## Rejected prototype

An intermediate attempt stored the single partition inline to avoid the small
boxed-slice dispatch. Native measurements and generated assembly showed that
it enlarged the hot dispatcher and made the default path worse. It was reverted.
The accepted implementation instead keeps a small inlineable one-partition
dispatcher, moves the heavier parallel path out of line, passes the callback by
value, and keeps the worker-start error compact. This restored the pre-I8
single-worker shape.

## Critical conclusion

I8 is admitted because exact semantics, lifecycle behavior, the default and
build guards, total-cache accounting, and the predeclared positive throughput
gate all pass. Explicit pattern partitions turn a 10,000-pattern, 8 MiB scan
from roughly 250 ms into roughly 100 ms on this 12-core machine.

The result is not linear scaling, an automatic worker heuristic, or a claim
that more workers are always better. The current design trades materially more
memory and duplicated corpus work for throughput and saturates early. Those
costs, along with the threshold discontinuities, are part of the result. I9
must carry exact worker counts and correctness evidence into the ordinary
cross-engine harness rather than collapsing this nuanced curve into one
unqualified "parallel" number.
