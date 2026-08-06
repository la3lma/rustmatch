# Parser fuzz target

The `parser` target sends arbitrary valid UTF-8 through the public pattern
registration boundary and builds every accepted pattern. Success and typed
rejection are both valid; a panic is not.

The ordinary quality gate compiles and lints this target. Run an actual campaign
with a nightly toolchain and `cargo-fuzz` installed:

```sh
cargo +nightly fuzz run parser
```

Long campaigns and retained crash artifacts belong in external evidence, not in
the pull-request CI job or this repository.

## H43-E1 cohort parity

The `cohort-parity` target always builds a mixed assertion-free/assertion-bearing
pattern set from supported templates, then compares ordinary and cohort modes
across fuzz-selected worker counts, cache budgets, flags, and raw UTF-16 input.
LibFuzzer minimizes any mismatch automatically:

```sh
cargo +nightly fuzz run cohort-parity
```
