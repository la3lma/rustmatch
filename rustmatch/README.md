# rustmatch

`rustmatch` is a Rust-native many-pattern regular-expression matcher. It
registers caller-identified patterns once, compiles them into shared matching
machinery, and reuses the immutable matcher across UTF-16 inputs.

```rust
use rustmatch::{MatcherBuilder, PatternId, Utf16Text};

let mut builder = MatcherBuilder::new();
builder.add(PatternId::new(1), "cat")?;
builder.add(PatternId::new(2), "dog")?;
let matcher = builder.build()?;
let input = Utf16Text::from("cat and dog");
let mut matches = Vec::new();

matcher.scan(&input, |matched| matches.push(matched))?;

assert_eq!(matches.len(), 2);
# Ok::<(), rustmatch::Error>(())
```

## Compatibility contract

The canonical API accepts UTF-16 code units and reports half-open UTF-16
offsets. It implements the documented consuming language and longest-match
event semantics of Java rmatch `2.0.0-RC1`, including overlapping starts.
Callback order is intentionally unspecified.

The supported pattern language includes literals, dot, character classes,
ASCII shorthand classes, alternation, grouping, greedy repetition, prefix or
typed case-insensitive flags, line anchors, and ASCII word boundaries. Counted
repetition is capped at 1,000 and group nesting is bounded explicitly.

## Status and limitations

The `0.1.x` line is pre-1.0. Public APIs follow Cargo's pre-1.0 SemVer
convention, but a future `0.2.0` may contain breaking changes.

- UTF-8 convenience input is not yet a compatibility API; offsets are UTF-16.
- Captures, look-around, backreferences, and lazy quantifiers are unsupported.
- `benchmark-internals` exposes unsupported diagnostics for repository tooling
  and is not intended for applications.
- AVX2 acceleration is detected at runtime on x86-64. Other targets use the
  exact portable path.

## Explicit experimental matcher

The non-default `unstable-assertion-prefix-v1` feature exposes a separate
matcher for narrowly eligible, assertion-bearing pattern sets with one proven
five-unit ASCII prefix. It is not selected automatically and does not alter
`MatcherBuilder` or `Matcher` merely because the feature is enabled.

Applications must opt in through both Cargo and the versioned experimental
module, then choose `RequireSpecialized` or `AllowExactFallback` for every
scan. The API is exact but unstable: it may change or disappear in a future
`0.x` release and should be selected only after workload-specific validation.
See the module documentation and the repository's retained H43 owner-exception
evidence before adopting it.

See the [repository README](https://github.com/la3lma/rustmatch) for the full
semantic contract, benchmark interpretation, and contribution policies.

Licensed under Apache-2.0.
