//! Parser for the documented UTF-16 compatibility syntax.

use crate::hir::{Hir, HirPattern};
use crate::predicate::SymbolPredicate;
use crate::{Error, PatternFlags, PatternId, Utf16Span};

const MAX_COUNTED_REPETITION: u16 = 1_000;

#[cfg(test)]
pub(crate) fn parse(pattern_id: PatternId, source: &str) -> Result<HirPattern, Error> {
    parse_with_flags(pattern_id, source, PatternFlags::NONE)
}

pub(crate) fn parse_with_flags(
    pattern_id: PatternId,
    source: &str,
    flags: PatternFlags,
) -> Result<HirPattern, Error> {
    let units: Vec<u16> = source.encode_utf16().collect();
    if units.is_empty() {
        return Err(Error::EmptyPattern { pattern_id });
    }
    let mut parser = Parser::new(pattern_id, &units, flags.is_case_insensitive());
    parser.consume_flag_prefix();
    let expression = parser.parse_alternation(false)?;
    if parser.index != units.len() {
        return Err(invalid(pattern_id, parser.index, parser.index + 1)?);
    }
    let pattern = HirPattern::new(pattern_id, expression);
    if pattern.nullable()
        && pattern.minimum_consumed() == Some(0)
        && !pattern.expression().can_consume()
    {
        return Err(invalid(pattern_id, 0, units.len())?);
    }
    Ok(pattern)
}

struct Parser<'a> {
    pattern_id: PatternId,
    units: &'a [u16],
    index: usize,
    case_insensitive: bool,
}

impl<'a> Parser<'a> {
    const fn new(pattern_id: PatternId, units: &'a [u16], case_insensitive: bool) -> Self {
        Self {
            pattern_id,
            units,
            index: 0,
            case_insensitive,
        }
    }

    fn consume_flag_prefix(&mut self) {
        if self.units.get(0..2) != Some(&[u16::from(b'('), u16::from(b'?')]) {
            return;
        }
        let mut cursor = 2;
        let mut saw_flag = false;
        let mut saw_case_insensitive = false;
        while let Some(&unit) = self.units.get(cursor) {
            match unit {
                value if value == u16::from(b'i') => {
                    saw_flag = true;
                    saw_case_insensitive = true;
                    cursor += 1;
                }
                value if value == u16::from(b's') => {
                    saw_flag = true;
                    cursor += 1;
                }
                _ => break,
            }
        }
        if saw_flag && self.units.get(cursor) == Some(&u16::from(b')')) {
            self.case_insensitive |= saw_case_insensitive;
            self.index = cursor + 1;
        }
    }

    fn parse_alternation(&mut self, in_group: bool) -> Result<Hir, Error> {
        let mut alternatives = vec![self.parse_sequence(in_group)?];
        while self.current_is(b'|') {
            self.index += 1;
            if self.index == self.units.len() || (in_group && self.current_is(b')')) {
                break;
            }
            alternatives.push(self.parse_sequence(in_group)?);
        }
        Ok(Hir::alternation(alternatives))
    }

    fn parse_sequence(&mut self, in_group: bool) -> Result<Hir, Error> {
        let mut expressions = Vec::new();
        while let Some(&unit) = self.units.get(self.index) {
            if unit == u16::from(b'|') || (in_group && unit == u16::from(b')')) {
                break;
            }
            if unit == u16::from(b')') {
                return Err(invalid(self.pattern_id, self.index, self.index + 1)?);
            }
            let atom = self.parse_atom()?;
            expressions.push(self.parse_repetition(atom)?);
        }
        Ok(Hir::sequence(expressions))
    }

    fn parse_atom(&mut self) -> Result<Hir, Error> {
        let unit = self.units[self.index];
        match unit {
            value if value == u16::from(b'.') => {
                self.index += 1;
                Ok(Hir::Predicate(SymbolPredicate::any()))
            }
            value if value == u16::from(b'\\') => self.parse_escape(),
            value if value == u16::from(b'[') => self.parse_character_class(),
            value if value == u16::from(b'(') => self.parse_group(),
            value if is_repetition_operator(value) => {
                Err(invalid(self.pattern_id, self.index, self.index + 1)?)
            }
            value if is_unsupported_regex_operator(value) => Err(unsupported(
                self.pattern_id,
                self.index,
                self.index + 1,
                value,
            )?),
            value => {
                self.index += 1;
                Ok(self.literal(value))
            }
        }
    }

    fn parse_group(&mut self) -> Result<Hir, Error> {
        let start = self.index;
        self.index += 1;
        if self.current_is(b'?') {
            self.index += 1;
            if !self.current_is(b':') {
                return Err(invalid(
                    self.pattern_id,
                    start,
                    self.index.min(self.units.len()),
                )?);
            }
            self.index += 1;
        }

        if self.current_is(b')') {
            self.index += 1;
            return Ok(Hir::Never);
        }

        let expression = self.parse_alternation(true)?;
        if !self.current_is(b')') {
            return Err(invalid(self.pattern_id, start, self.units.len())?);
        }
        self.index += 1;
        Ok(expression)
    }

    fn parse_repetition(&mut self, expression: Hir) -> Result<Hir, Error> {
        let start = self.index;
        let repeated = if self.current_is(b'?') {
            self.index += 1;
            Hir::repeat(expression, 0, Some(1))
        } else if self.current_is(b'*') {
            self.index += 1;
            Hir::repeat(expression, 0, None)
        } else if self.current_is(b'+') {
            self.index += 1;
            Hir::repeat(expression, 1, None)
        } else if self.current_is(b'{') {
            self.parse_counted_repetition(expression)?
        } else {
            return Ok(expression);
        };

        if self
            .units
            .get(self.index)
            .is_some_and(|&unit| is_repetition_operator(unit))
        {
            return Err(invalid(self.pattern_id, start, self.index + 1)?);
        }
        Ok(repeated)
    }

    fn parse_counted_repetition(&mut self, expression: Hir) -> Result<Hir, Error> {
        let start = self.index;
        self.index += 1;
        let min = self.parse_count(start)?;
        let max = if self.current_is(b'}') {
            self.index += 1;
            Some(min)
        } else if self.current_is(b',') {
            self.index += 1;
            if self.current_is(b'}') {
                self.index += 1;
                None
            } else {
                let max = self.parse_count(start)?;
                if !self.current_is(b'}') {
                    return Err(invalid(
                        self.pattern_id,
                        start,
                        self.index.min(self.units.len()),
                    )?);
                }
                self.index += 1;
                Some(max)
            }
        } else {
            return Err(invalid(
                self.pattern_id,
                start,
                self.index.min(self.units.len()),
            )?);
        };

        if max.is_some_and(|limit| limit < min)
            || max == Some(0)
            || min > MAX_COUNTED_REPETITION
            || max.is_some_and(|limit| limit > MAX_COUNTED_REPETITION)
        {
            return Err(invalid(self.pattern_id, start, self.index)?);
        }
        Ok(Hir::repeat(expression, min, max))
    }

    fn parse_count(&mut self, quantifier_start: usize) -> Result<u16, Error> {
        let number_start = self.index;
        let mut value = 0_u32;
        while let Some(&unit) = self.units.get(self.index) {
            if !(u16::from(b'0')..=u16::from(b'9')).contains(&unit) {
                break;
            }
            let digit = u32::from(unit - u16::from(b'0'));
            value = value
                .saturating_mul(10)
                .saturating_add(digit)
                .min(u32::from(MAX_COUNTED_REPETITION) + 1);
            self.index += 1;
        }
        if self.index == number_start || value > u32::from(MAX_COUNTED_REPETITION) {
            return Err(invalid(
                self.pattern_id,
                quantifier_start,
                self.index.max(number_start + 1).min(self.units.len()),
            )?);
        }
        Ok(u16::try_from(value).expect("accepted repetition count fits in u16"))
    }

    fn parse_escape(&mut self) -> Result<Hir, Error> {
        let start = self.index;
        self.index += 1;
        let Some(&escaped) = self.units.get(self.index) else {
            return Err(invalid(self.pattern_id, start, self.index)?);
        };
        self.index += 1;

        match escaped {
            value if is_escaped_literal(value) => Ok(self.literal(value)),
            value if value == u16::from(b'n') => Ok(Hir::Symbol(u16::from(b'\n'))),
            value if value == u16::from(b't') => Ok(Hir::Symbol(u16::from(b'\t'))),
            value if value == u16::from(b'r') => Ok(Hir::Symbol(u16::from(b'\r'))),
            value if value == u16::from(b'f') => Ok(Hir::Symbol(0x0c)),
            value if is_shorthand(value) => Ok(Hir::Predicate(shorthand(value))),
            0x62 | 0x42 => Err(unsupported(self.pattern_id, start, self.index, escaped)?),
            _ => Err(invalid(self.pattern_id, start, self.index)?),
        }
    }

    fn parse_character_class(&mut self) -> Result<Hir, Error> {
        let class_start = self.index;
        self.index += 1;
        let inverted = self.units.get(self.index) == Some(&u16::from(b'^'));
        if inverted {
            self.index += 1;
        }

        let mut predicate = SymbolPredicate::empty();
        let mut pending_literal: Option<(u16, usize)> = None;
        let mut range_start: Option<(u16, usize)> = None;

        loop {
            let Some(&unit) = self.units.get(self.index) else {
                return Err(invalid(self.pattern_id, class_start, self.index)?);
            };
            if unit == u16::from(b']') {
                if range_start.is_some() {
                    return Err(invalid(self.pattern_id, class_start, self.index + 1)?);
                }
                if let Some((literal, _)) = pending_literal {
                    self.insert_class_literal(&mut predicate, literal);
                }
                self.index += 1;
                if inverted {
                    predicate = predicate.complement();
                }
                return Ok(Hir::Predicate(predicate));
            }

            match self.parse_class_token()? {
                ClassToken::Literal(literal, position) => {
                    if let Some((start, range_position)) = range_start.take() {
                        if !self.insert_class_range(&mut predicate, start, literal) {
                            return Err(invalid(self.pattern_id, range_position, self.index)?);
                        }
                    } else if let Some((previous, _)) = pending_literal.replace((literal, position))
                    {
                        self.insert_class_literal(&mut predicate, previous);
                    }
                }
                ClassToken::Predicate(class) => {
                    if range_start.is_some() {
                        return Err(invalid(self.pattern_id, class_start, self.index)?);
                    }
                    if let Some((literal, _)) = pending_literal.take() {
                        self.insert_class_literal(&mut predicate, literal);
                    }
                    predicate = predicate.union(class);
                }
                ClassToken::RangeMarker(position) => {
                    let Some((literal, literal_position)) = pending_literal.take() else {
                        return Err(invalid(self.pattern_id, position, self.index)?);
                    };
                    range_start = Some((literal, literal_position));
                }
            }
        }
    }

    fn parse_class_token(&mut self) -> Result<ClassToken, Error> {
        let start = self.index;
        let unit = self.units[self.index];
        self.index += 1;
        if unit == u16::from(b'-') {
            return Ok(ClassToken::RangeMarker(start));
        }
        if unit != u16::from(b'\\') {
            return Ok(ClassToken::Literal(unit, start));
        }

        let Some(&escaped) = self.units.get(self.index) else {
            return Err(invalid(self.pattern_id, start, self.index)?);
        };
        self.index += 1;
        match escaped {
            0x5c | 0x5d | 0x5b | 0x2d | 0x5e => Ok(ClassToken::Literal(escaped, start)),
            value if value == u16::from(b'n') => Ok(ClassToken::Literal(u16::from(b'\n'), start)),
            value if value == u16::from(b't') => Ok(ClassToken::Literal(u16::from(b'\t'), start)),
            value if value == u16::from(b'r') => Ok(ClassToken::Literal(u16::from(b'\r'), start)),
            value if value == u16::from(b'f') => Ok(ClassToken::Literal(0x0c, start)),
            value if matches!(value, 0x64 | 0x77 | 0x73) => {
                Ok(ClassToken::Predicate(shorthand(value)))
            }
            _ => Err(invalid(self.pattern_id, start, self.index)?),
        }
    }

    fn current_is(&self, expected: u8) -> bool {
        self.units.get(self.index) == Some(&u16::from(expected))
    }

    fn literal(&self, symbol: u16) -> Hir {
        if !self.case_insensitive {
            return Hir::Symbol(symbol);
        }
        let (lower, upper) = crate::case_fold::lower_upper(symbol);
        if lower == upper {
            Hir::Symbol(symbol)
        } else {
            let mut predicate = SymbolPredicate::empty();
            predicate.insert(lower);
            predicate.insert(upper);
            Hir::Predicate(predicate)
        }
    }

    fn insert_class_literal(&self, predicate: &mut SymbolPredicate, symbol: u16) {
        predicate.insert(symbol);
        if self.case_insensitive {
            let (lower, upper) = crate::case_fold::lower_upper(symbol);
            predicate.insert(lower);
            predicate.insert(upper);
        }
    }

    fn insert_class_range(&self, predicate: &mut SymbolPredicate, start: u16, end: u16) -> bool {
        if !predicate.insert_range(start, end) {
            return false;
        }
        if self.case_insensitive {
            let (start_lower, start_upper) = crate::case_fold::lower_upper(start);
            let (end_lower, end_upper) = crate::case_fold::lower_upper(end);
            if start_lower != start_upper && end_lower != end_upper {
                if (start, end) != (start_lower, end_lower)
                    && !predicate.insert_range(start_lower, end_lower)
                {
                    return false;
                }
                if (start, end) != (start_upper, end_upper)
                    && !predicate.insert_range(start_upper, end_upper)
                {
                    return false;
                }
            }
        }
        true
    }
}

enum ClassToken {
    Literal(u16, usize),
    Predicate(SymbolPredicate),
    RangeMarker(usize),
}

fn shorthand(unit: u16) -> SymbolPredicate {
    let (predicate, inverted) = match unit {
        0x64 => (SymbolPredicate::digit(), false),
        0x44 => (SymbolPredicate::digit(), true),
        0x77 => (SymbolPredicate::word(), false),
        0x57 => (SymbolPredicate::word(), true),
        0x73 => (SymbolPredicate::whitespace(), false),
        0x53 => (SymbolPredicate::whitespace(), true),
        _ => unreachable!("caller admits only shorthand escapes"),
    };
    if inverted {
        predicate.complement()
    } else {
        predicate
    }
}

fn unsupported(
    pattern_id: PatternId,
    start: usize,
    end: usize,
    code_unit: u16,
) -> Result<Error, Error> {
    Ok(Error::UnsupportedPattern {
        pattern_id,
        span: span(pattern_id, start, end)?,
        code_unit,
    })
}

fn invalid(pattern_id: PatternId, start: usize, end: usize) -> Result<Error, Error> {
    Ok(Error::InvalidPattern {
        pattern_id,
        span: span(pattern_id, start, end)?,
    })
}

fn span(pattern_id: PatternId, start: usize, end: usize) -> Result<Utf16Span, Error> {
    Ok(Utf16Span::from_bounds(
        position(pattern_id, start)?,
        position(pattern_id, end)?,
    ))
}

fn position(pattern_id: PatternId, index: usize) -> Result<u64, Error> {
    u64::try_from(index).map_err(|_| Error::PatternTooLarge { pattern_id })
}

const fn is_unsupported_regex_operator(unit: u16) -> bool {
    matches!(unit, 0x5e | 0x24)
}

const fn is_repetition_operator(unit: u16) -> bool {
    matches!(unit, 0x3f | 0x2a | 0x2b | 0x7b)
}

const fn is_escaped_literal(unit: u16) -> bool {
    matches!(
        unit,
        0x5c | 0x2e
            | 0x2a
            | 0x2b
            | 0x3f
            | 0x5b
            | 0x5d
            | 0x28
            | 0x29
            | 0x7c
            | 0x5e
            | 0x24
            | 0x2d
            | 0x7b
            | 0x7d
    )
}

const fn is_shorthand(unit: u16) -> bool {
    matches!(unit, 0x64 | 0x44 | 0x77 | 0x57 | 0x73 | 0x53)
}

#[cfg(test)]
mod tests {
    use super::{parse, parse_with_flags};
    use crate::hir::Hir;
    use crate::predicate::SymbolPredicate;
    use crate::{Error, PatternFlags, PatternId, Utf16Span};

    #[test]
    fn prefix_and_typed_case_flags_lower_literals_to_java_predicates() -> Result<(), Error> {
        // Prepare
        let pattern_id = PatternId::new(49);

        // Test
        let inline = parse(pattern_id, "(?is)aΩ")?;
        let typed = parse_with_flags(pattern_id, "aΩ", PatternFlags::CASE_INSENSITIVE)?;
        let misplaced = parse(pattern_id, "a(?i)Ω");

        // Assert
        assert_eq!(inline.expression(), typed.expression());
        let Hir::Sequence(expressions) = inline.expression() else {
            panic!("folded literals did not lower to a sequence");
        };
        assert_eq!(expressions.len(), 2);
        assert!(
            expressions
                .iter()
                .all(|item| matches!(item, Hir::Predicate(_)))
        );
        assert!(matches!(misplaced, Err(Error::InvalidPattern { .. })));
        Ok(())
    }

    #[test]
    fn parser_rejects_unsupported_anchor_at_its_utf16_position() {
        // Prepare
        let pattern_id = PatternId::new(41);

        // Test
        let result = parse(pattern_id, "ab^");

        // Assert
        assert!(matches!(
            result,
            Err(Error::UnsupportedPattern {
                pattern_id: actual_id,
                span,
                code_unit
            }) if actual_id == pattern_id
                && span == Utf16Span::from_bounds(2, 3)
                && code_unit == u16::from(b'^')
        ));
    }

    #[test]
    fn parser_binds_repetition_to_one_preceding_expression() -> Result<(), Error> {
        // Prepare
        let pattern_id = PatternId::new(47);

        // Test
        let pattern = parse(pattern_id, "ab?(cd){2,3}e+")?;

        // Assert
        let Hir::Sequence(sequence) = pattern.expression() else {
            panic!("repeated pattern did not lower to a sequence");
        };
        assert_eq!(sequence.len(), 4);
        assert_eq!(sequence[0], Hir::Symbol(u16::from(b'a')));
        assert!(matches!(
            sequence[1],
            Hir::Repeat {
                min: 0,
                max: Some(1),
                ..
            }
        ));
        assert!(matches!(
            sequence[2],
            Hir::Repeat {
                min: 2,
                max: Some(3),
                ..
            }
        ));
        assert!(matches!(
            sequence[3],
            Hir::Repeat {
                min: 1,
                max: None,
                ..
            }
        ));
        assert_eq!(pattern.minimum_consumed(), Some(6));
        Ok(())
    }

    #[test]
    fn parser_enforces_counted_repetition_boundaries() {
        // Prepare
        let pattern_id = PatternId::new(48);
        let invalid = [
            "{2}a", "a{0}", "a{0,0}", "a{2,1}", "a{,2}", "a{2", "a{x}", "a{1001}", "a**", "a+?",
        ];

        // Test
        let invalid_results = invalid.map(|pattern| parse(pattern_id, pattern));
        let exact_limit = parse(pattern_id, "a{1000}");
        let open_zero = parse(pattern_id, "a{0,}");

        // Assert
        assert!(
            invalid_results
                .iter()
                .all(|result| matches!(result, Err(Error::InvalidPattern { .. })))
        );
        assert!(exact_limit.is_ok());
        assert!(open_zero.is_ok());
    }

    #[test]
    fn parser_lowers_dot_classes_and_escapes_to_expressions() -> Result<(), Error> {
        // Prepare
        let pattern_id = PatternId::new(42);

        // Test
        let pattern = parse(pattern_id, r"a.[b-d\d]\.")?;

        // Assert
        let Hir::Sequence(expressions) = pattern.expression() else {
            panic!("pattern did not lower to a sequence");
        };
        assert_eq!(expressions.len(), 4);
        assert_eq!(expressions[0], Hir::Symbol(u16::from(b'a')));
        assert_eq!(expressions[1], Hir::Predicate(SymbolPredicate::any()));
        let Hir::Predicate(class) = &expressions[2] else {
            panic!("character class did not lower to a predicate");
        };
        assert!(class.matches(u16::from(b'b')));
        assert!(class.matches(u16::from(b'c')));
        assert!(class.matches(u16::from(b'd')));
        assert!(class.matches(u16::from(b'7')));
        assert!(!class.matches(u16::from(b'a')));
        assert_eq!(expressions[3], Hir::Symbol(u16::from(b'.')));
        assert!(!pattern.nullable());
        assert_eq!(pattern.minimum_consumed(), Some(4));
        Ok(())
    }

    #[test]
    fn parser_flattens_nested_composition_and_tracks_empty_branches() -> Result<(), Error> {
        // Prepare
        let pattern_id = PatternId::new(43);

        // Test
        let pattern = parse(pattern_id, "x(|(a|b))y")?;

        // Assert
        let Hir::Sequence(sequence) = pattern.expression() else {
            panic!("outer sequence was not retained");
        };
        assert_eq!(sequence.len(), 3);
        let Hir::Alternation(branches) = &sequence[1] else {
            panic!("nested alternation was not retained");
        };
        assert_eq!(branches.len(), 3);
        assert!(!pattern.nullable());
        assert_eq!(pattern.minimum_consumed(), Some(2));
        Ok(())
    }

    #[test]
    fn parser_reports_malformed_escape_range_class_and_group_spans() {
        // Prepare
        let pattern_id = PatternId::new(44);
        let patterns = [r"ab\", "[z-a]", "[abc", "(ab", "ab)", "(?=ab)"];

        // Test
        let errors: Vec<_> = patterns
            .into_iter()
            .map(|pattern| parse(pattern_id, pattern))
            .collect();

        // Assert
        assert!(errors.iter().all(|result| matches!(
            result,
            Err(Error::InvalidPattern {
                pattern_id: actual_id,
                ..
            }) if *actual_id == pattern_id
        )));
    }

    #[test]
    fn parser_rejects_pure_zero_width_composition() {
        // Prepare
        let pattern_id = PatternId::new(45);

        // Test
        let results = ["|", "||", "(|)", "|[]"].map(|pattern| parse(pattern_id, pattern));

        // Assert
        assert!(
            results
                .iter()
                .all(|result| matches!(result, Err(Error::InvalidPattern { .. })))
        );
    }

    #[test]
    fn parser_is_total_over_every_one_and_two_byte_ascii_pattern() {
        // Prepare
        let pattern_id = PatternId::new(46);
        let symbols: Vec<char> = (0_u8..=127).map(char::from).collect();

        // Test
        for &first in &symbols {
            let _ = parse(pattern_id, &first.to_string());
            for &second in &symbols {
                let pattern = [first, second].iter().collect::<String>();
                let _ = parse(pattern_id, &pattern);
            }
        }

        // Assert
        // Reaching this point proves that bounded arbitrary ASCII syntax returns
        // a value or typed error instead of panicking.
    }
}
