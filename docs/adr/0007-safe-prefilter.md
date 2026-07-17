# ADR-0007: Conservative start acceleration and necessary-literal prefilter

- **Status:** Proposed pending the frozen I7 correctness and performance gate
- **Date:** 2026-07-18
- **Decision owners:** rustmatch maintainers

## Context

I6 removes repeated NFA interpretation for materialized deterministic states,
but the engine still considers every UTF-16 position as a possible match start.
For large corpora and selective pattern sets, most starts are impossible. A
prefilter can avoid that work, but a false negative would silently remove a
real match. Prefilter safety therefore matters more than prefilter speed.

## Decision

I7 may add two independent, conservative layers before the semantic engine:

1. An exact one- and two-ASCII-symbol start table derived from the compiled
   NFA. Non-ASCII or assertion-dependent cases are admitted conservatively.
2. A native multi-literal automaton used only when every pattern has a proven
   necessary literal at a fixed offset from its start. The first production
   extractor is deliberately limited to fixed literal prefixes.

The prefilter produces candidate start positions. The I6 semantic engine still
verifies every candidate and remains the sole source of match events. A hidden
benchmark control disables both layers so every admitted scenario can compare
the complete normalized event multiset with and without I7.

The necessary-literal proof is structural over normalized HIR. It may retain a
literal prefix only when every accepted path consumes those exact UTF-16 units
at the same offsets. Common prefixes of alternatives are safe; optional
prefixes, predicates, case-folded predicates, and assertions are not guessed.
Unsupported proof shapes return no hint and cause full literal-prefilter
bypass for the matcher.

The literal automaton is activated only for sufficiently large, fully
filterable workloads. A bounded prefix sample estimates candidate density. If
the sample is too dense, scanning falls back to the start table or ordinary I6
path rather than paying for a second full pass. Activation thresholds are
performance policy, never correctness assumptions.

## Consequences

- Disabling or bypassing I7 cannot change matching semantics.
- Mixed pattern sets remain correct even when one pattern has no safe hint;
  they simply do not use the full literal prefilter.
- Assertions retain the established context-sensitive engine path.
- A native sparse automaton avoids a new runtime dependency and makes memory,
  failure links, and candidate evidence inspectable.
- The first extractor intentionally misses profitable but harder cases. Future
  widening requires a new proof rule, adversarial tests, and measured benefit.
- Candidate collection uses bounded, input-proportional storage and reports its
  footprint in benchmark diagnostics.

## Admission evidence

Production activation requires the frozen protocol in
[`docs/experiments/i7-safe-prefilter.md`](../experiments/i7-safe-prefilter.md):
exact on/off event equality, extractor and automaton adversaries, explicit
bypass evidence, resource accounting, and a positive native Rust result beyond
noise in every declared activation region.
