# rustmatch implementation roadmap

**Last reviewed:** 2026-07-17  
**Overall implementation status:** **NOT STARTED**  
**Implementation increments started:** **0/12**  
**Implementation increments complete:** **0/12**  
**Next gate:** **I0 - time-boxed semantic charter**

This page is the at-a-glance map from the completed product planning to a
tested rustmatch release. The detailed requirements, architecture, use cases,
evidence contracts, and exit criteria remain in the [main README](../README.md).
This graph shows dependency and status; it does not replace those definitions.

## Dependency graph

```mermaid
flowchart TB
    subgraph PLAN[Planning foundation]
        P0["Product requirements and scope<br/>DONE"]
        P1["Architecture and executable-spine strategy<br/>DONE"]
        P2["Use-case evidence contracts<br/>DONE"]
    end

    subgraph BOOT[Bootstrap and first executable system]
        I0["I0 Semantic charter and ADRs<br/>NEXT - NOT STARTED"]
        F0["Fixture schema and Java oracle JSONL<br/>NOT STARTED"]
        W0["Cargo workspace, CI, and quality gate<br/>NOT STARTED"]
        A0["Public builder, input, span, and sink API<br/>NOT STARTED"]
        I1["I1 Executable ASCII-literal spine<br/>HIR -> shared NFA -> database -> scan -> events<br/>NOT STARTED"]
        B0["Benchmark-adapter correctness smoke<br/>NOT STARTED"]
        E0["UC evidence summary command<br/>NOT STARTED"]
    end

    subgraph SEM[Vertically integrated semantic growth]
        I2["I2 ASCII predicates<br/>NOT STARTED"]
        I3["I3 Alternation and grouping<br/>NOT STARTED"]
        I4["I4 Repetition and longest-match semantics<br/>NOT STARTED"]
        I5["I5 UTF-16, flags, anchors, and boundaries<br/>NOT STARTED"]
        S0["Documented rmatch 2.x semantic parity gate<br/>NOT STARTED"]
    end

    subgraph OPT[Optimization and scale]
        I6["I6 Lazy deterministic-state cache<br/>NOT STARTED"]
        I7["I7 Safe start acceleration and literal prefilter<br/>NOT STARTED"]
        I8["I8 Parallel pattern partitions<br/>NOT STARTED"]
        I9["I9 Full benchmark-harness integration<br/>NOT STARTED"]
        B1["Correctness-gated cross-engine receipts<br/>and complete thread sweeps<br/>NOT STARTED"]
    end

    subgraph HARD[Hardening]
        H1["Property, fuzz, Miri, and soak evidence<br/>NOT STARTED"]
        H2["Public API and rustdoc audit<br/>NOT STARTED"]
        H3["Dependency, license, security, and MSRV audit<br/>NOT STARTED"]
        I10["I10 Hardening gate<br/>NOT STARTED"]
    end

    subgraph RELEASE[Release preparation]
        R1["Packaged-artifact semantic and consumer tests<br/>NOT STARTED"]
        R2["Exact-artifact performance campaign<br/>NOT STARTED"]
        R3["Compatibility matrix, changelog, and package review<br/>NOT STARTED"]
        I11["I11 Release gate<br/>NOT STARTED"]
        PUB["Publish crate, signed tag, and GitHub release<br/>NOT STARTED"]
    end

    P0 --> I0
    P1 --> I0
    P2 --> I0

    I0 --> F0
    I0 --> W0
    I0 --> A0
    F0 --> I1
    W0 --> I1
    A0 --> I1
    I1 --> B0
    I1 --> E0

    I1 --> I2 --> I3 --> I4 --> I5 --> S0
    F0 -. differential evidence grows with every slice .-> S0
    E0 -. every slice must expose its UC evidence .-> S0

    S0 --> I6 --> I7 --> I8 --> I9 --> B1

    B1 --> H1
    B1 --> H2
    B1 --> H3
    H1 --> I10
    H2 --> I10
    H3 --> I10

    I10 --> R1
    I10 --> R2
    I10 --> R3
    R1 --> I11
    R2 --> I11
    R3 --> I11
    I11 --> PUB

    classDef done fill:#d8f3dc,stroke:#2d6a4f,color:#081c15,stroke-width:2px;
    classDef next fill:#fff3bf,stroke:#9c6f00,color:#332400,stroke-width:3px;
    classDef active fill:#dbeafe,stroke:#1d4ed8,color:#172554,stroke-width:3px;
    classDef todo fill:#f1f3f5,stroke:#868e96,color:#212529,stroke-width:1px;

    class P0,P1,P2 done;
    class I0 next;
    class F0,W0,A0,I1,B0,E0,I2,I3,I4,I5,S0,I6,I7,I8,I9,B1,H1,H2,H3,I10,R1,R2,R3,I11,PUB todo;
```

### Legend

| Appearance | Meaning |
|---|---|
| Green, `DONE` | Durable planning artifact or implementation evidence exists |
| Yellow, `NEXT - NOT STARTED` | The next dependency gate, with no implementation claim yet |
| Blue, `IN PROGRESS` | Work and at least one reproducible evidence command exist, but the exit criteria do not yet pass |
| Gray, `NOT STARTED` | No implementation evidence has been accepted |
| Solid arrow | Required dependency or critical-path progression |
| Dotted arrow | Evidence that accumulates across several increments |

## Milestone ledger

The graph is intentionally conservative. A milestone changes to `IN PROGRESS`
only after implementation or fixture work exists and at least one named
evidence command runs. It changes to `DONE` only when its README exit criteria
and use-case evidence bundle pass from a clean checkout.

| ID | Milestone | Status | Evidence needed for `DONE` |
|---|---|---|---|
| P0 | Product requirements and scope | Done | PRD, goals, non-goals, requirements, and release criteria in README |
| P1 | Architecture and executable-spine strategy | Done | Architecture, boundaries, ADR backlog, and vertical delivery strategy in README |
| P2 | Use-case evidence contracts | Done | Evidence classes plus UC-0 through UC-12 proof requirements in README |
| I0 | Time-boxed semantic charter | Next - not started | UTF-16 and event ADRs, fixture schema, pinned Java oracle protocol, first ASCII fixture set |
| I1 | Executable ASCII-literal spine | Not started | Public end-to-end literal matcher through real HIR, dense shared NFA, database, scan loop, sink, differential fixture, and benchmark smoke |
| I2 | ASCII predicates | Not started | Public differential fixtures and exhaustive ASCII predicate tests through the same spine |
| I3 | Alternation and grouping | Not started | Public composition fixtures, epsilon-closure invariants, and bounded HIR/NFA property comparison |
| I4 | Repetition and longest match | Not started | Exact overlap/ambiguity fixtures, quantifier binding, repetition limits, and Java event equality |
| I5 | UTF-16, flags, and assertions | Not started | Full documented syntax tier, exhaustive character/context evidence, and semantic parity manifests |
| I6 | Lazy deterministic-state cache | Not started | Optimized/baseline event equality, cache-budget fallback evidence, and measured improvement |
| I7 | Safe prefilter | Not started | Prefilter on/off event equality, adversarial safety fixtures, and measured activation policy |
| I8 | Parallel partitions | Not started | Event equality across worker counts, failure/deadlock tests, resource cleanup, and complete thread sweep |
| I9 | Benchmark integration | Not started | Packaged adapter accepted by the ordinary harness with validated, reproducible receipts |
| I10 | Hardening | Not started | Property/fuzz/Miri/soak evidence, public API/rustdoc audit, and dependency/license/security/MSRV audit |
| I11 | Release preparation | Not started | Exact package passes semantic, consumer, documentation, and performance gates; compatibility matrix and changelog complete |
| Publish | First stable release | Not started | Published crate, signed tag, GitHub release, and archived release receipts |

## Status update protocol

When work begins or a gate is completed:

1. Update the status text inside the corresponding Mermaid node.
2. Change its Mermaid class to `next`, `active`, or `done` as appropriate.
3. Update the milestone ledger with a link to the durable evidence.
4. Update the summary counts at the top of this page and in the README link.
5. Commit the roadmap update with the implementation or evidence that justifies
   it, not as an unsupported progress claim.

The roadmap status is evidence-driven. Code volume, elapsed time, and a green
unit test for one disconnected layer do not by themselves move a box forward.
