# H43-X1.5 formal explicit-matcher admission result

**Machine disposition:** investigate<br>
**Human decision:** retain and run one construction-only discriminator<br>
**Production status:** not merged, not published, excluded from progress<br>
**Policy:** unchanged G9-v3<br>
**Measured revision:** `a5b86c2b3943aad3121e1f06944e39331f17a474`

## Summary

The complete H43-X1.5 assay proves that the separate, feature-gated assertion
matcher is exact, strongly useful in its declared envelope, and isolated from
the public default. It does not yet certify the source for merge.

Both explicit targets passed every scan and total-work gate with 15 of 15
positive cycles. Scan time fell 99.569974% at 1 MiB and 99.780726% at 2 MiB,
equivalent to approximately 232.54x and 456.05x paired-geometric speedups.
All nine historical default guards passed; the largest default regression was
1.616900%. Four functional refusal probes and all exact-fallback identities
passed, and control/candidate allocation accounting was identical.

Ring C nevertheless failed one metric: preparation on the 2 MiB target
regressed 2.688833%, with 13 of 15 cycles adverse. That crosses the unchanged
2% investigation boundary but not the 3% veto. The formal disposition is
therefore `investigate`, and the exact artifact is not mergeable under the
certified-keeper rule.

## Frozen evidence

- Plan SHA-256:
  `f8b453e44ba64edad406d7a92198d297ef955db65f8f36bf158bd9d47f150ba5`
- Plan-content SHA-256:
  `0a78cb98596ee79301a4052824244fb3f0215d66ef4147c2bdf417445fa03590`
- Measurement revision: `ca16fbaa383dec32fe85ad580779119220b2708c`
- Feature-off artifact:
  `09a5c0e94240d6b2fdf8e6e96eb77b3e2cae718fa3adbe614dabe1c3b5378160`
- Feature-on artifact:
  `aaf41a96ccdaf175e7cc4286b3fda20108eed7a29e690fad51629e2647f72162`
- Allocation shim:
  `96cdd0cc8018d2d52aed56b21e03704af4eea178545b31dd82cea1ee8444aa5d`

The accepted window ran for approximately 5 hours 7 minutes and retained
1,020 paired receipts, 120 calibration receipts, 18 allocation receipts, four
functional probes, and 4,658 evidence files. No Docker, GPU, or external CPU
contamination occurred. Three setup or environmental windows remain separately
retained and unadmitted.

An independent audit revalidated every artifact and fixture hash, every
receipt, all AB/BA keys and orders, exact events, activation and fallback
identity, linked raw files, and the analyzer. Recomputing the complete result
reproduced the machine disposition exactly.

## Calibration and default safety

The two same-binary calibrations had maximum absolute effects of 0.228043% and
0.538597%. All nine Ring A cells and all six metrics stayed below 2%.

| Largest default effects | Regression |
| --- | ---: |
| dense 10,000-pattern, 50 MiB scan | 1.616900% |
| dense 10,000-pattern, 50 MiB `T_total(10)` | 1.609202% |
| dense 10,000-pattern, 50 MiB `T_total(1)` | 1.542845% |
| dense 10,000-pattern, 50 MiB process wall | 1.315474% |

The feature's existence therefore produced no formal default-path blocker.

## Explicit capability results

| Cell | Scan reduction | Approx. speedup | Positive cycles | Preparation | Result |
| --- | ---: | ---: | ---: | ---: | --- |
| assertion target, 1,000 patterns, 1 MiB | 99.569974% | 232.54x | 15/15 | 1.441057% faster | pass |
| assertion target, 1,000 patterns, 2 MiB | 99.780726% | 456.05x | 15/15 | 2.688833% slower | investigate |
| prefix-mismatch activated control, 1 MiB | 99.997580% | about 41,322x | 15/15 | 1.703077% faster | pass |
| mixed cohort, 1 MiB | 99.525217% | about 211x | 15/15 | 0.226084% slower | pass |
| density just below threshold, 1 MiB | 96.845463% | about 31.7x | 15/15 | 1.487339% slower | pass |

Both targets also reduced `T_total(1)` and `T_total(10)` by more than 99.55%,
so construction does not erase the demonstrated use-case value. The blocker is
the frozen independent preparation gate, not break-even economics.

The one-unit-short, density-at-threshold, and dense fallback disclosures were
exact and improved median scan time by 6.973192%, 3.464571%, and 3.638976%.
They were not performance gates. `RequireSpecialized` correctly refused the
short and dense cases instead of silently falling back.

## Critical interpretation

The 2 MiB preparation signal cannot be discarded: only 2 of 15 cycles favor
the specialized builder. It also does not yet establish a causal construction
cost. The 1 MiB target uses the same 1,000-pattern database and moved in the
opposite direction, input length should not affect pattern compilation, and
H43-A1-P1 previously found the generic and specialized compiler entries
phase-locally neutral.

H43-X1.6 subsequently froze a construction-only discriminator for these exact
separate matcher types, excluding corpus I/O and scan work. Its specialized
preparation was 0.808000% slower, below the unchanged 2% boundary, so the
X1.5 blocker did not reproduce phase-locally. That successor supports an
explicit owner review, but it does not retroactively turn this immutable result
into a pass.

## Cleanup anomaly

The timed stage completed cleanly. During post-stage restoration, starting an
overdue timer briefly activated its one-shot service and the wrapper's immediate
comparison reported a failure. The independent recovery receipt found every
unit already back in its exact pre-window state, all containers exited, and the
GPU idle. The harness was corrected to restore originally inactive dependent
units after active timers; the full 364-test harness suite passes. No timing was
rerun, removed, or admitted from a rejected window.

## Decision

Do not merge or publish `a5b86c2` automatically. Keep the source branch and all
evidence. H43-X1.6 completed as a valid neutral causal discriminator; move to
an explicit owner review without relabeling this result. The certified default,
cross-engine comparison, and merged-efficiency graph do not move.

## Evidence

- [Curated local evidence](../evidence/h43/x1.5/a5b86c2/README.md)
- [H43-X1.6 construction discriminator](h43-x1-6-construction-discriminator-result.md)
- [H43-X2 local result](h43-x2-result.md)
- [ADR-0009](../adr/0009-explicit-experimental-matcher.md)
- [Explicit capability strategy](h43-explicit-risk-tier-design.md)
