//! Compact predicates over the current 7-bit ASCII symbol domain.

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct AsciiPredicate(u128);

impl AsciiPredicate {
    pub(crate) const fn empty() -> Self {
        Self(0)
    }

    pub(crate) const fn any() -> Self {
        Self(u128::MAX)
    }

    pub(crate) fn digit() -> Self {
        Self::from_ranges(&[(b'0', b'9')])
    }

    pub(crate) fn word() -> Self {
        let mut predicate = Self::from_ranges(&[(b'a', b'z'), (b'A', b'Z'), (b'0', b'9')]);
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
        debug_assert!(symbol < 128);
        self.0 |= 1_u128 << symbol;
    }

    pub(crate) fn insert_range(&mut self, start: u16, end: u16) -> bool {
        if start > end || end >= 128 {
            return false;
        }
        let below_end = if end == 127 {
            u128::MAX
        } else {
            (1_u128 << (end + 1)) - 1
        };
        self.0 |= (u128::MAX << start) & below_end;
        true
    }

    pub(crate) const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub(crate) const fn complement(self) -> Self {
        Self(!self.0)
    }

    pub(crate) const fn matches(self, symbol: u16) -> bool {
        symbol < 128 && self.0 & (1_u128 << symbol) != 0
    }

    fn from_ranges(ranges: &[(u8, u8)]) -> Self {
        let mut predicate = Self::empty();
        for &(start, end) in ranges {
            let inserted = predicate.insert_range(u16::from(start), u16::from(end));
            debug_assert!(inserted);
        }
        predicate
    }
}

#[cfg(test)]
mod tests {
    use super::AsciiPredicate;

    #[test]
    fn any_matches_exactly_the_ascii_domain() {
        // Prepare
        let predicate = AsciiPredicate::any();

        // Test
        let ascii_results: Vec<_> = (0..=127).map(|symbol| predicate.matches(symbol)).collect();
        let outside_results = [128, 255, u16::MAX].map(|symbol| predicate.matches(symbol));

        // Assert
        assert_eq!(ascii_results, vec![true; 128]);
        assert_eq!(outside_results, [false; 3]);
    }

    #[test]
    fn shorthand_predicates_have_the_documented_ascii_members() {
        // Prepare
        let digit = AsciiPredicate::digit();
        let word = AsciiPredicate::word();
        let whitespace = AsciiPredicate::whitespace();

        // Test
        let actual_digits: Vec<_> = (0..128).filter(|&unit| digit.matches(unit)).collect();
        let actual_word: Vec<_> = (0..128).filter(|&unit| word.matches(unit)).collect();
        let actual_whitespace: Vec<_> = (0..128).filter(|&unit| whitespace.matches(unit)).collect();

        // Assert
        assert_eq!(
            actual_digits,
            (u16::from(b'0')..=u16::from(b'9')).collect::<Vec<_>>()
        );
        assert_eq!(actual_word.len(), 63);
        assert!(actual_word.contains(&u16::from(b'_')));
        assert!(actual_word.iter().all(|&unit| {
            char::from_u32(u32::from(unit)).is_some_and(|symbol| symbol.is_ascii_alphanumeric())
                || unit == u16::from(b'_')
        }));
        assert_eq!(actual_whitespace, vec![9, 10, 11, 12, 13, 32]);
    }

    #[test]
    fn ranges_union_and_complement_without_leaving_ascii() {
        // Prepare
        let mut range = AsciiPredicate::empty();
        let inserted = range.insert_range(u16::from(b'a'), u16::from(b'c'));
        let with_digit = range.union(AsciiPredicate::digit());

        // Test
        let complement = with_digit.complement();

        // Assert
        assert!(inserted);
        assert!(!range.insert_range(u16::from(b'z'), u16::from(b'a')));
        assert!(with_digit.matches(u16::from(b'b')));
        assert!(with_digit.matches(u16::from(b'4')));
        assert!(!complement.matches(u16::from(b'b')));
        assert!(complement.matches(u16::from(b'!')));
        assert!(!complement.matches(128));
    }
}
