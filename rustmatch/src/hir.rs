//! Minimal high-level representation shared by parser and NFA compiler.

use crate::PatternId;
use crate::predicate::AsciiPredicate;

#[derive(Debug)]
pub(crate) struct HirPattern {
    pattern_id: PatternId,
    expression: Hir,
}

impl HirPattern {
    pub(crate) const fn new(pattern_id: PatternId, expression: Hir) -> Self {
        Self {
            pattern_id,
            expression,
        }
    }

    pub(crate) const fn pattern_id(&self) -> PatternId {
        self.pattern_id
    }

    pub(crate) const fn expression(&self) -> &Hir {
        &self.expression
    }
}

#[derive(Debug)]
pub(crate) enum Hir {
    Sequence(Box<[HirAtom]>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HirAtom {
    Symbol(u16),
    Predicate(AsciiPredicate),
}
