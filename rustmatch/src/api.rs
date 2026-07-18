//! Minimal public lifecycle over the private compiler and engine.

use std::collections::HashSet;

use crate::engine;
use crate::hir::HirPattern;
use crate::nfa::{self, PatternDatabase};
use crate::parser;
use crate::prefilter::Prefilter;
use crate::{Error, Match, PatternFlags, PatternId, Utf16Text};

/// Collects and validates patterns before compiling an immutable matcher.
///
/// A rejected registration leaves the builder usable. Pattern IDs must be
/// unique, while equal pattern text may use different IDs.
#[derive(Debug)]
pub struct MatcherBuilder {
    pattern_ids: HashSet<PatternId>,
    patterns: Vec<HirPattern>,
    state_cache_budget: usize,
    prefilter_enabled: bool,
    literal_prefilter_enabled: bool,
}

impl Default for MatcherBuilder {
    fn default() -> Self {
        Self {
            pattern_ids: HashSet::new(),
            patterns: Vec::new(),
            state_cache_budget: engine::DEFAULT_STATE_CACHE_BUDGET,
            prefilter_enabled: true,
            literal_prefilter_enabled: true,
        }
    }
}

impl MatcherBuilder {
    /// Creates an empty matcher builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the maximum number of lazy deterministic states used by one scan.
    ///
    /// The default is 8,192 states. A budget of zero disables the cache and
    /// uses the exact NFA interpreter. If a nonzero budget fills, scanning
    /// continues through that same NFA path for states that could not be
    /// cached; the budget changes resource use, never matching semantics.
    /// Assertion-bearing pattern sets currently bypass the cache because their
    /// transitions depend on surrounding input context.
    pub fn state_cache_budget(&mut self, state_budget: usize) -> &mut Self {
        self.state_cache_budget = state_budget;
        self
    }

    /// Enables or disables all I7 candidate-start acceleration.
    ///
    /// This control exists only for repository benchmark and differential-test
    /// tooling. It is not part of rustmatch's supported application API.
    #[cfg(feature = "benchmark-internals")]
    #[doc(hidden)]
    pub fn prefilter_enabled(&mut self, enabled: bool) -> &mut Self {
        self.prefilter_enabled = enabled;
        self
    }

    /// Enables or disables only the I7 necessary-literal layer.
    ///
    /// The NFA-derived start table remains enabled. This control exists only
    /// for repository benchmark and differential-test tooling.
    #[cfg(feature = "benchmark-internals")]
    #[doc(hidden)]
    pub fn literal_prefilter_enabled(&mut self, enabled: bool) -> &mut Self {
        self.literal_prefilter_enabled = enabled;
        self
    }

    /// Registers one caller-identified pattern.
    ///
    /// The current executable spine accepts non-empty patterns composed of
    /// UTF-16 literals, dot, character classes, ranges, supported escapes,
    /// the ASCII shorthands `\d`, `\w`, and `\s` with their complements,
    /// alternation, plain or non-capturing groups, greedy quantifiers, prefix
    /// flags, line anchors, and ASCII word boundaries. Groups do not capture.
    /// Pure zero-width patterns, scoped flags, and lazy or possessive
    /// quantifiers return an error without modifying the builder.
    ///
    /// # Errors
    ///
    /// Returns [`Error::DuplicatePatternId`] if `pattern_id` is already used,
    /// [`Error::InvalidPattern`] for malformed syntax, or another pattern error
    /// if `pattern` is empty or outside the current syntax.
    pub fn add(&mut self, pattern_id: PatternId, pattern: &str) -> Result<(), Error> {
        self.add_with_flags(pattern_id, pattern, PatternFlags::NONE)
    }

    /// Registers one pattern with explicit compile-time matching options.
    ///
    /// [`PatternFlags::CASE_INSENSITIVE`] is equivalent to the leading `(?i)`
    /// syntax. It uses Java-compatible single-UTF-16-unit mappings and does not
    /// perform multi-code-point or locale-sensitive folding.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::add`]. A rejected registration does
    /// not reserve the pattern ID or otherwise modify the builder.
    pub fn add_with_flags(
        &mut self,
        pattern_id: PatternId,
        pattern: &str,
        flags: PatternFlags,
    ) -> Result<(), Error> {
        if self.pattern_ids.contains(&pattern_id) {
            return Err(Error::DuplicatePatternId { pattern_id });
        }
        let parsed = parser::parse_with_flags(pattern_id, pattern, flags)?;
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
        let database = nfa::compile(&self.patterns)?;
        let prefilter = Prefilter::compile(&self.patterns, &database);
        Ok(Matcher {
            database,
            prefilter,
            state_cache_budget: self.state_cache_budget,
            prefilter_enabled: self.prefilter_enabled,
            literal_prefilter_enabled: self.literal_prefilter_enabled,
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
    prefilter: Prefilter,
    state_cache_budget: usize,
    prefilter_enabled: bool,
    literal_prefilter_enabled: bool,
}

/// Scan-local cache counters intended only for the repository benchmark lane.
///
/// This type exists only with the `benchmark-internals` feature and is not part
/// of rustmatch's supported application API.
#[cfg(feature = "benchmark-internals")]
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScanDiagnostics {
    cache_budget: usize,
    cache_states: usize,
    cache_hits: u64,
    cache_misses: u64,
    fallback_transitions: u64,
    cache_table_bytes: usize,
    assertion_bypasses: u64,
    prefilter_path: crate::prefilter::PrefilterPath,
    prefilter_bypass: crate::prefilter::PrefilterBypass,
    prefilter_retained_bytes: usize,
    prefilter_candidate_bytes: usize,
    prefilter_admissions: u64,
    prefilter_candidate_starts: usize,
    prefilter_starts_scanned: usize,
    prefilter_starts_skipped: usize,
}

#[cfg(feature = "benchmark-internals")]
#[doc(hidden)]
impl ScanDiagnostics {
    /// Configured deterministic-state budget for this scan.
    #[must_use]
    pub const fn cache_budget(self) -> usize {
        self.cache_budget
    }

    /// Number of deterministic states materialized by this scan.
    #[must_use]
    pub const fn cache_states(self) -> usize {
        self.cache_states
    }

    /// Number of transitions served by a materialized cache slot.
    #[must_use]
    pub const fn cache_hits(self) -> u64 {
        self.cache_hits
    }

    /// Number of transitions materialized for the first time.
    #[must_use]
    pub const fn cache_misses(self) -> u64 {
        self.cache_misses
    }

    /// Number of transitions interpreted after cache-budget pressure.
    #[must_use]
    pub const fn fallback_transitions(self) -> u64 {
        self.fallback_transitions
    }

    /// Bytes occupied by direct ASCII transition tables.
    #[must_use]
    pub const fn cache_table_bytes(self) -> usize {
        self.cache_table_bytes
    }

    /// Number of scans routed around the cache because assertions were present.
    #[must_use]
    pub const fn assertion_bypasses(self) -> u64 {
        self.assertion_bypasses
    }

    /// Candidate-start path selected for this scan.
    #[must_use]
    pub const fn prefilter_path(self) -> &'static str {
        self.prefilter_path.label()
    }

    /// Reason the full literal prefilter was bypassed, or `"none"`.
    #[must_use]
    pub const fn prefilter_bypass(self) -> &'static str {
        self.prefilter_bypass.label()
    }

    /// Explicitly accounted bytes retained by immutable prefilter structures.
    #[must_use]
    pub const fn prefilter_retained_bytes(self) -> usize {
        self.prefilter_retained_bytes
    }

    /// Bytes retained by the scan-local candidate bitmap.
    #[must_use]
    pub const fn prefilter_candidate_bytes(self) -> usize {
        self.prefilter_candidate_bytes
    }

    /// Input starts admitted by the necessary-prefix membership filter.
    #[must_use]
    pub const fn prefilter_admissions(self) -> u64 {
        self.prefilter_admissions
    }

    /// Unique candidate starts produced by the literal prefilter.
    #[must_use]
    pub const fn prefilter_candidate_starts(self) -> usize {
        self.prefilter_candidate_starts
    }

    /// Start positions verified by the semantic engine.
    #[must_use]
    pub const fn prefilter_starts_scanned(self) -> usize {
        self.prefilter_starts_scanned
    }

    /// Start positions safely omitted before semantic verification.
    #[must_use]
    pub const fn prefilter_starts_skipped(self) -> usize {
        self.prefilter_starts_skipped
    }
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
    /// Returns [`Error::InputTooLarge`] if an input position cannot be
    /// represented by the public UTF-16 coordinate type.
    ///
    /// # Panics
    ///
    /// A panic from `sink` propagates to the caller; rustmatch does not catch
    /// application panics.
    pub fn scan(&self, input: &Utf16Text, sink: impl FnMut(Match)) -> Result<(), Error> {
        engine::scan(
            &self.database,
            &self.prefilter,
            input,
            self.state_cache_budget,
            self.prefilter_enabled,
            self.literal_prefilter_enabled,
            sink,
        )
    }

    /// Runs one scan and returns scan-local cache counters to benchmark tooling.
    ///
    /// This method exists only with the `benchmark-internals` feature and is
    /// not part of rustmatch's supported application API.
    #[cfg(feature = "benchmark-internals")]
    #[doc(hidden)]
    pub fn scan_with_diagnostics(
        &self,
        input: &Utf16Text,
        sink: impl FnMut(Match),
    ) -> Result<ScanDiagnostics, Error> {
        let stats = engine::scan_with_stats(
            &self.database,
            &self.prefilter,
            input,
            self.state_cache_budget,
            self.prefilter_enabled,
            self.literal_prefilter_enabled,
            sink,
        )?;
        Ok(ScanDiagnostics {
            cache_budget: self.state_cache_budget,
            cache_states: stats.cache_states,
            cache_hits: stats.cache_hits,
            cache_misses: stats.cache_misses,
            fallback_transitions: stats.fallback_transitions,
            cache_table_bytes: stats.cache_table_bytes,
            assertion_bypasses: stats.assertion_bypasses,
            prefilter_path: stats.prefilter_path,
            prefilter_bypass: stats.prefilter_bypass,
            prefilter_retained_bytes: stats.prefilter_retained_bytes,
            prefilter_candidate_bytes: stats.prefilter_candidate_bytes,
            prefilter_admissions: stats.prefilter_admissions,
            prefilter_candidate_starts: stats.prefilter_candidate_starts,
            prefilter_starts_scanned: stats.prefilter_starts_scanned,
            prefilter_starts_skipped: stats.prefilter_starts_skipped,
        })
    }
}
