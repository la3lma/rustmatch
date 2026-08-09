# Optimization decision policy

**Current version:** `G9-v3`

**Effective for newly frozen experiments:** 2026-08-04

## Purpose

Rustmatch uses performance gates to force better engineering, not to make an
arbitrary threshold look like a law of nature. The gates remain strict and
stable across a test series, while the decision vocabulary distinguishes
admission from learning.

An optimization can therefore be valuable evidence without being eligible for
formal certification. No average gain, competitor comparison, or appealing
mechanism can offset a failed certification guard. A production owner may
separately authorize an explicit risk exception, but that action does not turn
the failed or investigated evidence into a certified pass.

## Admission and learning are separate

The admission decision answers whether the exact candidate may enter the
production baseline. The learning disposition answers what the evidence
requires next.

| Disposition | Production code | Baseline | Required action |
|---|---|---|---|
| `merged` | admitted | advances | retain complete positive and guard evidence |
| `investigate` | not admitted | unchanged | retain evidence and freeze the smallest causal follow-up |
| `rejected` | not admitted | unchanged | retain the negative result; reopen only with materially new evidence |
| `inconclusive` | not admitted | unchanged | retain uncertainty; improve measurement before making a causal claim |

An owner-authorized production exception is tracked separately from this
formal disposition table. The source may be merged, but its machine
disposition, human outcome, and `optimization_admitted` value remain unchanged.

Only `merged` attempts enter the optimization-efficiency graph.

## Numerical ruling

Correctness and exact event semantics are prerequisites.

1. A repeatable guard regression greater than **3%** is an automatic
   certification veto for the exact candidate, regardless of gains elsewhere.
2. A demonstrated guard regression greater than **2%** through **3%** produces
   `investigate`, not admission and not a terminal algorithmic rejection.
3. Results at or below the frozen two-percent boundary still obey their
   predeclared noise, agreement, order-stability, and cell-specific rules.
4. Added performance complexity should normally earn at least a repeatable
   **5%** target improvement. A smaller gain needs an explicit simplification
   or maintenance rationale and may not conceal a demonstrated regression.
5. Correctness failure rejects the exact artifact. Incomplete or
   directionally unstable evidence is inconclusive.

The two-percent investigation boundary and three-percent veto are evaluated per
declared guard. Improvements elsewhere are never averaged against a regression.

## Owner-authorized production exception

The repository owner may explicitly direct that an exact investigated artifact
enter production after reviewing its complete retained evidence. This is a
production-risk decision, not an admission result. Such an exception must:

- name the exact measured candidate and production merge;
- preserve the original machine disposition, human outcome, lab note, raw
  receipts, thresholds, and unfavorable cells;
- state the known residual regression in the current ledger and comparison;
- receive merged-production graph credit only because production actually
  advanced, with the graph labeling the exception; and
- leave a follow-up path for removing the accepted regression under unchanged
  formal gates.

`B2-H-0042` is the first such exception. Its formal result remains
`investigate` with `optimization_admitted: false` because Wuthering regressed
2.111% with 13 of 15 negative crossover cycles. Exact H42 entered production
only through explicit owner authorization.

`H43-X1.9` is the second exception. Its explicit assertion matcher entered
production in merge `4996bcc16a9cc936789372d8184f4ef4893fd336` after exact
causal follow-up, while the complete R3 result remains rejected with
`optimization_admitted: false` because one preparation cell regressed
5.792673%. The exception applies only to the feature-gated separate matcher;
the default API and routing remain unchanged.

## High-discrepancy signal

Large opposing effects are evidence about mechanism boundaries. When a
candidate produces unusually large improvements in some cells and regressions
in others, the review must examine:

- whether the fast and slow cells activate the same code path;
- workload dimensions that separate them, including input size, pattern count,
  density, worker count, output volume, and semantic family;
- binary-layout, instrumentation, preparation, memory, scheduling, and
  measurement confounders;
- whether a safe classifier or structural isolation can retain the gain; and
- the smallest experiment that distinguishes the leading cause from credible
  alternatives.

A reviewer may assign `investigate` when this contrast contains a plausible,
separable signal, including when an automatic veto rejects the exact artifact.
That ruling cannot admit code, grant graph credit, weaken a guard, or authorize
ad hoc tuning. It creates an obligation to retain the discrepancy and propose a
bounded causal successor.

## Versioning

Thresholds and rulings are frozen before candidate measurements. A future
policy change receives a new version and effective date. Historical raw
measurements, machine dispositions, and admission decisions are never rewritten
to simulate consistency.

`B2-H-0009` remains rejected and unmerged under `G9-v1`. `G9-v2` added the
learning disposition that its shared-candidate mechanism warrants
investigation. The successor must pass the same two-percent, three-percent, and
five-percent numeric gates; the new label does not grant H9 admission.

`G9-v3` prospectively adds the calibrated adjacent AB/BA geometric crossover
estimator and directional-burden rule. It does not change the two-percent,
three-percent, or five-percent thresholds and does not rewrite any G9-v1/v2
result. The owner-exception mechanism records production governance separately
from formal certification.
