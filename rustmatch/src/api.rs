//! Minimal public lifecycle over the private compiler and engine.

use std::any::Any;
use std::collections::HashSet;
use std::mem::size_of;
use std::panic;
use std::thread;

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
    worker_count: usize,
    prefilter_enabled: bool,
    literal_prefilter_enabled: bool,
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
        if self.worker_count == 0 {
            return Err(Error::InvalidWorkerCount);
        }
        let partitions =
            compile_partitions(&self.patterns, self.worker_count, self.state_cache_budget)?;
        Ok(Matcher {
            partitions,
            requested_worker_count: self.worker_count,
            prefilter_enabled: self.prefilter_enabled,
            literal_prefilter_enabled: self.literal_prefilter_enabled,
        })
    }
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
    requested_worker_count: usize,
    prefilter_enabled: bool,
    literal_prefilter_enabled: bool,
}

#[derive(Debug)]
struct MatcherPartition {
    database: PatternDatabase,
    prefilter: Prefilter,
    state_cache_budget: usize,
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
        thread::scope(|scope| {
            let mut handles = Vec::with_capacity(self.partitions.len() - 1);
            let mut spawn_error = None;
            for (partition_index, partition) in self.partitions.iter().enumerate().skip(1) {
                if let Err(error) = before_spawn(partition_index) {
                    spawn_error = Some(error);
                    break;
                }
                let worker = thread::Builder::new()
                    .name(format!("rustmatch-worker-{partition_index}"))
                    .spawn_scoped(scope, move || {
                        before_scan(partition_index);
                        scan_partition(
                            partition,
                            input,
                            self.prefilter_enabled,
                            self.literal_prefilter_enabled,
                        )
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
                Some(scan_partition(
                    &self.partitions[0],
                    input,
                    self.prefilter_enabled,
                    self.literal_prefilter_enabled,
                ))
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
            let partitions = outputs
                .into_iter()
                .map(|output| output.expect("every successful partition returned output"))
                .collect::<Vec<_>>();
            Ok(ParallelScanOutput::new(partitions))
        })
    }

    #[cfg(feature = "benchmark-internals")]
    fn diagnostics(&self, stats: engine::ScanStats, buffered_events: usize) -> ScanDiagnostics {
        ScanDiagnostics {
            requested_worker_count: self.requested_worker_count,
            partition_count: self.partitions.len(),
            spawned_workers: self.partitions.len().saturating_sub(1),
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
    stats: engine::ScanStats,
}

struct ParallelScanOutput {
    partitions: Vec<PartitionScanOutput>,
    stats: engine::ScanStats,
}

impl ParallelScanOutput {
    fn new(partitions: Vec<PartitionScanOutput>) -> Self {
        let mut partition_stats = partitions.iter().map(|partition| partition.stats);
        let mut stats = partition_stats
            .next()
            .expect("parallel scans contain at least two partitions");
        for next in partition_stats {
            stats.merge_partition(next);
        }
        Self { partitions, stats }
    }

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
    let stats = engine::scan_with_stats(
        &partition.database,
        &partition.prefilter,
        input,
        partition.state_cache_budget,
        prefilter_enabled,
        literal_prefilter_enabled,
        |matched| events.push(matched),
    )?;
    Ok(PartitionScanOutput { events, stats })
}

#[cfg(test)]
mod tests {
    use std::panic::{AssertUnwindSafe, catch_unwind};

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
    fn matcher_is_send_and_sync() {
        // Prepare / Test
        fn assert_send_sync<T: Send + Sync>() {}

        // Assert
        assert_send_sync::<Matcher>();
    }

    fn parallel_test_matcher() -> Result<Matcher, Error> {
        let mut builder = MatcherBuilder::new();
        builder.worker_count(3);
        builder.add(PatternId::new(1), "one")?;
        builder.add(PatternId::new(2), "two")?;
        builder.add(PatternId::new(3), "three")?;
        builder.build()
    }
}
