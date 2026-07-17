# Compatibility fixtures and Java oracle

This directory contains the executable compatibility boundary between
rustmatch and the pinned Java reference. It is not a Java implementation of
rustmatch.

## Contract

- Each non-empty line in `fixtures/*.jsonl` is one fixture conforming to the
  versioned schema in `schema/`.
- Pattern and input symbols are raw UTF-16 code units. JSON strings are not the
  normative representation.
- The Java oracle resolves exactly `no.rmz:rmatch:2.0.0-RC1`, verifies the JAR
  SHA-256, uses a single matcher, and sorts events before writing JSONL.
- `rust_expectation` distinguishes behavior in the currently implemented Rust
  syntax slice from behavior already supported by the Java reference.
- `ascii-literal-v1` preserves the bootstrap literal contract, while
  `ascii-predicate-v1` grows independently as ASCII predicate syntax is carried
  through the same executable spine. `ascii-composition-v1` adds alternation,
  grouping, empty-branch behavior, longest-branch selection, and malformed
  group cases. `ascii-repetition-v1` adds greedy unary and counted repetition,
  quantifier binding, overlapping starts, longest-match ambiguity, the 1,000
  expansion limit, and malformed counts.
- `utf16-flags-v1` covers non-ASCII literals and ranges, supplementary and raw
  surrogate inputs, prefix flags, Java single-`char` folding, titlecase edge
  behavior, and malformed flag placement.
- `assertions-v1` covers line anchors and ASCII word boundaries at input and
  line edges, inside groups and alternatives, alongside flags, and around
  non-ASCII and raw-surrogate code units. It also pins the Java asymmetry that
  `$` needs a consumed character while `^` can follow a consumed newline.
- `expected/java-21-case-fold-v1.bin` records all 65,536 Java 21 lower/upper
  `char` pairs in a fixed binary format. The oracle regenerates it twice and
  compares its manifest and bytes before Rust differential evidence runs.
- A pure zero-width alternation records one intentional difference explicitly:
  Java accepts it and emits no events, while the Rust product contract rejects
  patterns that can only produce a zero-width match.
- Files under `expected/` are generated evidence. Change them only together
  with a reviewed fixture, oracle, or pinned-reference change.

Run the compatibility gate from the repository root:

```sh
cargo xtask oracle
```

The oracle command regenerates all fixture families twice, proves deterministic
output, and compares both results and manifests byte-for-byte with the committed
evidence. The Rust adapters can also be run separately:

```sh
cargo run -p rustmatch-compat -- verify-literals
cargo run -p rustmatch-compat -- verify-predicates
cargo run -p rustmatch-compat -- verify-composition
cargo run -p rustmatch-compat -- verify-repetition
cargo run -p rustmatch-compat -- verify-utf16-flags
cargo run -p rustmatch-compat -- verify-assertions
```

The command requires Java 21 and Maven. On macOS it selects an installed Java
21 automatically when the default Java is another release. The full repository
gate, `cargo xtask ci`, includes the same check.

## Dependency admission

The oracle has two runtime dependencies:

- Java rmatch `2.0.0-RC1`, because that exact published artifact is the object
  being observed.
- Jackson Core, because a streaming JSON parser and generator avoid a fragile
  handwritten JSON implementation without introducing databinding or an
  object-mapping model.

Neither dependency enters the Rust library or its runtime dependency graph.

The unpublished `rustmatch-compat` adapter uses Serde and Serde JSON to consume
the same JSONL without maintaining a second handwritten parser. It runs the
fixtures through the public Rust API and compares every supported event with
the pinned Java result. These dependencies likewise remain outside the
`rustmatch` library. Run that evidence directly with:

```sh
cargo xtask evidence
```

That command runs the pinned Java oracle, the Rust differential adapter, and
the correctness-gated benchmark smoke before printing the E0 summary for every
declared use case. A summary status of `pass` means the listed evidence ran and
the incomplete use cases were reported honestly; it does not promote `partial`
or `not-started` use cases to complete.
