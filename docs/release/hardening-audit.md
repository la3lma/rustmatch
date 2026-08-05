# 0.1.0 hardening audit

## Runtime dependencies and licenses

The public package has one normal dependency:

| Package | Source | Version | License | Purpose |
|---|---|---:|---|---|
| `rustmatch-simd` | Same repository | `0.1.x` | Apache-2.0 | Runtime-detected candidate discovery |

`rustmatch-simd` has no dependencies outside the Rust standard library.
Therefore the shipped dependency graph contains no third-party runtime crate,
native library, build script, or transitive license. Benchmark, compatibility,
fuzz, Java-oracle, and repository automation dependencies are not included in
either published package.

Both package archives carry an exact copy of the repository's Apache-2.0
license text. The package rehearsal compares those files byte-for-byte before
building either archive.

The exact package rehearsal verifies this claim from normalized package
manifests. Dependency changes require a renewed license, advisory, MSRV, and
package-content review.

## Unsafe-code audit

The public `rustmatch` crate forbids unsafe code. Unsafe code is isolated in
`rustmatch-simd`, where it is limited to the x86-64 AVX2 kernel and guarded by
runtime feature detection. Every unsafe block states its slice-bound,
target-feature, pointer-alignment, and gather-index argument. The safe wrapper
rejects unexpected table sizes, and the semantic engine verifies admitted
candidates independently.

Miri covers the safe semantic crate. Architecture intrinsics are covered by
scalar-oracle tests and ordinary native CI because Miri does not execute the
AVX2 kernel.

## Panic-policy audit

Expected caller failures return `Error`, including malformed or unsupported
syntax, duplicate or missing patterns, zero workers, excessive automaton size,
oversized coordinates, unavailable workers, and group nesting beyond 256.

Documented caller-visible panics are limited to a panic raised by the callback;
it is resumed on the caller after scoped workers stop. Allocation failure and
platform-level abort behavior follow the Rust standard library.

Internal `expect`, `unreachable`, and resumed-worker panic sites were grouped
and reviewed as invariants:

- checked numeric conversions bounded by parser, input, or state limits;
- touched-terminal entries that must contain an end position;
- cache roots and IDs bounded before insertion;
- parser dispatch reachable only for preclassified escapes;
- power-of-two prefilter table indices masked before conversion;
- one output slot for every successfully joined partition;
- compile-time Java case-table header and size invariants.

Property, fuzz, Miri, adversarial nesting, worker-panic, callback-panic, and
soak tests are the executable backstop for this audit. A reproducible
caller-controlled violation of an internal invariant is a release blocker.

## Resource behavior

- Counted repetition is capped at 1,000.
- Group nesting is capped at 256 and returns `PatternNestingTooDeep`.
- Dense state IDs return `PatternSetTooLarge` on representational overflow.
- The default deterministic cache budget is 8,192 states across partitions;
  exhaustion falls back to the exact NFA path.
- Input-sized candidate bitmaps and parallel event buffers are scan-local and
  are reported by hidden release diagnostics.
- The bounded release soak reuses matchers across worker/cache combinations and
  verifies stable results and cache accounting.

This is not a claim that arbitrary hostile inputs have constant memory use.
Package documentation states the input-sized and buffered-event costs.
