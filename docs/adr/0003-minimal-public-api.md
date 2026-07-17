# ADR-0003: Minimal public builder, input, and callback API

- **Status:** Accepted
- **Date:** 2026-07-17
- **Decision owners:** rustmatch maintainers

## Context

The first executable slice must be usable through the same conceptual boundary
as later releases without exposing implementation machinery. It must also stay
small enough that unsupported extension points do not become accidental 2.0
commitments.

The initial engine owns finite UTF-16 input, builds an immutable matcher, runs
single-threaded, and invokes an infallible callback. Borrowed/custom inputs,
fallible or concurrent sinks, iterators, async scanning, tuning, flags, and
runtime pattern mutation do not yet have enough implementation evidence for a
public contract.

## Decision

The crate root initially exposes exactly these types:

- `MatcherBuilder`
- `Matcher`
- `PatternId`
- `Utf16Text`
- `Match`
- `Utf16Span`
- one non-exhaustive `Error` enum

The lifecycle is:

```text
create builder -> add patterns -> build immutable matcher -> scan repeatedly
```

`PatternId` is a caller-provided `u32` domain value. IDs must be unique within
one builder; equal pattern text may be registered under different IDs.
`Utf16Text` owns its encoded `Vec<u16>`. `Utf16Span` contains zero-based,
half-open `u64` UTF-16 positions. `Match` contains only the pattern ID and span.

`Matcher::scan` accepts an infallible `FnMut(Match)` callback. Delivery order is
unspecified. This API permits a later parallel implementation to collect or
partition results before serialized callback delivery; a separately proven
concurrent or fallible sink can be added without changing this method.

Parser, HIR, NFA, state IDs, databases, scratch storage, engine, and callback
adaptation remain private modules. The first implementation lives in one
public crate; an internal crate split requires an actual build or ownership
benefit rather than architectural symmetry.

## Error boundary

Pattern errors occur in `MatcherBuilder::add` and leave the builder usable.
Building an empty matcher fails. The first-slice scan validates the complete
input before invoking the callback, so unsupported input cannot produce a
partial result. Expected user errors do not panic.

## Consequences

- The quick-start API is small, explicit, and compatible with the final UTF-16
  coordinate model.
- Runtime pattern mutation requires a new builder and matcher.
- The initial callback cannot return an application error. That capability is
  deferred rather than represented by an unused generic parameter.
- The initial input always materializes finite text. File-backed and bounded
  lookback inputs remain possible future adapters, not implied behavior.
- Public API growth must be justified by a working use case and evidence.

## Evidence

- The crate-level doctest compiles the complete build-scan-consume lifecycle.
- `one_literal_runs_through_the_complete_spine` uses every private execution
  layer through the public API.
- The NFA invariant test proves dense state IDs, valid edge ranges, reachable
  terminals, and the shared synthetic start for that walking spine.

