# Public API and versioning review

## Reviewed 0.1.0 surface

The default public API is intentionally small:

- `MatcherBuilder` registers patterns and configures workers/cache budget.
- `Matcher` is immutable, reusable, and safe to share between threads.
- `PatternId`, `PatternFlags`, `Utf16Text`, `Utf16Span`, and `Match` carry
  caller-visible values.
- `Error` is non-exhaustive and reports expected registration, build, resource,
  and scan failures.

Compiler, HIR, NFA, prefilter, partition, cache, and SIMD types remain private.
The `benchmark-internals` feature exposes hidden diagnostics only for retained
repository measurement tooling. Applications must not rely on that feature.

ADR-0009 authorizes the isolated `unstable-assertion-prefix-v1` API behind a
non-default feature and separate matcher type. H43-X1.9 admitted that exact
capability through an explicit owner exception. It is packageable but is not
part of the stable default API: its feature and module names deliberately
carry `unstable`, and the API may change or disappear in a future `0.y.0`
release. Published `0.1.x` patch releases nevertheless preserve its source
surface; incompatible removal waits for at least `0.2.0`. The default API above
remains the supported contract.

The complete H43 R3 assay remains rejected, not certified: ordinary default
safety passed, but one explicit-target preparation metric regressed 5.792673%
and crossed the unchanged veto. Two exact-fixture construction discriminators
subsequently measured only a repeatable 0.717%-0.976% premium. Owner exception
`4996bcc16a9cc936789372d8184f4ef4893fd336` accepted that residual risk for the
explicit product lane without relabeling R3, adding automatic routing, or
changing the default release surface.

The review confirms these deliberate contracts:

- Rejected registration does not modify or reserve an ID in the builder.
- Equal pattern text may be registered under different IDs; duplicate IDs are
  errors.
- Callback order is unspecified, while the event multiset is normative.
- Callback panics propagate after scoped workers have stopped; the matcher
  remains reusable.
- `Matcher`, `MatcherBuilder`, inputs, matches, and errors are `Send + Sync`.
- Public offsets are UTF-16 and cannot be silently reinterpreted as UTF-8 byte
  positions.

## SemVer policy

Rustmatch follows Cargo's leftmost-nonzero pre-1.0 convention:

- `0.1.z` patch releases preserve the supported public API and behavior.
- A breaking API or semantic-contract change requires at least `0.2.0` and a
  migration note.
- New supported API can appear in a patch release only when it cannot break
  existing source through trait, inference, exhaustiveness, or feature changes;
  otherwise it waits for the next minor line.
- Internal performance changes may ship in a patch release only after the
  ordinary correctness and performance admission gates pass.
- `benchmark-internals` is explicitly unstable and is not covered by the
  application API promise.
- `unstable-assertion-prefix-v1` and everything under its experimental module
  are explicitly unstable and carry no compatibility promise beyond the
  current `0.1.x` line.

After `1.0.0`, breaking supported-API changes require a major version.

## Deprecation policy

When practical, a supported API scheduled for removal is deprecated for at
least one incompatible-release cycle and its replacement is documented.
Security or correctness defects may require immediate restriction, but the
release notes must explain the impact and migration.

## MSRV policy

The `0.1.x` MSRV is Rust `1.85.0`. Every supported feature must compile on that
toolchain. Raising the MSRV is announced in the changelog and requires at least
the next `0.y.0` release while the project is pre-1.0.

## First-release migration

There is no migration from an earlier crates.io version. Git users should move
to `rustmatch = "0.1.0"` and use only the supported default public API above.
