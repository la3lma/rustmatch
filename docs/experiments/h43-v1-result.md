# H43-V1 shared-storage cohort carrier result

**Decision:** authorize the shared-storage runtime spine for H43-A1<br>
**Measured recovery:** `4f08cd9e0fc7253c4333a27b61d860b7255c7e81`<br>
**Representation baseline:** `3d48cb754d89da1359582627f743af87a86b342a`<br>
**Policy:** unchanged H43-V1 fail-fast rules under G9-v3<br>
**Consequence:** H43-A1 is available; V1 is not a production merge candidate

## Executive result

H43-V1 proves that assertion-free and assertion-bearing cohorts can share one
compiled pattern database without duplicating pattern bodies. The shared form
adds exactly one root state and no edges, predicates, terminals, or pattern
IDs. Exact event parity holds across the complete H43-E1 campaign and the two
focused mixed fixtures.

The first shared-view candidate retained 54.36% and 90.53% scan gains but
regressed median preparation by 20.42% and 32.11%. The database itself added
only 32 retained bytes. The regression came from eagerly compiling a 2,072-byte
assertion-free view start table and from temporary cohort pattern vectors, not
from shared multi-root storage.

The bounded R1 recovery made the carrier prefilter-neutral and removed those
temporary vectors. It retained 54.95% and 90.43% scan gains, reduced median
preparation to +2.595% and -1.035%, and added only 0 to 2 construction
allocations while requesting 178 fewer bytes. Every predeclared V1 gate passed.

## Focused evidence

Negative preparation deltas are improvements. Scan gains are positive.

| Fixture | Preparation delta | Paired geometric preparation | Scan gain | Paired geometric scan gain | Extra build allocations | Extra requested bytes |
|---|---:|---:|---:|---:|---:|---:|
| `mixed-balanced` | +2.595% | +4.297% | 54.949% | 54.502% | +2 | -178 B |
| `mixed-output` | -1.035% | +1.497% | 90.425% | 90.486% | 0 | -178 B |

The declared preparation rule uses the median of three alternating process
measurements. `mixed-balanced` passes narrowly at 2.595%; its paired geometric
delta is 4.297%. This does not invalidate the predeclared screen, but it is an
explicit caution for H43-A1 and the eventual formal campaign.

## Integrity and exactness

- The recovery window ran from `2026-08-06T21:49:55Z` through
  `2026-08-06T21:50:05Z` on Agogo.
- Docker and GPU contamination logs are empty. All managed containers remained
  exited with restart policy `no`.
- All ordinary/shared focused pairs retained identical match counts and event
  digests, including 75,040 events per `mixed-output` iteration.
- Complete repository CI passed, including 3,628 H43-E1 comparisons, Java
  compatibility evidence, strict Clippy, fuzz builds, rustdoc, and MSRV.
- The ordinary scan body remains 20,288 bytes and 4,205 instructions. Its
  canonical emitted stream is identical to the representation baseline at
  SHA-256 `03ed1a98690dcafa55450d060d169ead659c94b73f2f79580fe0ddfeac67a664`.

## Review boundaries

This decision authorizes a carrier, not a release or merge. The implementation
is benchmark-only, scans the two views sequentially, buffers events, and has no
specialized assertion backend. H43-A1 must reconstruct and isolate the H24
backend, prove exact failure and callback behavior, tune eligibility without
fixture identity, and pass focused plus formal G9 gates before any production
merge can be considered.

The preliminary whole-process allocation receipts are retained but are not
used as construction evidence because they include seven scans and therefore
count repeated event buffering. R1 uses a source-retained, construction-only
probe. See the [preliminary summary](h43-v1/summary.json), the
[recovery summary](h43-v1-r1/summary.json), and the
[recovery evidence manifest](h43-v1-r1/evidence-manifest.json).

## Retained artifacts

| Artifact | SHA-256 |
|---|---|
| Preliminary raw archive | `19cccb9e5aa04296c02febb7f5d39311907f7692175e76c2cd5ab6b4744b3f5f` |
| Recovery raw archive | `7a41decc320e766972f842a292b8a343d5395c85b3433cf110124fee91fce752` |
| Recovery summary | `dccec45a9cbe8ae2f92bf02d570260b7ba37f90ec701397c2b5b845c2cf472c1` |
| Recovery window state | `3a374bc7c494511368f05838c0570170f8a010a3d80beebf7c878ed4f7acb078` |

The raw archives and `SHA256SUMS` are retained under
`/Users/rmz/.codex/handoffs/rustmatch/h43-v1/` and independently on Agogo.
