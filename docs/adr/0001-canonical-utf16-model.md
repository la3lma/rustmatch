# ADR-0001: Canonical UTF-16 symbol and offset model

- **Status:** Accepted
- **Date:** 2026-07-17
- **Decision owners:** rustmatch maintainers

## Context

rustmatch is a Rust-native implementation of the observable rmatch contract.
Java rmatch reads Java `char` values and reports positions between those values.
Its symbols and offsets are consequently UTF-16 code units, not Unicode scalar
values, grapheme clusters, UTF-8 bytes, or display columns.

Rust strings are UTF-8 and cannot contain isolated UTF-16 surrogate code units.
Using Rust `char` or byte positions as the engine's canonical coordinates would
therefore diverge from Java for supplementary characters and make exhaustive
compatibility tests impossible.

## Decision

The compatibility engine uses these canonical concepts:

- `Symbol` is one unsigned 16-bit UTF-16 code unit.
- `Position` is an unsigned 64-bit count of code units from the beginning of
  the input.
- A match span is half-open: `[start_utf16, end_utf16)`.
- Patterns are parsed as code-unit sequences. The ordinary UTF-8 API encodes a
  Rust string with `encode_utf16`; compatibility fixtures may supply raw code
  units, including isolated surrogates.
- Inputs expose a stable, finite sequence of code units for the duration of a
  scan. The first public convenience input owns its `Vec<u16>`.
- Public coordinate types say `Utf16` in their names. An unqualified integer
  offset is not part of the public API.
- Conversion to UTF-8 byte ranges is a separate checked operation. It is not
  implicit and may fail at a boundary inside a surrogate pair or for raw
  unpaired surrogates.

Fixture files encode patterns and input as JSON arrays of integers from 0
through 65,535. This is deliberately more explicit than JSON strings: it makes
the normative symbols independent of JSON decoder behavior around surrogates.

## Consequences

- ASCII offsets equal both UTF-8 byte offsets and UTF-16 positions, so the
  first executable slice remains easy to inspect.
- Supplementary Unicode characters occupy two engine symbols, as they do in
  Java rmatch.
- A future UTF-8-native facade needs an explicit coordinate map and distinct
  span types.
- Truly unbounded input is not representable by the first owned input type.
  A future indexed or bounded-lookback input may be added without changing the
  canonical symbol model.
- Internal storage may use narrower indices when proven safe, but every public
  position and compatibility result is losslessly representable as `u64`.

## Evidence

- The pinned Java reference is `no.rmz:rmatch:2.0.0-RC1`.
- `no.rmz.rmatch.Buffer.charAt(long)` returns `char`.
- `no.rmz.rmatch.Action.performMatch(Buffer, long, long)` reports a half-open
  end position, as documented by the release and exercised by its semantic
  tests.
- Version 1 semantic fixtures use raw `input_utf16` and pattern `utf16` arrays.

