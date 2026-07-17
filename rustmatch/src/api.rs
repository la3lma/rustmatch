//! Minimal public lifecycle over the private compiler and engine.

use std::collections::HashSet;

use crate::engine;
use crate::hir::HirPattern;
use crate::nfa::{self, PatternDatabase};
use crate::parser;
use crate::{Error, Match, PatternId, Utf16Text};

/// Collects and validates patterns before compiling an immutable matcher.
///
/// A rejected registration leaves the builder usable. Pattern IDs must be
/// unique, while equal pattern text may use different IDs.
#[derive(Debug, Default)]
pub struct MatcherBuilder {
    pattern_ids: HashSet<PatternId>,
    patterns: Vec<HirPattern>,
}

impl MatcherBuilder {
    /// Creates an empty matcher builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers one caller-identified pattern.
    ///
    /// The current executable slice accepts non-empty patterns composed of
    /// 7-bit ASCII literals, dot, character classes, ranges, supported escapes,
    /// and the ASCII shorthands `\d`, `\w`, and `\s` with their complements.
    /// Other regex operators and non-ASCII code units return an error without
    /// modifying the builder.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DuplicatePatternId`] if `pattern_id` is already used,
    /// [`Error::InvalidPattern`] for malformed syntax, or another pattern error
    /// if `pattern` is empty or outside the current syntax.
    pub fn add(&mut self, pattern_id: PatternId, pattern: &str) -> Result<(), Error> {
        if self.pattern_ids.contains(&pattern_id) {
            return Err(Error::DuplicatePatternId { pattern_id });
        }
        let parsed = parser::parse(pattern_id, pattern)?;
        self.pattern_ids.insert(pattern_id);
        self.patterns.push(parsed);
        Ok(())
    }

    /// Compiles the registered patterns into an immutable reusable matcher.
    ///
    /// # Errors
    ///
    /// Returns [`Error::NoPatterns`] if nothing was registered, or
    /// [`Error::PatternSetTooLarge`] if dense internal state IDs would overflow.
    pub fn build(self) -> Result<Matcher, Error> {
        if self.patterns.is_empty() {
            return Err(Error::NoPatterns);
        }
        Ok(Matcher {
            database: nfa::compile(&self.patterns)?,
        })
    }
}

/// Immutable compiled matcher reusable across finite UTF-16 inputs.
///
/// The current engine is single-threaded. The matcher itself contains only
/// immutable compiled tables; each call to [`scan`](Self::scan) owns its
/// scratch storage. Callback delivery order is unspecified.
#[derive(Debug)]
pub struct Matcher {
    database: PatternDatabase,
}

impl Matcher {
    /// Scans one input and invokes `sink` for every match.
    ///
    /// For each pattern and eligible start position, the callback receives the
    /// longest match beginning there. Overlapping matches remain visible.
    /// The matcher can be scanned again after this method returns.
    ///
    /// # Errors
    ///
    /// The current slice returns [`Error::UnsupportedInput`] if any input code
    /// unit is outside 7-bit ASCII. The complete input is validated before the
    /// first callback, so this error never represents a partial result.
    ///
    /// # Panics
    ///
    /// A panic from `sink` propagates to the caller; rustmatch does not catch
    /// application panics.
    pub fn scan(&self, input: &Utf16Text, sink: impl FnMut(Match)) -> Result<(), Error> {
        engine::scan(&self.database, input, sink)
    }
}
