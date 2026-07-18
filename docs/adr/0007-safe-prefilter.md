# ADR-0007: Conservative start acceleration and necessary-literal prefilter

- **Status:** Accepted
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
2. A compact native prefix-membership filter used only when every pattern has
   a proven necessary literal at a fixed offset from its start. The first
   production extractor is deliberately limited to fixed literal prefixes.

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

The production filter represents exact three-unit ASCII prefixes in a direct
bit table. Four-unit prefixes and the first five units of longer prefixes use
separate one-hash bitsets. Hash collisions only create extra candidates; they
cannot suppress a match. A non-ASCII unit in the inspected prefix is admitted
conservatively. The compact prefix-to-pattern mapping remains available for
diagnostics, while the semantic engine alone creates match events.

The literal filter is activated only for sufficiently large, fully filterable
workloads. A bounded prefix sample estimates candidate density. If the sample
is too dense, scanning falls back to the start table or ordinary I6 path rather
than paying for a second full pass. Activation thresholds are performance
policy, never correctness assumptions.

## Consequences

- Disabling or bypassing I7 cannot change matching semantics.
- Mixed pattern sets remain correct even when one pattern has no safe hint;
  they simply do not use the full literal prefilter.
- Assertions retain the established context-sensitive engine path.
- The native filter avoids a new runtime dependency and makes retained memory,
  candidate density, and skipped starts inspectable.
- The first extractor intentionally misses profitable but harder cases. Future
  widening requires a new proof rule, adversarial tests, and measured benefit.
- Candidate collection uses bounded, input-proportional storage and reports its
  footprint in benchmark diagnostics.

## Rejected representation

The first prototype used a sparse Aho-Corasick-style trie with failure links.
It was semantically attractive and retained about 1.83 MiB for the 5,000-word
Wuthering fixture, but native profiling showed that candidate generation had
become a dominant cost. Linear lookup in small nodes and an anchored-trie
variant did not recover enough scan time. The compact prefix filter was chosen
because it preserved the same no-false-negative contract with a materially
smaller and faster hot path. This decision does not imply that trie-based
prefilters are generally poor; it records the result for this engine, fixture,
and implementation.

## Admission evidence

Production activation requires the frozen protocol in
[`docs/experiments/i7-safe-prefilter.md`](../experiments/i7-safe-prefilter.md):
exact on/off event equality, extractor and candidate-filter adversaries, explicit
bypass evidence, resource accounting, and a positive native Rust result beyond
noise in every declared activation region. Candidate
`37f69819d9231fb99d747d4ae48034204657c1f2` passed that protocol; the raw
receipts and critical interpretation are retained in the
[`I7 evidence package`](../evidence/i7/37f6981/README.md).
