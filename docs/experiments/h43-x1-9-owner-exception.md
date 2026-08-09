# H43-X1.9 explicit assertion matcher owner exception

**Production decision:** merged by explicit owner exception<br>
**Owner authorization:** 2026-08-09, retained in the project task<br>
**Production merge:** `4996bcc16a9cc936789372d8184f4ef4893fd336`<br>
**Exact measured candidate:** `a5b86c2b3943aad3121e1f06944e39331f17a474`<br>
**Formal R3 disposition:** rejected by automatic veto; unchanged<br>
**Formal `optimization_admitted`:** `false`; unchanged<br>
**Decision policy:** G9-v3 owner-authorized production exception

## Owner decision

The repository owner explicitly certified merging the feature-gated,
separate-type `assertion-prefix-v1` matcher through the documented production
exception mechanism after reviewing the complete R3 rejection and the bounded
H43-X1.8 causal follow-up.

This action advances production state. It does not certify R3, erase its
unfavorable cell, alter a threshold, or claim that the experimental matcher is
the default Rustmatch algorithm.

The merge also retains a post-measurement structure accessor gated behind the
unsupported `benchmark-internals` feature so the construction evidence remains
reproducible. The exact-R3 X1.8 discriminator removed that accessor from the
library tree and independently reached the same below-boundary conclusion; no
application configuration exposes the accessor by default.

## Evidence reviewed

The immutable five-hour R3 confirmation retained:

- 1,020 paired timing receipts, 120 calibration receipts, 18 allocation
  receipts, and four functional probes;
- exact semantics, exact fallback/refusal behavior, and passing allocation,
  RSS, total-work, service-restoration, and host-isolation lanes;
- all nine default-safety cells below their frozen limits;
- approximately 231.62x and 460.49x target scan speedups; and
- approximately 449.65x better one-scan total work at 2 MiB.

R3 also measured a 5.792673% preparation regression in the 2 MiB target with
11 of 15 adverse cycles. That result exceeds the unchanged 3% automatic veto.
R3 therefore remains rejected and may never be relabeled as a certified pass.

H43-X1.8 then measured exact-fixture construction without scanning in two
clean, balanced windows. The benchmark-layout run found a 0.717444% complete
construction premium. The stricter run placed a standalone diagnostic over
the byte-exact rejected R3 library tree and found a 0.976161% premium. Both
calibrations passed, every receipt validated, and neither run reproduced the
veto-sized magnitude.

## Production contract

The admitted capability remains deliberately difficult to activate
accidentally:

1. applications must enable the non-default Cargo feature
   `unstable-assertion-prefix-v1`;
2. applications must construct the separate
   `rustmatch::experimental::assertion_prefix_v1` matcher type; and
3. every scan must explicitly select `RequireSpecialized` or
   `AllowExactFallback`.

Enabling the feature alone does not modify `MatcherBuilder`, `Matcher`, or
their default scan dispatch. No automatic workload classifier, benchmark
identity, fixture hash, expected result, or machine identity participates in
routing. Static ineligibility and dynamic refusal are typed and observable.

## Accepted residual risk

The owner accepts a directionally real construction premium of approximately
0.8%-1.0% for the explicitly constructed matcher. The larger R3 process-level
preparation observation remains visible as an unresolved unfavorable formal
cell even though focused evidence does not support it as a stable builder
cost.

The exception grants production utility because the capability is non-default,
exact, explicitly requested, and produces an unusually large benefit on its
narrow eligible workload. It grants no default-performance claim and no
general workload-selection claim.

## Follow-up

H43-X1.10 retains a path for removing the accepted construction premium under
unchanged G9-v3 gates. It may begin only when a materially new builder or
representation mechanism predicts less registration or compilation work
without perturbing ordinary matcher layout. Another unchanged R3 run or
fixture-guided placement retuning is not authorized.

## Release consequence

The previous `0.1.0` package candidate `7305a24` remains valid historical
evidence for that exact source, but it is no longer the releasable head. The
H43 production source must complete a fresh package, documentation, consumer,
MSRV, and performance qualification before any release ceremony. No crate,
tag, or GitHub release is authorized by this exception.

## Retained evidence

- [Immutable R3 confirmation](h43-x1-7-r3-confirmation-result.md)
- [Exact-source causal follow-up](h43-x1-8-conditioned-preparation-result.md)
- [Experimental capability strategy](h43-explicit-risk-tier-design.md)
- [Owner-exception policy](../optimization-decision-policy.md#owner-authorized-production-exception)
- [Curated R3 evidence](../evidence/h43/x1.7/a5b86c2/README.md)
- [Curated X1.8 evidence](../evidence/h43/x1.8/8c27d23/README.md)
