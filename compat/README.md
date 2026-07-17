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
  through the same executable spine.
- Files under `expected/` are generated evidence. Change them only together
  with a reviewed fixture, oracle, or pinned-reference change.

Run the compatibility gate from the repository root:

```sh
cargo xtask oracle
```

The oracle command regenerates both fixture families twice, proves deterministic
output, and compares both results and manifests byte-for-byte with the committed
evidence. The Rust adapters can also be run separately:

```sh
cargo run -p rustmatch-compat -- verify-literals
cargo run -p rustmatch-compat -- verify-predicates
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
