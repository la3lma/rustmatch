//! Normalized high-level representation shared by parser and NFA compiler.

use crate::PatternId;
use crate::predicate::AsciiPredicate;

#[derive(Debug)]
pub(crate) struct HirPattern {
    pattern_id: PatternId,
    expression: Hir,
    nullable: bool,
    minimum_consumed: Option<usize>,
}

impl HirPattern {
    pub(crate) fn new(pattern_id: PatternId, expression: Hir) -> Self {
        let nullable = expression.nullable();
        let minimum_consumed = expression.minimum_consumed();
        Self {
            pattern_id,
            expression,
            nullable,
            minimum_consumed,
        }
    }

    pub(crate) const fn pattern_id(&self) -> PatternId {
        self.pattern_id
    }

    pub(crate) const fn expression(&self) -> &Hir {
        &self.expression
    }

    pub(crate) const fn nullable(&self) -> bool {
        self.nullable
    }

    pub(crate) const fn minimum_consumed(&self) -> Option<usize> {
        self.minimum_consumed
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Hir {
    Never,
    Epsilon,
    Symbol(u16),
    Predicate(AsciiPredicate),
    Sequence(Box<[Self]>),
    Alternation(Box<[Self]>),
    Repeat {
        expression: Box<Self>,
        min: u16,
        max: Option<u16>,
    },
}

impl Hir {
    pub(crate) fn sequence(expressions: Vec<Self>) -> Self {
        let mut normalized = Vec::with_capacity(expressions.len());
        for expression in expressions {
            match expression {
                Self::Never => return Self::Never,
                Self::Epsilon => {}
                Self::Sequence(nested) => normalized.extend(nested),
                other => normalized.push(other),
            }
        }
        match normalized.len() {
            0 => Self::Epsilon,
            1 => normalized.pop().expect("length was checked"),
            _ => Self::Sequence(normalized.into_boxed_slice()),
        }
    }

    pub(crate) fn alternation(expressions: Vec<Self>) -> Self {
        let mut normalized = Vec::with_capacity(expressions.len());
        for expression in expressions {
            match expression {
                Self::Never => {}
                Self::Alternation(nested) => normalized.extend(nested),
                other => normalized.push(other),
            }
        }
        match normalized.len() {
            0 => Self::Never,
            1 => normalized.pop().expect("length was checked"),
            _ => Self::Alternation(normalized.into_boxed_slice()),
        }
    }

    pub(crate) fn repeat(expression: Self, min: u16, max: Option<u16>) -> Self {
        debug_assert!(max.is_none_or(|limit| min <= limit));
        match (expression, min, max) {
            (_, _, Some(0)) | (Self::Epsilon, _, _) | (Self::Never, 0, _) => Self::Epsilon,
            (Self::Never, _, _) => Self::Never,
            (expression, 1, Some(1)) => expression,
            (expression, min, max) => Self::Repeat {
                expression: Box::new(expression),
                min,
                max,
            },
        }
    }

    pub(crate) fn nullable(&self) -> bool {
        match self {
            Self::Never | Self::Symbol(_) | Self::Predicate(_) => false,
            Self::Epsilon => true,
            Self::Sequence(expressions) => expressions.iter().all(Self::nullable),
            Self::Alternation(expressions) => expressions.iter().any(Self::nullable),
            Self::Repeat {
                expression, min, ..
            } => *min == 0 || expression.nullable(),
        }
    }

    pub(crate) fn minimum_consumed(&self) -> Option<usize> {
        match self {
            Self::Never => None,
            Self::Epsilon => Some(0),
            Self::Symbol(_) => Some(1),
            Self::Predicate(predicate) => (!predicate.is_empty()).then_some(1),
            Self::Sequence(expressions) => expressions.iter().try_fold(0_usize, |total, item| {
                item.minimum_consumed()
                    .map(|length| total.saturating_add(length))
            }),
            Self::Alternation(expressions) => {
                expressions.iter().filter_map(Self::minimum_consumed).min()
            }
            Self::Repeat {
                expression, min, ..
            } => {
                if *min == 0 {
                    Some(0)
                } else {
                    expression
                        .minimum_consumed()
                        .map(|length| length.saturating_mul(usize::from(*min)))
                }
            }
        }
    }

    pub(crate) fn can_consume(&self) -> bool {
        match self {
            Self::Never | Self::Epsilon => false,
            Self::Symbol(_) => true,
            Self::Predicate(predicate) => !predicate.is_empty(),
            Self::Sequence(expressions) => {
                self.minimum_consumed().is_some() && expressions.iter().any(Self::can_consume)
            }
            Self::Alternation(expressions) => expressions.iter().any(Self::can_consume),
            Self::Repeat {
                expression, max, ..
            } => *max != Some(0) && expression.can_consume(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Hir;

    #[test]
    fn normalization_flattens_composition_and_preserves_epsilon() {
        // Prepare
        let nested_sequence = Hir::sequence(vec![
            Hir::Symbol(u16::from(b'a')),
            Hir::sequence(vec![Hir::Epsilon, Hir::Symbol(u16::from(b'b'))]),
        ]);
        let nested_alternation = Hir::alternation(vec![
            nested_sequence,
            Hir::alternation(vec![Hir::Never, Hir::Epsilon]),
        ]);

        // Test
        let nullable = nested_alternation.nullable();
        let minimum_consumed = nested_alternation.minimum_consumed();
        let can_consume = nested_alternation.can_consume();

        // Assert
        let Hir::Alternation(branches) = nested_alternation else {
            panic!("alternation was not retained");
        };
        assert_eq!(branches.len(), 2);
        let Hir::Sequence(sequence) = &branches[0] else {
            panic!("sequence was not retained");
        };
        assert_eq!(sequence.len(), 2);
        assert!(nullable);
        assert_eq!(minimum_consumed, Some(0));
        assert!(can_consume);
    }

    #[test]
    fn impossible_sequence_and_alternation_are_normalized_algebraically() {
        // Prepare
        let impossible_sequence = Hir::sequence(vec![Hir::Symbol(u16::from(b'a')), Hir::Never]);
        let useful_alternation = Hir::alternation(vec![Hir::Never, Hir::Symbol(u16::from(b'b'))]);

        // Test / Assert
        assert_eq!(impossible_sequence, Hir::Never);
        assert_eq!(useful_alternation, Hir::Symbol(u16::from(b'b')));
    }

    #[test]
    fn repetition_metadata_distinguishes_nullable_and_consuming_paths() {
        // Prepare
        let star = Hir::repeat(Hir::Symbol(u16::from(b'a')), 0, None);
        let bounded = Hir::repeat(Hir::Symbol(u16::from(b'b')), 2, Some(4));
        let impossible = Hir::repeat(Hir::Never, 1, None);

        // Test / Assert
        assert!(star.nullable());
        assert_eq!(star.minimum_consumed(), Some(0));
        assert!(star.can_consume());
        assert!(!bounded.nullable());
        assert_eq!(bounded.minimum_consumed(), Some(2));
        assert!(bounded.can_consume());
        assert_eq!(impossible, Hir::Never);
    }
}
