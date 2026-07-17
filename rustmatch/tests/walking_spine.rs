//! Public end-to-end evidence for the first executable matcher spine.

use rustmatch::{Error, Matcher, MatcherBuilder, PatternId, Utf16Text};

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

#[test]
fn several_patterns_report_every_overlapping_start() -> Result<(), Error> {
    // Prepare
    let mut builder = MatcherBuilder::new();
    builder.add(PatternId::new(1), "a")?;
    builder.add(PatternId::new(2), "aa")?;
    let matcher = builder.build()?;

    // Test
    let mut events = collect(&matcher, "aaa")?;
    events.sort_unstable();

    // Assert
    assert_eq!(
        events,
        vec![(1, 0, 1), (1, 1, 2), (1, 2, 3), (2, 0, 2), (2, 1, 3)]
    );
    Ok(())
}

#[test]
fn equal_pattern_text_preserves_distinct_ids() -> Result<(), Error> {
    // Prepare
    let mut builder = MatcherBuilder::new();
    builder.add(PatternId::new(7), "cat")?;
    builder.add(PatternId::new(8), "cat")?;
    let matcher = builder.build()?;

    // Test
    let mut events = collect(&matcher, "cat")?;
    events.sort_unstable();

    // Assert
    assert_eq!(events, vec![(7, 0, 3), (8, 0, 3)]);
    Ok(())
}

#[test]
fn one_matcher_can_scan_empty_and_nonempty_inputs_repeatedly() -> Result<(), Error> {
    // Prepare
    let mut builder = MatcherBuilder::new();
    builder.add(PatternId::new(3), "ab")?;
    let matcher = builder.build()?;

    // Test
    let empty = collect(&matcher, "")?;
    let absent = collect(&matcher, "a")?;
    let at_end = collect(&matcher, "xxab")?;
    let repeated = collect(&matcher, "ab")?;

    // Assert
    assert!(empty.is_empty());
    assert!(absent.is_empty());
    assert_eq!(at_end, vec![(3, 2, 4)]);
    assert_eq!(repeated, vec![(3, 0, 2)]);
    Ok(())
}

#[test]
fn every_reserved_operator_is_rejected_without_poisoning_the_builder() -> Result<(), Error> {
    // Prepare
    let mut builder = MatcherBuilder::new();
    let operators = [
        '\\', '.', '^', '$', '|', '?', '*', '+', '(', ')', '[', ']', '{', '}',
    ];

    // Test
    let errors: Vec<_> = operators
        .into_iter()
        .zip(0_u32..)
        .map(|(operator, id)| builder.add(PatternId::new(id), &format!("a{operator}")))
        .collect();
    builder.add(PatternId::new(100), "valid")?;
    let matcher = builder.build()?;
    let recovered_events = collect(&matcher, "valid")?;

    // Assert
    assert!(
        errors
            .iter()
            .all(|result| matches!(result, Err(Error::UnsupportedPattern { .. })))
    );
    assert_eq!(recovered_events, vec![(100, 0, 5)]);
    Ok(())
}

#[test]
fn empty_non_ascii_and_duplicate_patterns_have_typed_errors() -> Result<(), Error> {
    // Prepare
    let empty_id = PatternId::new(1);
    let unicode_id = PatternId::new(2);
    let duplicate_id = PatternId::new(3);
    let mut builder = MatcherBuilder::new();

    // Test
    let empty = builder.add(empty_id, "");
    let non_ascii = builder.add(unicode_id, "snø");
    builder.add(duplicate_id, "first")?;
    let duplicate = builder.add(duplicate_id, "second");

    // Assert
    assert_eq!(
        empty,
        Err(Error::EmptyPattern {
            pattern_id: empty_id
        })
    );
    assert!(matches!(
        non_ascii,
        Err(Error::UnsupportedPattern {
            pattern_id,
            span,
            code_unit: 0x00f8,
        }) if pattern_id == unicode_id && span.start() == 2 && span.end() == 3
    ));
    assert_eq!(
        duplicate,
        Err(Error::DuplicatePatternId {
            pattern_id: duplicate_id
        })
    );
    Ok(())
}

#[test]
fn building_without_patterns_is_rejected() {
    // Prepare
    let builder = MatcherBuilder::new();

    // Test
    let result = builder.build();

    // Assert
    assert!(matches!(result, Err(Error::NoPatterns)));
}

#[test]
fn unsupported_input_is_rejected_before_any_callback_and_matcher_recovers() -> Result<(), Error> {
    // Prepare
    let mut builder = MatcherBuilder::new();
    builder.add(PatternId::new(5), "a")?;
    let matcher = builder.build()?;
    let invalid_input = Utf16Text::from("aøa");
    let mut callback_count = 0;

    // Test
    let invalid_result = matcher.scan(&invalid_input, |_| callback_count += 1);
    let recovered_events = collect(&matcher, "a")?;

    // Assert
    assert!(matches!(
        invalid_result,
        Err(Error::UnsupportedInput {
            position_utf16: 1,
            code_unit: 0x00f8,
        })
    ));
    assert_eq!(callback_count, 0);
    assert_eq!(recovered_events, vec![(5, 0, 1)]);
    Ok(())
}

fn collect(matcher: &Matcher, text: &str) -> Result<Vec<(u32, u64, u64)>, Error> {
    let input = Utf16Text::from(text);
    let mut events = Vec::new();
    matcher.scan(&input, |event| {
        events.push((
            event.pattern_id().get(),
            event.span().start(),
            event.span().end(),
        ));
    })?;
    Ok(events)
}
