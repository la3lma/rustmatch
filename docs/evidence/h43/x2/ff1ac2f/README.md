# H43-X2 local-certification evidence

This directory retains the reviewed local exactness and default-isolation
receipt for implementation commit `ff1ac2fdd0fe1370c33709aff9451c15c017f5fa`.

The evidence compares the pre-X2 baseline, candidate feature-off build, and
candidate feature-on/runtime-unused build using Rust 1.97.0 on
`x86_64-apple-darwin`. The probe source is
`tools/h43_x2_default_probe.rs`. See `local-certification.json` for type
layouts, normal event identity, normalized ordinary-symbol hashes, and test
counts. The complete `cargo xtask ci` pipeline also passed, including Java
differential oracles, H43-E1 semantic parity, and B0 benchmark smoke.

The receipt authorizes only the next explicit utility-evidence stage. It does
not authorize publication, release-baseline merge, or automatic routing.

See the [reviewed result](../../../../experiments/h43-x2-result.md) for the
decision and caveats.
