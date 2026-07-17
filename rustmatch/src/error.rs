//! Public failures at pattern, build, and input boundaries.

use std::error;
use std::fmt;

use crate::{PatternId, Utf16Span};

/// A pattern registration, matcher build, or scan failure.
///
/// This enum is non-exhaustive so later syntax and resource limits can retain
/// precise context without turning every new diagnostic into a breaking API
/// change. Expected invalid input is returned as an error and does not panic.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Error {
    /// The same caller-provided pattern ID was registered more than once.
    DuplicatePatternId {
        /// ID that had already been registered.
        pattern_id: PatternId,
    },
    /// A pattern contained no code units.
    EmptyPattern {
        /// ID supplied with the empty pattern.
        pattern_id: PatternId,
    },
    /// A pattern used a recognized construct that this implementation does not support.
    UnsupportedPattern {
        /// ID supplied with the rejected pattern.
        pattern_id: PatternId,
        /// Location of the rejected UTF-16 code unit.
        span: Utf16Span,
        /// First UTF-16 code unit of the unsupported construct.
        code_unit: u16,
    },
    /// A pattern contained malformed syntax.
    InvalidPattern {
        /// ID supplied with the malformed pattern.
        pattern_id: PatternId,
        /// Smallest useful UTF-16 span containing the malformed syntax.
        span: Utf16Span,
    },
    /// No patterns were registered before building.
    NoPatterns,
    /// The compiled automaton would exceed its dense state-ID range.
    PatternSetTooLarge,
    /// A pattern position could not be represented as a public UTF-16 offset.
    PatternTooLarge {
        /// ID supplied with the oversized pattern.
        pattern_id: PatternId,
    },
    /// An input position could not be represented as a public UTF-16 offset.
    InputTooLarge,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicatePatternId { pattern_id } => {
                write!(formatter, "pattern ID {pattern_id} is already registered")
            }
            Self::EmptyPattern { pattern_id } => {
                write!(formatter, "pattern {pattern_id} is empty")
            }
            Self::UnsupportedPattern {
                pattern_id,
                span,
                code_unit,
            } => write!(
                formatter,
                "pattern {pattern_id} contains an unsupported construct beginning with UTF-16 code unit U+{code_unit:04X} at [{}..{})",
                span.start(),
                span.end()
            ),
            Self::InvalidPattern { pattern_id, span } => write!(
                formatter,
                "pattern {pattern_id} contains malformed syntax at [{}..{})",
                span.start(),
                span.end()
            ),
            Self::NoPatterns => formatter.write_str("cannot build a matcher without patterns"),
            Self::PatternSetTooLarge => {
                formatter.write_str("pattern set exceeds the supported automaton size")
            }
            Self::PatternTooLarge { pattern_id } => {
                write!(formatter, "pattern {pattern_id} is too large")
            }
            Self::InputTooLarge => formatter.write_str("input is too large"),
        }
    }
}

impl error::Error for Error {}
