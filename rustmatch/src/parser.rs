//! Parser for the currently supported 7-bit ASCII syntax.

use crate::hir::{Hir, HirAtom, HirPattern};
use crate::predicate::AsciiPredicate;
use crate::{Error, PatternId, Utf16Span};

pub(crate) fn parse(pattern_id: PatternId, source: &str) -> Result<HirPattern, Error> {
    let units: Vec<u16> = source.encode_utf16().collect();
    if units.is_empty() {
        return Err(Error::EmptyPattern { pattern_id });
    }
    for (index, &unit) in units.iter().enumerate() {
        if unit > 0x7f {
            return Err(unsupported(pattern_id, index, index + 1, unit)?);
        }
    }

    let atoms = Parser::new(pattern_id, &units).parse_sequence()?;
    Ok(HirPattern::new(
        pattern_id,
        Hir::Sequence(atoms.into_boxed_slice()),
    ))
}

struct Parser<'a> {
    pattern_id: PatternId,
    units: &'a [u16],
    index: usize,
}

impl<'a> Parser<'a> {
    const fn new(pattern_id: PatternId, units: &'a [u16]) -> Self {
        Self {
            pattern_id,
            units,
            index: 0,
        }
    }

    fn parse_sequence(mut self) -> Result<Vec<HirAtom>, Error> {
        let mut atoms = Vec::with_capacity(self.units.len());
        while let Some(&unit) = self.units.get(self.index) {
            let atom = match unit {
                value if value == u16::from(b'.') => {
                    self.index += 1;
                    HirAtom::Predicate(AsciiPredicate::any())
                }
                value if value == u16::from(b'\\') => self.parse_escape()?,
                value if value == u16::from(b'[') => self.parse_character_class()?,
                value if is_unsupported_regex_operator(value) => {
                    return Err(unsupported(
                        self.pattern_id,
                        self.index,
                        self.index + 1,
                        value,
                    )?);
                }
                value => {
                    self.index += 1;
                    HirAtom::Symbol(value)
                }
            };
            atoms.push(atom);
        }
        Ok(atoms)
    }

    fn parse_escape(&mut self) -> Result<HirAtom, Error> {
        let start = self.index;
        self.index += 1;
        let Some(&escaped) = self.units.get(self.index) else {
            return Err(invalid(self.pattern_id, start, self.index)?);
        };
        self.index += 1;

        match escaped {
            value if is_escaped_literal(value) => Ok(HirAtom::Symbol(value)),
            value if value == u16::from(b'n') => Ok(HirAtom::Symbol(u16::from(b'\n'))),
            value if value == u16::from(b't') => Ok(HirAtom::Symbol(u16::from(b'\t'))),
            value if value == u16::from(b'r') => Ok(HirAtom::Symbol(u16::from(b'\r'))),
            value if value == u16::from(b'f') => Ok(HirAtom::Symbol(0x0c)),
            value if is_shorthand(value) => Ok(HirAtom::Predicate(shorthand(value))),
            0x62 | 0x42 => Err(unsupported(self.pattern_id, start, self.index, escaped)?),
            _ => Err(invalid(self.pattern_id, start, self.index)?),
        }
    }

    fn parse_character_class(&mut self) -> Result<HirAtom, Error> {
        let class_start = self.index;
        self.index += 1;
        let inverted = self.units.get(self.index) == Some(&u16::from(b'^'));
        if inverted {
            self.index += 1;
        }

        let mut predicate = AsciiPredicate::empty();
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
                    predicate.insert(literal);
                }
                self.index += 1;
                if inverted {
                    predicate = predicate.complement();
                }
                return Ok(HirAtom::Predicate(predicate));
            }

            match self.parse_class_token()? {
                ClassToken::Literal(literal, position) => {
                    if let Some((start, range_position)) = range_start.take() {
                        if !predicate.insert_range(start, literal) {
                            return Err(invalid(self.pattern_id, range_position, self.index)?);
                        }
                    } else if let Some((previous, _)) = pending_literal.replace((literal, position))
                    {
                        predicate.insert(previous);
                    }
                }
                ClassToken::Predicate(class) => {
                    if range_start.is_some() {
                        return Err(invalid(self.pattern_id, class_start, self.index)?);
                    }
                    if let Some((literal, _)) = pending_literal.take() {
                        predicate.insert(literal);
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
}

enum ClassToken {
    Literal(u16, usize),
    Predicate(AsciiPredicate),
    RangeMarker(usize),
}

fn shorthand(unit: u16) -> AsciiPredicate {
    let (predicate, inverted) = match unit {
        0x64 => (AsciiPredicate::digit(), false),
        0x44 => (AsciiPredicate::digit(), true),
        0x77 => (AsciiPredicate::word(), false),
        0x57 => (AsciiPredicate::word(), true),
        0x73 => (AsciiPredicate::whitespace(), false),
        0x53 => (AsciiPredicate::whitespace(), true),
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
    matches!(
        unit,
        0x5e | 0x24 | 0x7c | 0x3f | 0x2a | 0x2b | 0x28 | 0x29 | 0x7b
    )
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
    use super::parse;
    use crate::hir::{Hir, HirAtom};
    use crate::predicate::AsciiPredicate;
    use crate::{Error, PatternId, Utf16Span};

    #[test]
    fn parser_rejects_operator_at_its_utf16_position() {
        // Prepare
        let pattern_id = PatternId::new(41);

        // Test
        let result = parse(pattern_id, "ab+");

        // Assert
        assert!(matches!(
            result,
            Err(Error::UnsupportedPattern {
                pattern_id: actual_id,
                span,
                code_unit
            }) if actual_id == pattern_id
                && span == Utf16Span::from_bounds(2, 3)
                && code_unit == u16::from(b'+')
        ));
    }

    #[test]
    fn parser_lowers_dot_classes_and_escapes_to_atoms() -> Result<(), Error> {
        // Prepare
        let pattern_id = PatternId::new(42);

        // Test
        let pattern = parse(pattern_id, r"a.[b-d\d]\.")?;

        // Assert
        let Hir::Sequence(atoms) = pattern.expression();
        assert_eq!(atoms.len(), 4);
        assert_eq!(atoms[0], HirAtom::Symbol(u16::from(b'a')));
        assert_eq!(atoms[1], HirAtom::Predicate(AsciiPredicate::any()));
        let HirAtom::Predicate(class) = atoms[2] else {
            panic!("character class did not lower to a predicate");
        };
        assert!(class.matches(u16::from(b'b')));
        assert!(class.matches(u16::from(b'c')));
        assert!(class.matches(u16::from(b'd')));
        assert!(class.matches(u16::from(b'7')));
        assert!(!class.matches(u16::from(b'a')));
        assert_eq!(atoms[3], HirAtom::Symbol(u16::from(b'.')));
        Ok(())
    }

    #[test]
    fn parser_reports_malformed_escape_range_and_class_spans() {
        // Prepare
        let pattern_id = PatternId::new(43);
        let patterns = [r"ab\", "[z-a]", "[abc"];

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
    fn parser_is_total_over_every_one_and_two_byte_ascii_pattern() {
        // Prepare
        let pattern_id = PatternId::new(44);
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
