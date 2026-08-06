//! Minimal public lifecycle over the private compiler and engine.

use std::any::Any;
use std::collections::HashSet;
#[cfg(feature = "benchmark-internals")]
use std::mem::size_of;
use std::panic;
use std::thread;

#[cfg(feature = "benchmark-internals")]
use crate::cohort::CohortDiagnostics;
use crate::engine;
use crate::hir::HirPattern;
use crate::nfa::{self, PatternDatabase};
use crate::parser;
use crate::prefilter::Prefilter;
use crate::shared_candidate::{self, SharedCandidate};
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
    worker_count: usize,
    prefilter_enabled: bool,
    literal_prefilter_enabled: bool,
    #[cfg(feature = "benchmark-internals")]
    cohort_compilation_enabled: bool,
}

impl Default for MatcherBuilder {
    fn default() -> Self {
        Self {
            pattern_ids: HashSet::new(),
            patterns: Vec::new(),
            state_cache_budget: engine::DEFAULT_STATE_CACHE_BUDGET,
            worker_count: 1,
            prefilter_enabled: true,
            literal_prefilter_enabled: true,
            #[cfg(feature = "benchmark-internals")]
            cohort_compilation_enabled: false,
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

    /// Sets the requested number of pattern-partition scan workers.
    ///
    /// The default is one. The actual partition count is the smaller of this
    /// value and the number of registered patterns. One partition keeps the
    /// direct single-threaded path; additional partitions use scoped threads
    /// and collect their matches before serial callback delivery. No arbitrary
    /// upper cap is imposed, so callers remain responsible for choosing a
    /// worker count appropriate to their machine and workload.
    ///
    /// A value of zero is rejected by [`Self::build`].
    pub fn worker_count(&mut self, worker_count: usize) -> &mut Self {
        self.worker_count = worker_count;
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

    /// Classifies registered patterns without changing subsequent compilation.
    ///
    /// This diagnostic exists only for repository benchmark and differential
    /// tooling. The result is computed on demand and is not retained by the
    /// builder or matcher.
    #[cfg(feature = "benchmark-internals")]
    #[doc(hidden)]
    #[must_use]
    pub fn cohort_diagnostics(&self) -> CohortDiagnostics {
        CohortDiagnostics::classify(&self.patterns)
    }

    /// Enables semantically inert assertion-based cohort compilation.
    ///
    /// This control exists only for repository differential and benchmark
    /// tooling. Every cohort continues to use the ordinary generic compiler
    /// and engine. The default path remains uncohorted.
    #[cfg(feature = "benchmark-internals")]
    #[doc(hidden)]
    pub fn cohort_compilation_enabled(&mut self, enabled: bool) -> &mut Self {
        self.cohort_compilation_enabled = enabled;
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
    /// [`Error::InvalidPattern`] for malformed syntax,
    /// [`Error::PatternNestingTooDeep`] beyond the documented group limit, or
    /// another pattern error if `pattern` is empty or outside the current
    /// syntax.
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
        if self.worker_count == 0 {
            return Err(Error::InvalidWorkerCount);
        }
        #[cfg(feature = "benchmark-internals")]
        if self.cohort_compilation_enabled {
            return build_cohort_matcher(self);
        }
        let partitions =
            compile_partitions(&self.patterns, self.worker_count, self.state_cache_budget)?;
        Ok(Matcher {
            partitions,
            #[cfg(feature = "benchmark-internals")]
            requested_worker_count: self.worker_count,
            #[cfg(feature = "benchmark-internals")]
            cohort_layout: CohortLayout::disabled(),
            prefilter_enabled: self.prefilter_enabled,
            literal_prefilter_enabled: self.literal_prefilter_enabled,
        })
    }
}

#[cfg(feature = "benchmark-internals")]
fn build_cohort_matcher(builder: MatcherBuilder) -> Result<Matcher, Error> {
    let assertion_bearing_pattern_count = builder
        .patterns
        .iter()
        .filter(|pattern| pattern.expression().uses_assertions())
        .count();
    let assertion_free_pattern_count = builder.patterns.len() - assertion_bearing_pattern_count;

    if assertion_free_pattern_count == 0 || assertion_bearing_pattern_count == 0 {
        let partitions = compile_partitions(
            &builder.patterns,
            builder.worker_count,
            builder.state_cache_budget,
        )?;
        let partition_count = partitions.len();
        return Ok(Matcher {
            partitions,
            requested_worker_count: builder.worker_count,
            cohort_layout: CohortLayout::single(
                assertion_free_pattern_count,
                assertion_bearing_pattern_count,
                partition_count,
            ),
            prefilter_enabled: builder.prefilter_enabled,
            literal_prefilter_enabled: builder.literal_prefilter_enabled,
        });
    }

    let mut assertion_free = Vec::with_capacity(assertion_free_pattern_count);
    let mut assertion_bearing = Vec::with_capacity(assertion_bearing_pattern_count);
    for pattern in builder.patterns {
        if pattern.expression().uses_assertions() {
            assertion_bearing.push(pattern);
        } else {
            assertion_free.push(pattern);
        }
    }

    let [
        assertion_free_partition_count,
        assertion_bearing_partition_count,
    ] = allocate_cohort_partitions(
        [assertion_free.len(), assertion_bearing.len()],
        builder.worker_count,
    );
    let total_partition_count = assertion_free_partition_count + assertion_bearing_partition_count;
    let assertion_free_cache_budget = cohort_cache_budget(
        builder.state_cache_budget,
        total_partition_count,
        0,
        assertion_free_partition_count,
    );
    let assertion_bearing_cache_budget = cohort_cache_budget(
        builder.state_cache_budget,
        total_partition_count,
        assertion_free_partition_count,
        assertion_bearing_partition_count,
    );
    let mut partitions = compile_partitions(
        &assertion_free,
        assertion_free_partition_count,
        assertion_free_cache_budget,
    )?
    .into_vec();
    let split_partition = partitions.len();
    partitions.extend(
        compile_partitions(
            &assertion_bearing,
            assertion_bearing_partition_count,
            assertion_bearing_cache_budget,
        )?
        .into_vec(),
    );
    debug_assert_eq!(partitions.len(), total_partition_count);
    debug_assert_eq!(
        partitions
            .iter()
            .map(|partition| partition.state_cache_budget)
            .sum::<usize>(),
        builder.state_cache_budget
    );

    Ok(Matcher {
        partitions: partitions.into_boxed_slice(),
        requested_worker_count: builder.worker_count,
        cohort_layout: CohortLayout {
            enabled: true,
            assertion_free_pattern_count,
            assertion_bearing_pattern_count,
            assertion_free_partition_count,
            assertion_bearing_partition_count,
            split_partition: Some(split_partition),
        },
        prefilter_enabled: builder.prefilter_enabled,
        literal_prefilter_enabled: builder.literal_prefilter_enabled,
    })
}

#[cfg(feature = "benchmark-internals")]
fn allocate_cohort_partitions(
    pattern_counts: [usize; 2],
    requested_worker_count: usize,
) -> [usize; 2] {
    debug_assert!(pattern_counts.iter().all(|&count| count > 0));
    let total_patterns = pattern_counts.iter().sum::<usize>();
    let target = requested_worker_count.max(2).min(total_patterns);
    let mut allocations = [1_usize; 2];
    while allocations.iter().sum::<usize>() < target {
        let selected = (0..2)
            .filter(|&index| allocations[index] < pattern_counts[index])
            .max_by(|&left, &right| {
                let left_pressure = (pattern_counts[left] as u128) * (allocations[right] as u128);
                let right_pressure = (pattern_counts[right] as u128) * (allocations[left] as u128);
                left_pressure
                    .cmp(&right_pressure)
                    .then_with(|| right.cmp(&left))
            })
            .expect("target never exceeds total cohort capacity");
        allocations[selected] += 1;
    }
    allocations
}

#[cfg(feature = "benchmark-internals")]
fn cohort_cache_budget(
    total_budget: usize,
    total_partitions: usize,
    partition_start: usize,
    partition_count: usize,
) -> usize {
    let budget_per_partition = total_budget / total_partitions;
    let extra_partitions = total_budget % total_partitions;
    let extra_in_cohort = extra_partitions
        .saturating_sub(partition_start)
        .min(partition_count);
    budget_per_partition
        .saturating_mul(partition_count)
        .saturating_add(extra_in_cohort)
}

fn compile_partitions(
    patterns: &[HirPattern],
    requested_worker_count: usize,
    total_state_cache_budget: usize,
) -> Result<Box<[MatcherPartition]>, Error> {
    let partition_count = requested_worker_count.min(patterns.len());
    let patterns_per_partition = patterns.len() / partition_count;
    let extra_patterns = patterns.len() % partition_count;
    let cache_states_per_partition = total_state_cache_budget / partition_count;
    let extra_cache_states = total_state_cache_budget % partition_count;
    let mut partitions = Vec::with_capacity(partition_count);
    let mut start = 0;

    for partition_index in 0..partition_count {
        let pattern_count = patterns_per_partition + usize::from(partition_index < extra_patterns);
        let end = start + pattern_count;
        let partition_patterns = &patterns[start..end];
        let database = nfa::compile(partition_patterns)?;
        let prefilter = Prefilter::compile(partition_patterns, &database);
        let state_cache_budget =
            cache_states_per_partition + usize::from(partition_index < extra_cache_states);
        partitions.push(MatcherPartition {
            database,
            prefilter,
            state_cache_budget,
        });
        start = end;
    }

    debug_assert_eq!(start, patterns.len());
    debug_assert_eq!(
        partitions
            .iter()
            .map(|partition| partition.state_cache_budget)
            .sum::<usize>(),
        total_state_cache_budget
    );
    Ok(partitions.into_boxed_slice())
}

/// Immutable compiled matcher reusable across finite UTF-16 inputs.
///
/// The matcher contains only immutable compiled tables; each call to
/// [`scan`](Self::scan) owns its scratch storage. A matcher configured with
/// several workers scans deterministic pattern partitions concurrently, joins
/// every worker, and then invokes the callback serially on the caller thread.
/// Callback delivery order is unspecified.
#[derive(Debug)]
pub struct Matcher {
    partitions: Box<[MatcherPartition]>,
    #[cfg(feature = "benchmark-internals")]
    requested_worker_count: usize,
    #[cfg(feature = "benchmark-internals")]
    cohort_layout: CohortLayout,
    prefilter_enabled: bool,
    literal_prefilter_enabled: bool,
}

#[cfg(feature = "benchmark-internals")]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CohortLayout {
    enabled: bool,
    assertion_free_pattern_count: usize,
    assertion_bearing_pattern_count: usize,
    assertion_free_partition_count: usize,
    assertion_bearing_partition_count: usize,
    split_partition: Option<usize>,
}

#[cfg(feature = "benchmark-internals")]
impl CohortLayout {
    const fn disabled() -> Self {
        Self {
            enabled: false,
            assertion_free_pattern_count: 0,
            assertion_bearing_pattern_count: 0,
            assertion_free_partition_count: 0,
            assertion_bearing_partition_count: 0,
            split_partition: None,
        }
    }

    const fn single(
        assertion_free_pattern_count: usize,
        assertion_bearing_pattern_count: usize,
        partition_count: usize,
    ) -> Self {
        Self {
            enabled: true,
            assertion_free_pattern_count,
            assertion_bearing_pattern_count,
            assertion_free_partition_count: if assertion_free_pattern_count == 0 {
                0
            } else {
                partition_count
            },
            assertion_bearing_partition_count: if assertion_bearing_pattern_count == 0 {
                0
            } else {
                partition_count
            },
            split_partition: None,
        }
    }

    fn cohort_count(self) -> usize {
        usize::from(self.assertion_free_pattern_count > 0)
            + usize::from(self.assertion_bearing_pattern_count > 0)
    }
}

/// Immutable cohort-layout facts for repository evidence tooling.
///
/// This type exists only with `benchmark-internals` and is not part of the
/// supported application API.
#[cfg(feature = "benchmark-internals")]
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MatcherCohortDiagnostics {
    enabled: bool,
    cohort_count: usize,
    assertion_free_pattern_count: usize,
    assertion_bearing_pattern_count: usize,
    assertion_free_partition_count: usize,
    assertion_bearing_partition_count: usize,
    total_partition_count: usize,
    total_cache_budget: usize,
    assertion_free_retained_bytes: usize,
    assertion_bearing_retained_bytes: usize,
    spawned_workers: usize,
    buffers_before_delivery: bool,
}

#[cfg(feature = "benchmark-internals")]
#[doc(hidden)]
impl MatcherCohortDiagnostics {
    #[must_use]
    pub const fn enabled(self) -> bool {
        self.enabled
    }

    #[must_use]
    pub const fn cohort_count(self) -> usize {
        self.cohort_count
    }

    #[must_use]
    pub const fn assertion_free_pattern_count(self) -> usize {
        self.assertion_free_pattern_count
    }

    #[must_use]
    pub const fn assertion_bearing_pattern_count(self) -> usize {
        self.assertion_bearing_pattern_count
    }

    #[must_use]
    pub const fn assertion_free_partition_count(self) -> usize {
        self.assertion_free_partition_count
    }

    #[must_use]
    pub const fn assertion_bearing_partition_count(self) -> usize {
        self.assertion_bearing_partition_count
    }

    #[must_use]
    pub const fn total_partition_count(self) -> usize {
        self.total_partition_count
    }

    #[must_use]
    pub const fn total_cache_budget(self) -> usize {
        self.total_cache_budget
    }

    #[must_use]
    pub const fn assertion_free_retained_bytes(self) -> usize {
        self.assertion_free_retained_bytes
    }

    #[must_use]
    pub const fn assertion_bearing_retained_bytes(self) -> usize {
        self.assertion_bearing_retained_bytes
    }

    #[must_use]
    pub const fn spawned_workers(self) -> usize {
        self.spawned_workers
    }

    #[must_use]
    pub const fn buffers_before_delivery(self) -> bool {
        self.buffers_before_delivery
    }
}

#[derive(Debug)]
struct MatcherPartition {
    database: PatternDatabase,
    prefilter: Prefilter,
    state_cache_budget: usize,
}

#[cfg(feature = "benchmark-internals")]
impl MatcherPartition {
    fn retained_bytes(&self) -> usize {
        self.database
            .retained_bytes()
            .saturating_add(self.prefilter.retained_bytes())
    }
}

/// Scan-local cache counters intended only for the repository benchmark lane.
///
/// This type exists only with the `benchmark-internals` feature and is not part
/// of rustmatch's supported application API.
#[cfg(feature = "benchmark-internals")]
#[doc(hidden)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScanDiagnostics {
    requested_worker_count: usize,
    partition_count: usize,
    spawned_workers: usize,
    database_retained_bytes: usize,
    total_cache_budget: usize,
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
    buffered_events: usize,
    buffered_event_bytes: usize,
}

#[cfg(feature = "benchmark-internals")]
#[doc(hidden)]
impl ScanDiagnostics {
    /// Worker count requested when the matcher was built.
    #[must_use]
    pub const fn requested_worker_count(self) -> usize {
        self.requested_worker_count
    }

    /// Actual pattern partition count used by this matcher.
    #[must_use]
    pub const fn partition_count(self) -> usize {
        self.partition_count
    }

    /// Scoped worker threads started in addition to the caller thread.
    #[must_use]
    pub const fn spawned_workers(self) -> usize {
        self.spawned_workers
    }

    /// Explicitly accounted bytes retained by compiled pattern databases.
    #[must_use]
    pub const fn database_retained_bytes(self) -> usize {
        self.database_retained_bytes
    }

    /// Configured total deterministic-state budget for this scan.
    #[must_use]
    pub const fn cache_budget(self) -> usize {
        self.total_cache_budget
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

    /// Match events retained before serialized callback delivery.
    #[must_use]
    pub const fn buffered_events(self) -> usize {
        self.buffered_events
    }

    /// Bytes occupied by retained [`Match`] values, excluding vector capacity.
    #[must_use]
    pub const fn buffered_event_bytes(self) -> usize {
        self.buffered_event_bytes
    }
}

impl Matcher {
    /// Returns the immutable cohort layout selected at build time.
    ///
    /// This diagnostic exists only for repository benchmark and differential
    /// tooling. It does not alter matcher execution.
    #[cfg(feature = "benchmark-internals")]
    #[doc(hidden)]
    #[must_use]
    pub fn cohort_execution_diagnostics(&self) -> MatcherCohortDiagnostics {
        let split_partition = self
            .cohort_layout
            .split_partition
            .unwrap_or(self.partitions.len());
        let assertion_free_retained_bytes = if self.cohort_layout.assertion_free_pattern_count > 0 {
            self.partitions[..split_partition]
                .iter()
                .map(MatcherPartition::retained_bytes)
                .sum()
        } else {
            0
        };
        let assertion_bearing_start = if self.cohort_layout.split_partition.is_some() {
            split_partition
        } else {
            0
        };
        let assertion_bearing_retained_bytes =
            if self.cohort_layout.assertion_bearing_pattern_count > 0 {
                self.partitions[assertion_bearing_start..]
                    .iter()
                    .map(MatcherPartition::retained_bytes)
                    .sum()
            } else {
                0
            };
        MatcherCohortDiagnostics {
            enabled: self.cohort_layout.enabled,
            cohort_count: self.cohort_layout.cohort_count(),
            assertion_free_pattern_count: self.cohort_layout.assertion_free_pattern_count,
            assertion_bearing_pattern_count: self.cohort_layout.assertion_bearing_pattern_count,
            assertion_free_partition_count: self.cohort_layout.assertion_free_partition_count,
            assertion_bearing_partition_count: self.cohort_layout.assertion_bearing_partition_count,
            total_partition_count: self.partitions.len(),
            total_cache_budget: self
                .partitions
                .iter()
                .map(|partition| partition.state_cache_budget)
                .sum(),
            assertion_free_retained_bytes,
            assertion_bearing_retained_bytes,
            spawned_workers: self.spawned_worker_count(),
            buffers_before_delivery: self.partitions.len() > 1,
        }
    }

    /// Scans one input and invokes `sink` for every match.
    ///
    /// For each pattern and eligible start position, the callback receives the
    /// longest match beginning there. Overlapping matches remain visible.
    /// The matcher can be scanned again after this method returns.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InputTooLarge`] if an input position cannot be
    /// represented by the public UTF-16 coordinate type, or
    /// [`Error::WorkerUnavailable`] if a configured scoped worker cannot be
    /// started. No callback is invoked before a worker-start or scan error is
    /// known not to have occurred.
    ///
    /// # Panics
    ///
    /// A panic from `sink` propagates to the caller after every scoped worker
    /// has stopped. An internal worker panic is joined and resumed on the
    /// caller thread.
    #[inline]
    pub fn scan(&self, input: &Utf16Text, sink: impl FnMut(Match)) -> Result<(), Error> {
        if let [partition] = self.partitions.as_ref() {
            return engine::scan(
                &partition.database,
                &partition.prefilter,
                input,
                partition.state_cache_budget,
                self.prefilter_enabled,
                self.literal_prefilter_enabled,
                sink,
            );
        }

        self.scan_parallel_and_deliver(input, sink)
    }

    #[inline(never)]
    fn scan_parallel_and_deliver(
        &self,
        input: &Utf16Text,
        mut sink: impl FnMut(Match),
    ) -> Result<(), Error> {
        let output = self.scan_parallel(input)?;
        output.deliver(&mut sink);
        Ok(())
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
        mut sink: impl FnMut(Match),
    ) -> Result<ScanDiagnostics, Error> {
        if let [partition] = self.partitions.as_ref() {
            let stats = engine::scan_with_stats(
                &partition.database,
                &partition.prefilter,
                input,
                partition.state_cache_budget,
                self.prefilter_enabled,
                self.literal_prefilter_enabled,
                sink,
            )?;
            return Ok(self.diagnostics(stats, 0));
        }

        let output = self.scan_parallel(input)?;
        let buffered_events = output.buffered_events();
        let stats = output.stats;
        output.deliver(&mut sink);
        Ok(self.diagnostics(stats, buffered_events))
    }

    fn scan_parallel(&self, input: &Utf16Text) -> Result<ParallelScanOutput, Error> {
        self.scan_parallel_with_hooks(input, &|_| Ok(()), &|_| {})
    }

    fn scan_parallel_with_hooks<BeforeSpawn, BeforeScan>(
        &self,
        input: &Utf16Text,
        before_spawn: &BeforeSpawn,
        before_scan: &BeforeScan,
    ) -> Result<ParallelScanOutput, Error>
    where
        BeforeSpawn: Fn(usize) -> Result<(), Error>,
        BeforeScan: Fn(usize) + Sync,
    {
        debug_assert!(self.partitions.len() > 1);
        #[cfg(feature = "benchmark-internals")]
        if let Some(split_partition) = self.cohort_split_partition() {
            if self.requested_worker_count == 1 {
                let mut outputs = self.scan_partition_group_with_hooks(
                    &self.partitions[..split_partition],
                    0,
                    input,
                    before_spawn,
                    before_scan,
                )?;
                outputs.extend(self.scan_partition_group_with_hooks(
                    &self.partitions[split_partition..],
                    split_partition,
                    input,
                    before_spawn,
                    before_scan,
                )?);
                return Ok(ParallelScanOutput::new(outputs));
            }
            return self.scan_cohort_partition_groups_with_hooks(
                split_partition,
                input,
                before_spawn,
                before_scan,
            );
        }
        let partitions = self.scan_partition_group_with_hooks(
            &self.partitions,
            0,
            input,
            before_spawn,
            before_scan,
        )?;
        Ok(ParallelScanOutput::new(partitions))
    }

    #[cfg(feature = "benchmark-internals")]
    fn scan_cohort_partition_groups_with_hooks<BeforeSpawn, BeforeScan>(
        &self,
        split_partition: usize,
        input: &Utf16Text,
        before_spawn: &BeforeSpawn,
        before_scan: &BeforeScan,
    ) -> Result<ParallelScanOutput, Error>
    where
        BeforeSpawn: Fn(usize) -> Result<(), Error>,
        BeforeScan: Fn(usize) + Sync,
    {
        debug_assert!(self.requested_worker_count > 1);
        let assertion_free = &self.partitions[..split_partition];
        let assertion_bearing = &self.partitions[split_partition..];
        let assertion_free_plan = self.shared_candidate_plan(assertion_free, input)?;
        let assertion_bearing_plan = self.shared_candidate_plan(assertion_bearing, input)?;
        let partitions = thread::scope(|scope| {
            let mut handles = Vec::with_capacity(self.partitions.len().saturating_sub(1));
            let mut spawn_error = None;
            for (partition_index, partition) in self.partitions.iter().enumerate().skip(1) {
                if let Err(error) = before_spawn(partition_index) {
                    spawn_error = Some(error);
                    break;
                }
                let shared = if partition_index < split_partition {
                    assertion_free_plan
                        .as_ref()
                        .map(|plan| plan.partition(partition_index))
                } else {
                    assertion_bearing_plan
                        .as_ref()
                        .map(|plan| plan.partition(partition_index - split_partition))
                };
                let worker = thread::Builder::new()
                    .name(format!("rustmatch-worker-{partition_index}"))
                    .spawn_scoped(scope, move || {
                        before_scan(partition_index);
                        match shared {
                            Some(shared) => scan_partition_shared(partition, input, shared),
                            None => scan_partition(
                                partition,
                                input,
                                self.prefilter_enabled,
                                self.literal_prefilter_enabled,
                            ),
                        }
                    });
                if let Ok(handle) = worker {
                    handles.push((partition_index, handle));
                } else {
                    spawn_error = Some(Error::WorkerUnavailable);
                    break;
                }
            }

            let caller_result = if spawn_error.is_none() {
                before_scan(0);
                Some(
                    match assertion_free_plan.as_ref().map(|plan| plan.partition(0)) {
                        Some(shared) => scan_partition_shared(&self.partitions[0], input, shared),
                        None => scan_partition(
                            &self.partitions[0],
                            input,
                            self.prefilter_enabled,
                            self.literal_prefilter_enabled,
                        ),
                    },
                )
            } else {
                None
            };
            let mut outputs: Vec<Option<PartitionScanOutput>> =
                (0..self.partitions.len()).map(|_| None).collect();
            let mut scan_error = None;
            if let Some(result) = caller_result {
                match result {
                    Ok(output) => outputs[0] = Some(output),
                    Err(error) => scan_error = Some(error),
                }
            }
            let mut worker_panic: Option<Box<dyn Any + Send + 'static>> = None;
            for (partition_index, handle) in handles {
                match handle.join() {
                    Ok(Ok(output)) => outputs[partition_index] = Some(output),
                    Ok(Err(error)) => {
                        if scan_error.is_none() {
                            scan_error = Some(error);
                        }
                    }
                    Err(payload) => {
                        if worker_panic.is_none() {
                            worker_panic = Some(payload);
                        }
                    }
                }
            }

            if let Some(payload) = worker_panic {
                panic::resume_unwind(payload);
            }
            if let Some(error) = spawn_error.or(scan_error) {
                return Err(error);
            }
            Ok(outputs
                .into_iter()
                .map(|output| output.expect("every successful partition returned output"))
                .collect::<Vec<_>>())
        })?;
        Ok(ParallelScanOutput::new(partitions))
    }

    fn scan_partition_group_with_hooks<BeforeSpawn, BeforeScan>(
        &self,
        partitions: &[MatcherPartition],
        partition_offset: usize,
        input: &Utf16Text,
        before_spawn: &BeforeSpawn,
        before_scan: &BeforeScan,
    ) -> Result<Vec<PartitionScanOutput>, Error>
    where
        BeforeSpawn: Fn(usize) -> Result<(), Error>,
        BeforeScan: Fn(usize) + Sync,
    {
        debug_assert!(!partitions.is_empty());
        let shared_plan = self.shared_candidate_plan(partitions, input)?;
        thread::scope(|scope| {
            let mut handles = Vec::with_capacity(partitions.len().saturating_sub(1));
            let mut spawn_error = None;
            for (local_index, partition) in partitions.iter().enumerate().skip(1) {
                let partition_index = partition_offset + local_index;
                if let Err(error) = before_spawn(partition_index) {
                    spawn_error = Some(error);
                    break;
                }
                let shared = shared_plan.as_ref().map(|plan| plan.partition(local_index));
                let worker = thread::Builder::new()
                    .name(format!("rustmatch-worker-{partition_index}"))
                    .spawn_scoped(scope, move || {
                        before_scan(partition_index);
                        match shared {
                            Some(shared) => scan_partition_shared(partition, input, shared),
                            None => scan_partition(
                                partition,
                                input,
                                self.prefilter_enabled,
                                self.literal_prefilter_enabled,
                            ),
                        }
                    });
                if let Ok(handle) = worker {
                    handles.push((local_index, handle));
                } else {
                    spawn_error = Some(Error::WorkerUnavailable);
                    break;
                }
            }

            let caller_result = if spawn_error.is_none() {
                before_scan(partition_offset);
                Some(match shared_plan.as_ref().map(|plan| plan.partition(0)) {
                    Some(shared) => scan_partition_shared(&partitions[0], input, shared),
                    None => scan_partition(
                        &partitions[0],
                        input,
                        self.prefilter_enabled,
                        self.literal_prefilter_enabled,
                    ),
                })
            } else {
                None
            };
            let mut outputs: Vec<Option<PartitionScanOutput>> =
                (0..partitions.len()).map(|_| None).collect();
            let mut scan_error = None;
            if let Some(result) = caller_result {
                match result {
                    Ok(output) => outputs[0] = Some(output),
                    Err(error) => scan_error = Some(error),
                }
            }
            let mut worker_panic: Option<Box<dyn Any + Send + 'static>> = None;
            for (local_index, handle) in handles {
                match handle.join() {
                    Ok(Ok(output)) => outputs[local_index] = Some(output),
                    Ok(Err(error)) => {
                        if scan_error.is_none() {
                            scan_error = Some(error);
                        }
                    }
                    Err(payload) => {
                        if worker_panic.is_none() {
                            worker_panic = Some(payload);
                        }
                    }
                }
            }

            if let Some(payload) = worker_panic {
                panic::resume_unwind(payload);
            }
            if let Some(error) = spawn_error.or(scan_error) {
                return Err(error);
            }
            let partitions = outputs
                .into_iter()
                .map(|output| output.expect("every successful partition returned output"))
                .collect::<Vec<_>>();
            Ok(partitions)
        })
    }

    fn shared_candidate_plan(
        &self,
        partitions: &[MatcherPartition],
        input: &Utf16Text,
    ) -> Result<Option<shared_candidate::SharedCandidatePlan>, Error> {
        if !shared_candidate::is_eligible(
            partitions.len(),
            input.units().len(),
            self.prefilter_enabled,
            self.literal_prefilter_enabled,
        ) {
            return Ok(None);
        }
        let prefilters = partitions
            .iter()
            .map(|partition| &partition.prefilter)
            .collect::<Vec<_>>();
        shared_candidate::plan(
            &prefilters,
            input.units(),
            self.prefilter_enabled,
            self.literal_prefilter_enabled,
        )
    }

    #[cfg(feature = "benchmark-internals")]
    fn cohort_split_partition(&self) -> Option<usize> {
        self.cohort_layout.split_partition
    }

    #[cfg(feature = "benchmark-internals")]
    fn spawned_worker_count(&self) -> usize {
        self.cohort_split_partition().map_or_else(
            || self.partitions.len().saturating_sub(1),
            |split_partition| {
                if self.requested_worker_count == 1 {
                    split_partition.saturating_sub(1)
                        + self
                            .partitions
                            .len()
                            .saturating_sub(split_partition)
                            .saturating_sub(1)
                } else {
                    self.partitions.len().saturating_sub(1)
                }
            },
        )
    }

    #[cfg(all(test, feature = "benchmark-internals"))]
    fn scan_parallel_with_hooks_and_deliver<BeforeSpawn, BeforeScan>(
        &self,
        input: &Utf16Text,
        before_spawn: &BeforeSpawn,
        before_scan: &BeforeScan,
        mut sink: impl FnMut(Match),
    ) -> Result<(), Error>
    where
        BeforeSpawn: Fn(usize) -> Result<(), Error>,
        BeforeScan: Fn(usize) + Sync,
    {
        let output = self.scan_parallel_with_hooks(input, before_spawn, before_scan)?;
        output.deliver(&mut sink);
        Ok(())
    }

    #[cfg(feature = "benchmark-internals")]
    fn diagnostics(&self, stats: engine::ScanStats, buffered_events: usize) -> ScanDiagnostics {
        ScanDiagnostics {
            requested_worker_count: self.requested_worker_count,
            partition_count: self.partitions.len(),
            spawned_workers: self.spawned_worker_count(),
            database_retained_bytes: self
                .partitions
                .iter()
                .map(|partition| partition.database.retained_bytes())
                .sum(),
            total_cache_budget: self
                .partitions
                .iter()
                .map(|partition| partition.state_cache_budget)
                .sum(),
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
        }
    }
}

struct PartitionScanOutput {
    events: Vec<Match>,
    #[cfg(feature = "benchmark-internals")]
    stats: engine::ScanStats,
}

struct ParallelScanOutput {
    partitions: Vec<PartitionScanOutput>,
    #[cfg(feature = "benchmark-internals")]
    stats: engine::ScanStats,
}

impl ParallelScanOutput {
    fn new(partitions: Vec<PartitionScanOutput>) -> Self {
        #[cfg(feature = "benchmark-internals")]
        let mut partition_stats = partitions.iter().map(|partition| partition.stats);
        #[cfg(feature = "benchmark-internals")]
        let mut stats = partition_stats
            .next()
            .expect("parallel scans contain at least two partitions");
        #[cfg(feature = "benchmark-internals")]
        for next in partition_stats {
            stats.merge_partition(next);
        }
        Self {
            partitions,
            #[cfg(feature = "benchmark-internals")]
            stats,
        }
    }

    #[cfg(feature = "benchmark-internals")]
    fn buffered_events(&self) -> usize {
        self.partitions
            .iter()
            .map(|partition| partition.events.len())
            .sum()
    }

    fn deliver(self, sink: &mut impl FnMut(Match)) {
        for partition in self.partitions {
            for event in partition.events {
                sink(event);
            }
        }
    }
}

fn scan_partition(
    partition: &MatcherPartition,
    input: &Utf16Text,
    prefilter_enabled: bool,
    literal_prefilter_enabled: bool,
) -> Result<PartitionScanOutput, Error> {
    let mut events = Vec::new();
    #[cfg(feature = "benchmark-internals")]
    let stats = engine::scan_with_stats(
        &partition.database,
        &partition.prefilter,
        input,
        partition.state_cache_budget,
        prefilter_enabled,
        literal_prefilter_enabled,
        |matched| events.push(matched),
    )?;
    #[cfg(not(feature = "benchmark-internals"))]
    engine::scan(
        &partition.database,
        &partition.prefilter,
        input,
        partition.state_cache_budget,
        prefilter_enabled,
        literal_prefilter_enabled,
        |matched| events.push(matched),
    )?;
    Ok(PartitionScanOutput {
        events,
        #[cfg(feature = "benchmark-internals")]
        stats,
    })
}

fn scan_partition_shared(
    partition: &MatcherPartition,
    input: &Utf16Text,
    shared: SharedCandidate<'_>,
) -> Result<PartitionScanOutput, Error> {
    let mut events = Vec::new();
    #[cfg(feature = "benchmark-internals")]
    let stats = engine::scan_with_shared_candidates_and_stats(
        &partition.database,
        &partition.prefilter,
        input,
        partition.state_cache_budget,
        shared,
        |matched| events.push(matched),
    )?;
    #[cfg(not(feature = "benchmark-internals"))]
    engine::scan_with_shared_candidates_and_stats(
        &partition.database,
        &partition.prefilter,
        input,
        partition.state_cache_budget,
        shared,
        |matched| events.push(matched),
    )
    .map(|_| ())?;
    Ok(PartitionScanOutput {
        events,
        #[cfg(feature = "benchmark-internals")]
        stats,
    })
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "benchmark-internals")]
    use std::cell::Cell;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    #[cfg(feature = "benchmark-internals")]
    use std::sync::{Condvar, Mutex};
    #[cfg(feature = "benchmark-internals")]
    use std::time::Duration;

    #[cfg(feature = "benchmark-internals")]
    use super::{CohortDiagnostics, allocate_cohort_partitions};
    use super::{Matcher, MatcherBuilder};
    use crate::{Error, PatternId, Utf16Text};

    #[test]
    fn partition_assignment_and_cache_budgets_are_balanced() -> Result<(), Error> {
        // Prepare
        let mut builder = MatcherBuilder::new();
        builder.worker_count(3).state_cache_budget(8);
        for pattern_id in 1..=10 {
            builder.add(PatternId::new(pattern_id), &format!("p{pattern_id}"))?;
        }

        // Test
        let matcher = builder.build()?;
        let pattern_counts = matcher
            .partitions
            .iter()
            .map(|partition| partition.database.pattern_count())
            .collect::<Vec<_>>();
        let cache_budgets = matcher
            .partitions
            .iter()
            .map(|partition| partition.state_cache_budget)
            .collect::<Vec<_>>();

        // Assert
        assert_eq!(pattern_counts, [4, 3, 3]);
        assert_eq!(cache_budgets, [3, 3, 2]);
        Ok(())
    }

    #[test]
    fn oversubscribed_request_is_bounded_by_pattern_count() -> Result<(), Error> {
        // Prepare
        let mut builder = MatcherBuilder::new();
        builder.worker_count(1_000);
        builder.add(PatternId::new(1), "one")?;
        builder.add(PatternId::new(2), "two")?;

        // Test
        let matcher = builder.build()?;

        // Assert
        #[cfg(feature = "benchmark-internals")]
        assert_eq!(matcher.requested_worker_count, 1_000);
        assert_eq!(matcher.partitions.len(), 2);
        Ok(())
    }

    #[test]
    fn controlled_spawn_failure_returns_before_delivery() -> Result<(), Error> {
        // Prepare
        let matcher = parallel_test_matcher()?;
        let input = Utf16Text::from("one two three");
        let before_spawn = |partition_index| {
            if partition_index == 2 {
                Err(Error::WorkerUnavailable)
            } else {
                Ok(())
            }
        };

        // Test
        let result = matcher.scan_parallel_with_hooks(&input, &before_spawn, &|_| {});

        // Assert
        assert!(matches!(result, Err(Error::WorkerUnavailable)));
        Ok(())
    }

    #[test]
    fn controlled_worker_panic_is_joined_and_matcher_remains_reusable() -> Result<(), Error> {
        // Prepare
        let matcher = parallel_test_matcher()?;
        let input = Utf16Text::from("one two three");

        // Test
        let panic_result = catch_unwind(AssertUnwindSafe(|| {
            let _ = matcher.scan_parallel_with_hooks(&input, &|_| Ok(()), &|partition_index| {
                assert_ne!(partition_index, 1, "controlled worker panic");
            });
        }));
        let mut recovered = Vec::new();
        matcher.scan(&input, |matched| recovered.push(matched.pattern_id()))?;

        // Assert
        assert!(panic_result.is_err());
        recovered.sort_unstable();
        assert_eq!(
            recovered,
            [PatternId::new(1), PatternId::new(2), PatternId::new(3)]
        );
        Ok(())
    }

    #[test]
    fn shared_parallel_candidates_match_the_private_path() -> Result<(), Error> {
        // Prepare
        let single = literal_test_matcher(1)?;
        let parallel = literal_test_matcher(2)?;
        let mut units = vec![u16::from(b'x'); 1024 * 1024];
        place(&mut units, 128, "left0255needle");
        place(&mut units, 65_664, "right0511needle");
        place(&mut units, 131_200, "left9999near-miss");
        let input = Utf16Text::from_units(units);

        // Test
        let single_events = collect_events(&single, &input)?;
        let parallel_events = collect_events(&parallel, &input)?;

        // Assert
        assert_eq!(parallel_events, single_events);
        assert_eq!(parallel_events, [(255, 128, 142), (511, 65_664, 65_679)]);
        Ok(())
    }

    #[test]
    fn matcher_is_send_and_sync() {
        // Prepare / Test
        fn assert_send_sync<T: Send + Sync>() {}

        // Assert
        assert_send_sync::<Matcher>();
    }

    #[cfg(feature = "benchmark-internals")]
    #[test]
    fn cohort_diagnostics_are_stable_across_worker_partitioning() -> Result<(), Error> {
        // Prepare
        let (single_diagnostics, single) = cohort_test_matcher(1)?;
        let (parallel_diagnostics, parallel) = cohort_test_matcher(4)?;
        let input = Utf16Text::from("abc\ncat far");

        // Test
        let single_events = collect_events(&single, &input)?;
        let parallel_events = collect_events(&parallel, &input)?;

        // Assert
        assert_eq!(single_diagnostics, parallel_diagnostics);
        assert_eq!(parallel_events, single_events);
        assert_eq!(
            single_diagnostics.assertion_free_pattern_ids(),
            [PatternId::new(10), PatternId::new(30)]
        );
        assert_eq!(
            single_diagnostics.assertion_bearing_pattern_ids(),
            [PatternId::new(20), PatternId::new(40)]
        );
        Ok(())
    }

    #[cfg(feature = "benchmark-internals")]
    #[test]
    fn cohort_partition_allocation_is_bounded_balanced_and_deterministic() {
        // Prepare / Test / Assert
        assert_eq!(allocate_cohort_partitions([8, 2], 1), [1, 1]);
        assert_eq!(allocate_cohort_partitions([8, 2], 4), [3, 1]);
        assert_eq!(allocate_cohort_partitions([8, 2], 8), [6, 2]);
        assert_eq!(allocate_cohort_partitions([1, 9], 4), [1, 3]);
        assert_eq!(allocate_cohort_partitions([4, 4], 99), [4, 4]);
    }

    #[cfg(feature = "benchmark-internals")]
    #[test]
    fn mixed_cohorts_preserve_global_ids_and_total_resource_budgets() -> Result<(), Error> {
        // Prepare
        let matcher = mixed_cohort_test_matcher(4, 9, true)?;

        // Test
        let layout = matcher.cohort_execution_diagnostics();
        let assertion_free_ids = matcher.partitions[..2]
            .iter()
            .flat_map(|partition| {
                (0..partition.database.pattern_count())
                    .map(|ordinal| partition.database.pattern_id(ordinal).get())
            })
            .collect::<Vec<_>>();
        let assertion_bearing_ids = matcher.partitions[2..]
            .iter()
            .flat_map(|partition| {
                (0..partition.database.pattern_count())
                    .map(|ordinal| partition.database.pattern_id(ordinal).get())
            })
            .collect::<Vec<_>>();

        // Assert
        assert!(layout.enabled());
        assert_eq!(layout.cohort_count(), 2);
        assert_eq!(layout.assertion_free_pattern_count(), 4);
        assert_eq!(layout.assertion_bearing_pattern_count(), 4);
        assert_eq!(layout.assertion_free_partition_count(), 2);
        assert_eq!(layout.assertion_bearing_partition_count(), 2);
        assert_eq!(layout.total_partition_count(), 4);
        assert_eq!(layout.total_cache_budget(), 9);
        assert!(layout.assertion_free_retained_bytes() > 0);
        assert!(layout.assertion_bearing_retained_bytes() > 0);
        assert_eq!(layout.spawned_workers(), 3);
        assert!(layout.buffers_before_delivery());
        assert_eq!(assertion_free_ids, [10, 30, 50, 70]);
        assert_eq!(assertion_bearing_ids, [20, 40, 60, 80]);
        assert!(
            matcher.partitions[..2]
                .iter()
                .all(|partition| !partition.database.uses_assertions())
        );
        assert!(
            matcher.partitions[2..]
                .iter()
                .all(|partition| partition.database.uses_assertions())
        );
        assert_eq!(
            matcher
                .partitions
                .iter()
                .map(|partition| partition.state_cache_budget)
                .collect::<Vec<_>>(),
            [3, 2, 2, 2]
        );
        Ok(())
    }

    #[cfg(feature = "benchmark-internals")]
    #[test]
    fn one_cohort_keeps_the_direct_unbuffered_scan_path() -> Result<(), Error> {
        // Prepare
        let mut assertion_free_builder = MatcherBuilder::new();
        assertion_free_builder.cohort_compilation_enabled(true);
        assertion_free_builder.add(PatternId::new(1), "abc")?;
        let assertion_free = assertion_free_builder.build()?;
        let mut assertion_bearing_builder = MatcherBuilder::new();
        assertion_bearing_builder.cohort_compilation_enabled(true);
        assertion_bearing_builder.add(PatternId::new(2), "^abc$")?;
        let assertion_bearing = assertion_bearing_builder.build()?;
        let input = Utf16Text::from("abc");

        // Test
        let free_scan = assertion_free.scan_with_diagnostics(&input, |_| {})?;
        let bearing_scan = assertion_bearing.scan_with_diagnostics(&input, |_| {})?;
        let free_layout = assertion_free.cohort_execution_diagnostics();
        let bearing_layout = assertion_bearing.cohort_execution_diagnostics();

        // Assert
        assert_eq!(free_layout.cohort_count(), 1);
        assert_eq!(free_layout.assertion_free_pattern_count(), 1);
        assert_eq!(free_layout.assertion_bearing_pattern_count(), 0);
        assert_eq!(bearing_layout.cohort_count(), 1);
        assert_eq!(bearing_layout.assertion_free_pattern_count(), 0);
        assert_eq!(bearing_layout.assertion_bearing_pattern_count(), 1);
        assert!(free_layout.assertion_free_retained_bytes() > 0);
        assert_eq!(free_layout.assertion_bearing_retained_bytes(), 0);
        assert_eq!(bearing_layout.assertion_free_retained_bytes(), 0);
        assert!(bearing_layout.assertion_bearing_retained_bytes() > 0);
        assert!(!free_layout.buffers_before_delivery());
        assert!(!bearing_layout.buffers_before_delivery());
        assert_eq!(free_scan.buffered_events(), 0);
        assert_eq!(bearing_scan.buffered_events(), 0);
        Ok(())
    }

    #[cfg(feature = "benchmark-internals")]
    #[test]
    fn mixed_cohorts_match_the_unsplit_multiset_across_worker_counts() -> Result<(), Error> {
        // Prepare
        let input = mixed_cohort_input();

        // Test / Assert
        for worker_count in [1, 2, 4, 8] {
            let baseline = mixed_cohort_test_matcher(worker_count, 17, false)?;
            let cohort = mixed_cohort_test_matcher(worker_count, 17, true)?;
            let expected = collect_events(&baseline, &input)?;
            let first = collect_events(&cohort, &input)?;
            let second = collect_events(&cohort, &input)?;
            let mut diagnostic_events = Vec::new();
            let scan_diagnostics = cohort.scan_with_diagnostics(&input, |matched| {
                diagnostic_events.push((
                    matched.pattern_id().get(),
                    matched.span().start(),
                    matched.span().end(),
                ));
            })?;
            diagnostic_events.sort_unstable();

            assert_eq!(first, expected, "worker count {worker_count}");
            assert_eq!(second, expected, "worker count {worker_count}");
            assert_eq!(diagnostic_events, expected, "worker count {worker_count}");
            assert_eq!(scan_diagnostics.buffered_events(), expected.len());
            assert_eq!(scan_diagnostics.cache_budget(), 17);
            let layout = cohort.cohort_execution_diagnostics();
            assert_eq!(layout.total_cache_budget(), 17);
            if worker_count == 1 {
                assert_eq!(layout.total_partition_count(), 2);
                assert_eq!(layout.spawned_workers(), 0);
            }
        }
        Ok(())
    }

    #[cfg(feature = "benchmark-internals")]
    #[test]
    fn mixed_cohort_partitions_share_one_bounded_multi_worker_scope() -> Result<(), Error> {
        // Prepare
        let matcher = mixed_cohort_test_matcher(4, 17, true)?;
        let input = mixed_cohort_input();
        let starts = ScanStartGate::new(4);

        // Test
        let output = matcher.scan_parallel_with_hooks(&input, &|_| Ok(()), &|partition_index| {
            starts.enter(partition_index);
        })?;
        let mut entered = starts.entered();
        entered.sort_unstable();

        // Assert
        assert_eq!(entered, [0, 1, 2, 3]);
        assert_eq!(output.partitions.len(), 4);
        assert_eq!(matcher.spawned_worker_count(), 3);
        Ok(())
    }

    #[cfg(feature = "benchmark-internals")]
    #[test]
    fn one_requested_worker_keeps_mixed_cohorts_sequential() -> Result<(), Error> {
        // Prepare
        let matcher = mixed_cohort_test_matcher(1, 17, true)?;
        let input = mixed_cohort_input();
        let spawn_count = Cell::new(0_usize);
        let scan_order = Mutex::new(Vec::new());

        // Test
        let output = matcher.scan_parallel_with_hooks(
            &input,
            &|_| {
                spawn_count.set(spawn_count.get() + 1);
                Ok(())
            },
            &|partition_index| {
                scan_order
                    .lock()
                    .expect("scan order lock must not be poisoned")
                    .push(partition_index);
            },
        )?;

        // Assert
        assert_eq!(spawn_count.get(), 0);
        assert_eq!(
            *scan_order
                .lock()
                .expect("scan order lock must not be poisoned"),
            [0, 1]
        );
        assert_eq!(output.partitions.len(), 2);
        assert_eq!(matcher.spawned_worker_count(), 0);
        Ok(())
    }

    #[cfg(feature = "benchmark-internals")]
    #[test]
    fn every_cohort_spawn_failure_precedes_delivery_and_preserves_reuse() -> Result<(), Error> {
        // Prepare / Test / Assert
        let input = mixed_cohort_input();
        let mut injected_failures = 0;
        for worker_count in [2, 3, 4, 8] {
            let matcher = mixed_cohort_test_matcher(worker_count, 9, true)?;
            let spawnable_partitions = (1..matcher.partitions.len()).collect::<Vec<_>>();
            for failed_partition in spawnable_partitions {
                let delivered = Cell::new(0_usize);
                let before_spawn = |partition_index| {
                    if partition_index == failed_partition {
                        Err(Error::WorkerUnavailable)
                    } else {
                        Ok(())
                    }
                };
                let result = matcher.scan_parallel_with_hooks_and_deliver(
                    &input,
                    &before_spawn,
                    &|_| {},
                    |_| delivered.set(delivered.get() + 1),
                );

                assert!(matches!(result, Err(Error::WorkerUnavailable)));
                assert_eq!(delivered.get(), 0);
                assert!(!collect_events(&matcher, &input)?.is_empty());
                injected_failures += 1;
            }
        }

        assert_eq!(injected_failures, 13);
        Ok(())
    }

    #[cfg(feature = "benchmark-internals")]
    #[test]
    fn every_cohort_partition_panic_precedes_delivery_and_preserves_reuse() -> Result<(), Error> {
        // Prepare / Test / Assert
        let input = mixed_cohort_input();
        let mut injected_panics = 0;
        for worker_count in [1, 2, 3, 4, 8] {
            let matcher = mixed_cohort_test_matcher(worker_count, 9, true)?;
            for failed_partition in 0..matcher.partitions.len() {
                let delivered = Cell::new(0_usize);
                let panic_result = catch_unwind(AssertUnwindSafe(|| {
                    let _ = matcher.scan_parallel_with_hooks_and_deliver(
                        &input,
                        &|_| Ok(()),
                        &|partition_index| {
                            assert_ne!(
                                partition_index, failed_partition,
                                "controlled cohort partition panic"
                            );
                        },
                        |_| delivered.set(delivered.get() + 1),
                    );
                }));

                assert!(panic_result.is_err());
                assert_eq!(delivered.get(), 0);
                assert!(!collect_events(&matcher, &input)?.is_empty());
                injected_panics += 1;
            }
        }

        assert_eq!(injected_panics, 19);
        Ok(())
    }

    #[cfg(feature = "benchmark-internals")]
    #[test]
    fn sink_panic_parity_leaves_ordinary_and_cohort_matchers_reusable() -> Result<(), Error> {
        // Prepare / Test / Assert
        let input = mixed_cohort_input();
        for worker_count in [1, 2, 4, 8] {
            for cohort_enabled in [false, true] {
                let matcher = mixed_cohort_test_matcher(worker_count, 9, cohort_enabled)?;
                let expected = collect_events(&matcher, &input)?;
                let panic_result = catch_unwind(AssertUnwindSafe(|| {
                    let _ = matcher.scan(&input, |_| panic!("controlled cohort sink panic"));
                }));
                let recovered = collect_events(&matcher, &input)?;

                assert!(panic_result.is_err());
                assert_eq!(recovered, expected);
            }
        }
        Ok(())
    }

    #[cfg(feature = "benchmark-internals")]
    #[test]
    fn simultaneous_scans_preserve_ordinary_and_cohort_parity() -> Result<(), Error> {
        // Prepare
        let inputs = [
            mixed_cohort_input(),
            Utf16Text::from("foo\nabc"),
            Utf16Text::from_units(vec![0xd800, u16::from(b'a'), u16::from(b'r')]),
            Utf16Text::from("no matches here"),
        ];

        // Test / Assert
        for cohort_enabled in [false, true] {
            let matcher = mixed_cohort_test_matcher(4, 17, cohort_enabled)?;
            let simultaneous = std::thread::scope(|scope| {
                inputs
                    .iter()
                    .map(|input| scope.spawn(|| collect_events(&matcher, input)))
                    .collect::<Vec<_>>()
                    .into_iter()
                    .map(|handle| handle.join().expect("bounded scan must not panic"))
                    .collect::<Result<Vec<_>, Error>>()
            })?;
            let sequential = inputs
                .iter()
                .map(|input| collect_events(&matcher, input))
                .collect::<Result<Vec<_>, Error>>()?;

            assert_eq!(simultaneous, sequential);
        }
        Ok(())
    }

    fn parallel_test_matcher() -> Result<Matcher, Error> {
        let mut builder = MatcherBuilder::new();
        builder.worker_count(3);
        builder.add(PatternId::new(1), "one")?;
        builder.add(PatternId::new(2), "two")?;
        builder.add(PatternId::new(3), "three")?;
        builder.build()
    }

    fn literal_test_matcher(worker_count: usize) -> Result<Matcher, Error> {
        let mut builder = MatcherBuilder::new();
        builder.worker_count(worker_count);
        for ordinal in 0..512_u32 {
            let prefix = if ordinal < 256 { "left" } else { "right" };
            builder.add(
                PatternId::new(ordinal),
                &format!("{prefix}{ordinal:04}needle"),
            )?;
        }
        builder.build()
    }

    #[cfg(feature = "benchmark-internals")]
    fn cohort_test_matcher(worker_count: usize) -> Result<(CohortDiagnostics, Matcher), Error> {
        let mut builder = MatcherBuilder::new();
        builder.worker_count(worker_count);
        builder.add(PatternId::new(10), "abc")?;
        builder.add(PatternId::new(20), "^abc$")?;
        builder.add(PatternId::new(30), "(?:far|foo)")?;
        builder.add(PatternId::new(40), r"\bcat")?;
        let diagnostics = builder.cohort_diagnostics();
        builder.build().map(|matcher| (diagnostics, matcher))
    }

    #[cfg(feature = "benchmark-internals")]
    fn mixed_cohort_test_matcher(
        worker_count: usize,
        cache_budget: usize,
        cohort_enabled: bool,
    ) -> Result<Matcher, Error> {
        let mut builder = MatcherBuilder::new();
        builder
            .worker_count(worker_count)
            .state_cache_budget(cache_budget)
            .cohort_compilation_enabled(cohort_enabled);
        builder.add(PatternId::new(10), "abc")?;
        builder.add(PatternId::new(20), "^abc$")?;
        builder.add(PatternId::new(30), "(?:far|foo)")?;
        builder.add(PatternId::new(40), r"\bcat")?;
        builder.add(PatternId::new(50), "abc")?;
        builder.add(PatternId::new(60), "foo$")?;
        builder.add(PatternId::new(70), "a+")?;
        builder.add(PatternId::new(80), r"far\B")?;
        builder.build()
    }

    #[cfg(feature = "benchmark-internals")]
    fn mixed_cohort_input() -> Utf16Text {
        Utf16Text::from("abc\ncat far foo\naaa\nabc")
    }

    fn collect_events(matcher: &Matcher, input: &Utf16Text) -> Result<Vec<(u32, u64, u64)>, Error> {
        let mut events = Vec::new();
        matcher.scan(input, |matched| {
            events.push((
                matched.pattern_id().get(),
                matched.span().start(),
                matched.span().end(),
            ));
        })?;
        events.sort_unstable();
        Ok(events)
    }

    fn place(input: &mut [u16], start: usize, text: &str) {
        let encoded = text.encode_utf16().collect::<Vec<_>>();
        input[start..start + encoded.len()].copy_from_slice(&encoded);
    }

    #[cfg(feature = "benchmark-internals")]
    struct ScanStartGate {
        expected: usize,
        entered: Mutex<Vec<usize>>,
        all_entered: Condvar,
    }

    #[cfg(feature = "benchmark-internals")]
    impl ScanStartGate {
        fn new(expected: usize) -> Self {
            Self {
                expected,
                entered: Mutex::new(Vec::with_capacity(expected)),
                all_entered: Condvar::new(),
            }
        }

        fn enter(&self, partition_index: usize) {
            let mut entered = self
                .entered
                .lock()
                .expect("scan-start gate must not be poisoned");
            entered.push(partition_index);
            if entered.len() == self.expected {
                self.all_entered.notify_all();
                return;
            }
            let (entered, _) = self
                .all_entered
                .wait_timeout_while(entered, Duration::from_secs(2), |entered| {
                    entered.len() < self.expected
                })
                .expect("scan-start gate must not be poisoned");
            assert_eq!(
                entered.len(),
                self.expected,
                "all cohort partitions must start before any partition scans"
            );
        }

        fn entered(&self) -> Vec<usize> {
            self.entered
                .lock()
                .expect("scan-start gate must not be poisoned")
                .clone()
        }
    }
}
