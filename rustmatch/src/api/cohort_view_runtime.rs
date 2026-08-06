//! Benchmark-only shared-storage cohort representation.

use std::mem::size_of;

use crate::engine;
use crate::nfa::{self, PatternDatabase, PatternDatabaseView};
use crate::prefilter::Prefilter;
use crate::{Error, Match, Utf16Text};

use super::{MatcherBuilder, ScanDiagnostics};

#[derive(Debug)]
struct SharedCohortView {
    descriptor: PatternDatabaseView,
    prefilter: Prefilter,
    state_cache_budget: usize,
}

/// Structural facts about one benchmark-only shared cohort database.
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SharedCohortDiagnostics {
    state_count: usize,
    edge_count: usize,
    predicate_count: usize,
    terminal_count: usize,
    pattern_count: usize,
    assertion_free_pattern_count: usize,
    assertion_bearing_pattern_count: usize,
    database_retained_bytes: usize,
    prefilter_retained_bytes: usize,
}

impl SharedCohortDiagnostics {
    /// Number of NFA states retained by the shared database.
    #[must_use]
    pub const fn state_count(self) -> usize {
        self.state_count
    }

    /// Number of NFA edges retained by the shared database.
    #[must_use]
    pub const fn edge_count(self) -> usize {
        self.edge_count
    }

    /// Number of interned predicates retained by the shared database.
    #[must_use]
    pub const fn predicate_count(self) -> usize {
        self.predicate_count
    }

    /// Number of terminal ordinals retained by the shared database.
    #[must_use]
    pub const fn terminal_count(self) -> usize {
        self.terminal_count
    }

    /// Total number of registered patterns.
    #[must_use]
    pub const fn pattern_count(self) -> usize {
        self.pattern_count
    }

    /// Number of assertion-free patterns reachable from the first view.
    #[must_use]
    pub const fn assertion_free_pattern_count(self) -> usize {
        self.assertion_free_pattern_count
    }

    /// Number of assertion-bearing patterns reachable from the second view.
    #[must_use]
    pub const fn assertion_bearing_pattern_count(self) -> usize {
        self.assertion_bearing_pattern_count
    }

    /// Explicit bytes retained by the single shared pattern database.
    #[must_use]
    pub const fn database_retained_bytes(self) -> usize {
        self.database_retained_bytes
    }

    /// Explicit bytes retained by both cohort-specific prefilters.
    #[must_use]
    pub const fn prefilter_retained_bytes(self) -> usize {
        self.prefilter_retained_bytes
    }
}

/// Benchmark-only matcher over two roots in one compiled pattern database.
#[doc(hidden)]
#[derive(Debug)]
pub struct SharedCohortMatcher {
    database: PatternDatabase,
    assertion_free: SharedCohortView,
    assertion_bearing: SharedCohortView,
    requested_worker_count: usize,
    prefilter_enabled: bool,
    literal_prefilter_enabled: bool,
}

impl SharedCohortMatcher {
    /// Returns immutable storage facts for H43-V1 evidence.
    #[must_use]
    pub fn structure_diagnostics(&self) -> SharedCohortDiagnostics {
        SharedCohortDiagnostics {
            state_count: self.database.state_count(),
            edge_count: self.database.edge_count(),
            predicate_count: self.database.predicate_count(),
            terminal_count: self.database.terminal_count(),
            pattern_count: self.database.pattern_count(),
            assertion_free_pattern_count: self.assertion_free.descriptor.pattern_count,
            assertion_bearing_pattern_count: self.assertion_bearing.descriptor.pattern_count,
            database_retained_bytes: self.database.retained_bytes(),
            prefilter_retained_bytes: self
                .assertion_free
                .prefilter
                .retained_bytes()
                .saturating_add(self.assertion_bearing.prefilter.retained_bytes()),
        }
    }

    /// Scans both cohort roots and delivers events only after both succeed.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InputTooLarge`] if an input coordinate overflows.
    pub fn scan(&self, input: &Utf16Text, mut sink: impl FnMut(Match)) -> Result<(), Error> {
        let (events, _) = self.scan_buffered(input)?;
        for matched in events {
            sink(matched);
        }
        Ok(())
    }

    /// Scans both roots and returns benchmark-only execution counters.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InputTooLarge`] if an input coordinate overflows.
    pub fn scan_with_diagnostics(
        &self,
        input: &Utf16Text,
        mut sink: impl FnMut(Match),
    ) -> Result<ScanDiagnostics, Error> {
        let (events, stats) = self.scan_buffered(input)?;
        let buffered_events = events.len();
        for matched in events {
            sink(matched);
        }
        Ok(ScanDiagnostics {
            requested_worker_count: self.requested_worker_count,
            partition_count: self.cohort_count(),
            spawned_workers: 0,
            database_retained_bytes: self.database.retained_bytes(),
            total_cache_budget: self
                .assertion_free
                .state_cache_budget
                .saturating_add(self.assertion_bearing.state_cache_budget),
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
            buffered_events,
            buffered_event_bytes: buffered_events.saturating_mul(size_of::<Match>()),
        })
    }

    fn scan_buffered(&self, input: &Utf16Text) -> Result<(Vec<Match>, engine::ScanStats), Error> {
        let mut events = Vec::new();
        let mut combined: Option<engine::ScanStats> = None;
        for view in [&self.assertion_free, &self.assertion_bearing] {
            if view.descriptor.pattern_count == 0 {
                continue;
            }
            let stats = engine::scan_view_with_stats(
                engine::ViewScanConfig {
                    database: &self.database,
                    root: view.descriptor.root,
                    uses_assertions: view.descriptor.uses_assertions,
                    prefilter: &view.prefilter,
                    state_cache_budget: view.state_cache_budget,
                    prefilter_enabled: self.prefilter_enabled,
                    literal_prefilter_enabled: self.literal_prefilter_enabled,
                },
                input,
                |matched| events.push(matched),
            )?;
            if let Some(existing) = &mut combined {
                existing.merge_partition(stats);
            } else {
                combined = Some(stats);
            }
        }
        Ok((
            events,
            combined.expect("a built matcher has at least one populated cohort"),
        ))
    }

    fn cohort_count(&self) -> usize {
        usize::from(self.assertion_free.descriptor.pattern_count > 0)
            + usize::from(self.assertion_bearing.descriptor.pattern_count > 0)
    }
}

pub(super) fn build_matcher(builder: MatcherBuilder) -> Result<SharedCohortMatcher, Error> {
    let shared = nfa::compile_shared_cohorts(&builder.patterns)?;
    let (database, assertion_free_descriptor, assertion_bearing_descriptor) = shared.into_parts();
    let mut assertion_free_patterns = Vec::with_capacity(assertion_free_descriptor.pattern_count);
    let mut assertion_bearing_patterns =
        Vec::with_capacity(assertion_bearing_descriptor.pattern_count);
    for pattern in builder.patterns {
        if pattern.expression().uses_assertions() {
            assertion_bearing_patterns.push(pattern);
        } else {
            assertion_free_patterns.push(pattern);
        }
    }

    let cohort_count = usize::from(!assertion_free_patterns.is_empty())
        + usize::from(!assertion_bearing_patterns.is_empty());
    let free_cache_budget = if assertion_free_patterns.is_empty() {
        0
    } else {
        builder.state_cache_budget / cohort_count + builder.state_cache_budget % cohort_count
    };
    let bearing_cache_budget = if assertion_bearing_patterns.is_empty() {
        0
    } else {
        builder.state_cache_budget / cohort_count
    };
    let assertion_free_prefilter = Prefilter::compile_view(
        &assertion_free_patterns,
        &database,
        assertion_free_descriptor.root,
        assertion_free_descriptor.uses_assertions,
    );
    let assertion_bearing_prefilter = Prefilter::compile_view(
        &assertion_bearing_patterns,
        &database,
        assertion_bearing_descriptor.root,
        assertion_bearing_descriptor.uses_assertions,
    );

    Ok(SharedCohortMatcher {
        database,
        assertion_free: SharedCohortView {
            descriptor: assertion_free_descriptor,
            prefilter: assertion_free_prefilter,
            state_cache_budget: free_cache_budget,
        },
        assertion_bearing: SharedCohortView {
            descriptor: assertion_bearing_descriptor,
            prefilter: assertion_bearing_prefilter,
            state_cache_budget: bearing_cache_budget,
        },
        requested_worker_count: builder.worker_count,
        prefilter_enabled: builder.prefilter_enabled,
        literal_prefilter_enabled: builder.literal_prefilter_enabled,
    })
}
