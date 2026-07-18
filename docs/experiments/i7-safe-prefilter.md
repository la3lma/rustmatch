# I7 experiment: safe start acceleration and literal prefilter

## Status

Complete. The protocol was frozen before production implementation; its
thresholds, fixtures, and baseline were not relaxed after candidate results
were known. Candidate `37f69819d9231fb99d747d4ae48034204657c1f2` passed every
correctness, performance, resource, and build gate. The retained receipts and
critical analysis are in the
[`I7 evidence package`](../evidence/i7/37f6981/README.md).

## Baseline and comparison design

- Frozen pre-I7 revision:
  **`284571996b343041d374f2620905f32d63dbafdc`**.
- Semantic scan comparison: the final I7 candidate measured with its hidden
  prefilter control disabled and enabled in the same release binary.
- Build and retained-memory comparison: frozen pre-I7 revision versus the final
  candidate on the same identified runner.
- Every timing pair runs on the same physical allocation, alternates order on a
  failed first comparison, scrubs an identified cache working set, and retains
  raw JSON receipts.
- Java rmatch and RegexSet may motivate designs but do not count as I7
  admission evidence.

## Production activation region

The initial full literal prefilter may activate only when all of these hold:

- the pattern set contains at least 256 patterns;
- the finite input contains at least 1 MiB of UTF-16 units;
- every pattern has a structurally proven fixed literal prefix of at least
  three UTF-16 units;
- the pattern set contains no assertions;
- a bounded sample reports candidate density below 50% of possible starts.

The exact one-/two-symbol start table may operate outside that region when its
NFA-derived table is available. Any uncertain symbol or context is admitted,
not skipped.

## Correctness gate

- Complete normalized event multisets must be identical with I7 disabled and
  enabled for every focused and Wuthering scenario.
- Property-generated supported HIR/input cases compare I7 on/off and the
  reference interpreter where practical.
- Required adversaries cover:
  - common and divergent alternation prefixes;
  - optional prefixes and nullable repetition;
  - fixed and variable repetition;
  - predicates, case-insensitive syntax, and non-ASCII UTF-16 units;
  - line anchors and word boundaries;
  - overlapping literals and failure-link suffix outputs;
  - input ending partway through a literal;
  - mixed filterable and unfilterable pattern sets;
  - dense candidates that force the sampled-density bypass.
- Diagnostics must distinguish disabled, unavailable, below-threshold,
  assertion, dense-sample, start-table, and literal-prefilter paths.

Any event difference rejects I7 regardless of speed.

## Focused performance gate

Times are medians of at least seven measured scans after three warmups. A
positive result must improve by at least 10% and 2 ms; both conditions apply.
A guard fails only after a reverse-order repeat confirms slowdown greater than
3% and 1 ms; both conditions apply.

| Scenario | Patterns | Corpus | Expected path | Gate |
|---|---:|---:|---|---|
| generated literal sparse | 1,000 | 8 MiB | literal prefilter | positive |
| generated literal sparse | 5,000 | 8 MiB | literal prefilter | positive |
| generated literal dense | 1,000 | 8 MiB | measured literal or density bypass | guard |
| generated mixed sparse | 1,000 | 8 MiB | proven common prefixes | positive |
| assertion sparse | 1,000 | 8 MiB | assertion bypass | guard |
| mixed unfilterable | 1,000 | 8 MiB | start table or full bypass | guard |
| short literal corpus | 5,000 | 16 KiB | below-size bypass | guard |

The retained Wuthering Heights campaign uses 1,000, 5,000, and 10,000 literal
patterns over the 8 MiB expanded corpus. Each count is a positive gate with the
same 10% plus 2 ms threshold. The original 675,259-byte corpus remains an
exploratory continuity line because it is below the initial 1 MiB activation
threshold.

## Resource and build gate

- No unsafe Rust and no new runtime dependency.
- Candidate storage is bounded and reported separately for the immutable
  filter and scan-local candidate set.
- The 10,000-pattern Wuthering filter must retain no more than 8 MiB of
  explicitly accounted prefilter storage.
- Candidate-position storage must not exceed one bit per possible UTF-16 start
  plus 64 KiB of fixed metadata.
- Median 10,000-pattern build time may regress by at most 25 ms relative to the
  frozen pre-I7 revision. The absolute allowance is intentional because a
  reusable matcher can reasonably spend one bounded setup cost to avoid work
  on every large scan; the measured cost must still be reported, not hidden.
- Non-activation scans may not allocate the full candidate bitmap.

## Critical analysis requirement

Passing thresholds is necessary but insufficient. The final evidence must
explain absolute scan and build times, candidate and event density, starts
skipped, filter size, candidate storage, fallback/bypass reasons, profile
hotspots, pattern-count scaling, corpus-size scaling, and any scenario where
the prefilter loses. Diagnostic controls must separate filter scan cost from
semantic verification and event delivery. A green threshold without this
analysis does not complete I7.
