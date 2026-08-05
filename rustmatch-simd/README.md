# rustmatch-simd

`rustmatch-simd` contains audited architecture-specific candidate-discovery
kernels used by the public [`rustmatch`](https://crates.io/crates/rustmatch)
crate.

This is an implementation dependency, not the supported application API.
Applications should depend on `rustmatch`. The crate detects AVX2 at runtime on
x86-64 and reports that acceleration is unavailable on other targets, allowing
the parent engine to use its exact portable path.

Every unsafe block has a local safety argument. Semantic matching remains in
the safe parent engine and verifies every candidate admitted by these kernels.

Licensed under Apache-2.0.
