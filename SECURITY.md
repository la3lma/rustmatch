# Security Policy

Rustmatch is not yet released and must not be treated as production-ready.
Security reports are nevertheless welcome from the first line of code.

## Supported versions

Until the first release, only the current `main` revision is in scope. After
releases begin, this section will name the maintained release lines and their
support periods explicitly. Old development snapshots receive no security
support unless a maintainer says otherwise.

## Reporting a vulnerability

Do not report a suspected vulnerability in a public issue, discussion, pull
request, benchmark receipt, or test fixture.

Email `la3lma@gmail.com` with the subject `[rustmatch security]`. When the
repository enables GitHub private vulnerability reporting, that channel will
also be accepted and will become the preferred route.

Please include as much of the following as is safe to share:

- affected release, commit, feature, or public API;
- impact and a realistic attack or failure scenario;
- minimal reproduction steps or a proof of concept;
- platform, Rust version, features, and relevant resource limits;
- whether the report involves panic, memory safety, denial of service,
  incorrect matching, confidentiality, or dependency exposure; and
- any proposed disclosure deadline.

The maintainer aims to acknowledge a report within seven days, provide an
initial assessment within fourteen days, and agree on a disclosure plan after
the impact is understood. These are good-faith targets, not a service-level
agreement. Reporters will be credited unless they prefer anonymity.

## Scope and disclosure

Security-relevant failures may include unsafe memory behavior, crashes or
unbounded resource use on untrusted input, silent result corruption with a
security consequence, package or build compromise, and vulnerable
dependencies that are reachable in rustmatch.

Ordinary correctness bugs, unsupported syntax, and benchmark disagreements
without a security consequence belong in the issue tracker. Performance
claims require the project's normal evidence protocol and are not themselves
security reports.

The project will coordinate a fix, advisory, release, and public disclosure in
proportion to the risk. Please allow a reasonable remediation period before
publishing details, while recognizing that disclosure cannot be postponed
indefinitely.
