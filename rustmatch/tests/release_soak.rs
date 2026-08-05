//! Bounded release soak for matcher reuse and scan-local resource accounting.

#![cfg(feature = "benchmark-internals")]

use rustmatch::{Error, Match, MatcherBuilder, PatternId, Utf16Text};

#[test]
#[ignore = "bounded release soak; run through scripts/release-readiness.sh"]
fn repeated_scans_preserve_results_and_cache_bounds() -> Result<(), Error> {
    let patterns = (0_u32..64)
        .map(|ordinal| format!("needle{ordinal:03}"))
        .collect::<Vec<_>>();
    let line = patterns.join(" padding ");
    let input = Utf16Text::from(format!("{line}\n").repeat(128));
    let mut reference = None;

    for worker_count in [1, 4] {
        for cache_budget in [0, 8_192] {
            let mut builder = MatcherBuilder::new();
            builder
                .worker_count(worker_count)
                .state_cache_budget(cache_budget);
            for (ordinal, pattern) in patterns.iter().enumerate() {
                builder.add(
                    PatternId::new(u32::try_from(ordinal).expect("fixture ordinal fits u32")),
                    pattern,
                )?;
            }
            let matcher = builder.build()?;

            for iteration in 0..24 {
                let mut digest = EventDigest::default();
                let diagnostics = matcher.scan_with_diagnostics(&input, |matched| {
                    digest.record(matched);
                })?;

                assert!(diagnostics.cache_states() <= cache_budget);
                if cache_budget == 0 {
                    assert_eq!(diagnostics.cache_states(), 0);
                    assert_eq!(diagnostics.cache_table_bytes(), 0);
                }
                assert!(diagnostics.database_retained_bytes() > 0);
                assert!(diagnostics.prefilter_retained_bytes() > 0);

                match reference {
                    Some(expected) => assert_eq!(
                        digest, expected,
                        "workers={worker_count}, cache={cache_budget}, iteration={iteration}"
                    ),
                    None => reference = Some(digest),
                }
            }
        }
    }

    Ok(())
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct EventDigest {
    count: u64,
    xor: u64,
    sum: u64,
}

impl EventDigest {
    fn record(&mut self, matched: Match) {
        let span = matched.span();
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        for value in [
            u64::from(matched.pattern_id().get()),
            span.start(),
            span.end(),
        ] {
            hash ^= value;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        self.count = self.count.saturating_add(1);
        self.xor ^= hash;
        self.sum = self.sum.wrapping_add(hash);
    }
}
