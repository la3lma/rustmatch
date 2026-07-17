//! Public end-to-end evidence for the first executable matcher spine.

use rustmatch::{MatcherBuilder, PatternId, Utf16Text};

#[test]
fn one_literal_runs_through_the_complete_spine() -> Result<(), rustmatch::Error> {
    // Prepare
    let pattern_id = PatternId::new(7);
    let mut builder = MatcherBuilder::new();
    builder.add(pattern_id, "cat")?;
    let matcher = builder.build()?;
    let input = Utf16Text::from("a cat naps");
    let mut events = Vec::new();

    // Test
    matcher.scan(&input, |matched| events.push(matched))?;

    // Assert
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].pattern_id(), pattern_id);
    assert_eq!(events[0].span().start(), 2);
    assert_eq!(events[0].span().end(), 5);
    Ok(())
}
