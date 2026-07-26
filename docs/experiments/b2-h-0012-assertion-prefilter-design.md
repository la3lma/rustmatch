# B2-H-0012: assertion-bearing conservative prefilter

> **Measured outcome:** investigate; machine-rejected and not merged. The
> sparse targets improved approximately 185x to 385x, but Wuthering regressed
> 3.038% and crossed the unchanged automatic veto. See the
> [retained result](b2-h-0012-assertion-prefilter-result.md).

**Status:** frozen design; source implementation not yet measured

**Baseline:** `68e43fc` (`codex/optimization-scale`)

**Decision policy:** G9-v2

## Observation

Rustmatch disables every prefilter when a compiled pattern database contains an
assertion. The retained 1,000-pattern `\bwordNNNN\b` fixture therefore invokes
the contextual NFA from every input start and takes roughly 76 seconds per scan
over an 8 MiB ASCII corpus. In these patterns the leading word-boundary
assertion consumes no input and the following literal is necessary at the same
start position.

## Hypothesis

For an assertion-bearing partition where every pattern has a structurally
proven necessary literal at offset zero after supported leading zero-width
nodes, the existing literal bitmap can conservatively remove impossible starts.
Every retained start still runs through the unchanged contextual assertion
engine, so the bitmap may add false positives but must not create false
negatives.

The target must improve by at least 5%. Every output count and normalized event
digest must remain exact. A demonstrated regression blocks production
admission; a loss above 3% is an automatic veto, and a repeatable loss from 2%
through 3% requires investigation under G9-v2 without moving either threshold.

## Semantic proof boundary

The candidate may:

1. Step past leading `Assertion` and `Epsilon` HIR nodes because they consume
   zero UTF-16 units.
2. Reuse the existing necessary-prefix proof and false-positive-only literal
   bitmap after those nodes.
3. Pass only the bitmap's ascending start positions to the existing contextual
   assertion verifier.

The candidate may not:

1. Treat predicates, optional consuming expressions, nullable repetitions, or
   unsupported zero-width constructs as transparent.
2. Use the context-free start table for an assertion-bearing database.
3. Route an assertion-bearing partition through H11 shared candidates or the
   assertion-free shared semantic engine.
4. Cache contextual assertion closure under the context-free state-cache key.
5. Change parser semantics, match selection, pattern IDs, event order, public
   API behavior, prefilter thresholds, H11 thresholds, or benchmark code.

If one pattern in a partition lacks a proven literal of at least three UTF-16
units, the whole assertion partition retains the existing all-start path.
Inputs below the existing one-Mi-unit threshold also retain the existing path.

## Implementation boundary

Allowed source paths:

- `rustmatch/src/engine.rs`
- `rustmatch/src/prefilter.rs`

Allowed test paths:

- `rustmatch/src/engine.rs`
- `rustmatch/src/prefilter.rs`
- `rustmatch/src/tests.rs`

The `Prefilter` records whether its literal proof belongs to an
assertion-bearing database. Such a filter can plan private candidates but must
return no `SharedLiteralFilter`. `scan_with_assertions` accepts an explicit
ascending start iterator. The ordinary assertion-free dispatch remains
structurally unchanged.

## Correctness gates

Before timing:

- focused necessary-prefix tests cover leading boundary and line assertions,
  epsilon, trailing assertions, alternation, nullable prefixes, predicates,
  and unfilterable mixed sets;
- direct assertion-engine tests prove that an explicit candidate iterator
  produces the same events as all starts for word-boundary, non-boundary,
  line-start, line-end, empty, adjacent, and non-ASCII contexts;
- assertion-bearing prefilters cannot expose an H11 shared filter;
- the complete locked workspace suite passes with all features;
- formatting and Clippy with warnings denied pass.

Every timed baseline/candidate pair additionally requires identical pattern and
corpus hashes, exact event count, and exact normalized event digest.

## Frozen performance matrix

The admission matrix uses six balanced `AB, BA, AB, BA, AB, BA` pairs per cell.
Each process performs two warmups and retains five scans. Primary throughput is
Mbit/s for an already prepared matcher. Process wall time, preparation time,
peak RSS, prefilter path, bypass reason, admissions, candidate starts, starts
scanned/skipped, assertion bypasses, requested workers, and actual partitions
are retained diagnostics.

Targets:

- 1,000 sparse word-boundary patterns, 1 MiB, one worker.
- 1,000 sparse word-boundary patterns, 1 MiB, two workers.
- 1,000 sparse word-boundary patterns, 2 MiB, one worker.

Neighbors and semantic guards:

- 500 sparse word-boundary patterns, 1 MiB, one worker.
- 256 mixed boundary/line-assertion patterns with adversarial contexts, 1 MiB,
  one worker.
- 1,000 dense word-boundary patterns, 1 MiB, one worker; the density bypass
  must remain conservative.
- 1,000 assertion-bearing patterns with one unfilterable member, 1 MiB, one
  worker; the all-start fallback must remain active.
- 1,000 sparse word-boundary patterns below 1 MiB, one worker; the input-size
  fallback must remain active.

Broader regression guards:

- 1,000 sparse assertion-free literals, 1 MiB, one worker.
- 10,000 sparse assertion-free literals, 50 MiB, sixteen workers; H11 remains
  eligible and exact.
- 10,000 Wuthering literals, 8 MiB, sixty-four workers.

The 8 MiB assertion fixture remains a scale confirmation after the full frozen
matrix passes. It does not replace any target or relax the six-pair rule.

## Stop and admission rules

Stop and retain the attempt if any semantic test fails, an assertion-bearing
filter reaches H11, a target fails the 5% floor, a guard has a demonstrated
regression, the 3% automatic veto fires, or host contamination occurs.

Merge only the exact measured candidate revision after all gates pass. A passed
candidate updates the optimization attempt ledger, current cross-engine
comparison, roadmap, and HTML universe. A rejected or inconclusive candidate
updates the same retained lab history but is excluded from the merged
improvement graph.
