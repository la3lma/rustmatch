# Contributing to rustmatch

This document is the durable policy for roadmap task **Q0: Documentation and
Rust hygiene**. The repository is still in design, so some commands below will
become executable with the Cargo workspace in increment I1. The standards apply
as soon as their corresponding files exist.

Before contributing, read the [Code of Conduct](CODE_OF_CONDUCT.md) and the
[governance model](GOVERNANCE.md). Search the
[issue tracker](https://github.com/la3lma/rustmatch/issues) and the
[roadmap](docs/roadmap.md), then use the issue form that matches the work.
Security vulnerabilities and conduct concerns must use the private channels in
[SECURITY.md](SECURITY.md) and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md), not a
public issue.

Contributions are accepted under the repository's Apache License 2.0. The
project does not currently require a separate contributor license agreement or
Developer Certificate of Origin sign-off.

## The goal: neat for comprehension

Rustmatch should be neat for practical reasons:

- a reader can find the public contract and the code that implements it;
- invariants, units, ownership, concurrency, and failure behavior are explicit;
- tests explain behavior without requiring a debugger;
- tools can run one documented command and receive structured results;
- a change can be reviewed without reverse-engineering unrelated machinery.

Neatness is not an excuse for abstraction, churn, or tool-driven busywork. A
smaller direct implementation is preferable when it is easier to prove,
measure, and maintain. Refactoring must improve comprehension, correctness,
evidence, or measured performance.

## Pull-request rules

Every pull request must:

1. Name the roadmap task or UC it advances, using the stable symbolic ID.
2. Keep one coherent purpose; unrelated cleanup belongs in another change.
3. Add or update the smallest useful test before or with behavior changes.
4. Update public documentation and evidence contracts in the same change.
5. Pass every required functional CI job.
6. Run the external regression protocol before merge when the hot path changes.
7. Supply positive Rust performance evidence when the change is an
   optimization.
8. Leave no unexplained generated files, lint suppressions, or TODO markers.

A green CI performance tripwire is supplementary. It does not replace the
external performance receipts required by the
[testing and regression policy](README.md#testing-and-regression-policy).

## Rust code hygiene

### Formatting and linting

- Use the repository's pinned stable Rust toolchain and canonical `rustfmt`.
- Run Clippy for the whole workspace, all targets, and relevant features with
  warnings denied.
- Do not add broad crate-level `allow` attributes to silence actionable
  warnings.
- A necessary lint exception is local, names the lint, and explains why the
  code is clearer or more correct with the exception.
- Do not commit compiler warnings, dead code, unused dependencies, or stale
  feature flags.

### Visibility and API surface

- Keep items private by default.
- Use `pub(crate)` only for a demonstrated cross-module need.
- Treat every `pub` item in a published crate as a compatibility commitment.
- Do not expose AST, HIR, automaton, cache, arena, worker, or diagnostic
  implementation types merely to simplify internal wiring.
- Prefer sealed traits or concrete types until users have a real extension
  requirement.

### Types and naming

- Use domain types such as `PatternId`, `StateId`, `Utf16Position`, and
  `Utf16Span` instead of interchangeable integer arguments.
- Use enums instead of boolean mode parameters when the call site would be
  ambiguous.
- Names describe roles and units. Avoid abbreviations outside established
  domain terms recorded in the architecture.
- Keep roadmap, UC, fixture, and evidence IDs stable after publication.
- Conversions that can lose range, unit, or encoding information are explicit
  and checked.

### Ownership and data flow

- Prefer immutable compiled data and scan-local mutable scratch state.
- Make sharing, synchronization, and thread-safety requirements visible in
  types and rustdoc.
- Avoid cloning to satisfy the borrow checker without first understanding the
  ownership boundary.
- Avoid advanced lifetime, trait, macro, or generic machinery unless it
  removes real duplication or enforces a useful invariant.
- Keep hot-path allocation visible and measurable.

### Errors and panics

- Expected user, pattern, input, configuration, and resource failures return
  typed errors.
- Errors retain the relevant pattern ID, span, coordinate unit, or operation.
- Do not use `unwrap`, `expect`, indexing, or assertions on public input unless
  an earlier checked invariant makes the operation infallible and the reason is
  locally obvious.
- Panic only for an internal invariant violation. Document public panic
  conditions when they cannot be eliminated.
- Preserve source errors where they help diagnosis.

### Unsafe Rust

- The initial core denies unsafe code.
- A later `unsafe` block requires a written safety invariant, focused tests,
  Miri or equivalent evidence where applicable, independent review, and a
  positive Rust performance result beyond noise.
- Keep unsafe code small and wrapped by a safe interface that checks its
  preconditions.
- Java precedent cannot justify unsafe Rust.

### Dependencies

- Prefer `std` until a dependency provides a measured or substantial
  maintenance benefit.
- Record why each dependency exists and which public or internal boundary owns
  it.
- Review license, maintenance, MSRV, default features, transitive footprint,
  and security advisories.
- Disable unnecessary default features.
- Remove unused dependencies promptly.
- Dependency upgrades pass functional and applicable performance regression
  gates.

## Documentation standard

### Public rustdoc

Every public crate, module, type, trait, function, method, field, and error
variant has useful rustdoc. Documentation states, where relevant:

- what the item does and when to use it;
- coordinate and size units;
- ownership and borrowing behavior;
- matcher lifecycle and mutability;
- concurrency, `Send`, and `Sync` expectations;
- errors and recovery behavior;
- panic and safety conditions;
- complexity or allocation behavior when callers need it; and
- a minimal compiled example.

Examples should use only supported public API. They compile as doctests unless
there is a documented reason they cannot.

### Internal documentation

- Start each non-trivial module with its responsibility and boundary.
- Comment invariants, representation choices, proof obligations, and surprising
  performance constraints.
- Do not narrate assignments, loops, or syntax already clear from the code.
- Link design decisions to an ADR, roadmap ID, issue, or retained benchmark
  receipt.
- Update or remove comments when behavior changes; stale comments are defects.

### Project documentation

- Commands are copy-pasteable from the repository root.
- Paths and links are relative when they refer to repository content.
- Structured file formats have a versioned schema and an example.
- Generated files identify their source and reproduction command.
- Release notes describe behavior, compatibility, limitations, and evidence,
  not internal activity logs.

## Test standard

- Prefer small deterministic fixtures with exact expected events or errors.
- Name tests after observable behavior, not implementation methods.
- Use explicit `Prepare`, `Test`, and `Assert` comments when the stages would
  otherwise require interpretation.
- Keep unit tests close to implementation and public behavior tests at crate
  boundaries.
- Every bug fix starts with a failing regression test.
- Every parser feature includes positive, malformed, unsupported, and boundary
  cases.
- Every optimization compares output with the bypassable semantic baseline.
- Avoid sleeps, wall-clock races, random seeds that are not retained, and tests
  that depend on execution order.
- Property and fuzz failures are minimized into deterministic fixtures.

## Machine-readable project hygiene

- Every roadmap task and gate has a unique stable symbolic short name.
- Every UC evidence item names the UC, command, fixture, expected result, and
  revision.
- JSON and JSONL outputs use versioned schemas.
- Benchmark receipts retain complete inputs, versions, hashes, environment,
  correctness status, and raw measurements.
- CI summaries should be useful to a person but derive from machine-readable
  results rather than replacing them.
- Hidden local setup is a defect; setup and generated prerequisites belong in
  documented scripts or Cargo tasks.

## Required commands

Increment I1 will provide one stable aggregate command, planned as
`cargo xtask ci`, that runs the required local equivalent of PR CI. Its
constituent checks are expected to include:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo test --workspace --doc
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
```

The workspace may add focused schema, fixture, license, dependency, and UC
evidence commands. Scheduled and release workflows add the heavier audit,
fuzz, Miri, package, consumer, and external benchmark gates described in the
[roadmap](docs/roadmap.md).

## Review checklist

- [ ] The PR names a stable roadmap or UC ID.
- [ ] Public behavior and limitations are documented.
- [ ] Tests are small, deterministic, and exact.
- [ ] Formatting, Clippy, tests, and rustdoc pass.
- [ ] Visibility is no broader than required.
- [ ] Errors preserve useful context and expected failures do not panic.
- [ ] New dependencies or unsafe code have explicit justification and evidence.
- [ ] Hot-path changes have external regression receipts.
- [ ] Claimed optimizations have a positive Rust result beyond noise.
- [ ] Generated and machine-readable artifacts are reproducible and validated.

See [SUPPORT.md](SUPPORT.md) for help and reporting boundaries.
