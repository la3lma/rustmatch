# H43 explicit experimental-backend risk tier

**Status:** contingency design; not authorized for implementation<br>
**Trigger:** conservative H43-A1-P1/H43-A2 closes without an admission-eligible automatic path<br>
**Policy relationship:** additive to G9-v3; never rewrites a machine ruling<br>
**Production default:** unchanged exact matcher

## Goal

Allow informed users to request a narrowly documented, exact specialized
backend when they value its measured upside and accept its performance
variance, without exposing default users to automatic experimental routing.
This tier may accept performance risk. It may not accept incorrect events,
memory unsafety, unbounded storage, callback/failure changes, hidden fallback,
or misleading benchmark claims.

## Two-key opt-in

The backend must require both keys:

1. a non-default Cargo feature such as `experimental-backends`; and
2. an explicit builder choice such as
   `ExperimentalBackend::AssertionPrefixV1`.

Enabling the Cargo feature alone must not change matcher construction, scan
planning, code dispatch, or performance. Selecting the runtime policy without
the feature must be impossible to compile. The normal builder remains the
only default.

```rust,ignore
use rustmatch::{ExperimentalBackend, MatcherBuilder};

let mut builder = MatcherBuilder::new();
builder.experimental_backend(ExperimentalBackend::AssertionPrefixV1);
```

Names are illustrative until an API review. The public enum must describe an
experimental policy, not promise a permanent internal algorithm name.

## Eligibility and fallback

The first policy may activate only when every assertion-bearing pattern proves
the same nonzero five-unit ASCII consumed prefix after its leading assertion,
the cohort has at least 256 patterns, the input has at least one MiB of UTF-16
units, and a bounded 64 Ki-unit sample is below the frozen density ceiling.

- Static ineligibility should fail construction with an explicit diagnostic,
  because the caller deliberately selected this backend.
- Dynamic short or dense inputs should use the exact generic fallback and
  report the reason through opt-in diagnostics.
- No workload name, fixture hash, expected output, timing history, machine
  identity, or post-hoc cutoff may influence routing.
- Users must be able to select the conservative backend explicitly for an A/B
  comparison with the same pattern set and input.

## Risk contract

| Dimension | Experimental tier rule |
|---|---|
| Correctness | Exact event, UTF-16, assertion, callback, failure, and reuse gates remain absolute. |
| Default isolation | Feature absent and feature-present/runtime-off artifacts must prove unchanged default behavior and code path. |
| Performance claim | Claim only the predeclared eligible cohort and publish target, near-boundary, preparation, memory, and fallback results together. |
| Regressions | Opt-in performance regressions do not become default regressions; every measured adverse cell remains prominent documentation and evidence. |
| Resources | Bounded candidate storage, overflow checks, allocation accounting, and a conservative exact fallback remain mandatory. |
| Compatibility | Experimental API stability and removal policy must be documented before publication. |
| Observability | Diagnostics report requested policy, eligibility facts, activated backend, fallback reason, candidate count, and retained bytes. |

This is not a blanket waiver. A candidate with incorrect results, unbounded
memory, unexplained catastrophic behavior inside its declared eligible domain,
or any effect on runtime-off users is rejected even from the experimental tier.

## Incremental engineering path

1. Complete H43-A1-P1 and retain the conservative decision.
2. Freeze an API and SemVer review before adding source.
3. Move the benchmark-only backend behind the two-key opt-in without changing
   its eligibility constants or evidence fixtures.
4. Prove compile-time absence when the feature is disabled and runtime
   non-activation when the feature is enabled but not selected.
5. Run the complete semantic/failure/callback suite against generic and
   experimental policies.
6. Run target, boundary, dense, short, ordinary, preparation, allocation, RSS,
   and whole-process screens on an exclusive host.
7. Publish a clearly labeled experimental evidence table; do not add the
   result to the merged optimization graph unless production actually changes.
8. Require a separate owner decision for publishing the feature or later
   promoting any policy to automatic selection.

## Reject criteria

Reject the experimental route before publication if any of the following is
observed:

- any semantic, panic-reuse, callback-atomicity, or error mismatch;
- feature-disabled or runtime-off code dispatches through the backend;
- selection requires benchmark-specific or timing-derived facts;
- candidate memory is not bounded by input length with checked arithmetic;
- dynamic fallback is silent or diagnostics misstate activation;
- the declared eligible domain contains a repeatable severe regression without
  a causal explanation and an honest way for callers to avoid it; or
- documentation implies the experimental backend is universally faster or
  part of the default matcher.

## Promotion rule

The experimental switch does not create a shortcut to automatic selection.
Promotion requires a new frozen H43-S1 selector, near-boundary guards, and the
same complete formal G9 campaign required of any default optimization. All
experimental adverse evidence remains part of that review.
