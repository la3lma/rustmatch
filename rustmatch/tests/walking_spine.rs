//! Public end-to-end evidence for the first executable matcher spine.

use rustmatch::{Error, Matcher, MatcherBuilder, PatternFlags, PatternId, Utf16Text};

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
fn unsupported_and_malformed_syntax_do_not_poison_the_builder() -> Result<(), Error> {
    // Prepare
    let mut builder = MatcherBuilder::new();
    let unsupported = ['^', '$'];
    let malformed = ['\\', '['];

    // Test
    let unsupported_errors: Vec<_> = unsupported
        .into_iter()
        .zip(0_u32..)
        .map(|(operator, id)| builder.add(PatternId::new(id), &format!("a{operator}")))
        .collect();
    let malformed_errors: Vec<_> = malformed
        .into_iter()
        .zip(50_u32..)
        .map(|(operator, id)| builder.add(PatternId::new(id), &format!("a{operator}")))
        .collect();
    builder.add(PatternId::new(99), "]}")?;
    builder.add(PatternId::new(100), "valid")?;
    let matcher = builder.build()?;
    let recovered_events = collect(&matcher, "]} valid")?;

    // Assert
    assert!(
        unsupported_errors
            .iter()
            .all(|result| matches!(result, Err(Error::UnsupportedPattern { .. })))
    );
    assert!(
        malformed_errors
            .iter()
            .all(|result| matches!(result, Err(Error::InvalidPattern { .. })))
    );
    assert_eq!(recovered_events, vec![(99, 0, 2), (100, 3, 8)]);
    Ok(())
}

#[test]
fn alternation_reports_the_longest_branch_at_each_start() -> Result<(), Error> {
    // Prepare
    let mut builder = MatcherBuilder::new();
    builder.add(PatternId::new(400), "cat|dog")?;
    builder.add(PatternId::new(401), "a|ab|abc")?;
    builder.add(PatternId::new(402), "ab|ac")?;
    let matcher = builder.build()?;

    // Test
    let mut events = collect(&matcher, "cat dog abc ac")?;
    events.sort_unstable();

    // Assert
    assert_eq!(
        events,
        vec![
            (400, 0, 3),
            (400, 4, 7),
            (401, 1, 2),
            (401, 8, 11),
            (401, 12, 13),
            (402, 8, 10),
            (402, 12, 14),
        ]
    );
    Ok(())
}

#[test]
fn plain_and_non_capturing_groups_compose_without_exposing_captures() -> Result<(), Error> {
    // Prepare
    let mut builder = MatcherBuilder::new();
    builder.add(PatternId::new(410), "(a|b)c")?;
    builder.add(PatternId::new(411), "(?:ab)c")?;
    builder.add(PatternId::new(412), "(a(b|c))d")?;
    let matcher = builder.build()?;

    // Test
    let mut events = collect(&matcher, "ac bc abc abd acd")?;
    events.sort_unstable();

    // Assert
    assert_eq!(
        events,
        vec![
            (410, 0, 2),
            (410, 3, 5),
            (410, 7, 9),
            (410, 14, 16),
            (411, 6, 9),
            (412, 10, 13),
            (412, 14, 17),
        ]
    );
    Ok(())
}

#[test]
fn empty_alternatives_follow_the_pinned_java_contract() -> Result<(), Error> {
    // Prepare
    let mut builder = MatcherBuilder::new();
    builder.add(PatternId::new(420), "x(|a)y")?;
    builder.add(PatternId::new(421), "x(a|)y")?;
    builder.add(PatternId::new(422), "x()y")?;
    let matcher = builder.build()?;

    // Test
    let mut events = collect(&matcher, "xy xay")?;
    events.sort_unstable();

    // Assert
    assert_eq!(events, vec![(420, 0, 2), (420, 3, 6), (421, 3, 6)]);
    Ok(())
}

#[test]
fn malformed_groups_are_rejected_without_poisoning_the_builder() -> Result<(), Error> {
    // Prepare
    let mut builder = MatcherBuilder::new();

    // Test
    let unclosed = builder.add(PatternId::new(430), "(ab");
    let unopened = builder.add(PatternId::new(431), "ab)");
    let unsupported = builder.add(PatternId::new(432), "(?=ab)");
    builder.add(PatternId::new(433), "(?:ab)")?;
    let matcher = builder.build()?;
    let recovered = collect(&matcher, "ab")?;

    // Assert
    assert!(matches!(unclosed, Err(Error::InvalidPattern { .. })));
    assert!(matches!(unopened, Err(Error::InvalidPattern { .. })));
    assert!(matches!(unsupported, Err(Error::InvalidPattern { .. })));
    assert_eq!(recovered, vec![(433, 0, 2)]);
    Ok(())
}

#[test]
fn unary_repetition_binds_one_atom_and_keeps_every_overlapping_start() -> Result<(), Error> {
    // Prepare
    let mut builder = MatcherBuilder::new();
    builder.add(PatternId::new(500), "a+")?;
    builder.add(PatternId::new(501), "ab?")?;
    builder.add(PatternId::new(502), "(ab)+")?;
    let matcher = builder.build()?;

    // Test
    let mut events = collect(&matcher, "ababab aaa")?;
    events.sort_unstable();

    // Assert
    assert_eq!(
        events,
        vec![
            (500, 0, 1),
            (500, 2, 3),
            (500, 4, 5),
            (500, 7, 10),
            (500, 8, 10),
            (500, 9, 10),
            (501, 0, 2),
            (501, 2, 4),
            (501, 4, 6),
            (501, 7, 8),
            (501, 8, 9),
            (501, 9, 10),
            (502, 0, 6),
            (502, 2, 6),
            (502, 4, 6),
        ]
    );
    Ok(())
}

#[test]
fn star_can_be_absent_or_extend_to_the_longest_end() -> Result<(), Error> {
    // Prepare
    let mut builder = MatcherBuilder::new();
    builder.add(PatternId::new(510), "ab*")?;
    let matcher = builder.build()?;

    // Test
    let events = collect(&matcher, "a abbb")?;

    // Assert
    assert_eq!(events, vec![(510, 0, 1), (510, 2, 6)]);
    Ok(())
}

#[test]
fn counted_repetition_composes_with_atoms_groups_and_following_text() -> Result<(), Error> {
    // Prepare
    let mut builder = MatcherBuilder::new();
    builder.add(PatternId::new(520), "a{2,3}")?;
    builder.add(PatternId::new(521), "(ab){2,3}")?;
    builder.add(PatternId::new(522), "a{0,2}b")?;
    let matcher = builder.build()?;

    // Test
    let mut events = collect(&matcher, "aaaa ababab aab")?;
    events.sort_unstable();

    // Assert
    assert_eq!(
        events,
        vec![
            (520, 0, 3),
            (520, 1, 4),
            (520, 2, 4),
            (520, 12, 14),
            (521, 5, 11),
            (521, 7, 11),
            (522, 5, 7),
            (522, 6, 7),
            (522, 7, 9),
            (522, 8, 9),
            (522, 9, 11),
            (522, 10, 11),
            (522, 12, 15),
            (522, 13, 15),
            (522, 14, 15),
        ]
    );
    Ok(())
}

#[test]
fn malformed_and_excessive_repetition_leave_the_builder_usable() -> Result<(), Error> {
    // Prepare
    let mut builder = MatcherBuilder::new();
    let malformed_patterns = [
        "?a", "*a", "+a", "{2}a", "a{0}", "a{2,1}", "a{x}", "a{1001}",
    ];

    // Test
    let errors = malformed_patterns
        .into_iter()
        .zip(530_u32..)
        .map(|(pattern, id)| builder.add(PatternId::new(id), pattern))
        .collect::<Vec<_>>();
    builder.add(PatternId::new(550), "a{2}")?;
    let matcher = builder.build()?;
    let recovered = collect(&matcher, "aaa")?;

    // Assert
    assert!(
        errors
            .iter()
            .all(|result| matches!(result, Err(Error::InvalidPattern { .. })))
    );
    assert_eq!(recovered, vec![(550, 0, 2), (550, 1, 3)]);
    Ok(())
}

#[test]
fn maximum_counted_repetition_runs_through_the_complete_spine() -> Result<(), Error> {
    // Prepare
    let mut builder = MatcherBuilder::new();
    builder.add(PatternId::new(560), "a{1000}")?;
    let matcher = builder.build()?;
    let input = "a".repeat(1_000);

    // Test
    let events = collect(&matcher, &input)?;

    // Assert
    assert_eq!(events, vec![(560, 0, 1_000)]);
    Ok(())
}

#[test]
fn dot_matches_every_ascii_code_unit_including_newline() -> Result<(), Error> {
    // Prepare
    let pattern_id = PatternId::new(200);
    let mut builder = MatcherBuilder::new();
    builder.add(pattern_id, ".")?;
    let matcher = builder.build()?;
    let input: String = (0_u8..=127).map(char::from).collect();
    let expected: Vec<_> = (0_u64..128)
        .map(|start| (pattern_id.get(), start, start + 1))
        .collect();

    // Test
    let events = collect(&matcher, &input)?;

    // Assert
    assert_eq!(events, expected);
    Ok(())
}

#[test]
fn dot_composes_with_literals_through_the_complete_spine() -> Result<(), Error> {
    // Prepare
    let mut builder = MatcherBuilder::new();
    builder.add(PatternId::new(201), "a.")?;
    builder.add(PatternId::new(202), ".b")?;
    let matcher = builder.build()?;

    // Test
    let mut events = collect(&matcher, "a\nb")?;
    events.sort_unstable();

    // Assert
    assert_eq!(events, vec![(201, 0, 2), (202, 1, 3)]);
    Ok(())
}

#[test]
fn ascii_classes_and_shorthands_match_their_complete_truth_tables() -> Result<(), Error> {
    // Prepare
    let patterns = [
        (300, r"\d"),
        (301, r"\D"),
        (302, r"\w"),
        (303, r"\W"),
        (304, r"\s"),
        (305, r"\S"),
        (306, "[a-cx]"),
        (307, "[^a]"),
        (308, r"[\d_]"),
        (309, r"[\t ]"),
    ];
    let mut builder = MatcherBuilder::new();
    for (id, pattern) in patterns {
        builder.add(PatternId::new(id), pattern)?;
    }
    let matcher = builder.build()?;
    let input: String = (0_u8..=127).map(char::from).collect();

    // Test
    let events = collect(&matcher, &input)?;
    let starts_for = |pattern_id| {
        events
            .iter()
            .filter(|(id, start, end)| *id == pattern_id && *end == *start + 1)
            .map(|(_, start, _)| *start)
            .collect::<Vec<_>>()
    };

    // Assert
    let digits: Vec<_> = (u64::from(b'0')..=u64::from(b'9')).collect();
    let word: Vec<_> = (0_u8..=127)
        .filter(|unit| unit.is_ascii_alphanumeric() || *unit == b'_')
        .map(u64::from)
        .collect();
    let whitespace = vec![9, 10, 11, 12, 13, 32];
    let complement = |members: &[u64]| {
        (0_u64..128)
            .filter(|unit| !members.contains(unit))
            .collect::<Vec<_>>()
    };
    assert_eq!(starts_for(300), digits);
    assert_eq!(starts_for(301), complement(&digits));
    assert_eq!(starts_for(302), word);
    assert_eq!(starts_for(303), complement(&word));
    assert_eq!(starts_for(304), whitespace);
    assert_eq!(starts_for(305), complement(&whitespace));
    assert_eq!(starts_for(306), vec![97, 98, 99, 120]);
    assert_eq!(starts_for(307), complement(&[97]));
    assert_eq!(starts_for(308), [digits, vec![95]].concat());
    assert_eq!(starts_for(309), vec![9, 32]);
    Ok(())
}

#[test]
fn literal_and_control_escapes_compose_with_plain_literals() -> Result<(), Error> {
    // Prepare
    let mut builder = MatcherBuilder::new();
    builder.add(PatternId::new(320), r"a\.b")?;
    builder.add(PatternId::new(321), r"\\")?;
    builder.add(PatternId::new(322), r"\n\t\r\f")?;
    builder.add(PatternId::new(323), r"\]}")?;
    let matcher = builder.build()?;

    // Test
    let mut events = collect(&matcher, "axb a.b\\\n\t\r\x0c]}")?;
    events.sort_unstable();

    // Assert
    assert_eq!(
        events,
        vec![(320, 4, 7), (321, 7, 8), (322, 8, 12), (323, 12, 14)]
    );
    Ok(())
}

#[test]
fn empty_and_duplicate_patterns_have_typed_errors() -> Result<(), Error> {
    // Prepare
    let empty_id = PatternId::new(1);
    let duplicate_id = PatternId::new(3);
    let mut builder = MatcherBuilder::new();

    // Test
    let empty = builder.add(empty_id, "");
    builder.add(duplicate_id, "first")?;
    let duplicate = builder.add(duplicate_id, "second");

    // Assert
    assert_eq!(
        empty,
        Err(Error::EmptyPattern {
            pattern_id: empty_id
        })
    );
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
fn utf16_literals_ranges_and_raw_surrogates_run_through_the_spine() -> Result<(), Error> {
    // Prepare
    let mut builder = MatcherBuilder::new();
    builder.add(PatternId::new(5), "snø")?;
    builder.add(PatternId::new(6), "[α-γ]")?;
    builder.add(PatternId::new(7), ".")?;
    let matcher = builder.build()?;
    let input = Utf16Text::from_units(vec![0x0073, 0x006e, 0x00f8, 0x03b2, 0xd800]);
    let mut events = Vec::new();

    // Test
    matcher.scan(&input, |event| {
        events.push((
            event.pattern_id().get(),
            event.span().start(),
            event.span().end(),
        ));
    })?;
    events.sort_unstable();

    // Assert
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
        ]
    );
    Ok(())
}

#[test]
fn inline_and_typed_case_flags_compose_through_the_complete_spine() -> Result<(), Error> {
    // Prepare
    let mut builder = MatcherBuilder::new();
    builder.add(PatternId::new(600), "(?i)cat")?;
    builder.add_with_flags(PatternId::new(601), "ω+", PatternFlags::CASE_INSENSITIVE)?;
    builder.add(PatternId::new(602), "(?i)[a-c]z")?;
    builder.add(PatternId::new(603), "(?s)a.b")?;
    let matcher = builder.build()?;

    // Test
    let mut events = collect(&matcher, "CAT cAt Ωω Az bZ a\nb")?;
    events.sort_unstable();

    // Assert
    assert_eq!(
        events,
        vec![
            (600, 0, 3),
            (600, 4, 7),
            (601, 8, 10),
            (601, 9, 10),
            (602, 11, 13),
            (602, 14, 16),
            (603, 17, 20),
        ]
    );
    Ok(())
}

#[test]
fn supplementary_character_keeps_java_utf16_coordinates() -> Result<(), Error> {
    // Prepare
    let mut builder = MatcherBuilder::new();
    builder.add(PatternId::new(610), "😀")?;
    let matcher = builder.build()?;

    // Test
    let events = collect(&matcher, "x😀y")?;

    // Assert
    assert_eq!(events, vec![(610, 1, 3)]);
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
