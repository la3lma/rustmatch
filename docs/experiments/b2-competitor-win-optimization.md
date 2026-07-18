# B2/G9 experiment: competitor-win diagnosis and Rustmatch admission

## Status

Planned after B1. This protocol does not alter the frozen B1 campaign, its
runner, or its interpretation. B1 supplies the measurements; B2 turns confirmed
competitor wins into bounded Rust-native hypotheses; G9 decides whether any
candidate is better than the existing Rustmatch implementation.

## Baseline and purpose

RegexSet is a diagnostic competitor. Its wins identify workloads where
Rustmatch deserves closer inspection, especially after worker confirmation and
semantic classification. RegexSet is not the optimization baseline and matching
or beating it is not the condition for accepting code.

For each experiment, the baseline is the exact Rustmatch production revision
that exists before the candidate is implemented. That revision, build profile,
toolchain, lockfile, host state, fixture hashes, worker counts, warm-up policy,
measurement boundary, and result digest are frozen before candidate timing.
Later Rustmatch improvements become the baseline for still later experiments;
an older convenient baseline may not be reused to manufacture a win.

## B2: diagnose before changing code

After B1 is complete:

1. Build a ranked table of confirmed cells where RegexSet beats Rustmatch.
   Keep native set-membership and complete-event lanes separate, and show the
   work each engine performs.
2. Require complete declared worker sweeps and confirmation receipts before a
   cell is called a win. Provisional or worker-asymmetric points can nominate
   follow-up measurements, but not an optimization.
3. Group the winning cells by pattern family, count, corpus size, density,
   selected worker count, preparation cost, scan cost, memory behavior, and
   output volume.
4. Profile existing Rustmatch on the exact weak fixtures. Use the confirmed
   Rustmatch worker optimum as well as one worker when that distinction helps
   separate algorithmic cost from scheduling or partitioning cost.
5. State a mechanism, not merely a correlation: for example excess candidate
   starts, transition-cache pressure, state-set hashing, event delivery,
   partition imbalance, synchronization, or memory bandwidth.
6. Add diagnostic controls that can distinguish the mechanism from plausible
   alternatives. A zero-match control, density sweep, pattern-count neighbor,
   or cache-pressure corpus is preferable to guessing from one ratio.
7. Record each plausible mechanism as a bounded experiment. Record implausible,
   semantically irrelevant, or prohibitively complex ideas as non-candidates so
   the same dead end is not repeatedly rediscovered.

An optimization may also improve workloads where RegexSet does not win. That is
welcome and belongs in the retained result, but it does not relax the requirement
to explain and measure the originally declared target.

## Candidate experiment contract

Before implementation, record:

- the exact existing Rustmatch baseline revision and artifact identity;
- the proposed Rust-native mechanism and the profile evidence behind it;
- target fixtures, worker modes, and the primary metric;
- exact output count and digest requirements;
- the noise model and minimum meaningful improvement;
- the broader guard set, including neighboring pattern counts, both corpus
  sizes, density controls, Wuthering Heights, and relevant semantic families;
- unacceptable regressions in scan time, preparation time, memory, latency,
  fallback behavior, or another scenario; and
- any proposed selective activation rule.

Prior art from RegexSet, Java rmatch, Hyperscan, or another engine may inspire
the mechanism. The Rust implementation must still fit Rustmatch's semantics and
architecture; copying a representation because it is successful elsewhere is
not evidence that it belongs here.

Each hypothesis should normally use its own branch or pull request and retain
both successful and rejected receipts. Several tiny mechanical commits may
support one hypothesis, but unrelated optimization ideas must not be bundled in
a way that prevents attribution.

## Candidate admission gate (G9)

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

B2 and G9 complete only when:

1. every confirmed RegexSet-winning B1 cell has a semantic label and a reasoned
   disposition: investigated candidate, explained task difference, duplicate
   mechanism, or documented non-candidate;
2. exact weak-fixture Rustmatch profiles and diagnostic controls are retained;
3. every implemented candidate has a frozen current-Rustmatch baseline and a
   reproducible base/candidate evidence bundle;
4. accepted candidates pass G9 and rejected candidates leave no enabled
   production-path complexity; and
5. the roadmap and critical B1/B2 report link to the retained evidence.
