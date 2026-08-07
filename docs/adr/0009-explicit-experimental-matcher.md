# ADR-0009: Isolate explicit experimental matchers from the certified default

- **Status:** Accepted for prototype; not accepted for publication
- **Date:** 2026-08-07
- **Decision owners:** rustmatch maintainers

## Context

H43-A1 reproduced an exact assertion-prefix mechanism with about 204x and 408x
paired-geometric target throughput at one and two MiB. Its exact focused
artifact remains machine-rejected because one fresh-process preparation pair
moved an inactive guard beyond the unchanged three-percent veto.

H43-A1-P1 subsequently timed only the two shared-cohort build entry points in
one process. Across 372 retained builds, its three pooled effects were
`+0.0681%`, `-0.0274%`, and `+0.0057%`; allocations, requested bytes, and
retained structures were identical in every pair. This resolves the causal
preparation question but does not rewrite the H43-A1 ruling.

The project wants expert callers to be able to request this narrow capability
without adding a branch, field, layout change, or automatic selector to the
ordinary matcher. Cargo feature unification means a feature flag alone cannot
prove deliberate application intent.

## Decision

The first source prototype will use two independent opt-in keys:

1. a narrow, non-default, versioned Cargo feature named
   `unstable-assertion-prefix-v1`; and
2. explicit construction of a separate type under
   `rustmatch::experimental::assertion_prefix_v1`.

The normal `MatcherBuilder`, `Matcher`, and `Error` types receive no new field,
variant, method, or scan dispatch for this capability. Enabling the feature
without constructing the separate type must leave ordinary behavior and hot
code unchanged.

The provisional prototype surface is:

```rust,ignore
use rustmatch::experimental::assertion_prefix_v1::{
    AssertionPrefixMatcherBuilder, ScanPolicy,
};

let mut builder = AssertionPrefixMatcherBuilder::new();
builder.add(PatternId::new(1), r"\bABCDE[0-9]+")?;
let matcher = builder.build()?;
let report = matcher.scan_with_policy(
    &input,
    ScanPolicy::RequireSpecialized,
    |matched| matches.push(matched),
)?;
assert!(report.specialized_backend_activated());
```

Exact names may change before publication, but the separate-type and explicit
policy boundaries may not.

## Eligibility and fallback contract

Static semantic eligibility is decided at construction. Ineligible pattern
sets return a typed experimental build error rather than silently producing an
ordinary matcher. The v1 proof requires an assertion-bearing cohort whose
patterns share the documented nonzero five-unit ASCII consumed prefix and meet
the frozen minimum cohort size.

Dynamic input suitability is selected explicitly on every scan:

- `RequireSpecialized` returns a typed refusal before callback delivery when
  the frozen input-size or density rule would not activate specialization.
- `AllowExactFallback` permits the current generic exact scanner and returns a
  report containing the fallback reason.

No scan policy implements `Default`. The caller must name the desired behavior.
Reports expose requested policy, static eligibility, activated backend,
fallback reason, candidate count, and retained candidate bytes. No benchmark
identity, fixture hash, expected output, timing history, or machine identity
may participate in routing.

## Error and semantic boundary

Experimental construction and scan errors live inside the versioned module and
wrap ordinary `rustmatch::Error` where appropriate. They do not expand the
certified default error enum merely to support an unpublished capability.

The experimental label permits performance variance, not semantic variance.
Exact event multisets, UTF-16 coordinates, assertions, callback atomicity,
panic/reuse behavior, checked arithmetic, and bounded memory remain absolute
gates.

## Stability policy

The feature and module are not part of the default 0.1.0 surface. Before any
crates.io publication, the project must choose one of these outcomes:

- stabilize the versioned v1 surface for the complete `0.1.x` line;
- move it to a separately versioned companion crate; or
- keep it unpublished and remove the prototype.

The word `unstable` is a warning, not a hidden SemVer waiver. If the feature is
published in `0.1.x`, removing or incompatibly changing its public API requires
at least `0.2.0` and a migration note. Performance magnitude is not a SemVer
guarantee; exact behavior and observability are.

## Admission lanes

The prototype may be proposed for a non-default merge only after three
independent lanes pass:

1. **Default safety:** feature absent and feature present/runtime-unused normal
   matcher semantics, layout, hot-symbol disassembly, and performance satisfy
   the existing G9-v3 rules.
2. **Explicit exactness:** differential semantics, failures, callback panic and
   reuse, overflow, allocation, RSS, and bounded-storage gates pass.
3. **Explicit utility:** target, boundary, fallback, adversarial, preparation,
   and total-work `T_build + N * T_scan` evidence supports the published
   capability envelope.

Passing these lanes authorizes only the explicit v1 capability. Any future
automatic selector requires its own full unchanged G9 campaign.

## Consequences

- Ordinary users cannot pay an experimental runtime branch or object-layout
  cost merely because another dependency enables a Cargo feature.
- Expert intent is visible in source and in every scan report.
- The exceptional target opportunity can be tested as a real product surface
  without relabeling H43-A1 or changing its benchmark series.
- The same compiler and engine remain private; the prototype should reuse them
  through narrow `pub(crate)` seams rather than exposing internal NFA types.
- A separate type duplicates a small amount of builder/API plumbing. That is
  accepted to protect the default boundary.
- README cross-engine numbers continue to describe the certified default. Any
  opt-in comparison must be labeled separately.

## Rejected alternatives

Adding `ExperimentalBackend` to `MatcherBuilder` is rejected for v1 because it
can change normal object layout and dispatch. A feature flag without a separate
type is rejected because Cargo unification is not deliberate runtime intent.
Silent generic fallback is rejected because it can hide the absence of the
requested speedup. A companion crate is deferred because splitting private
compiler machinery would create more default-code churn before demand is
proven. Automatic selection remains blocked on full G9 evidence.

## Evidence and implementation gate

- [H43-A1 result](../experiments/h43-a1-result.md)
- [H43-A1-P1 result](../experiments/h43-a1-p1-result.md)
- [Explicit capability strategy](../experiments/h43-explicit-risk-tier-design.md)
- [H43-X2 local result](../experiments/h43-x2-result.md)
- [H43-X1.5 formal result](../experiments/h43-x1-5-formal-admission-result.md)

This ADR authorizes a benchmark-internal source prototype and local semantic
tests. It does not authorize publication, merge into the release baseline, or
formal performance claims. Exclusive-host timing begins only after the source
prototype passes local exactness and both default-isolation configurations.

H43-X2 passed those local gates at implementation `ff1ac2f`. H43-X1.5 then
completed the full explicit utility assay at `a5b86c2`: default safety,
correctness, activation, allocation, scan, and total-work gates passed, but the
2 MiB target's preparation metric regressed 2.688833%. The unchanged G9-v3
disposition is `investigate`. Publication and merge remain unauthorized while
H43-X1.6 isolates that construction signal.
