//! Deterministic generated properties over the supported public lifecycle.

use rustmatch::{Error, MatcherBuilder, PatternId, Utf16Text};

const ALPHABET: &[u8] = b"abcxyz012";

#[test]
fn generated_literal_sets_match_an_independent_utf16_oracle() -> Result<(), Error> {
    let mut random = XorShift32::new(0x7a31_94d2);

    for case in 0_u32..64 {
        let pattern_count = 1 + random.bounded(8) as usize;
        let patterns = (0..pattern_count)
            .map(|_| generated_pattern(&mut random))
            .collect::<Vec<_>>();
        let input = generated_input(&mut random);
        let expected = literal_oracle(&patterns, &input);

        for worker_count in [1, 2, 4] {
            for cache_budget in [0, 1, 8_192] {
                let mut builder = MatcherBuilder::new();
                builder
                    .worker_count(worker_count)
                    .state_cache_budget(cache_budget);
                for (ordinal, pattern) in patterns.iter().enumerate() {
                    builder.add(
                        PatternId::new(
                            u32::try_from(ordinal).expect("generated pattern count fits u32"),
                        ),
                        pattern,
                    )?;
                }
                let matcher = builder.build()?;
                let text = Utf16Text::from_units(input.clone());
                let mut actual = Vec::new();
                matcher.scan(&text, |matched| {
                    let span = matched.span();
                    assert!(span.start() <= span.end());
                    assert!(span.end() <= text.as_units().len() as u64);
                    actual.push((matched.pattern_id().get(), span.start(), span.end()));
                })?;
                actual.sort_unstable();

                assert_eq!(
                    actual, expected,
                    "case={case}, workers={worker_count}, cache={cache_budget}"
                );
            }
        }
    }

    Ok(())
}

#[test]
fn supported_public_values_are_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<rustmatch::Matcher>();
    assert_send_sync::<MatcherBuilder>();
    assert_send_sync::<Utf16Text>();
    assert_send_sync::<rustmatch::Match>();
    assert_send_sync::<Error>();
}

fn generated_pattern(random: &mut XorShift32) -> String {
    let length = 1 + random.bounded(6) as usize;
    (0..length)
        .map(|_| char::from(random_symbol(random)))
        .collect()
}

fn generated_input(random: &mut XorShift32) -> Vec<u16> {
    let length = random.bounded(97) as usize;
    (0..length)
        .map(|_| {
            if random.bounded(16) == 0 {
                0xd800 + u16::try_from(random.bounded(0x800)).expect("bounded surrogate offset")
            } else {
                u16::from(random_symbol(random))
            }
        })
        .collect()
}

fn random_symbol(random: &mut XorShift32) -> u8 {
    let upper = u32::try_from(ALPHABET.len()).expect("alphabet length fits u32");
    let index = usize::try_from(random.bounded(upper)).expect("alphabet index fits usize");
    ALPHABET[index]
}

fn literal_oracle(patterns: &[String], input: &[u16]) -> Vec<(u32, u64, u64)> {
    let mut expected = Vec::new();
    for (ordinal, pattern) in patterns.iter().enumerate() {
        let units = pattern.encode_utf16().collect::<Vec<_>>();
        for start in 0..=input.len().saturating_sub(units.len()) {
            if input[start..].starts_with(&units) {
                expected.push((
                    u32::try_from(ordinal).expect("generated pattern count fits u32"),
                    u64::try_from(start).expect("generated input length fits u64"),
                    u64::try_from(start + units.len()).expect("generated input length fits u64"),
                ));
            }
        }
    }
    expected.sort_unstable();
    expected
}

struct XorShift32(u32);

impl XorShift32 {
    const fn new(seed: u32) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }

    fn bounded(&mut self, upper: u32) -> u32 {
        self.next() % upper
    }
}
