//! Predicates over the complete UTF-16 code-unit domain.

#[cfg(feature = "benchmark-internals")]
use std::mem::size_of;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) struct SymbolPredicate {
    ascii: u128,
    non_ascii_ranges: Vec<(u16, u16)>,
}

impl SymbolPredicate {
    pub(crate) const fn empty() -> Self {
        Self {
            ascii: 0,
            non_ascii_ranges: Vec::new(),
        }
    }

    pub(crate) fn any() -> Self {
        Self {
            ascii: u128::MAX,
            non_ascii_ranges: vec![(128, u16::MAX)],
        }
    }

    pub(crate) fn digit() -> Self {
        Self::from_ranges(&[(u16::from(b'0'), u16::from(b'9'))])
    }

    pub(crate) fn word() -> Self {
        let mut predicate = Self::from_ranges(&[
            (u16::from(b'a'), u16::from(b'z')),
            (u16::from(b'A'), u16::from(b'Z')),
            (u16::from(b'0'), u16::from(b'9')),
        ]);
        predicate.insert(u16::from(b'_'));
        predicate
    }

    pub(crate) fn whitespace() -> Self {
        let mut predicate = Self::empty();
        for symbol in [b' ', b'\t', b'\n', 0x0b, 0x0c, b'\r'] {
            predicate.insert(u16::from(symbol));
        }
        predicate
    }

    pub(crate) fn insert(&mut self, symbol: u16) {
        let inserted = self.insert_range(symbol, symbol);
        debug_assert!(inserted);
    }

    pub(crate) fn insert_range(&mut self, start: u16, end: u16) -> bool {
        if start > end {
            return false;
        }
        if start < 128 {
            let ascii_end = end.min(127);
            self.ascii |= ascii_range_mask(start, ascii_end);
        }
        if end >= 128 {
            self.non_ascii_ranges.push((start.max(128), end));
            normalize_ranges(&mut self.non_ascii_ranges);
        }
        true
    }

    pub(crate) fn union(mut self, other: Self) -> Self {
        self.ascii |= other.ascii;
        self.non_ascii_ranges.extend(other.non_ascii_ranges);
        normalize_ranges(&mut self.non_ascii_ranges);
        self
    }

    pub(crate) fn complement(&self) -> Self {
        let mut complement = Self {
            ascii: !self.ascii,
            non_ascii_ranges: Vec::new(),
        };
        let mut next = 128_u32;
        for &(start, end) in &self.non_ascii_ranges {
            let start = u32::from(start);
            if next < start {
                complement.non_ascii_ranges.push((
                    u16::try_from(next).expect("UTF-16 range start"),
                    u16::try_from(start - 1).expect("UTF-16 range end"),
                ));
            }
            next = u32::from(end) + 1;
        }
        if let Ok(range_start) = u16::try_from(next) {
            complement.non_ascii_ranges.push((range_start, u16::MAX));
        }
        complement
    }

    pub(crate) fn matches(&self, symbol: u16) -> bool {
        if symbol < 128 {
            return self.ascii & (1_u128 << symbol) != 0;
        }
        let index = self
            .non_ascii_ranges
            .partition_point(|&(_, end)| end < symbol);
        self.non_ascii_ranges
            .get(index)
            .is_some_and(|&(start, end)| start <= symbol && symbol <= end)
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.ascii == 0 && self.non_ascii_ranges.is_empty()
    }

    #[cfg(feature = "benchmark-internals")]
    pub(crate) fn retained_bytes(&self) -> usize {
        size_of::<Self>().saturating_add(
            self.non_ascii_ranges
                .capacity()
                .saturating_mul(size_of::<(u16, u16)>()),
        )
    }

    fn from_ranges(ranges: &[(u16, u16)]) -> Self {
        let mut predicate = Self::empty();
        for &(start, end) in ranges {
            let inserted = predicate.insert_range(start, end);
            debug_assert!(inserted);
        }
        predicate
    }
}

fn ascii_range_mask(start: u16, end: u16) -> u128 {
    debug_assert!(start <= end && end < 128);
    let below_end = if end == 127 {
        u128::MAX
    } else {
        (1_u128 << (end + 1)) - 1
    };
    (u128::MAX << start) & below_end
}

fn normalize_ranges(ranges: &mut Vec<(u16, u16)>) {
    ranges.sort_unstable();
    let mut merged: Vec<(u16, u16)> = Vec::with_capacity(ranges.len());
    for (start, end) in ranges.drain(..) {
        if let Some((_, previous_end)) = merged.last_mut() {
            if start <= previous_end.saturating_add(1) {
                *previous_end = (*previous_end).max(end);
                continue;
            }
        }
        merged.push((start, end));
    }
    *ranges = merged;
}

#[cfg(test)]
mod tests {
    use super::SymbolPredicate;

    #[test]
    fn any_matches_the_complete_utf16_domain() {
        // Prepare
        let predicate = SymbolPredicate::any();

        // Test
        let unmatched = (0..=u16::MAX).find(|&symbol| !predicate.matches(symbol));

        // Assert
        assert_eq!(unmatched, None);
    }

    #[test]
    fn shorthand_predicates_keep_the_documented_ascii_members() {
        // Prepare
        let digit = SymbolPredicate::digit();
        let word = SymbolPredicate::word();
        let whitespace = SymbolPredicate::whitespace();

        // Test
        let actual_digits: Vec<_> = (0..=u16::MAX).filter(|&unit| digit.matches(unit)).collect();
        let actual_word: Vec<_> = (0..=u16::MAX).filter(|&unit| word.matches(unit)).collect();
        let actual_whitespace: Vec<_> = (0..=u16::MAX)
            .filter(|&unit| whitespace.matches(unit))
            .collect();

        // Assert
        assert_eq!(
            actual_digits,
            (u16::from(b'0')..=u16::from(b'9')).collect::<Vec<_>>()
        );
        assert_eq!(actual_word.len(), 63);
        assert!(actual_word.contains(&u16::from(b'_')));
        assert_eq!(actual_whitespace, vec![9, 10, 11, 12, 13, 32]);
    }

    #[test]
    fn ranges_union_and_complement_cover_ascii_and_non_ascii() {
        // Prepare
        let mut range = SymbolPredicate::empty();
        let inserted_ascii = range.insert_range(u16::from(b'a'), u16::from(b'c'));
        let inserted_non_ascii = range.insert_range(0x03b1, 0x03b3);
        let with_digit = range.union(SymbolPredicate::digit());

        // Test
        let complement = with_digit.complement();

        // Assert
        assert!(inserted_ascii);
        assert!(inserted_non_ascii);
        assert!(
            !with_digit
                .clone()
                .insert_range(u16::from(b'z'), u16::from(b'a'))
        );
        assert!(with_digit.matches(u16::from(b'b')));
        assert!(with_digit.matches(0x03b2));
        assert!(!complement.matches(u16::from(b'b')));
        assert!(!complement.matches(0x03b2));
        assert!(complement.matches(u16::from(b'!')));
        assert!(complement.matches(0xd800));
        assert!(complement.matches(u16::MAX));
    }

    #[test]
    fn overlapping_and_adjacent_ranges_are_normalized() {
        // Prepare
        let mut predicate = SymbolPredicate::empty();

        // Test
        assert!(predicate.insert_range(200, 210));
        assert!(predicate.insert_range(190, 199));
        assert!(predicate.insert_range(205, 220));

        // Assert
        assert_eq!(predicate.non_ascii_ranges, vec![(190, 220)]);
    }
}
