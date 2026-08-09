# H43-X1.7 R3 explicit-matcher confirmation result

**Machine disposition:** rejected by automatic veto<br>
**Learning disposition:** investigate<br>
**Production status at result time:** not merged, not published, excluded from progress<br>
**Subsequent action:** capability merged by H43-X1.9 owner exception; this R3
result remains rejected<br>
**Policy:** unchanged G9-v3<br>
**Measured Rustmatch revision:** `a5b86c2b3943aad3121e1f06944e39331f17a474`

## Summary

H43-X1.7 R3 was the single prospectively authorized complete confirmation of
the explicit `assertion-prefix-v1` matcher. It reused the exact X1.5 source,
artifacts, fixtures, matrix, thresholds, and analyzer after hardening only the
host service-restoration wrapper.

The five-hour confirmation was clean and valid. It retained 1,020 paired
receipts, 120 same-binary calibration receipts, 18 allocation receipts, and
four functional probes. All nine default-safety cells passed, all exactness,
activation, fallback, allocation, RSS, and total-work gates passed, and no
Docker, GPU, host-workload, or repository contamination occurred.

The two explicit targets retained approximately 231.62x and 460.49x paired
scan speedups with 15 of 15 positive cycles. The 2 MiB target nevertheless
measured a 5.792673% preparation regression with 11 of 15 adverse cycles. That
single metric exceeds the unchanged 3% automatic veto, so the exact R3 result
is rejected and can never be relabeled as a certified pass.

## Decisive results

| Cell | Scan reduction | Approx. speedup | Preparation | Direction | Result |
| --- | ---: | ---: | ---: | ---: | --- |
| assertion target, 1,000 patterns, 1 MiB | 99.568257% | 231.62x | 1.623562% slower | 3 faster / 12 slower | preparation pass |
| assertion target, 1,000 patterns, 2 MiB | 99.782838% | 460.49x | **5.792673% slower** | 4 faster / 11 slower | **automatic veto** |

At 2 MiB, `T_total(1)` still improved 99.777604%, approximately 449.65x.
The veto therefore concerns the separately declared construction latency, not
scan throughput, end-to-end economics, correctness, or default behavior.

The 2 MiB raw preparation medians were 878,104 ns for the generic matcher and
906,086.5 ns for the assertion matcher, a pooled-median difference of 3.187%.
The predeclared adjacent AB/BA estimator produced the larger 5.793% ruling.
Its cycle effects ranged from an 8.165% candidate improvement to a 13.737%
candidate regression. Control and candidate run-to-run coefficients of
variation were approximately 6.9% and 7.3%, respectively.

## Interpretation

The direction burden supports a real small construction cost, but the large
magnitude is suspicious: the 1 MiB and 2 MiB cells compile the same patterns,
and input length is not consumed by either builder. The frozen metric must
still veto R3 because post-hoc exclusion, threshold changes, and rerunning
until one sample passes are prohibited.

This high discrepancy authorized one smaller causal successor, H43-X1.8. It
retains R3 unchanged while testing whether the 5.793% magnitude survives exact
corpus/allocator preconditioning without the multi-second scan phase.

## Frozen evidence

- Confirmation SHA-256:
  `c9d3b0046149c8f7fc2c2609196d005bc72bac45751d19f4e486309d363b85a1`
- Confirmation-content SHA-256:
  `1acb2d9502db576802076355dddb5e40f23adb8a46366d28adc81cc8f67e82ae`
- Window-state SHA-256:
  `c7cd2634532097ccdf89018bfa440801cb03d01fccd376da9fa2b66723ee21a0`
- Analysis SHA-256:
  `76bd5026ceab11750cf33a0848e833500c077b112fb6feee2f813886ec55b924`
- Campaign-manifest SHA-256:
  `8aa503972ebe3e3a2ecf19ad48254b28ed9998d565748fc82cafa4b139e1c2e8`
- Confirmation-manifest SHA-256:
  `3f685e3bf393ececbadaf0b3abf9dfea008011659b0ff9722d2309be2a419b56`

The guarded stage ran from `2026-08-08T15:34:18Z` through
`2026-08-08T20:40:55Z`; exact service restoration completed at
`2026-08-08T20:41:00Z`.

## Decision

Do not merge R3 as a certified keeper and do not run another unchanged full
assay. Preserve the veto and use H43-X1.8 to decide whether the remaining risk
is a stable algorithmic cost or process-level measurement coupling. Any later
merge requires an explicit owner exception and must continue to report R3 as
rejected.

## Evidence

- [Subsequent owner exception](h43-x1-9-owner-exception.md)
- [Curated local evidence](../evidence/h43/x1.7/a5b86c2/README.md)
- [H43-X1.8 conditioned preparation result](h43-x1-8-conditioned-preparation-result.md)
- [H43-X1.5 immutable admission result](h43-x1-5-formal-admission-result.md)
- [Optimization decision policy](../optimization-decision-policy.md)
