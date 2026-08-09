# H43-X1.8 corpus-conditioned preparation result

**Machine classification:** R3 preparation blocker not reproduced<br>
**Human decision:** causal investigation complete; owner exception review available<br>
**Production status:** not merged, not published, excluded from progress<br>
**Policy:** unchanged G9-v3<br>
**Exact production parent:** `a5b86c2b3943aad3121e1f06944e39331f17a474`

## Summary

H43-X1.8 retained the exact 1,000-pattern and 2 MiB R3 fixtures, materialized
and retained the 2 MiB UTF-16 corpus before timing, and measured only pattern
registration, compilation, and complete matcher preparation. It ran 15
balanced adjacent AB/BA cycles plus 15-cycle generic/generic and
assertion/assertion calibrations under the same guarded Agogo service policy.
It performed no scans and cannot replace or relabel the full R3 result.

Two independently built diagnostics agree. The first used the existing
benchmark-internal structure accessor and measured 0.717444% slower complete
preparation. A stricter follow-up put the diagnostic in a separate binary over
the exact rejected R3 `rustmatch` library tree, eliminating even that accessor
as a possible code-layout confounder; it measured 0.976161% slower complete
preparation. Both are below the unchanged 2% investigation boundary.

The reproducible cost is therefore approximately 0.8%-1.0%, not zero, but the
R3 5.792673% veto magnitude did not reproduce. No source optimization or
profile-guided change is justified for a sub-millisecond cost this small.

## Results

Positive reductions mean the assertion matcher was faster.

| Diagnostic | Complete preparation | Direction | Registration | Compilation | Classification |
| --- | ---: | ---: | ---: | ---: | --- |
| benchmark-layout source `5f0c2ae` | -0.717444% | 2 faster / 13 slower | -0.183816% | -0.818943% | below 2% |
| exact-R3 library source `a5b86c2`, diagnostic `8c27d23` | **-0.976161%** | 1 faster / 14 slower | -0.760461% | -0.998470% | **below 2%** |

Exact-R3 pooled medians were 789,444.5 ns versus 796,217.5 ns for complete
preparation, 352,717.5 ns versus 353,143.5 ns for registration, and 436,702 ns
versus 442,537 ns for compilation. Run-to-run coefficients of variation stayed
below 1.6%, compared with roughly 6%-7% in the R3 full-process preparation
receipts.

Each run retained 60 primary and 120 calibration receipts. All 360 receipts
validated, both calibration sets passed, all stderr files were empty, every
artifact and fixture hash matched, and both guarded windows restored services
exactly. The three managed containers remained exited with restart policy
`no`, Docker had no active containers, and the GPU was idle.

## Causal conclusion

The assertion matcher has a small, directionally consistent construction
premium. The full R3 process amplified that premium into a veto-sized paired
effect while also showing much larger short-phase variance. The most plausible
remaining explanation is process-level timing coupling around a sub-millisecond
metric inside invocations whose scans last many seconds, not a 5.8% builder
algorithm cost.

This conclusion does not make R3 pass. It does establish that another source
tweak or another unchanged five-hour assay would have poor scientific value.
The next decision is governance:

1. leave the feature unmerged because formal certification failed; or
2. merge the non-default, deliberately selected matcher through the documented
   owner-exception mechanism, retaining R3's rejection and disclosing the
   approximately 1% construction premium.

Given exact semantics, complete default isolation, 231x-460x target scan gains,
passing one-scan total-work economics, and a repeatable cost below 1%, the
engineering recommendation is option 2. It is an explicit risk acceptance,
not a certified G9-v3 pass.

## Frozen evidence

### Exact-R3 discriminator

- Diagnostic revision: `8c27d2350125e3b0de6a8abdd65eff1947535b09`
- Diagnostic tree: `9ecd1f38b662c61ce276c2134f690097817e60c5`
- Exact production parent: `a5b86c2b3943aad3121e1f06944e39331f17a474`
- Plan SHA-256:
  `e626359e8fcc01425b35c682533c20ff998ae5b3341043e6c2001c4d36ce8ec2`
- Plan-content SHA-256:
  `0a5fe76ed78808ee8869fcf269a3ff19d828aacc28b4f780c72ca5bdd1bf84cd`
- Artifact SHA-256:
  `147a273face61547b95e4190925beeeb09b06399c85c7f9da384952f41667bb4`
- Service-snapshot SHA-256:
  `3826c2954e90395c3eab268a02113de65d05540052b1a2d2e68258d99d2691ac`
- Window-state SHA-256:
  `cadfbb9685dd6ff7e0ddaff9031f809608bef378acf867cd2f0951cba2548517`

### Independent benchmark-layout discriminator

- Diagnostic revision: `5f0c2aee21a1b4ee078a1ca1bb34af87a6995e98`
- Plan SHA-256:
  `af7677980d42f49ff374ccd1a6d6cc1d8fed52b7a46cc5b1701e99246af25e11`
- Artifact SHA-256:
  `67d84b483fe7f5bb24e3b59ffe684935de8738172dd375ab714d3a7fb41709dd`
- Window-state SHA-256:
  `cadfd92d236d339dc0948413fa5ddd1681c3c871bd4c49b657078d9a2f24aaaa`

Both runs used pattern SHA-256
`80ded42e964dcbc601cbec3dfb3eb72eb045a69ef4f3dcf82d1c6dbcdace8d9c`
and corpus SHA-256
`25c0959ca9e93ce265a598bdd8f8199692d277d4a395875a1d621877568fa265`.

## Verification

- `cargo test --workspace --all-features` at exact R3 plus the standalone diagnostic
- `cargo test -p rustmatch-bench --features experimental-showcase`
- 382 measurement-harness tests on Agogo
- independent raw-receipt recomputation and hash audit
- archive listing verification for both retained handoff bundles

## Evidence

- [Curated local evidence](../evidence/h43/x1.8/8c27d23/README.md)
- [Immutable R3 result](h43-x1-7-r3-confirmation-result.md)
- [Earlier construction discriminator](h43-x1-6-construction-discriminator-result.md)
- [Optimization decision policy](../optimization-decision-policy.md)
