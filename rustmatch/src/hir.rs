//! Minimal high-level representation shared by parser and NFA compiler.

use crate::{PatternId, Utf16Span};

#[derive(Debug)]
pub(crate) struct HirPattern {
    pattern_id: PatternId,
    expression: Hir,
    source_span: Utf16Span,
}

impl HirPattern {
    pub(crate) const fn new(
        pattern_id: PatternId,
        expression: Hir,
        source_span: Utf16Span,
    ) -> Self {
        Self {
            pattern_id,
            expression,
            source_span,
        }
    }

    pub(crate) const fn pattern_id(&self) -> PatternId {
        self.pattern_id
    }

    pub(crate) const fn expression(&self) -> &Hir {
        &self.expression
    }

    pub(crate) const fn source_span(&self) -> Utf16Span {
        self.source_span
    }
}

#[derive(Debug)]
pub(crate) enum Hir {
    Literal(Box<[u16]>),
}
