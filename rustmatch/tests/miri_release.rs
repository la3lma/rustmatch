//! Focused Miri coverage for the supported public matcher lifecycle.

use std::panic::{AssertUnwindSafe, catch_unwind};

use rustmatch::{Error, MatcherBuilder, PatternId, Utf16Text};

#[test]
fn miri_release_public_lifecycle() -> Result<(), Error> {
    let mut builder = MatcherBuilder::new();
    builder.worker_count(2).state_cache_budget(1);
    builder.add(PatternId::new(1), "a+")?;
    builder.add(PatternId::new(2), "cat")?;
    let matcher = builder.build()?;
    let input = Utf16Text::from_units(vec![0x61, 0x61, 0xd800, 0x63, 0x61, 0x74]);

    let callback_panic = catch_unwind(AssertUnwindSafe(|| {
        let _ = matcher.scan(&input, |_| panic!("controlled callback panic"));
    }));
    assert!(callback_panic.is_err());

    for _ in 0..2 {
        let mut events = Vec::new();
        matcher.scan(&input, |matched| {
            let span = matched.span();
            events.push((matched.pattern_id().get(), span.start(), span.end()));
        })?;
        events.sort_unstable();
        assert_eq!(events, [(1, 0, 2), (1, 1, 2), (1, 4, 5), (2, 3, 6)]);
    }

    Ok(())
}
