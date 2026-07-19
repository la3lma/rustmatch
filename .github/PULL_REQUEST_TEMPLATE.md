## Roadmap or use-case ID

<!-- Name the stable symbolic ID, for example I1, UC-2, or Q1. -->

## Purpose

<!-- Explain the user-visible or evidence-visible result, not only the files changed. -->

## Evidence

<!-- List commands, fixtures, retained receipts, and exact pass conditions. -->

## Performance classification

- [ ] This change does not touch a performance-sensitive path.
- [ ] This semantic change touches a performance-sensitive path and has
      external non-regression receipts.
- [ ] This is an optimization authorized by a reviewed B2-H hypothesis and has
      correctness parity plus a positive Rust result beyond the predeclared
      noise threshold against frozen current Rustmatch.

## Optimization authorization

<!-- For an optimization, name the reviewed B2-H hypothesis ID and the exact
authorization artifact. Otherwise write N/A. A competitor win or an appealing
implementation idea is not authorization to start optimization work. -->

## Review checklist

- [ ] The change has one coherent purpose.
- [ ] Tests use explicit Prepare / Test / Assert comments where useful.
- [ ] Required functional CI passes for the final revision.
- [ ] Public behavior, errors, limitations, and rustdoc are updated.
- [ ] Visibility is no broader than required.
- [ ] New dependencies, unsafe code, or lint exceptions are justified.
- [ ] Compatibility fixtures or ADRs are updated when semantics change.
- [ ] Generated and machine-readable evidence is reproducible and validated.
- [ ] The changelog is updated when users or release reviewers need the change.
