use std::mem::{align_of, size_of};

use rustmatch::{Error, Match, Matcher, MatcherBuilder, PatternId, Utf16Text};

fn main() -> Result<(), Error> {
    let mut builder = MatcherBuilder::new();
    builder.add(PatternId::new(10), "cat")?;
    builder.add(PatternId::new(20), r"\bword[0-9]+\b")?;
    let matcher = builder.build()?;
    let input = Utf16Text::from("cat word42 concatenate word7");
    let mut events = Vec::new();
    matcher.scan(&input, |matched| events.push(matched))?;
    events.sort_unstable_by_key(|matched| {
        (
            matched.pattern_id().get(),
            matched.span().start(),
            matched.span().end(),
        )
    });

    println!(
        concat!(
            "{{\"matcher_builder_size\":{},\"matcher_builder_align\":{},",
            "\"matcher_size\":{},\"matcher_align\":{},",
            "\"error_size\":{},\"error_align\":{},",
            "\"event_count\":{},\"event_digest\":\"{:016x}\"}}"
        ),
        size_of::<MatcherBuilder>(),
        align_of::<MatcherBuilder>(),
        size_of::<Matcher>(),
        align_of::<Matcher>(),
        size_of::<Error>(),
        align_of::<Error>(),
        events.len(),
        event_digest(&events),
    );
    Ok(())
}

fn event_digest(events: &[Match]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for matched in events {
        for byte in matched
            .pattern_id()
            .get()
            .to_le_bytes()
            .into_iter()
            .chain(matched.span().start().to_le_bytes())
            .chain(matched.span().end().to_le_bytes())
        {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}
