# Project Governance

Rustmatch uses a maintainer-led model with public reasoning, reviewable changes,
and evidence-driven technical gates. The model is deliberately lightweight
while the project is small, but it is not implicit.

## Roles

- **Maintainer:** Bjorn Remseth ([@la3lma](https://github.com/la3lma)) is the
  current project maintainer and final decision maker.
- **Contributor:** anyone who reports, documents, designs, tests, reviews, or
  implements a project improvement under the contribution rules.
- **Reviewer:** a contributor trusted for a particular area or change. Review
  authority is scoped; it does not silently create general maintainer powers.

Additional maintainers may be appointed after sustained, constructive work and
demonstrated care for compatibility, evidence, users, and the community.

## Decisions

Routine decisions happen in focused issues and pull requests. The maintainer
seeks relevant input, states material tradeoffs, and records the decision in
the code, documentation, fixture, benchmark receipt, or architecture decision
record that makes it durable.

Changes to observable semantics, the public API, compatibility promises,
security posture, performance gates, or major architecture require an issue or
ADR before merge. Significant disagreement should produce a smaller experiment
or clearer evidence rather than an appeal to authority alone. The maintainer
makes the final call when consensus does not emerge and records why.

The semantic and performance gates in the README cannot be waived silently.
Any exception must be explicit, narrow, justified, and revisited.

## Roadmap and releases

Roadmap state changes only with the evidence described in
[docs/roadmap.md](docs/roadmap.md). The maintainer controls release timing,
signing, and publication, but release artifacts must satisfy the documented
gates. Urgency is not evidence.

## Conduct and conflicts

All project participation follows the [Code of Conduct](CODE_OF_CONDUCT.md).
The maintainer should disclose material conflicts of interest and avoid
reviewing their own conduct complaint. The current lack of an independent
project moderator is documented in the Code of Conduct and should be corrected
as the maintainer group grows.

## Changing governance and succession

Governance changes use a pull request with a clear rationale and a reasonable
review period. If the current maintainer steps down, they should transfer
repository, package, signing, and publication access to a trusted successor or
archive the project clearly. If the maintainer becomes unavailable, active
contributors may fork under the Apache 2.0 license; no document can guarantee
transfer of accounts that they do not control.
