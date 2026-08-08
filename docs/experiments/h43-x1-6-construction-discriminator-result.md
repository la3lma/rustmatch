# H43-X1.6 separate-type construction discriminator result

**Machine classification:** X1.5 construction blocker not reproduced<br>
**Human decision:** valid causal discriminator; owner review authorized<br>
**Production status:** not merged, not published, excluded from progress<br>
**Policy:** unchanged G9-v3<br>
**Measured Rustmatch revision:** `c47461695cbe7628f0f45ed61dd56bc2687ddd8d`<br>
**Measurement revision:** `204bce7c4e9cb168987995b02a66101fffb9f539`

## Summary

H43-X1.6 isolated construction of the generic matcher and the separate
`assertion-prefix-v1` matcher behind the same feature-on executable. It used
the exact same 1,000-pattern fixture for both variants and excluded corpus I/O,
corpus size, scanning, event delivery, and result serialization from the timed
interval.

The specialized matcher prepared 0.808000% slower than the generic matcher,
with 14 of 15 cycles adverse. This is directional evidence of a small cost,
but it is safely below the unchanged 2% investigation boundary and does not
reproduce X1.5's 2.688833% process-level preparation regression. Registration
was 0.218342% slower and compilation was 1.103926% slower; neither crossed the
boundary. Both same-type calibrations passed.

The machine classification is therefore
`x1.5-construction-blocker-not-reproduced`. This result narrows the causal
uncertainty. It does not rewrite X1.5 from `investigate` to `pass`, certify a
merge, or move the production baseline.

## Primary result

Positive effects mean that the specialized matcher was faster. Median
reductions are the median of 15 adjacent AB/BA cycle effects.

| Metric | Generic median | Specialized median | Median reduction | Direction | Gate |
| --- | ---: | ---: | ---: | ---: | --- |
| complete preparation | 789,475 ns | 795,771 ns | -0.808000% | 1 faster / 14 slower | below 2% |
| registration | 347,758.5 ns | 348,009 ns | -0.218342% | 6 faster / 9 slower | below 2% |
| compilation | 431,511.5 ns | 436,266 ns | -1.103926% | 4 faster / 11 slower | below 2% |

The direction count means the small preparation cost should not be called
zero. The size means it cannot account for the frozen X1.5 blocker under the
predeclared decision rule.

## Calibration and resources

The generic/generic and specialized/specialized same-type calibrations each
retained 15 adjacent AB/BA cycles. Their largest adverse median effects were
0.494013% for generic registration and 0.234753% for specialized compilation;
all calibration metrics remained below 2%.

Three allocation pairs were exactly equal for both variants:

| Allocation fact | Generic | Specialized |
| --- | ---: | ---: |
| `malloc` calls | 2 | 2 |
| requested bytes | 4,568 | 4,568 |
| peak live usable bytes | 4,576 | 4,576 |
| live usable bytes at exit | 4,576 | 4,576 |
| `calloc`, aligned, `realloc`, and `free` calls | 0 | 0 |

The structure receipts confirmed that both variants compiled the same 1,000
patterns and one partition. The specialized database additionally retained its
documented 11,002 states, 11,000 edges, 1,000 terminals, and shared `word0`
prefix. Every receipt matched the frozen structure.

## Frozen evidence

- Plan SHA-256:
  `977fc87428e30af7e27f84730734521a2dcc17ec7fa6718eff6b3fa70d6ea92b`
- Plan-content SHA-256:
  `e47de7058920793128ceeae8c5bac1a63bca22e90d93cea9482748633064e39a`
- Rustmatch tree:
  `ca629d9ecba284d0b0e6a23506bdeff7a3b6d84a`
- Feature-on executable:
  `2970451b0d4b6658c286cf5ae520ebfcc76cbd35202ba22b162a30b90956b677`
- Pattern fixture:
  `80ded42e964dcbc601cbec3dfb3eb72eb045a69ef4f3dcf82d1c6dbcdace8d9c`
- Allocation shim:
  `96cdd0cc8018d2d52aed56b21e03704af4eea178545b31dd82cea1ee8444aa5d`
- Service snapshot:
  `9228c82ed8d94b0a34f938c3cf4d73dddfbb28071947f796883ef842dd15a7b5`
- Stage command set:
  `5c604b86326dfc51459af909225eb0bc1609adec8fcb0d78626e29b79c5274de`

The accepted stage ran from `2026-08-08T10:11:27.069120Z` through
`2026-08-08T10:11:27.824884Z`. It retained 60 primary, 120 calibration, and
six allocation invocations: 186 receipts with linked stdout, stderr, and time
records. All stderr files were empty.

The guarded window closed cleanly. Managed services and timers were restored
exactly, all three managed containers remained exited with restart policy
`no`, Docker had no active container, the GPU was idle, and no external CPU or
repository contamination was observed.

An independent audit recomputed every primary effect from the raw receipts,
validated all hashes, orders, structures, allocations, linked files, host
guards, and service restoration, and reproduced the machine classification.

## Interpretation and next gate

X1.6 answers the narrow causal question prospectively asked by X1.5: the exact
separate-type construction path did not reproduce a G9-v3 investigation-level
regression when measured without unrelated process work. This increases
confidence that X1.5's sign-changing 2 MiB preparation result was not a stable
cost of the new matcher builder.

It does not erase the adverse X1.5 receipt, permit a post-hoc threshold change,
or authorize repeated full assays until one happens to pass. The next step is
an explicit owner review. That review may authorize either a separately named,
prospectively justified full confirmation or an owner exception that retains
X1.5's `investigate` classification and residual risk. No merge is automatic.

## Verification

The measurement command and benchmark-only diagnostics passed:

- `cargo test -p rustmatch-bench --features experimental-showcase`
- `cargo test -p rustmatch --all-features`
- `cargo test --workspace --all-features`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo fmt --check`
- 33 focused measurement-harness tests
- 367 full measurement-harness tests, with two expected skips

## Evidence

- [Curated local evidence](../evidence/h43/x1.6/c474616/README.md)
- [Immutable X1.5 result](h43-x1-5-formal-admission-result.md)
- [ADR-0009](../adr/0009-explicit-experimental-matcher.md)
- [Explicit capability strategy](h43-explicit-risk-tier-design.md)

