# Rustmatch 0.1.0 compatibility matrix

This is the support contract proposed for the first public release. A target is
not release-supported until the release-readiness workflow passes for the exact
candidate commit.

## Toolchains and platforms

| Surface | 0.1.0 contract |
|---|---|
| Rust edition | 2024 |
| Minimum Rust version | 1.85.0 |
| Continuously tested hosts | Linux x86-64 and ARM64, macOS ARM64 and Intel, Windows x86-64 |
| Independent cross-compile target | AArch64 Linux |
| SIMD acceleration | Runtime-detected AVX2 on x86-64 |
| Portable behavior | Exact non-SIMD path when AVX2 is absent |
| Runtime environment | Rust standard library; no `no_std` support |

The non-x86 path is semantically supported, not promised to match AVX2
throughput. Architectures outside the tested matrix are best effort for 0.1.x.

### Preview portability evidence

| Target | Evidence level | Release effect |
|---|---|---|
| Windows ARM64 (`aarch64-pc-windows-msvc`) | Native public-preview run passed ([31333621251](https://github.com/la3lma/rustmatch/actions/runs/31333621251)) | Best effort; non-blocking and not part of the 0.1.0 support contract |

## Semantic reference

| Surface | 0.1.0 contract |
|---|---|
| Behavioral reference | Maven Central `no.rmz:rmatch:2.0.0-RC1` |
| Reference JAR SHA-256 | `05542b4778d004bd40037539567a0fff31b65b3bf234c61ef1493046c3e71a8a` |
| Input coordinates | Owned UTF-16 code units |
| Match coordinates | Zero-based, half-open UTF-16 spans |
| Event selection | Longest event for each pattern and eligible start |
| Overlap | Preserved across distinct starts and patterns |
| Callback order | Unspecified |

The retained Java fixtures cover literals, predicates, composition,
repetition, UTF-16 and flags, anchors, and boundaries. Compatibility means the
same event multiset for supported syntax, not source or implementation parity.

## Supported syntax

- UTF-16 literals, including isolated surrogate code units through raw input.
- Dot and positive or negated character classes with ranges.
- ASCII `\d`, `\w`, and `\s`, plus their complements.
- Alternation, plain groups, and non-capturing groups.
- Greedy `?`, `*`, `+`, `{m}`, `{m,n}`, and `{m,}` repetition, with bounds up
  to 1,000.
- Prefix `(?i)` and `(?s)` flags and typed case-insensitive registration.
- `^`, `$`, `\b`, and `\B` assertions.
- Group nesting up to 256 levels; deeper patterns return a typed error.

## Known limitations

- Captures, look-around, backreferences, lazy quantifiers, possessive
  quantifiers, and scoped flags are unsupported.
- Pure zero-width patterns are rejected.
- Case folding follows Java single-UTF-16-unit behavior rather than full
  Unicode multi-code-point or locale-sensitive folding.
- UTF-8 convenience input is not yet a Java-compatible coordinate API.
- Parallel matching serializes callback delivery after workers complete and
  may retain match events temporarily.
- Candidate filters and SIMD kernels are conservative accelerators; the exact
  semantic engine remains authoritative.
- The `benchmark-internals` feature is for repository tooling and is outside
  the supported application API.
- The non-default `unstable-assertion-prefix-v1` feature is exact but outside
  the stable default API. It supports only its documented eligible assertion
  cohort, separate matcher type, typed refusal/fallback contract, and explicit
  scan policies. Incompatible API changes wait for at least `0.2.0`.

Performance comparisons and their semantic qualifications are maintained in
the [current cross-engine snapshot](../experiments/h43-cross-engine-snapshot.md).
