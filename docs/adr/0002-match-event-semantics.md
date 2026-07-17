# ADR-0002: Normative match-event selection and ordering exclusions

- **Status:** Accepted
- **Date:** 2026-07-17
- **Decision owners:** rustmatch maintainers

## Context

A many-pattern matcher can report several legitimate matches at one input
position. Greedy path selection, overlapping starts, duplicate pattern text,
parallel delivery, and callback order must not be left to implementation
accident if Rust and Java results are to remain comparable.

The published Java reference has a clear semantic rule: for every registered
pattern and every start position, report the longest accepted match from that
start. Matches at other starts remain visible even when they overlap or are
contained by a longer match.

## Decision

The normative result of a scan is a multiset of events:

```text
(pattern_id, start_utf16, end_utf16)
```

For each registered pattern identity and each input start position:

1. Consider every legal consuming path beginning at that position.
2. If at least one path accepts after consuming input, select the greatest end
   position.
3. Emit exactly one event for that pattern identity and start position.
4. Preserve events at other starts, including overlapping and nested events.
5. Preserve separate caller identities when equal pattern text is registered
   under different identifiers.

Spans are half-open. Pure zero-width patterns do not emit events; assertions
may constrain a consuming match. Invalid or unsupported registration fails
before scanning and before any event is delivered for that fixture case.

Callback delivery order is explicitly unspecified. Correctness comparisons
sort events by unsigned pattern ID, start position, then end position. Sorting
is an oracle and test operation, not a promise made by the runtime API.

## Consequences

- `a` and `aa` over `aaa` report five events: three for `a` and two for `aa`.
- Two registrations of `a` with different IDs each report their own events.
- Implementations may partition patterns and deliver callbacks concurrently
  without changing semantics.
- A callback sink that cares about order must collect and order events itself.
- Optimized engines are compared with the unoptimized semantic path as event
  multisets, never by incidental callback sequence.

## Evidence

- The pinned `2.0.0-RC1` `BasicMatchingSemanticsTest` states and tests the
  longest-per-pattern-and-start rule.
- The published `Matcher` API documents that callback order is unspecified.
- Version 1 oracle output is deterministically sorted before serialization and
  is stable across repeated runs.

