# B2-H-0013 assertion prefilter recovery

**Status:** reviewed and authorized for one candidate implementation

**Authorization date:** 2026-07-27

**Accepted source baseline:** `c1d2bf0caab59204829e6517576e8fc6d0763ffb`

**Decision policy:** G9-v2, unchanged

## Question

Can H12's exact conservative assertion candidates retain material target gains
while eliminating the inactive Wuthering regression and reducing preparation
cost enough to pass the existing keeper gates?

H12 is retained as an investigated, machine-rejected artifact. This experiment
does not reinterpret or amend that result.

## Causal model

H12 established three distinct facts:

1. Conservative literal candidates preserve exact assertion semantics.
2. Sparse assertion targets improved roughly 185 to 385 times, and the mixed
   adversary improved roughly 720 times.
3. The exact artifact was not admissible because Wuthering regressed 3.038%
   and assertion preparation regressed 11.16% to 22.32%.

Wuthering contains no assertions and cannot execute H12's new candidate path.
H12 nevertheless emitted five sink-specialized assertion-plan functions and
changed ordinary scanner size and placement. The leading hypothesis is
separable monomorphization or hot-layout interference.

The preparation loss has a separate source. Necessary-prefix analysis returns
recursively allocated vectors, and `LiteralPrefilter::compile` initializes all
three large bit tables even when one prefix-width family is present. The
1,000-pattern H12 fixtures predominantly require only the five-unit table.

## Candidate design

The candidate must be a clean descendant of the accepted baseline and may
change only:

- `rustmatch/src/engine.rs`;
- `rustmatch/src/prefilter.rs`; and
- one new assertion-engine source module if separation requires it.

### Stable assertion dispatch

- Represent assertion starts with one concrete enum covering all starts and a
  borrowed candidate iterator.
- Accept the event sink through one erased `&mut dyn FnMut(Match)` boundary.
- Compile one non-generic assertion-plan function rather than one copy per sink
  and iterator type.
- Keep the boundary out of line. A cold or separately compiled boundary is
  permitted when focused target evidence confirms that it preserves material
  benefit.
- Do not alter the ordinary no-assertion transition algorithms, H11 shared
  planning, callback semantics, or public API.

### Lower-cost preparation

- Replace recursively allocated prefix vectors with a bounded stack value or
  equivalent allocation-free accumulator.
- Stop prefix proof once the filter's maximum useful width is known.
- Allocate only literal bit tables used by at least one proven prefix width.
- Omit private assertion-filter metadata that is needed only by shared routing.
- Do not defer matcher preparation into a warm-up or scan phase.

The semantic NFA remains authoritative. Candidate generation may create false
positives but must never omit a possible event.

## Required static and functional evidence

Before performance timing:

- complete `cargo xtask ci` passes;
- exact all-start and candidate-start assertion scans agree across boundaries,
  line edges, alternation, repetition, raw UTF-16, non-ASCII input, and
  overlapping candidates;
- small, dense, below-threshold, and unfilterable assertion sets retain their
  frozen fallback paths;
- assertion filters remain unavailable to H11 shared planning;
- the ordinary Wuthering scanner has no call into assertion code;
- candidate-only assertion monomorphizations are absent; and
- ordinary scanner symbol size, mnemonic sequence, alignment, callers, and
  focused counters are compared with the accepted baseline.

Static mismatch is diagnostic rather than an automatic veto, but an unexplained
ordinary-path change blocks the expensive matrix.

## Focused pre-gate

Use exact baseline and candidate Linux artifacts on exclusive Agogo:

- H12 boundary target, 1,000 patterns, 1 MiB, one worker;
- H12 mixed assertion adversary, 256 patterns, 1 MiB, one worker;
- Wuthering, 10,000 patterns, 8 MiB, 64 workers;
- H11 shared guard, 10,000 patterns, 50 MiB, 16 workers; and
- repeated matcher preparation for the three assertion target shapes.

Each focused timing comparison uses at least six balanced AB/BA pairs.
Proceed to the full matrix only when:

- exact events and digests pass;
- both assertion scans retain at least a 5% gain;
- Wuthering and H11 shared primary effects remain inside the two-percent noise
  boundary without a negative order stratum; and
- assertion preparation has no demonstrated loss above two percent.

Rejected focused runs and diagnostics remain retained and never enter the
admission result.

## Frozen admission matrix

The full matrix reuses H12's exact eleven cell identities, fixture bytes,
pattern bytes, expected event counts, normalized event digests, worker counts,
two warm-ups, five retained scans, and six balanced
`AB, BA, AB, BA, AB, BA` pairs:

- three sparse assertion targets;
- the 500-pattern assertion neighbor;
- the mixed assertion adversary;
- below-threshold, dense, and unfilterable assertion guards;
- the ordinary literal guard;
- the H11 shared guard; and
- the Wuthering guard.

The primary target floor remains 5%. Correctness is mandatory. Demonstrated
guard regressions block admission, repeatable losses above 2% require
investigation, and losses above 3% trigger the automatic veto. Secondary
preparation, process-wall, and RSS evidence retains the same ruling.

## Decision

Merge only the exact measured candidate if G9-v2 returns
`admission-eligible`, the human review accepts it, repository CI passes again
at the destination, and provenance proves that the merged source is the
measured source.

Every other outcome is retained in the lab notebook and optimization ledger.
Investigated or rejected H13 code must not enter production or the merged
efficiency graph.
