//! Bounded semantic evidence executed only on a big-endian target.

#![cfg(target_endian = "big")]

use std::panic::{AssertUnwindSafe, catch_unwind};

use rustmatch::{Error, MatcherBuilder, PatternFlags, PatternId, Utf16Text};

type Event = (u32, u64, u64);

#[test]
fn raw_utf16_case_folding_predicates_and_event_digest_are_exact() -> Result<(), Error> {
    println!(
        "rustmatch-target-probe target_arch={} target_endian=big target_os={}",
        std::env::consts::ARCH,
        std::env::consts::OS
    );
    assert_eq!(std::env::consts::ARCH, "s390x");

    let mut builder = MatcherBuilder::new();
    builder.add(PatternId::new(5), "snø")?;
    builder.add(PatternId::new(6), "[α-γ]")?;
    builder.add(PatternId::new(7), ".")?;
    builder.add_with_flags(PatternId::new(601), "ω+", PatternFlags::CASE_INSENSITIVE)?;
    let matcher = builder.build()?;
    let input = Utf16Text::from_units(vec![
        0x0073, 0x006e, 0x00f8, 0x03b2, 0xd800, 0x0020, 0x03a9, 0x03c9,
    ]);
    let mut events = Vec::new();

    matcher.scan(&input, |event| {
        events.push((
            event.pattern_id().get(),
            event.span().start(),
            event.span().end(),
        ));
    })?;
    events.sort_unstable();

    assert_eq!(
        events,
        vec![
            (5, 0, 3),
            (6, 3, 4),
            (7, 0, 1),
            (7, 1, 2),
            (7, 2, 3),
            (7, 3, 4),
            (7, 4, 5),
            (7, 5, 6),
            (7, 6, 7),
            (7, 7, 8),
            (601, 6, 8),
            (601, 7, 8),
        ]
    );
    assert_eq!(event_digest(&events), 0x1283_fe66_d624_e467);
    Ok(())
}

#[test]
fn callback_panic_recovery_and_builder_lifecycle_remain_exact() -> Result<(), Error> {
    let mut builder = MatcherBuilder::new();
    assert!(matches!(
        builder.add(PatternId::new(1), "["),
        Err(Error::InvalidPattern { .. })
    ));
    builder.add(PatternId::new(2), "ab")?;
    let matcher = builder.build()?;
    let input = Utf16Text::from("zab");

    let panic_result = catch_unwind(AssertUnwindSafe(|| {
        let _ = matcher.scan(&input, |_| panic!("controlled callback panic"));
    }));
    assert!(panic_result.is_err());

    for _ in 0..4 {
        let mut events = Vec::new();
        matcher.scan(&input, |event| {
            events.push((
                event.pattern_id().get(),
                event.span().start(),
                event.span().end(),
            ));
        })?;
        assert_eq!(events, vec![(2, 1, 3)]);
    }
    Ok(())
}

fn event_digest(events: &[Event]) -> u64 {
    let mut digest = 0xcbf2_9ce4_8422_2325_u64;
    for &(pattern_id, start, end) in events {
        for byte in pattern_id
            .to_le_bytes()
            .into_iter()
            .chain(start.to_le_bytes())
            .chain(end.to_le_bytes())
        {
            digest ^= u64::from(byte);
            digest = digest.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    digest
}
