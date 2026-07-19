# B2/G9 experiment: full-dataset analysis, hypotheses, and Rustmatch admission

## Status

Planned after B1. This protocol does not alter the frozen B1 campaign, its
runner, or its interpretation. B1 supplies the complete retained evidence. B2
examines that evidence before any optimization work begins. G9 then decides
whether a bounded candidate is better than the existing Rustmatch
implementation.

The sequence is mandatory:

1. complete and validate the test series;
2. examine the complete dataset quantitatively and automatically;
3. examine the results qualitatively and reason about the processes that could
   have produced them;
4. record and review explicit hypotheses and discriminating experiments; and
5. only then implement one bounded candidate at a time and repeat
   **hypothesize, test, evaluate, accept or reject**.

No optimization branch or production candidate may start before the B2 review
gate is complete. Exploring data, writing analysis tools, and improving the
measurement or reporting harness are not optimization candidates, but they must
not change or reinterpret the frozen B1 measurements.

The executable B1/B2 evidence workflow lives in the private-until-published
[`rmatch-performance-measurements`](https://github.com/la3lma/rmatch-performance-measurements)
repository. Its
[`B2 analysis protocol`](https://github.com/la3lma/rmatch-performance-measurements/blob/main/docs/b2-analysis.md)
generates the evidence audit and automatic tables, requires a structured human
qualitative review, binds a critical review to exact hashes, and emits a
reviewed `review-manifest.json`. This document remains the Rustmatch-side
contract; the benchmark repository supplies the retained evidence and
authorization artifact.

## Baseline and purpose

RegexSet is a diagnostic competitor. Its wins identify workloads where
Rustmatch deserves closer inspection, especially after worker confirmation and
semantic classification. RegexSet is not the optimization baseline, and
matching or beating it is not the condition for accepting code. Competitor wins
are one important part of B2, not the organizing principle for the analysis.

The full B1 dataset is the subject of B2. That includes workloads where
Rustmatch wins, loses, crosses over, scales unexpectedly, saturates early,
incurs high preparation or process cost, produces many events, or disagrees
with a simple performance model. Apparently strong results require scrutiny as
well as weak ones: an unexplained win may expose a task difference, a reporting
error, or a useful mechanism.

For each later candidate experiment, the baseline is the exact Rustmatch
production revision that exists before the candidate is implemented. That
revision, build profile, toolchain, lockfile, host state, fixture hashes, worker
counts, warm-up policy, measurement boundary, and result digest are frozen
before candidate timing. Later Rustmatch improvements become the baseline for
still later experiments; an older convenient baseline may not be reused to
manufacture a win.

## B2-A: evidence integrity and completeness

Before interpreting performance:

1. Prove that every required campaign point, confirmation, cache-pressure
   control, and profile is present or has an explicit disposition.
2. Verify revision, fixture, plan, image, toolchain, host, command, output count,
   digest, timing-boundary, and service-isolation identities from retained
   receipts rather than filenames or memory.
3. Keep provisional measurements visibly separate from confirmed measurements.
   Never silently substitute a partial worker sweep for a declared optimum.
4. Keep native set-membership results separate from complete match-event
   enumeration. State exactly what every engine is asked to compute.
5. Audit failures, retries, exclusions, outliers, and missing cells. Exclusion
   requires a recorded reason and must not be chosen because a result is
   inconvenient.
6. Regenerate the principal tables and plots from raw retained receipts. The
   analysis is not ready if a result cannot be traced back to its inputs.

## B2-Q: quantitative and automatic analysis

Analyze the entire valid dataset, not only aggregate winners. The regenerable
analysis must include at least:

- absolute scan throughput and time, preparation cost, whole-process effort,
  memory, output volume, and validated profile counters where available;
- adjacent pattern-count slopes and scaling across 1,000, 2,500, 5,000, 7,500,
  and 10,000 patterns rather than interpolation across distant endpoints;
- corpus-size effects, cache-pressure effects, and the interaction between
  pattern count and corpus size;
- sparse/dense sensitivity, match-event density, and the cost of complete event
  delivery;
- worker speedup, parallel efficiency, retained throughput, selected optima,
  early saturation, oversubscription, and partition imbalance;
- preparation-versus-scan tradeoffs and the distinction between measured scan
  speed and total machine effort;
- crossovers, discontinuities, cliffs, non-monotonic behavior, heavy tails,
  unstable rankings, and anomalous cells;
- patterns shared by related workloads, using clustering or automated anomaly
  detection where it clarifies rather than obscures the raw measurements; and
- uncertainty, repetition drift, effect sizes, and sensitivity to the chosen
  noise threshold.

Summary ratios may nominate questions, but they may not hide intermediate
cliffs or asymmetric work. Every automatic finding must link to the underlying
receipts and be reproducible by a checked-in command or script. Negative and
unexpected results are retained.

## B2-L: qualitative and causal analysis

Numbers describe behavior; they do not by themselves identify a mechanism. For
each major scaling regime, crossover, cliff, anomaly, competitor advantage, and
unexpected Rustmatch advantage:

1. Inspect the exact workload, retained profiles, implementation path, output
   contract, and relevant Rustmatch architecture.
2. Explain the plausible process behind the observation in concrete terms, such
   as candidate-start volume, transition-cache behavior, state-set hashing,
   event construction, partition balance, synchronization, allocation, memory
   locality, or bandwidth.
3. Record credible alternative explanations and confounders, including task
   asymmetry, warm-up, preparation boundaries, worker selection, cache state,
   event density, scheduling, and measurement noise.
4. Distinguish observation, inference, and demonstrated mechanism. A compelling
   story is still a hypothesis until a discriminating experiment supports it.
5. Compare related cells and controls to test whether the explanation is
   consistent with the rest of the dataset, not merely one attractive ratio.

Competitor implementations, earlier Rustmatch work, profiles, external advice,
and lab notes may suggest mechanisms. They are sources of hypotheses, not
evidence that an optimization works in Rustmatch.

## B2-H: hypothesis registry and experiment design

Turn the combined quantitative and qualitative review into a ranked hypothesis
registry. Each entry records:

- a stable identifier and the precise observation it addresses;
- the proposed mechanism and credible alternatives;
- supporting and conflicting evidence;
- target cells, neighboring controls, semantic guard cases, and expected
  directional results;
- the exact current-Rustmatch baseline to be frozen if implementation begins;
- the primary metric, noise model, minimum meaningful improvement, and stop
  condition;
- correctness, memory, preparation, latency, and broader-regression guards;
- estimated impact, confidence, implementation cost, complexity, and risk; and
- a disposition: experiment candidate, further measurement needed, explained
  without code, duplicate mechanism, or documented non-candidate.

Rank experiments by expected value, not by how interesting they are to
implement. Prefer the smallest experiment that can distinguish the proposed
mechanism from alternatives. A zero-match control, density sweep,
pattern-count neighbor, cache-pressure fixture, one-worker comparison, or
instrumented no-output path is often more informative than immediately changing
the production engine.

## B2 review gate

Optimization work may begin only after a review confirms that:

1. B2-A has established a complete, traceable evidence base;
2. B2-Q covers the entire dataset with regenerable quantitative outputs;
3. B2-L gives a reasoned disposition to every material regime, crossover,
   cliff, anomaly, and competitor win;
4. observations, inferences, and demonstrated mechanisms are clearly separated;
5. the B2-H registry contains ranked, bounded, falsifiable experiments with
   controls and stop conditions; and
6. the reviewed analysis and registry are committed and linked from the
   roadmap.

If the review exposes missing or ambiguous evidence, return to measurement or
analysis. Do not fill the gap with an implementation guess.

Passing review is represented by a committed benchmark-repository
`review-manifest.json`, not by an informal statement. Before an optimization
branch starts, verify and record:

- the exact benchmark-repository commit containing the reviewed archive;
- the SHA-256 of `review-manifest.json`;
- that its pre-review evidence, qualitative analysis, hypothesis registry, and
  critical-review hashes validate;
- that `optimization_authorized` is `true`; and
- that the selected stable B2-H ID appears in `authorized_hypotheses`.

Authorization is per hypothesis. A passing B2 review does not authorize an
unlisted idea, and a listed idea still has to satisfy the current-Rustmatch G9
admission gate below.

## Candidate experiment contract

After the B2 review, and before implementing each candidate, record:

- the hypothesis-registry identifier;
- the benchmark-repository review commit and reviewed-manifest SHA-256 that
  authorize that identifier;
- the exact existing Rustmatch baseline revision and artifact identity;
- the proposed Rust-native mechanism and the evidence behind it;
- target fixtures, worker modes, controls, and the primary metric;
- exact output count and digest requirements;
- the noise model and minimum meaningful improvement;
- the broader guard set, including neighboring pattern counts, both corpus
  sizes, density controls, Wuthering Heights, and relevant semantic families;
- unacceptable regressions in scan time, preparation time, memory, latency,
  fallback behavior, or another scenario; and
- the stop condition and any proposed selective activation rule.

Prior art from RegexSet, Java rmatch, Hyperscan, or another engine may inspire
the mechanism. The Rust implementation must still fit Rustmatch's semantics and
architecture; copying a representation because it is successful elsewhere is
not evidence that it belongs here.

Each hypothesis should normally use its own branch or pull request and retain
both successful and rejected receipts. Several tiny mechanical commits may
support one hypothesis, but unrelated optimization ideas must not be bundled in
a way that prevents attribution.

## G9 experiment loop and candidate admission gate

For every candidate:

1. **Hypothesize:** select one reviewed B2-H entry and freeze its baseline,
   expected result, controls, guards, and stop condition.
2. **Test:** implement one bounded mechanism, pass semantic correctness, then
   collect repeated and reverse-order base/candidate evidence.
3. **Evaluate:** inspect absolute values, scaling shape, profiles, process cost,
   memory, outliers, neighboring workloads, and alternative explanations. A
   threshold result without critical interpretation is incomplete.
4. **Accept or reject:** retain production code only if every admission
   condition below holds. Otherwise remove it from the production path and
   preserve the experiment and its lesson.
5. **Repeat:** update the hypothesis registry and choose the next experiment
   from the evidence now available. Do not stack an unproven optimization under
   another candidate.

A candidate enters the Rustmatch production path only when all of these hold:

1. Baseline and candidate produce the same required event count and normalized
   digest on every correctness-gated comparison.
2. They use the same fixture bytes, pattern set, host allocation, build mode,
   worker request, warm-up policy, repetitions, and timing boundary.
3. Repeated and reverse-order receipts show a positive improvement beyond the
   predeclared noise threshold over the frozen **existing Rustmatch** baseline
   on the declared target set.
4. The broader guard set contains no unaccepted material regression. A narrow
   win may justify a selective path only when its activation rule is safe,
   explicit, independently measured, and itself covered by correctness tests.
5. Preparation cost, process wall effort, peak memory, and profile changes are
   reported beside scan throughput rather than hidden behind it.
6. The result is critically interpreted: absolute times, scaling shape,
   outliers, mechanism evidence, limits, and alternative explanations are
   discussed.

RegexSet parity is neither necessary nor sufficient for admission. A candidate
that remains slower than RegexSet may still be a good Rustmatch optimization if
it demonstrably improves current Rustmatch. A candidate that beats RegexSet but
does not improve current Rustmatch is rejected. Neutral, inconclusive, or slower
candidates are removed from the production path and retained only as documented
experiments.

## Exit criteria

B2 completes only when the full-dataset analysis and review gate are complete.
In particular:

1. evidence integrity and campaign completeness are demonstrated;
2. the quantitative report covers every valid workload and major scaling
   dimension;
3. every material regime, crossover, cliff, anomaly, and confirmed
   RegexSet-winning cell has a reasoned disposition;
4. qualitative analysis relates the measurements to plausible processes and
   records alternatives and confounders;
5. the reviewed hypothesis registry identifies the next bounded experiments as
   well as documented non-candidates; and
6. no optimization candidate was started before this evidence was committed and
   reviewed.

G9 completes for an implemented candidate only when:

1. it has a frozen current-Rustmatch baseline and a reproducible
   base/candidate evidence bundle;
2. accepted code passes every admission condition and broader guard;
3. rejected code leaves no enabled production-path complexity; and
4. the roadmap, registry, and critical B1/B2 report link to the retained result.
