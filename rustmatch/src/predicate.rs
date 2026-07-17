//! Compact predicates over the current 7-bit ASCII symbol domain.

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct AsciiPredicate(u128);

impl AsciiPredicate {
    pub(crate) const fn any() -> Self {
        Self(u128::MAX)
    }

    pub(crate) const fn matches(self, symbol: u16) -> bool {
        symbol < 128 && self.0 & (1_u128 << symbol) != 0
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
}
