# H43-A1-P1 phase-local preparation discriminator result

**Date:** 2026-08-07<br>
**Candidate:** `8ad52d9600cb7de3c98ee345fd82fb06f9e7d429`<br>
**Runner:** `f5cabeef7129fff6367671ff2d8c6baf1f1cced9`<br>
**Plan SHA-256:** `02903ae31638d4e3da220530224f6aef1d7a11d6c3b8eb4eefd2206e9e692753`<br>
**Window:** `2026-08-07T09:35:59Z` through `09:36:09Z`<br>
**Disposition:** valid; authorize H43-A2 and the X1.1 ADR<br>
**Historical H43-A1 disposition:** unchanged machine rejection

## Decision

H43-A1-P1 finds no systematic construction cost from enabling the isolated
assertion-prefix policy bit. It authorizes conservative H43-A2 eligibility work
and the separately governed X1.1 explicit-capability ADR. It does not admit
H43-A1, remove its rejected preparation cell, or alter any historical numeric
threshold.

The original 20.366% preparation aggregate was dominated by one fresh process
whose matcher preparation and unrelated input preparation both rose about
fourteenfold. After process startup, filesystem input, TSV parsing,
regular-expression registration, diagnostics, destruction, and serialization
were removed from the timed interval, every phase-local effect was near zero.
The two entry points also retained identical allocations, requested bytes, and
compiled structure in every adjacent pair.

The causal interpretation is therefore strong: the rejecting H43-A1 cycle was
not a repeatable compiler or representation cost caused by the specialization
bit. The immutable rejection remains a valid statement about that exact
fresh-process window, while this discriminator resolves the narrower causal
question it authorized.

## Frozen method

Each pattern fixture was read and parsed once by one process. Before every
timed build, a fully registered `MatcherBuilder` was prepared outside the
measurement interval. The interval consumed that builder through exactly one
of these entry points:

- `build_shared_cohort_diagnostic`; or
- `build_shared_cohort_assertion_diagnostic`.

For each fixture, three AB+BA rounds warmed the process. Thirty-one retained AB
pairs and thirty-one retained BA pairs then produced 124 builds. Across three
fixtures, the clean window retained 372 builds. The predeclared close rule
required all of the following on one fixture:

- pooled paired-geometric construction regression above 3%;
- both order-specific regressions above 3%; and
- at least 27 of 31 specialized builds slower in each order.

No fixture approached that complete rule.

## Results

| Fixture | Generic median | Specialized median | Pooled paired delta | AB / BA delta | Slower pairs AB / BA | Paired bootstrap 95% interval |
|---|---:|---:|---:|---:|---:|---:|
| assertion-prefix target, 1,000 patterns | 404.932 us | 404.972 us | +0.0681% | +0.0527% / +0.0836% | 20 / 16 | -0.1066% to +0.2765% |
| adversarial assertions, 256 patterns | 107.594 us | 107.649 us | -0.0274% | +0.0966% / -0.1512% | 18 / 12 | -0.5930% to +0.5129% |
| ordinary literals, 1,000 patterns | 372.371 us | 372.340 us | +0.0057% | -0.0118% / +0.0233% | 14 / 15 | -0.1515% to +0.1651% |

Positive deltas mean the specialized entry point was slower. All intervals
span zero, both orders remain close to zero, and every directional count is
well below the frozen 27-of-31 burden.

Every one of the 186 adjacent generic/specialized comparisons had:

- zero difference in allocation calls;
- zero difference in requested bytes;
- equal state, edge, predicate, terminal, pattern, cohort, database-byte, and
  prefilter-byte facts; and
- only the expected final specialization boolean difference.

## Window integrity

- Candidate revision and tree matched the frozen plan.
- All three source hashes and all fixture hashes matched.
- The candidate workspace all-feature test suite passed before measurement.
- The local independent analyzer recomputation was byte-identical to the
  retained summary.
- Docker event and contamination logs were both empty.
- The GPU remained idle.
- `llms5090-ollama`, `llms5090-openwebui`, and
  `hive-build-telemetryd-agogo` remained `exited` with restart policy `no`.
- The post-window SHA-256 manifest verifies.

An earlier `09:35:05Z` launch stopped before creating a run directory because
non-interactive SSH did not expose Cargo in `PATH`. Its console and PID sidecar
are retained in the compact bundle. It contains no timing and is not part of
the accepted window.

## Decisions enabled

1. **H43-A2 is authorized.** Causal eligibility and fallback boundaries may be
   explored under fresh predeclared guards. Any automatic result still owes a
   complete unchanged G9 campaign.
2. **X1.1 is authorized.** ADR-0009 may freeze the versioned feature, separate
   matcher type, explicit scan policies, diagnostics, SemVer boundary, and
   three admission lanes.
3. **H43-A1 remains rejected.** This result explains its outlier; it does not
   rerun, exclude, or admit the original artifact.

## Evidence

- [Reviewed evidence index](../evidence/h43/a1-p1/8ad52d9/README.md)
- [Frozen discriminator plan](h43-a1-p1-screen-plan.json)
- [ADR-0009 explicit matcher isolation](../adr/0009-explicit-experimental-matcher.md)
- Full retained evidence:
  `/home/rmz/git/h43-a1-p1-screen-20260807T093559Z`
- Compact bundle:
  `/Users/rmz/.codex/handoffs/rustmatch/h43-a1-p1/h43-a1-p1-screen-20260807T093559Z-compact.tar.gz`
- Compact bundle SHA-256:
  `cd014709818bc76c422f7656494ad7f201ff03af91b15f39aa54ec5f21ea9ab0`
