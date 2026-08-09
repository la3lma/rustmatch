//! Conservative pattern classification for cohort-planning diagnostics.

use crate::PatternId;
use crate::hir::{Assertion, Hir, HirPattern};

const MAX_PREFIX_UNITS: usize = 32;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct AssertionKinds(u8);

impl AssertionKinds {
    const LINE_START: u8 = 1 << 0;
    const LINE_END: u8 = 1 << 1;
    const WORD_BOUNDARY: u8 = 1 << 2;
    const NON_WORD_BOUNDARY: u8 = 1 << 3;

    fn insert(&mut self, assertion: Assertion) {
        self.0 |= match assertion {
            Assertion::LineStart => Self::LINE_START,
            Assertion::LineEnd => Self::LINE_END,
            Assertion::WordBoundary => Self::WORD_BOUNDARY,
            Assertion::NonWordBoundary => Self::NON_WORD_BOUNDARY,
        };
    }

    const fn contains(self, flag: u8) -> bool {
        self.0 & flag != 0
    }

    const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MaximumConsumed {
    Impossible,
    Finite(usize),
    Unbounded,
}

#[derive(Debug)]
struct PrefixSummary {
    units: Vec<u16>,
    exact: bool,
}

impl PrefixSummary {
    fn empty_exact() -> Self {
        Self {
            units: Vec::new(),
            exact: true,
        }
    }
}

/// Read-only pattern facts exposed only to repository benchmark tooling.
///
/// These diagnostics are not part of rustmatch's supported application API.
#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PatternDiagnostics {
    pattern_id: PatternId,
    registration_ordinal: usize,
    assertions: AssertionKinds,
    nullable: bool,
    minimum_consumed: Option<usize>,
    maximum_consumed: MaximumConsumed,
    ascii_only: bool,
    proven_exact_literal_units: Option<usize>,
    necessary_prefix_units: usize,
    hir_nodes: usize,
}

impl PatternDiagnostics {
    fn classify(registration_ordinal: usize, pattern: &HirPattern) -> Self {
        let assertions = assertion_kinds(pattern.expression());
        let prefix = prefix_summary(pattern.expression());
        let proven_exact_literal_units =
            (assertions.is_empty() && prefix.exact).then_some(prefix.units.len());
        Self {
            pattern_id: pattern.pattern_id(),
            registration_ordinal,
            assertions,
            nullable: pattern.nullable(),
            minimum_consumed: pattern.minimum_consumed(),
            maximum_consumed: maximum_consumed(pattern.expression()),
            ascii_only: ascii_only(pattern.expression()),
            proven_exact_literal_units,
            necessary_prefix_units: prefix.units.len(),
            hir_nodes: hir_nodes(pattern.expression()),
        }
    }

    /// Caller-supplied pattern identity.
    #[must_use]
    pub const fn pattern_id(&self) -> PatternId {
        self.pattern_id
    }

    /// Zero-based registration order, retained independently of cohort order.
    #[must_use]
    pub const fn registration_ordinal(&self) -> usize {
        self.registration_ordinal
    }

    /// Conservative cohort label derived only from immutable pattern syntax.
    #[must_use]
    pub const fn cohort(&self) -> &'static str {
        if self.assertions.is_empty() {
            "assertion-free"
        } else {
            "assertion-bearing"
        }
    }

    /// Whether any line or word assertion appears in the pattern.
    #[must_use]
    pub const fn uses_assertions(&self) -> bool {
        !self.assertions.is_empty()
    }

    /// Whether a line-start assertion appears in the pattern.
    #[must_use]
    pub const fn uses_line_start(&self) -> bool {
        self.assertions.contains(AssertionKinds::LINE_START)
    }

    /// Whether a line-end assertion appears in the pattern.
    #[must_use]
    pub const fn uses_line_end(&self) -> bool {
        self.assertions.contains(AssertionKinds::LINE_END)
    }

    /// Whether an ASCII word-boundary assertion appears in the pattern.
    #[must_use]
    pub const fn uses_word_boundary(&self) -> bool {
        self.assertions.contains(AssertionKinds::WORD_BOUNDARY)
    }

    /// Whether an ASCII non-word-boundary assertion appears in the pattern.
    #[must_use]
    pub const fn uses_non_word_boundary(&self) -> bool {
        self.assertions.contains(AssertionKinds::NON_WORD_BOUNDARY)
    }

    /// Whether the normalized expression can accept without consuming input.
    #[must_use]
    pub const fn nullable(&self) -> bool {
        self.nullable
    }

    /// Minimum consumed UTF-16 units, or `None` for an impossible expression.
    #[must_use]
    pub const fn minimum_consumed(&self) -> Option<usize> {
        self.minimum_consumed
    }

    /// Finite maximum consumed UTF-16 units, if one is conservatively known.
    #[must_use]
    pub const fn maximum_consumed(&self) -> Option<usize> {
        match self.maximum_consumed {
            MaximumConsumed::Finite(maximum) => Some(maximum),
            MaximumConsumed::Impossible | MaximumConsumed::Unbounded => None,
        }
    }

    /// Whether the normalized expression has no finite consuming upper bound.
    #[must_use]
    pub const fn has_unbounded_maximum(&self) -> bool {
        matches!(self.maximum_consumed, MaximumConsumed::Unbounded)
    }

    /// Whether every code unit that the expression can consume is ASCII.
    #[must_use]
    pub const fn is_ascii_only(&self) -> bool {
        self.ascii_only
    }

    /// Length of a proven exact assertion-free literal, when at most 32 units.
    #[must_use]
    pub const fn proven_exact_literal_units(&self) -> Option<usize> {
        self.proven_exact_literal_units
    }

    /// Length of a conservative necessary consumed prefix, capped at 32 units.
    #[must_use]
    pub const fn necessary_prefix_units(&self) -> usize {
        self.necessary_prefix_units
    }

    /// Whether the necessary prefix meets the existing three-unit filter floor.
    #[must_use]
    pub const fn has_filterable_prefix(&self) -> bool {
        self.necessary_prefix_units >= 3
    }

    /// Number of normalized HIR nodes, saturated at `usize::MAX`.
    #[must_use]
    pub const fn hir_nodes(&self) -> usize {
        self.hir_nodes
    }
}

/// Stable assertion-free/assertion-bearing sorting for benchmark diagnostics.
///
/// This type exists only with the `benchmark-internals` feature and is not part
/// of rustmatch's supported application API. Compilation and scanning continue
/// to use registration-order pattern partitions.
#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CohortDiagnostics {
    patterns: Box<[PatternDiagnostics]>,
    assertion_free_pattern_ids: Box<[PatternId]>,
    assertion_bearing_pattern_ids: Box<[PatternId]>,
}

impl CohortDiagnostics {
    pub(crate) fn classify(patterns: &[HirPattern]) -> Self {
        let patterns = patterns
            .iter()
            .enumerate()
            .map(|(ordinal, pattern)| PatternDiagnostics::classify(ordinal, pattern))
            .collect::<Box<[_]>>();
        let mut assertion_free_pattern_ids = Vec::new();
        let mut assertion_bearing_pattern_ids = Vec::new();
        for pattern in &patterns {
            if pattern.uses_assertions() {
                assertion_bearing_pattern_ids.push(pattern.pattern_id());
            } else {
                assertion_free_pattern_ids.push(pattern.pattern_id());
            }
        }
        Self {
            patterns,
            assertion_free_pattern_ids: assertion_free_pattern_ids.into_boxed_slice(),
            assertion_bearing_pattern_ids: assertion_bearing_pattern_ids.into_boxed_slice(),
        }
    }

    /// Per-pattern facts in original registration order.
    #[must_use]
    pub fn patterns(&self) -> &[PatternDiagnostics] {
        &self.patterns
    }

    /// Pattern IDs in the assertion-free cohort, preserving registration order.
    #[must_use]
    pub fn assertion_free_pattern_ids(&self) -> &[PatternId] {
        &self.assertion_free_pattern_ids
    }

    /// Pattern IDs in the assertion-bearing cohort, preserving registration order.
    #[must_use]
    pub fn assertion_bearing_pattern_ids(&self) -> &[PatternId] {
        &self.assertion_bearing_pattern_ids
    }

    /// Number of non-empty diagnostic cohorts.
    #[must_use]
    pub fn cohort_count(&self) -> usize {
        usize::from(!self.assertion_free_pattern_ids.is_empty())
            + usize::from(!self.assertion_bearing_pattern_ids.is_empty())
    }
}

fn assertion_kinds(expression: &Hir) -> AssertionKinds {
    let mut kinds = AssertionKinds::default();
    visit_assertions(expression, &mut kinds);
    kinds
}

fn visit_assertions(expression: &Hir, kinds: &mut AssertionKinds) {
    match expression {
        Hir::Assertion(assertion) => kinds.insert(*assertion),
        Hir::Sequence(expressions) | Hir::Alternation(expressions) => {
            for expression in expressions {
                visit_assertions(expression, kinds);
            }
        }
        Hir::Repeat { expression, .. } => visit_assertions(expression, kinds),
        Hir::Never | Hir::Epsilon | Hir::Symbol(_) | Hir::Predicate(_) => {}
    }
}

fn maximum_consumed(expression: &Hir) -> MaximumConsumed {
    match expression {
        Hir::Never => MaximumConsumed::Impossible,
        Hir::Epsilon | Hir::Assertion(_) => MaximumConsumed::Finite(0),
        Hir::Symbol(_) => MaximumConsumed::Finite(1),
        Hir::Predicate(predicate) => {
            if predicate.is_empty() {
                MaximumConsumed::Impossible
            } else {
                MaximumConsumed::Finite(1)
            }
        }
        Hir::Sequence(expressions) => {
            let mut maximum = 0_usize;
            for expression in expressions {
                match maximum_consumed(expression) {
                    MaximumConsumed::Impossible => return MaximumConsumed::Impossible,
                    MaximumConsumed::Finite(next) => {
                        maximum = maximum.saturating_add(next);
                    }
                    MaximumConsumed::Unbounded => return MaximumConsumed::Unbounded,
                }
            }
            MaximumConsumed::Finite(maximum)
        }
        Hir::Alternation(expressions) => {
            let mut maximum = None;
            for expression in expressions {
                match maximum_consumed(expression) {
                    MaximumConsumed::Impossible => {}
                    MaximumConsumed::Finite(next) => {
                        maximum = Some(maximum.map_or(next, |current: usize| current.max(next)));
                    }
                    MaximumConsumed::Unbounded => return MaximumConsumed::Unbounded,
                }
            }
            maximum.map_or(MaximumConsumed::Impossible, MaximumConsumed::Finite)
        }
        Hir::Repeat {
            expression,
            max: Some(max),
            ..
        } => match maximum_consumed(expression) {
            MaximumConsumed::Impossible => MaximumConsumed::Impossible,
            MaximumConsumed::Finite(unit) => {
                MaximumConsumed::Finite(unit.saturating_mul(usize::from(*max)))
            }
            MaximumConsumed::Unbounded if *max == 0 => MaximumConsumed::Finite(0),
            MaximumConsumed::Unbounded => MaximumConsumed::Unbounded,
        },
        Hir::Repeat {
            expression,
            max: None,
            ..
        } => match maximum_consumed(expression) {
            MaximumConsumed::Impossible => MaximumConsumed::Impossible,
            MaximumConsumed::Finite(0) => MaximumConsumed::Finite(0),
            MaximumConsumed::Finite(_) | MaximumConsumed::Unbounded => MaximumConsumed::Unbounded,
        },
    }
}

fn ascii_only(expression: &Hir) -> bool {
    match expression {
        Hir::Never | Hir::Epsilon | Hir::Assertion(_) => true,
        Hir::Symbol(symbol) => *symbol < 128,
        Hir::Predicate(predicate) => predicate.is_ascii_only(),
        Hir::Sequence(expressions) | Hir::Alternation(expressions) => {
            expressions.iter().all(ascii_only)
        }
        Hir::Repeat { expression, .. } => ascii_only(expression),
    }
}

fn prefix_summary(expression: &Hir) -> PrefixSummary {
    match expression {
        Hir::Never | Hir::Predicate(_) => PrefixSummary {
            units: Vec::new(),
            exact: false,
        },
        Hir::Epsilon | Hir::Assertion(_) => PrefixSummary::empty_exact(),
        Hir::Symbol(symbol) => PrefixSummary {
            units: vec![*symbol],
            exact: true,
        },
        Hir::Sequence(expressions) => {
            let mut units = Vec::new();
            let mut exact = true;
            for (index, expression) in expressions.iter().enumerate() {
                let child = prefix_summary(expression);
                let complete_length = units.len().saturating_add(child.units.len());
                append_bounded(&mut units, &child.units);
                exact &= child.exact;
                if !child.exact
                    || complete_length > MAX_PREFIX_UNITS
                    || (units.len() == MAX_PREFIX_UNITS && index + 1 < expressions.len())
                {
                    exact = false;
                    break;
                }
            }
            PrefixSummary { units, exact }
        }
        Hir::Alternation(expressions) => {
            let mut expressions = expressions.iter();
            let Some(first) = expressions.next() else {
                return PrefixSummary {
                    units: Vec::new(),
                    exact: false,
                };
            };
            let first = prefix_summary(first);
            let mut units = first.units;
            let mut exact = first.exact;
            for expression in expressions {
                let candidate = prefix_summary(expression);
                exact &= candidate.exact && candidate.units == units;
                let shared = units
                    .iter()
                    .zip(candidate.units.iter())
                    .take_while(|(left, right)| left == right)
                    .count();
                units.truncate(shared);
            }
            PrefixSummary { units, exact }
        }
        Hir::Repeat {
            expression,
            min,
            max,
        } => {
            if *min == 0 {
                return PrefixSummary {
                    units: Vec::new(),
                    exact: *max == Some(0),
                };
            }
            let child = prefix_summary(expression);
            if !child.exact {
                return PrefixSummary {
                    units: child.units,
                    exact: false,
                };
            }
            let mut units = Vec::new();
            for repetition in 0..*min {
                let complete_length = units.len().saturating_add(child.units.len());
                append_bounded(&mut units, &child.units);
                if complete_length > MAX_PREFIX_UNITS
                    || (units.len() == MAX_PREFIX_UNITS && repetition + 1 < *min)
                {
                    return PrefixSummary {
                        units,
                        exact: false,
                    };
                }
            }
            PrefixSummary {
                units,
                exact: *max == Some(*min),
            }
        }
    }
}

fn append_bounded(output: &mut Vec<u16>, suffix: &[u16]) {
    let available = MAX_PREFIX_UNITS.saturating_sub(output.len());
    output.extend(suffix.iter().copied().take(available));
}

fn hir_nodes(expression: &Hir) -> usize {
    let descendants = match expression {
        Hir::Sequence(expressions) | Hir::Alternation(expressions) => {
            expressions.iter().fold(0_usize, |count, expression| {
                count.saturating_add(hir_nodes(expression))
            })
        }
        Hir::Repeat { expression, .. } => hir_nodes(expression),
        Hir::Never | Hir::Epsilon | Hir::Assertion(_) | Hir::Symbol(_) | Hir::Predicate(_) => 0,
    };
    1_usize.saturating_add(descendants)
}

#[cfg(test)]
mod tests {
    use super::CohortDiagnostics;
    use crate::PatternId;
    use crate::parser;

    #[test]
    fn classifier_sorts_cohorts_stably_and_records_conservative_facts() {
        // Prepare
        let sources = [
            (1, "abc"),
            (2, "^abc$"),
            (3, r"\bcat\B"),
            (4, "[a-z]+"),
            (5, "é"),
            (6, "(?:ab){2,4}"),
            (7, "(?:foo|far)"),
        ];
        let patterns = sources
            .iter()
            .map(|&(id, source)| parser::parse(PatternId::new(id), source))
            .collect::<Result<Vec<_>, _>>()
            .expect("test patterns parse");

        // Test
        let diagnostics = CohortDiagnostics::classify(&patterns);
        let facts = diagnostics.patterns();

        // Assert
        assert_eq!(
            diagnostics.assertion_free_pattern_ids(),
            [
                PatternId::new(1),
                PatternId::new(4),
                PatternId::new(5),
                PatternId::new(6),
                PatternId::new(7),
            ]
        );
        assert_eq!(
            diagnostics.assertion_bearing_pattern_ids(),
            [PatternId::new(2), PatternId::new(3)]
        );
        assert_eq!(diagnostics.cohort_count(), 2);
        assert_eq!(facts[0].registration_ordinal(), 0);
        assert_eq!(facts[0].cohort(), "assertion-free");
        assert_eq!(facts[0].minimum_consumed(), Some(3));
        assert_eq!(facts[0].maximum_consumed(), Some(3));
        assert_eq!(facts[0].proven_exact_literal_units(), Some(3));
        assert_eq!(facts[0].necessary_prefix_units(), 3);
        assert!(facts[0].has_filterable_prefix());
        assert!(facts[0].is_ascii_only());
        assert!(!facts[0].nullable());

        assert_eq!(facts[1].cohort(), "assertion-bearing");
        assert!(facts[1].uses_line_start());
        assert!(facts[1].uses_line_end());
        assert_eq!(facts[1].maximum_consumed(), Some(3));
        assert_eq!(facts[1].proven_exact_literal_units(), None);
        assert_eq!(facts[1].necessary_prefix_units(), 3);

        assert!(facts[2].uses_word_boundary());
        assert!(facts[2].uses_non_word_boundary());
        assert_eq!(facts[2].necessary_prefix_units(), 3);
        assert!(facts[3].has_unbounded_maximum());
        assert!(facts[3].is_ascii_only());
        assert!(!facts[4].is_ascii_only());
        assert_eq!(facts[5].minimum_consumed(), Some(4));
        assert_eq!(facts[5].maximum_consumed(), Some(8));
        assert_eq!(facts[5].necessary_prefix_units(), 4);
        assert_eq!(facts[5].proven_exact_literal_units(), None);
        assert_eq!(facts[6].necessary_prefix_units(), 1);
        assert!(!facts[6].has_filterable_prefix());
        assert!(facts.iter().all(|fact| fact.hir_nodes() > 0));
    }

    #[test]
    fn classifier_keeps_equal_pattern_text_as_distinct_registrations() {
        // Prepare
        let patterns = [10, 20]
            .into_iter()
            .map(|id| parser::parse(PatternId::new(id), "same"))
            .collect::<Result<Vec<_>, _>>()
            .expect("test patterns parse");

        // Test
        let diagnostics = CohortDiagnostics::classify(&patterns);

        // Assert
        assert_eq!(
            diagnostics.assertion_free_pattern_ids(),
            [PatternId::new(10), PatternId::new(20)]
        );
        assert_eq!(diagnostics.patterns()[0].pattern_id(), PatternId::new(10));
        assert_eq!(diagnostics.patterns()[1].pattern_id(), PatternId::new(20));
    }

    #[test]
    fn exact_literal_proof_stops_conservatively_at_the_prefix_cap() {
        // Prepare
        let patterns = ["x".repeat(32), "x".repeat(33)]
            .iter()
            .enumerate()
            .map(|(ordinal, source)| {
                parser::parse(
                    PatternId::new(u32::try_from(ordinal).expect("test ordinal")),
                    source,
                )
            })
            .collect::<Result<Vec<_>, _>>()
            .expect("test patterns parse");

        // Test
        let diagnostics = CohortDiagnostics::classify(&patterns);

        // Assert
        assert_eq!(
            diagnostics.patterns()[0].proven_exact_literal_units(),
            Some(32)
        );
        assert_eq!(diagnostics.patterns()[1].proven_exact_literal_units(), None);
        assert_eq!(diagnostics.patterns()[1].necessary_prefix_units(), 32);
    }
}
