//! First-slice parser that admits only non-empty 7-bit ASCII literals.

use crate::hir::{Hir, HirPattern};
use crate::{Error, PatternId, Utf16Span};

pub(crate) fn parse(pattern_id: PatternId, source: &str) -> Result<HirPattern, Error> {
    let units: Vec<u16> = source.encode_utf16().collect();
    if units.is_empty() {
        return Err(Error::EmptyPattern { pattern_id });
    }

    for (index, &unit) in units.iter().enumerate() {
        if unit > 0x7f || is_regex_operator(unit) {
            let start = position(pattern_id, index)?;
            return Err(Error::UnsupportedPattern {
                pattern_id,
                span: Utf16Span::from_bounds(start, start + 1),
                code_unit: unit,
            });
        }
    }

    let end = position(pattern_id, units.len())?;
    Ok(HirPattern::new(
        pattern_id,
        Hir::Literal(units.into_boxed_slice()),
        Utf16Span::from_bounds(0, end),
    ))
}

fn position(pattern_id: PatternId, index: usize) -> Result<u64, Error> {
    u64::try_from(index).map_err(|_| Error::PatternTooLarge { pattern_id })
}

const fn is_regex_operator(unit: u16) -> bool {
    matches!(
        unit,
        0x5c | 0x2e
            | 0x5e
            | 0x24
            | 0x7c
            | 0x3f
            | 0x2a
            | 0x2b
            | 0x28
            | 0x29
            | 0x5b
            | 0x5d
            | 0x7b
            | 0x7d
    )
}

#[cfg(test)]
mod tests {
    use super::parse;
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
}
