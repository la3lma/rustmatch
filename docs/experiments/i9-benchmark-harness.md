# I9 experiment: ordinary benchmark-harness integration

## Status

Protocol frozen before the external adapter and runner command are implemented.
Input semantics, timing boundaries, modes, receipt fields, and acceptance rules
below may not be weakened after results are known.

## Repositories and source identity

The engine implementation and minimal runner live in `la3lma/rustmatch`. The
ordinary cross-engine orchestration, containers, scenarios, receipts, and plots
live in the private `la3lma/rmatch-performance-measurements` repository. I9
extends that harness; it does not create a second benchmark framework.

The harness prepares a clean Docker build context with `git archive` from an
explicit local rustmatch repository and commit. The image is built with the
repository's locked dependencies and pinned Rust toolchain. It records the full
Git commit, Rust version, container tag, and inspected image ID. Dirty working
tree content, GitHub credentials, and an ambiguous branch tip may not enter the
image.

## Comparable input contract

The existing deterministic harness fixture supplies `patterns.tsv`,
`corpus.bin`, and a manifest with exact file hashes and expected match count.
The rustmatch lane accepts only the current ASCII intersection:

- every TSV row contains one positive decimal pattern ID, one tab, and one
  non-empty ASCII expression;
- IDs are unique and fit rustmatch's `PatternId` domain;
- the corpus contains only ASCII bytes; and
- the manifest's pattern and corpus hashes are retained in the outer receipt.

The adapter maps each ASCII corpus byte to the same-valued UTF-16 code unit.
Consequently input bytes and rustmatch input units are identical for this lane.
It rejects non-ASCII data rather than silently comparing UTF-8 byte offsets with
UTF-16 coordinates. Richer Unicode fixtures require a separately specified
cross-engine encoding contract.

## Runner modes

One runner command consumes `PATTERNS_TSV CORPUS REPEATS WARMUPS MODE` and
supports exactly these modes:

- `nfa`: one worker, zero deterministic-cache states, and all I7 prefilters
  disabled;
- `single`: one worker with the ordinary optimized cache and prefilters; and
- a positive decimal worker count: the ordinary optimized path with that exact
  requested pattern-partition count.

The runner does not infer a worker count from available CPUs. The external
campaign may sweep counts and select a measured winner, but every retained
receipt records the requested and actual partition counts.

## Timing and callback boundaries

File reads and ASCII-to-UTF-16 conversion complete before matcher timing and are
reported separately as input preparation. `prepare_ns` covers builder creation,
pattern registration, and matcher build. Each warm-up and measured `scan_ns`
covers only scanning an already prepared matcher and input plus a minimal
non-allocating callback count. Process and container startup remain visible in
the outer harness receipt but are not scan throughput.

The runner checks that every warm-up and measured scan emits the same count. It
also emits a stable order-independent event digest. The outer harness rejects a
run unless the count agrees with the fixture manifest. Cross-engine event
digests are compared only between engines whose event identity semantics are
known to be equivalent; count equality is mandatory for every engine.

Task throughput is original logical corpus bytes divided by the median time to
apply the complete pattern set. It is not multiplied by pattern count or worker
count.

## Machine-readable output

The final stdout line is one JSON object containing at least:

- schema and runner versions;
- engine name and exact rustmatch Git commit;
- Rust compiler version;
- mode, requested workers, and actual partitions;
- expression count, corpus bytes, and UTF-16 input units;
- input preparation and matcher preparation nanoseconds;
- warm-up and measured scan arrays plus median scan nanoseconds;
- logical Mbit/s;
- matches per iteration and event digest;
- cache, prefilter, retained-database, candidate-bitmap, and buffered-event
  diagnostics from one verified measured scan; and
- a correctness status.

Human-readable progress goes to stderr or precedes the final JSON line. A
successful process with missing or malformed required fields is still rejected
by the external parser.

## Harness integration

The performance repository receives:

- a source-context preparation command pinned to an explicit Git commit;
- a multi-stage Rust container build with a small runtime image;
- `build-rustmatch-rust` and `run-rustmatch-rust` Make targets parallel to the
  existing engines;
- `rustmatch-rust` support in the common runner and receipt schema;
- validation tests for command construction, parsing, source identity, and
  failed correctness; and
- plot support through the existing generic receipt path, without a one-off
  converter.

## Admission sequence

1. Unit-test parsing, mode validation, ASCII rejection, stable event evidence,
   and malformed input in the rustmatch repository.
2. Build an image from the exact candidate commit and run a small generated
   smoke fixture.
3. Run the 1,000-pattern 8 MiB diverse-literal and mixed-regex scenarios in
   `nfa`, `single`, and an explicit parallel mode. Every count must match the
   manifest and repeated event evidence must agree.
4. Confirm that archived receipts can be plotted by the ordinary pipeline.
5. Merge I9 only after both repositories' checks and the smoke receipts pass.

I9 integration is complete at that point. The broader B1 campaign then runs
1,000/2,500/5,000/7,500/10,000 patterns over 8 MiB and 50 MiB, zero/sparse/dense
variants where available, diverse literals and the validated mixed-regex
intersection, NFA and optimized one-worker baselines, and complete declared
worker sweeps. B1 requires critical scaling and profiling analysis in addition
to green correctness checks.
