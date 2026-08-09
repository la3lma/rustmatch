# Big-endian portability smoke

This non-blocking release-readiness lane executes a bounded semantic subset on
`s390x-unknown-linux-gnu` under QEMU user emulation. It is defense-in-depth
portability evidence, not native support or performance evidence.

## Frozen execution identity

- Rust target: `s390x-unknown-linux-gnu`
- Emulator path: `cross` 0.2.5 QEMU user emulation
- `cross` container: `ghcr.io/cross-rs/s390x-unknown-linux-gnu:0.2.5`
- Container index digest:
  `sha256:0d8edc92c39158abe211fddfc9617bf5b7865c2ae0f58873f9cf0c58bb315271`
- GitHub runner: pinned `ubuntu-24.04`
- Checkout and artifact actions: immutable commit SHAs in the workflow

The test log must contain `target_arch=s390x target_endian=big`. The companion
host-identity artifact deliberately records execution mode `emulated`, the
little-endian x86-64 runner host, and the declared s390x target so emulation can
never be mistaken for native execution.

## Semantic inventory

The bounded integration test covers raw UTF-16 including an isolated surrogate,
non-ASCII predicates, Java-compatible single-unit case folding, exact half-open
event spans, a frozen event digest, callback-panic recovery, and builder/scan
lifecycle reuse. A separate target test proves the AVX2 kernel refuses
execution without modifying its output. The ordinary case-fold unit suite also
byte-swaps every encoded table value and proves that the table validator rejects
the deliberate byte-order corruption.

Allocator, RSS, concurrency timing, and all performance claims are out of
scope. A failure is retained for investigation but cannot block the 0.1.0
release unless a separate support decision promotes this lane.

## Local reproduction

Install `cross` 0.2.5 and provide Docker with Linux `binfmt_misc`/QEMU support,
then run exactly one repository command:

```bash
scripts/big-endian-smoke.sh
```
