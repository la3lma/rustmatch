# Rustmatch 0.1.0 prerelease candidate evidence

This directory archives the non-publishing verification of the first public
package candidate.

## Frozen source and packages

| Item | Value |
|---|---|
| Candidate commit | `7305a24ba0f67219ea0d3026e45db5ad22d71734` |
| Accepted parent | `8522bdb7b5902682ad4dc61bb280734bfe632fc0` |
| `rustmatch-simd-0.1.0.crate` SHA-256 | `07825e3a04dbe888c6729c42be126865d9cda6aaf60d3df38e38325b588393ad` |
| `rustmatch-0.1.0.crate` SHA-256 | `cdff31c0a0ae63ff54734c03029a553b61ea373dadfdfee9feb06596fa707ceb` |
| Package rehearsal | Passed from the clean candidate commit without publishing |

The documentation and receipt archive containing this page is intentionally a
later evidence-only commit. The source and package identity under test remains
the candidate above.

## Designated-host performance

Both guards ran sequentially on `agogo.local-release-0.1.0` with no active
Docker container or GPU process. Event counts and digests matched exactly.
These are coarse release regression guards, not new optimization-admission or
cross-engine performance claims.

| Guard | Workload | Parent median | Candidate median | Delta | Result |
|---|---|---:|---:|---:|---|
| C1 | 64 literals, 1 MiB | 450,483 ns | 428,000 ns | 4.99% faster | Pass |
| C2 | 5,000 Wuthering patterns, 8 MiB, one worker | 86,762,350 ns | 85,995,097 ns | 0.88% faster | Pass |

The C2 event multiset contains 926,975 events with digest
`multiset64:a4951bf3ff11544c:fbd1551d7952f762:553c45b0cb5deffc` for both revisions.

## Receipt inventory

| Receipt | SHA-256 |
|---|---|
| [`c1/base.json`](c1/base.json) | `13867e546980980bb1024f5b8d07ceab119f713f1e536bd72ce3cdd5c57e32c8` |
| [`c1/candidate.json`](c1/candidate.json) | `3c42b463e957d9d9dec82b4630e339d63b50a639b4e2c0f2d00a86fbcc85b177` |
| [`c1/comparison.json`](c1/comparison.json) | `2f70aba290025d32e57d529a7c68fccfd27b24c573e8a136c52af8ed5a8443be` |
| [`c2/base.json`](c2/base.json) | `06b4ab54704bb9bec2740997fa4bec0d6da945de02602a8387aa4ebc6c08a8f8` |
| [`c2/candidate.json`](c2/candidate.json) | `d93227afeb67bb451ff80309a90003e353e288fccb935e8b1c97dd54817b2041` |
| [`c2/comparison.json`](c2/comparison.json) | `80427123c68d67d8c0437ec2f742635e9ca8d6d5f65fb7d4a70b76cd5bc23701` |

## GitHub verification

- [Release-readiness matrix](https://github.com/la3lma/rustmatch/actions/runs/30984431112)
- [Repository quality and hosted tripwires](https://github.com/la3lma/rustmatch/actions/runs/30984431069)
- [Draft prerelease pull request](https://github.com/la3lma/rustmatch/pull/38)

No crate, tag, or GitHub release was created during this verification.
