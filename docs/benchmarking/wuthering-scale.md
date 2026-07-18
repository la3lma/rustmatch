# Wuthering Heights scale campaign

The `I6-B1` campaign makes optimization work visible before it is admitted. It
uses the original rmatch development corpus and word list, but reports only
Rust-on-Rust engineering evidence. Cross-engine claims belong in the separate
performance-measurements repository.

## Workload

- The corpus is the approximately 675 KiB Wuthering Heights fixture retained by
  the Java rmatch repository.
- The pattern source is that repository's list of words found in the book.
- Each run selects 1,000, 5,000, and 10,000 distinct non-empty lines in stable
  lexical order and escapes them as literal regular expressions.
- The complete corpus is scanned after every cache scrub. Compilation and scan
  medians are recorded separately.
- One warm-up and three measured scans keep the unoptimized baseline tractable.
  The focused I6 admission campaign uses three warm-ups and seven measurements.

The scale lane intentionally complements rather than replaces the focused I6
fixtures. Wuthering Heights reveals pattern-count scaling and cache sensitivity;
small generated fixtures isolate the mechanism and retain exact event lists.
It remains the stable development and regression corpus while the engine is
still gaining large improvements. Once its runs become too short or too narrow
to guide tuning, larger and more diverse corpora must supplement it rather than
erase the historical line.

A same-sized no-match control is useful when interpreting pattern scaling.
Wuthering's event count grows with the selected word set; a no-match input
separates result-delivery and fallback growth from the cost of scanning one
additional input unit.

## Cache pressure

Before each warm-up and measured scan, the runner writes one word per nominal
64-byte line across a dedicated scrub buffer. The default buffer is 256 MiB,
large enough to exceed ordinary workstation caches and the 128 MiB last-level
cache of the current high-throughput runner. The receipt records the actual
size and a digest proving that the scrub pass executed. This is controlled
cache pressure, not a claim that software can portably flush every hardware
cache.

## Run and inspect

The corpus remains outside this repository. With sibling `rustmatch` and
`rmatch` checkouts:

```console
scripts/i6-campaign.sh
open target/benchmark-results/index.html
```

Override `RMATCH_REPOSITORY`, `PATTERN_SOURCE`, or `CORPUS_SOURCE` when the
source checkout is elsewhere. `CORPUS_BYTES`, `CACHE_SCRUB_BYTES`, and
`PATTERN_COUNTS` control an explicitly identified exploratory variant. The
script refuses a dirty tree unless `I6_ALLOW_DIRTY=1` is deliberately set.
`I6_CARGO_TARGET` selects and records a native target path when the host
toolchain would otherwise produce an emulated binary. Always verify the output
of `file` before making an architecture claim.

The generated HTML table includes revision, runner, dimensions, event count,
cache-state and fallback counts when available, selected candidate-start path,
verified starts, compilation median, scan median, throughput, source paths, and
source digests. Raw JSON receipts remain authoritative; the table is only a
view.

For I7 admission, `scripts/i7-wuthering-campaign.sh` measures the same native
candidate binary with start acceleration disabled and enabled. It uses three
warmups and seven measured scans at 1,000, 5,000, and 10,000 patterns over the
8 MiB expanded corpus, requires exact event evidence and a 10% plus 2 ms win at
every point, and separately compares 10,000-pattern build time with the frozen
pre-I7 revision. The generated `index.html` places both paths side by side.

## Coarse CI tripwire

`scripts/c2-wuthering-tripwire.sh` runs 5,000 patterns over a deterministic
8 MiB expansion of the retained fixture on both PR base and candidate in one
GitHub Actions job. The expanded size deliberately crosses I7's full-prefilter
activation threshold. The job requires identical source hashes and event
evidence, requires the default one-worker path, retries once in reverse order,
and fails only when slowdown exceeds both 100% and 50 ms. This broad `C2`
threshold is a smoke alarm for severe regressions. It is not an optimization
admission result and does not replace the native stable-machine campaign or its
critical analysis.

The tripwire fails its GitHub Actions workflow when both limits are crossed.
The project merge protocol treats that as blocking pending investigation, but
the repository currently has no GitHub branch-protection rule that prevents an
administrator from merging a failed workflow.
