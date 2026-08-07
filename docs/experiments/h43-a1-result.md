# H43-A1 isolated assertion-prefix backend result

**Date:** 2026-08-07<br>
**Candidate:** `8ad52d9600cb7de3c98ee345fd82fb06f9e7d429`<br>
**Runner:** `6bfe6c94d8d79e0a2a3de86a42c6ad16cfb95108`<br>
**Policy:** G9-v3 focused investigation, not admission<br>
**Machine disposition:** rejected<br>
**Learning disposition:** investigate<br>
**Production state:** unchanged; no merge candidate

## Decision

H43-A1 recovered the H24 mechanism over H43-V1's single shared pattern store
without exposing an automatic selector or public API. It reproduced exceptional
exact scan gains and kept ordinary production code out of the benchmark-only
feature. The exact focused artifact is nevertheless machine-rejected because
one of thirteen adversarial preparation pairs produced a 20.366% geometric
regression under the frozen analyzer.

That ruling is immutable. It does not erase the stronger causal evidence: the
failing aggregate is dominated by one transient cycle in which specialized
preparation rose from 0.250 ms to 3.539 ms while unrelated input preparation
simultaneously rose from 0.772 ms to 10.709 ms. The other twelve preparation
pairs cluster around 0.220-0.474 ms, specialized median preparation is 1.713%
faster, allocation counts are identical, and the two build functions invoke
the same compiler before storing one different boolean. This is a
high-discrepancy `investigate` result, not permission to discard the outlier or
admit the candidate.

## What was tested

- The source correspondence, eligibility rule, target cells, inactive guards,
  memory limits, and numeric thresholds were frozen before measurement.
- One exact benchmark binary supplied both forced-generic `cohort-view-1` and
  forced-specialized `cohort-assert-1` modes.
- Thirteen adjacent alternating process cycles used two warmups and five
  retained scans per mode on an exclusive `agogo.local` CPU set.
- Exact event digest, event count, activation, allocations, requested bytes,
  peak RSS, Docker, GPU, managed-service, and host-workload evidence were
  mandatory.
- Workspace tests, failure atomicity, callback-panic reuse, short-input,
  prefix-mismatch, dense-input fallback, and ordinary production codegen gates
  passed before timing.

## Results

| Cell | Generic median | Specialized median | Paired geometric result | Exactness / activation |
|---|---:|---:|---:|---|
| assertion boundary, 1,000 patterns, 1 MiB | 7.739 s | 37.726 ms | 99.510% less scan time, about 204.3x throughput | exact; activated |
| assertion boundary, 1,000 patterns, 2 MiB | 15.405 s | 37.656 ms | 99.755% less scan time, about 407.6x throughput | exact; activated |
| adversarial assertions, 256 patterns, 1 MiB | 1.387 s | 1.398 s | 3.006% faster scan; 20.366% slower preparation | exact; did not activate |
| ordinary literals, 1,000 patterns, 1 MiB | 4.189 ms | 4.315 ms | 0.776% slower scan; 2.444% slower preparation | exact; did not activate |

The target throughput multipliers above are derived from the paired geometric
scan result. The direct ratio of aggregate medians is about 205.1x at 1 MiB and
409.1x at 2 MiB. Both views are retained so the report does not imply false
precision from one summary statistic.

No cell reported an event or activation error. The specialized representation
added zero allocation calls and four requested bytes. Peak RSS changed by
-0.46% through 0.00%, far below the frozen one-MiB absolute materiality rule.
The host guard recorded zero contamination lines and zero Docker events.

## Why the artifact is still rejected

The frozen analyzer evaluates paired geometric preparation deltas. Adversarial
cycle 5 measured a 1,314.5% specialized preparation spike, which is sufficient
to move the thirteen-pair aggregate beyond the unchanged three-percent veto.
Post-hoc removal would change the test series and is prohibited. A second
fresh window cannot be run merely until a favorable sample appears.

The raw cycle is also evidence against a systematic compiler regression:

- input preparation spiked by about 13.9x in the same specialized process;
- matcher preparation spiked by about 14.1x;
- allocation calls and usable-byte peaks remained identical;
- the specialized backend did not activate on this guard;
- the specialized preparation median across all cycles was lower; and
- the ordinary production scanner's canonical disassembly hash was unchanged
  between baseline and the no-feature candidate build.

The candidate therefore remains rejected, while the mechanism receives the
separate G9-v3 high-discrepancy `investigate` disposition.

## Next conservative discriminator

H43-A1-P1 should measure construction without changing this result. Freeze a
single-process AB/BA preparation microscope that parses the fixture once,
alternates the two build entry points, records at least 31 independent builds
per order, and reports allocations plus phase-local timings. Its purpose is to
test the causal claim that preparation is identical apart from the final
policy bit; it is not an admission substitute.

- If the discriminator finds a repeatable greater-than-three-percent build
  cost, stop automatic H43-A2 work and preserve the cause.
- If it is neutral and allocation-equivalent, H43-A2 may investigate static
  eligibility and density boundaries under fresh predeclared guards.
- Any eventual production artifact must still pass the unchanged complete
  formal campaign. This focused result cannot admit or merge code.

## Risk-tier contingency

An explicit experimental backend remains a separate fallback if conservative
automatic selection cannot pass. The [explicit capability strategy](h43-explicit-risk-tier-design.md)
requires both a narrow non-default Cargo feature and deliberate construction of
a separate matcher type, preserves exact fallback and all semantic gates, and
keeps the current default artifact unchanged. It is a governance and API
option, not a way to relabel H43-A1 or weaken its historical guardrails.

## Evidence

- [Reviewed evidence index](../evidence/h43/a1/8ad52d9/README.md)
- [Frozen focused plan](../evidence/h43/a1/8ad52d9/frozen-plan.json)
- [Analyzer summary](../evidence/h43/a1/8ad52d9/summary.json)
- [Window state](../evidence/h43/a1/8ad52d9/window-state.json)
- [Analyzer console](../evidence/h43/a1/8ad52d9/analyzer-console.json)
