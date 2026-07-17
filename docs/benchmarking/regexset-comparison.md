# Early comparison with Rust `RegexSet`

- **Roadmap task:** B0
- **Initial pinned competitor:** `regex` 1.13.1
- **Status:** Active; harness smoke passes, retained stable-machine campaign pending

## Why this belongs early

[`regex::RegexSet`](https://docs.rs/regex/1.13.1/regex/struct.RegexSet.html)
is the established, highly optimized pure-Rust API for searching many regular
expressions in one pass. A Rust many-pattern matcher that ignores it would not
understand its most relevant existing alternative.

The APIs do not return the same information. `RegexSet` answers whether any
pattern matched and which pattern indices matched somewhere. It deliberately
does not return match locations. rustmatch reports a longest match for every
pattern and every matching start position, including overlaps. A single number
that silently compares those different jobs would be advertising, not
evidence.

## Two correctness-gated lanes

### RS-NATIVE: native set membership

Both engines compile the same accepted pattern set and scan the same input.
The rustmatch events are reduced to the set of pattern IDs that matched at
least once and compared with `RegexSet::matches`.

This is `RegexSet`'s native workload and likely its strongest result. rustmatch
still computes its richer event stream, so the receipt must say that the work
is asymmetric. This lane answers whether rustmatch remains usable when an
application only needs set membership; it does not define semantic parity.

### RS-EVENTS: complete rustmatch event semantics

`RegexSet` first identifies candidate patterns. Individually compiled
`regex::Regex` values then recover matches for those candidates with repeated
`find_at` searches. Each search resumes one byte after the previous match's
start, rather than after its end, so overlapping literal starts are preserved.
The normalized `(pattern_id, start, end)` multiset must equal rustmatch before
a timing is retained.

This lane answers the actual rustmatch workload. It is necessarily less native
to `RegexSet`, and the result must not be presented as a general indictment of
the `regex` crate.

## Receipt requirements

Every retained point records:

- rustmatch revision and package hash;
- exact `regex` version and Cargo lockfile hash;
- pattern and corpus generator versions, seeds, sizes, and hashes;
- accepted/rejected pattern counts;
- compile time separately from scan time;
- warm-up and measured iteration counts;
- event count and normalized event hash for RS-EVENTS;
- matching-pattern count and normalized ID-set hash for RS-NATIVE;
- process, CPU, operating-system, compiler, profile, and thread settings;
- raw samples, summary statistics, and the predeclared noise rule.

Both engines run single-threaded first. Later throughput campaigns may run
several independent `RegexSet` scans in parallel, just as every other engine is
allowed to use the available machine. The thread sweep and optimum remain part
of the receipt rather than becoming an unexplained winner-only setting.

## Admission rule

B0 is a harness and honesty milestone, not a promise that the unoptimized I1
engine wins. It passes when both lanes are reproducible, results are validated,
and a smoke receipt survives. Performance work may then use RegexSet results to
prioritize experiments, but an optimization enters rustmatch only after its own
baseline/candidate gate shows a positive Rust improvement beyond noise.

Run the first correctness-gated release-profile smoke locally with:

```sh
cargo xtask bench-smoke
```

The smoke uses a small deterministic generated fixture and prints one JSON
receipt. It is included in `cargo xtask ci`, but it is not a stable-machine
performance campaign and its timing fields must not be quoted as a result.

## CI smoke alarm versus stable-machine evidence

The C1 runner and comparator validate exact generated literal events before
timing and compare seven-scan medians. The pull-request lane uses isolated base
and candidate Cargo targets on one GitHub runner and reruns in reverse order
before failing. Its deliberately broad threshold requires both more than 50
percent slowdown and at least 100 ms of absolute regression. Receipts are
retained for 30 days.

C1 does not time `RegexSet` and cannot admit an optimization. It is only a
smoke alarm for catastrophic rustmatch regressions. RegexSet scaling results
and performance-sensitive rustmatch changes require correctness-gated,
base/candidate measurements on the designated stable machine, currently
`agogo.local`, with the complete provenance described above.

The runner and comparator are ordinary release-profile commands so the same
receipt contract can be exercised locally:

```sh
RUSTMATCH_BENCH_REVISION="$(git rev-parse HEAD)" \
RUSTMATCH_BENCH_RUNNER=local \
  cargo run --locked --release --package rustmatch-bench -- literal-tripwire \
  > candidate.json

target/release/rustmatch-bench compare-tripwire base.json candidate.json
```

Repository contributors normally use the complete same-runner driver instead:

```sh
BASE_SHA=origin/main scripts/c1-tripwire.sh
```

The pull-request workflow invokes that same script and retains its three JSON
receipts. This keeps CI orchestration thin and lets the comparison path be
reproduced before a branch is pushed. The driver resolves both revisions to
commit SHAs and refuses to label a dirty working tree as a reproducible result;
`C1_ALLOW_DIRTY=1` exists only for harness development and its output must not
be retained as evidence.

### Initial hosted-runner calibration

The activation PR ran the same base and candidate three times on 2026-07-17.
All attempts passed without invoking the retry path:

| Attempt | Base median | Candidate median | Candidate change |
|---|---:|---:|---:|
| 1 | 439.9 ms | 432.5 ms | -1.68% |
| 2 | 566.0 ms | 569.4 ms | +0.60% |
| 3 | 563.8 ms | 566.1 ms | +0.40% |

The large shift in absolute time between hosted runners is expected noise and
is evidence for the same-runner design, not a rustmatch performance result.
The retained receipts belong to [GitHub Actions run
29596635864](https://github.com/la3lma/rustmatch/actions/runs/29596635864).
