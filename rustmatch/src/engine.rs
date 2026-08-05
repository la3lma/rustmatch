//! Forward NFA scan with scan-local, reusable scratch storage.

use std::collections::HashMap;

use crate::hir::Assertion;
use crate::nfa::{EdgeKind, PatternDatabase, StateId};
use crate::prefilter::{Prefilter, PrefilterBypass, PrefilterPath, ScanPlan};
use crate::{Error, Match, Utf16Span, Utf16Text};

pub(crate) const DEFAULT_STATE_CACHE_BUDGET: usize = 8_192;

#[path = "shared_engine.rs"]
mod shared;
pub(crate) use shared::scan_with_shared_candidates_and_stats;

pub(crate) fn scan(
    database: &PatternDatabase,
    prefilter: &Prefilter,
    input: &Utf16Text,
    state_cache_budget: usize,
    prefilter_enabled: bool,
    literal_prefilter_enabled: bool,
    sink: impl FnMut(Match),
) -> Result<(), Error> {
    scan_with_stats(
        database,
        prefilter,
        input,
        state_cache_budget,
        prefilter_enabled,
        literal_prefilter_enabled,
        sink,
    )
    .map(|_| ())
}

pub(crate) fn scan_with_stats(
    database: &PatternDatabase,
    prefilter: &Prefilter,
    input: &Utf16Text,
    state_cache_budget: usize,
    prefilter_enabled: bool,
    literal_prefilter_enabled: bool,
    mut sink: impl FnMut(Match),
) -> Result<ScanStats, Error> {
    let units = input.units();
    let plan = prefilter.plan(units, prefilter_enabled, literal_prefilter_enabled);
    let mut metrics = ScanStats {
        prefilter_path: plan.path(),
        prefilter_bypass: plan.bypass(),
        prefilter_retained_bytes: plan.retained_bytes(),
        prefilter_candidate_bytes: plan.candidate_bytes(),
        prefilter_admissions: plan.admissions(),
        prefilter_candidate_starts: plan.candidate_count(),
        ..ScanStats::default()
    };

    if database.uses_assertions() {
        scan_with_assertions(database, input, &mut sink)?;
        metrics.assertion_bypasses = 1;
        metrics.prefilter_starts_scanned = units.len();
    } else {
        match &plan {
            ScanPlan::All { .. } => {
                metrics.prefilter_starts_scanned = units.len();
                scan_without_assertions_dispatch(
                    database,
                    input,
                    state_cache_budget,
                    0..units.len(),
                    &mut metrics,
                    &mut sink,
                )?;
            }
            ScanPlan::StartTable { table, .. } => {
                let mut starts_scanned = 0_usize;
                let starts = (0..units.len())
                    .filter(|&start| table.allows(units, start))
                    .inspect(|_| starts_scanned += 1);
                scan_without_assertions_dispatch(
                    database,
                    input,
                    state_cache_budget,
                    starts,
                    &mut metrics,
                    &mut sink,
                )?;
                metrics.prefilter_starts_scanned = starts_scanned;
            }
            ScanPlan::Candidates { candidates, .. } => {
                metrics.prefilter_starts_scanned = candidates.count();
                scan_without_assertions_dispatch(
                    database,
                    input,
                    state_cache_budget,
                    candidates.iter(),
                    &mut metrics,
                    &mut sink,
                )?;
            }
        }
    }
    metrics.prefilter_starts_skipped = units.len().saturating_sub(metrics.prefilter_starts_scanned);
    Ok(metrics)
}

fn scan_without_assertions_dispatch(
    database: &PatternDatabase,
    input: &Utf16Text,
    state_cache_budget: usize,
    starts: impl Iterator<Item = usize>,
    metrics: &mut ScanStats,
    sink: impl FnMut(Match),
) -> Result<(), Error> {
    if state_cache_budget == 0 {
        scan_without_assertions_nfa(database, input, starts, sink)
    } else {
        scan_without_assertions_cached(database, input, state_cache_budget, starts, metrics, sink)
    }
}

// Keep this loop structurally identical to the pre-assertion engine. Context
// support is pay-for-use and must not enlarge the ordinary transition path.
fn scan_without_assertions_nfa(
    database: &PatternDatabase,
    input: &Utf16Text,
    starts: impl Iterator<Item = usize>,
    mut sink: impl FnMut(Match),
) -> Result<(), Error> {
    let units = input.units();
    let mut scratch = Scratch::new(database);

    for start in starts {
        scratch.reset_start();
        extend_epsilon_closure(
            database,
            database.root(),
            &mut scratch.active,
            &mut scratch.active_seen,
            &mut scratch.stack,
        );

        for (position, &symbol) in units.iter().enumerate().skip(start) {
            scratch.reset_next();
            for &state in &scratch.active {
                for edge in database.edges_from(state) {
                    if database.edge_matches(edge.kind, symbol) {
                        extend_epsilon_closure(
                            database,
                            edge.target,
                            &mut scratch.next,
                            &mut scratch.next_seen,
                            &mut scratch.stack,
                        );
                    }
                }
            }

            let end = position
                .checked_add(1)
                .ok_or(Error::InputTooLarge)
                .and_then(position_utf16)?;
            for &state in &scratch.next {
                for &ordinal in database.terminals_at(state) {
                    record_terminal(
                        &mut scratch.best_end,
                        &mut scratch.touched_ordinals,
                        ordinal,
                        end,
                    );
                }
            }
            std::mem::swap(&mut scratch.active, &mut scratch.next);
            std::mem::swap(&mut scratch.active_seen, &mut scratch.next_seen);
            if scratch.active.is_empty() {
                break;
            }
        }

        let start_utf16 = position_utf16(start)?;
        scratch.touched_ordinals.sort_unstable();
        for &ordinal in &scratch.touched_ordinals {
            let end_utf16 = scratch.best_end[ordinal].expect("a touched terminal has an end");
            sink(Match::new(
                database.pattern_id(ordinal),
                Utf16Span::from_bounds(start_utf16, end_utf16),
            ));
        }
    }
    Ok(())
}

fn scan_without_assertions_cached(
    database: &PatternDatabase,
    input: &Utf16Text,
    state_cache_budget: usize,
    starts: impl Iterator<Item = usize>,
    metrics: &mut ScanStats,
    mut sink: impl FnMut(Match),
) -> Result<(), Error> {
    let units = input.units();
    let mut scratch = Scratch::new(database);
    let mut cache = DeterministicCache::new(database, state_cache_budget, &mut scratch);

    for start in starts {
        scratch.reset_cached_start();
        let mut cursor = ScanCursor::Cached(0);

        for (position, &symbol) in units.iter().enumerate().skip(start) {
            let outcome = match cursor {
                ScanCursor::Cached(deterministic_state) => {
                    cache.transition(database, deterministic_state, symbol, &mut scratch, metrics)
                }
                ScanCursor::Uncached => {
                    metrics.fallback_transitions += 1;
                    let generation = scratch.reset_cached_transition();
                    compute_transition(
                        database,
                        &scratch.active,
                        symbol,
                        &mut scratch.next,
                        &mut scratch.cache_seen,
                        generation,
                        &mut scratch.stack,
                    );
                    if scratch.next.is_empty() {
                        TransitionOutcome::Dead
                    } else if let Some(deterministic_state) = cache.lookup(&scratch.next) {
                        TransitionOutcome::Cached(deterministic_state)
                    } else {
                        TransitionOutcome::Uncached
                    }
                }
            };

            let end = position
                .checked_add(1)
                .ok_or(Error::InputTooLarge)
                .and_then(position_utf16)?;
            match outcome {
                TransitionOutcome::Dead => break,
                TransitionOutcome::Cached(deterministic_state) => {
                    for &ordinal in cache.terminals(deterministic_state) {
                        record_terminal(
                            &mut scratch.best_end,
                            &mut scratch.touched_ordinals,
                            ordinal,
                            end,
                        );
                    }
                    cursor = ScanCursor::Cached(deterministic_state);
                }
                TransitionOutcome::Uncached => {
                    for &state in &scratch.next {
                        for &ordinal in database.terminals_at(state) {
                            record_terminal(
                                &mut scratch.best_end,
                                &mut scratch.touched_ordinals,
                                ordinal,
                                end,
                            );
                        }
                    }
                    std::mem::swap(&mut scratch.active, &mut scratch.next);
                    cursor = ScanCursor::Uncached;
                }
            }
        }

        let start_utf16 = position_utf16(start)?;
        scratch.touched_ordinals.sort_unstable();
        for &ordinal in &scratch.touched_ordinals {
            let end_utf16 = scratch.best_end[ordinal].expect("a touched terminal has an end");
            sink(Match::new(
                database.pattern_id(ordinal),
                Utf16Span::from_bounds(start_utf16, end_utf16),
            ));
        }
    }

    metrics.cache_states = cache.len();
    metrics.cache_table_bytes = cache.table_bytes();
    Ok(())
}

fn compute_transition(
    database: &PatternDatabase,
    source: &[StateId],
    symbol: u16,
    output: &mut Vec<StateId>,
    visited: &mut [u32],
    generation: u32,
    stack: &mut Vec<StateId>,
) {
    for &state in source {
        for edge in database.edges_from(state) {
            if database.edge_matches(edge.kind, symbol) {
                extend_epsilon_closure_generation(
                    database,
                    edge.target,
                    output,
                    visited,
                    generation,
                    stack,
                );
            }
        }
    }
    output.sort_unstable();
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScanCursor {
    Cached(u32),
    Uncached,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TransitionOutcome {
    Dead,
    Cached(u32),
    Uncached,
}

const UNKNOWN_TRANSITION: u32 = u32::MAX;
const DEAD_TRANSITION: u32 = u32::MAX - 1;
const FALLBACK_TRANSITION: u32 = u32::MAX - 2;

struct DeterministicState {
    nfa_states: Box<[StateId]>,
    terminals: Box<[usize]>,
    ascii_transitions: Box<[u32; 128]>,
    non_ascii_transitions: Vec<(u16, u32)>,
}

impl DeterministicState {
    fn new(database: &PatternDatabase, nfa_states: Box<[StateId]>) -> Self {
        let terminals = nfa_states
            .iter()
            .flat_map(|&state| database.terminals_at(state).iter().copied())
            .collect();
        Self {
            nfa_states,
            terminals,
            ascii_transitions: Box::new([UNKNOWN_TRANSITION; 128]),
            non_ascii_transitions: Vec::new(),
        }
    }

    fn transition(&self, symbol: u16) -> u32 {
        if symbol < 128 {
            self.ascii_transitions[usize::from(symbol)]
        } else {
            self.non_ascii_transitions
                .iter()
                .find_map(|&(candidate, transition)| (candidate == symbol).then_some(transition))
                .unwrap_or(UNKNOWN_TRANSITION)
        }
    }

    fn set_transition(&mut self, symbol: u16, transition: u32) {
        if symbol < 128 {
            self.ascii_transitions[usize::from(symbol)] = transition;
        } else {
            self.non_ascii_transitions.push((symbol, transition));
        }
    }
}

struct DeterministicCache {
    budget: usize,
    states: Vec<DeterministicState>,
    hash_buckets: HashMap<u64, Vec<u32>>,
}

impl DeterministicCache {
    fn new(database: &PatternDatabase, budget: usize, scratch: &mut Scratch) -> Self {
        debug_assert!(budget > 0);
        scratch.reset_start();
        extend_epsilon_closure(
            database,
            database.root(),
            &mut scratch.active,
            &mut scratch.active_seen,
            &mut scratch.stack,
        );
        scratch.active.sort_unstable();
        let root_states = std::mem::take(&mut scratch.active).into_boxed_slice();
        let mut cache = Self {
            budget,
            states: Vec::with_capacity(budget.min(64)),
            hash_buckets: HashMap::new(),
        };
        let root = cache
            .intern(database, &root_states)
            .expect("a nonzero cache budget must admit its root state");
        debug_assert_eq!(root, 0);
        cache
    }

    fn len(&self) -> usize {
        self.states.len()
    }

    fn table_bytes(&self) -> usize {
        self.states.len() * 128 * std::mem::size_of::<u32>()
    }

    fn terminals(&self, state: u32) -> &[usize] {
        &self.states[state as usize].terminals
    }

    fn lookup(&self, nfa_states: &[StateId]) -> Option<u32> {
        self.hash_buckets
            .get(&state_set_hash(nfa_states))
            .and_then(|candidates| {
                candidates.iter().copied().find(|&candidate| {
                    self.states[candidate as usize].nfa_states.as_ref() == nfa_states
                })
            })
    }

    fn intern(&mut self, database: &PatternDatabase, nfa_states: &[StateId]) -> Option<u32> {
        if let Some(state) = self.lookup(nfa_states) {
            return Some(state);
        }
        if self.states.len() >= self.budget || self.states.len() >= FALLBACK_TRANSITION as usize {
            return None;
        }
        let state = u32::try_from(self.states.len()).expect("cache state IDs are bounded");
        self.states.push(DeterministicState::new(
            database,
            nfa_states.to_vec().into_boxed_slice(),
        ));
        self.hash_buckets
            .entry(state_set_hash(nfa_states))
            .or_default()
            .push(state);
        Some(state)
    }

    fn transition(
        &mut self,
        database: &PatternDatabase,
        deterministic_state: u32,
        symbol: u16,
        scratch: &mut Scratch,
        metrics: &mut ScanStats,
    ) -> TransitionOutcome {
        let transition = self.states[deterministic_state as usize].transition(symbol);
        match transition {
            UNKNOWN_TRANSITION => {
                metrics.cache_misses += 1;
                let generation = scratch.reset_cached_transition();
                compute_transition(
                    database,
                    &self.states[deterministic_state as usize].nfa_states,
                    symbol,
                    &mut scratch.next,
                    &mut scratch.cache_seen,
                    generation,
                    &mut scratch.stack,
                );
                let transition = if scratch.next.is_empty() {
                    DEAD_TRANSITION
                } else {
                    self.intern(database, &scratch.next)
                        .unwrap_or(FALLBACK_TRANSITION)
                };
                self.states[deterministic_state as usize].set_transition(symbol, transition);
                Self::outcome_for(transition, metrics)
            }
            known => {
                if known == FALLBACK_TRANSITION {
                    metrics.fallback_transitions += 1;
                    let generation = scratch.reset_cached_transition();
                    compute_transition(
                        database,
                        &self.states[deterministic_state as usize].nfa_states,
                        symbol,
                        &mut scratch.next,
                        &mut scratch.cache_seen,
                        generation,
                        &mut scratch.stack,
                    );
                    TransitionOutcome::Uncached
                } else {
                    metrics.cache_hits += 1;
                    Self::outcome_for(known, metrics)
                }
            }
        }
    }

    fn outcome_for(transition: u32, metrics: &mut ScanStats) -> TransitionOutcome {
        match transition {
            DEAD_TRANSITION => TransitionOutcome::Dead,
            FALLBACK_TRANSITION => {
                metrics.fallback_transitions += 1;
                TransitionOutcome::Uncached
            }
            deterministic_state => TransitionOutcome::Cached(deterministic_state),
        }
    }
}

fn state_set_hash(states: &[StateId]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for state in states {
        for byte in state.index().to_le_bytes() {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    hash
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ScanStats {
    pub(crate) cache_states: usize,
    pub(crate) cache_hits: u64,
    pub(crate) cache_misses: u64,
    pub(crate) fallback_transitions: u64,
    pub(crate) cache_table_bytes: usize,
    pub(crate) assertion_bypasses: u64,
    pub(crate) prefilter_path: PrefilterPath,
    pub(crate) prefilter_bypass: PrefilterBypass,
    pub(crate) prefilter_retained_bytes: usize,
    pub(crate) prefilter_candidate_bytes: usize,
    pub(crate) prefilter_admissions: u64,
    pub(crate) prefilter_candidate_starts: usize,
    pub(crate) prefilter_starts_scanned: usize,
    pub(crate) prefilter_starts_skipped: usize,
}

impl ScanStats {
    #[cfg(feature = "benchmark-internals")]
    pub(crate) fn merge_partition(&mut self, other: Self) {
        self.cache_states = self.cache_states.saturating_add(other.cache_states);
        self.cache_hits = self.cache_hits.saturating_add(other.cache_hits);
        self.cache_misses = self.cache_misses.saturating_add(other.cache_misses);
        self.fallback_transitions = self
            .fallback_transitions
            .saturating_add(other.fallback_transitions);
        self.cache_table_bytes = self
            .cache_table_bytes
            .saturating_add(other.cache_table_bytes);
        self.assertion_bypasses = self
            .assertion_bypasses
            .saturating_add(other.assertion_bypasses);
        if self.prefilter_path != other.prefilter_path {
            self.prefilter_path = PrefilterPath::MixedParallel;
        }
        if self.prefilter_bypass != other.prefilter_bypass {
            self.prefilter_bypass = PrefilterBypass::MixedParallel;
        }
        self.prefilter_retained_bytes = self
            .prefilter_retained_bytes
            .saturating_add(other.prefilter_retained_bytes);
        self.prefilter_candidate_bytes = self
            .prefilter_candidate_bytes
            .saturating_add(other.prefilter_candidate_bytes);
        self.prefilter_admissions = self
            .prefilter_admissions
            .saturating_add(other.prefilter_admissions);
        self.prefilter_candidate_starts = self
            .prefilter_candidate_starts
            .saturating_add(other.prefilter_candidate_starts);
        self.prefilter_starts_scanned = self
            .prefilter_starts_scanned
            .saturating_add(other.prefilter_starts_scanned);
        self.prefilter_starts_skipped = self
            .prefilter_starts_skipped
            .saturating_add(other.prefilter_starts_skipped);
    }
}

fn scan_with_assertions(
    database: &PatternDatabase,
    input: &Utf16Text,
    mut sink: impl FnMut(Match),
) -> Result<(), Error> {
    let units = input.units();
    let mut scratch = Scratch::new(database);

    for start in 0..units.len() {
        scratch.reset_start();
        extend_assertion_closure(
            database,
            database.root(),
            AssertionPosition::before(units, start),
            &mut scratch.active,
            &mut scratch.active_seen,
            &mut scratch.stack,
        );

        for (position, &symbol) in units.iter().enumerate().skip(start) {
            scratch.reset_next();
            let next_position = position.checked_add(1).ok_or(Error::InputTooLarge)?;
            for &state in &scratch.active {
                for edge in database.edges_from(state) {
                    if database.edge_matches(edge.kind, symbol) {
                        extend_assertion_closure(
                            database,
                            edge.target,
                            AssertionPosition::after(units, next_position),
                            &mut scratch.next,
                            &mut scratch.next_seen,
                            &mut scratch.stack,
                        );
                    }
                }
            }
            let end = position_utf16(next_position)?;
            for &state in &scratch.next {
                for &ordinal in database.terminals_at(state) {
                    record_terminal(
                        &mut scratch.best_end,
                        &mut scratch.touched_ordinals,
                        ordinal,
                        end,
                    );
                }
            }
            std::mem::swap(&mut scratch.active, &mut scratch.next);
            std::mem::swap(&mut scratch.active_seen, &mut scratch.next_seen);
            if scratch.active.is_empty() {
                break;
            }
        }

        let start_utf16 = position_utf16(start)?;
        scratch.touched_ordinals.sort_unstable();
        for &ordinal in &scratch.touched_ordinals {
            let end_utf16 = scratch.best_end[ordinal].expect("a touched terminal has an end");
            sink(Match::new(
                database.pattern_id(ordinal),
                Utf16Span::from_bounds(start_utf16, end_utf16),
            ));
        }
    }
    Ok(())
}

fn position_utf16(index: usize) -> Result<u64, Error> {
    u64::try_from(index).map_err(|_| Error::InputTooLarge)
}

#[derive(Clone, Copy)]
struct AssertionPosition<'a> {
    input: &'a [u16],
    index: usize,
    after_current: bool,
}

impl<'a> AssertionPosition<'a> {
    const fn before(input: &'a [u16], index: usize) -> Self {
        Self {
            input,
            index,
            after_current: false,
        }
    }

    const fn after(input: &'a [u16], index: usize) -> Self {
        Self {
            input,
            index,
            after_current: true,
        }
    }
}

fn extend_epsilon_closure(
    database: &PatternDatabase,
    seed: StateId,
    output: &mut Vec<StateId>,
    visited: &mut [bool],
    stack: &mut Vec<StateId>,
) {
    if visited[seed.index()] {
        return;
    }
    visited[seed.index()] = true;
    stack.push(seed);
    while let Some(state) = stack.pop() {
        output.push(state);
        for edge in database.edges_from(state) {
            if edge.kind == EdgeKind::Epsilon && !visited[edge.target.index()] {
                visited[edge.target.index()] = true;
                stack.push(edge.target);
            }
        }
    }
}

fn extend_epsilon_closure_generation(
    database: &PatternDatabase,
    seed: StateId,
    output: &mut Vec<StateId>,
    visited: &mut [u32],
    generation: u32,
    stack: &mut Vec<StateId>,
) {
    if visited[seed.index()] == generation {
        return;
    }
    visited[seed.index()] = generation;
    stack.push(seed);
    while let Some(state) = stack.pop() {
        output.push(state);
        for edge in database.edges_from(state) {
            if edge.kind == EdgeKind::Epsilon && visited[edge.target.index()] != generation {
                visited[edge.target.index()] = generation;
                stack.push(edge.target);
            }
        }
    }
}

fn extend_assertion_closure(
    database: &PatternDatabase,
    seed: StateId,
    position: AssertionPosition<'_>,
    output: &mut Vec<StateId>,
    visited: &mut [bool],
    stack: &mut Vec<StateId>,
) {
    if visited[seed.index()] {
        return;
    }
    visited[seed.index()] = true;
    stack.push(seed);
    while let Some(state) = stack.pop() {
        output.push(state);
        for edge in database.edges_from(state) {
            let follows_without_consuming = match edge.kind {
                EdgeKind::Epsilon => true,
                EdgeKind::Assertion(assertion) => assertion_matches(
                    assertion,
                    position.input,
                    position.index,
                    position.after_current,
                ),
                EdgeKind::Symbol(_) | EdgeKind::Predicate(_) => false,
            };
            if follows_without_consuming && !visited[edge.target.index()] {
                visited[edge.target.index()] = true;
                stack.push(edge.target);
            }
        }
    }
}

fn assertion_matches(
    assertion: Assertion,
    input: &[u16],
    position: usize,
    after_current: bool,
) -> bool {
    match assertion {
        Assertion::LineStart => position == 0 || input.get(position - 1) == Some(&u16::from(b'\n')),
        Assertion::LineEnd => {
            after_current
                && (position == input.len() || input.get(position) == Some(&u16::from(b'\n')))
        }
        Assertion::WordBoundary => is_word_boundary(input, position),
        Assertion::NonWordBoundary => !is_word_boundary(input, position),
    }
}

fn is_word_boundary(input: &[u16], position: usize) -> bool {
    let before = position
        .checked_sub(1)
        .and_then(|index| input.get(index))
        .is_some_and(|&symbol| is_ascii_word(symbol));
    let after = input
        .get(position)
        .is_some_and(|&symbol| is_ascii_word(symbol));
    before != after
}

const fn is_ascii_word(symbol: u16) -> bool {
    matches!(symbol, 0x30..=0x39 | 0x41..=0x5a | 0x5f | 0x61..=0x7a)
}

struct Scratch {
    active: Vec<StateId>,
    next: Vec<StateId>,
    stack: Vec<StateId>,
    active_seen: Vec<bool>,
    next_seen: Vec<bool>,
    cache_seen: Vec<u32>,
    cache_generation: u32,
    best_end: Vec<Option<u64>>,
    touched_ordinals: Vec<usize>,
}

fn record_terminal(
    best_end: &mut [Option<u64>],
    touched_ordinals: &mut Vec<usize>,
    ordinal: usize,
    end: u64,
) {
    if best_end[ordinal].is_none() {
        touched_ordinals.push(ordinal);
    }
    best_end[ordinal] = Some(end);
}

impl Scratch {
    fn new(database: &PatternDatabase) -> Self {
        let state_count = database.state_count();
        Self {
            active: Vec::with_capacity(state_count),
            next: Vec::with_capacity(state_count),
            stack: Vec::with_capacity(state_count),
            active_seen: vec![false; state_count],
            next_seen: vec![false; state_count],
            cache_seen: vec![0; state_count],
            cache_generation: 0,
            best_end: vec![None; database.pattern_count()],
            touched_ordinals: Vec::with_capacity(database.pattern_count().min(64)),
        }
    }

    fn reset_start(&mut self) {
        self.active.clear();
        self.active_seen.fill(false);
        self.reset_terminals();
        self.stack.clear();
    }

    fn reset_cached_start(&mut self) {
        self.active.clear();
        self.next.clear();
        self.reset_terminals();
        self.stack.clear();
    }

    fn reset_terminals(&mut self) {
        for ordinal in self.touched_ordinals.drain(..) {
            self.best_end[ordinal] = None;
        }
    }

    fn reset_next(&mut self) {
        self.next.clear();
        self.next_seen.fill(false);
        self.stack.clear();
    }

    fn reset_cached_transition(&mut self) -> u32 {
        self.next.clear();
        self.stack.clear();
        self.cache_generation = self.cache_generation.wrapping_add(1);
        if self.cache_generation == 0 {
            self.cache_seen.fill(0);
            self.cache_generation = 1;
        }
        self.cache_generation
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        DEFAULT_STATE_CACHE_BUDGET, Scratch, assertion_matches, extend_epsilon_closure,
        is_ascii_word, scan, scan_with_stats,
    };
    use crate::hir::Assertion;
    use crate::hir::Hir;
    use crate::nfa;
    use crate::parser::parse;
    use crate::prefilter::Prefilter;
    use crate::{PatternId, Utf16Text};

    type EventTuple = (u32, u64, u64);
    type CollectedScan = (Vec<EventTuple>, super::ScanStats);

    #[test]
    fn default_cache_budget_bounds_direct_ascii_tables_to_four_mebibytes() {
        // Prepare
        let bytes_per_state = 128 * std::mem::size_of::<u32>();

        // Test
        let maximum_table_bytes = DEFAULT_STATE_CACHE_BUDGET * bytes_per_state;

        // Assert
        assert_eq!(maximum_table_bytes, 4 * 1024 * 1024);
    }

    #[test]
    fn cache_visit_generation_wrap_clears_old_marks() -> Result<(), crate::Error> {
        // Prepare
        let patterns = [parse(PatternId::new(1), "alpha")?];
        let database = nfa::compile(&patterns)?;
        let mut scratch = Scratch::new(&database);
        scratch.cache_seen.fill(u32::MAX);
        scratch.cache_generation = u32::MAX;

        // Test
        let generation = scratch.reset_cached_transition();

        // Assert
        assert_eq!(generation, 1);
        assert!(scratch.cache_seen.iter().all(|&mark| mark == 0));
        Ok(())
    }

    #[test]
    fn epsilon_closure_reaches_empty_branch_without_duplicate_states() -> Result<(), crate::Error> {
        // Prepare
        let patterns = [parse(PatternId::new(1), "|a")?];
        let database = nfa::compile(&patterns)?;
        let mut states = Vec::new();
        let mut visited = vec![false; database.state_count()];
        let mut stack = Vec::new();

        // Test
        extend_epsilon_closure(
            &database,
            database.root(),
            &mut states,
            &mut visited,
            &mut stack,
        );

        // Assert
        let unique: BTreeSet<_> = states.iter().map(|state| state.index()).collect();
        assert_eq!(states.len(), unique.len());
        assert!(
            states
                .iter()
                .any(|&state| !database.terminals_at(state).is_empty())
        );
        Ok(())
    }

    #[test]
    fn assertion_context_exhausts_ascii_boundaries_and_line_edges() {
        // Prepare
        let symbols = std::iter::once(None)
            .chain((0_u16..=127).map(Some))
            .collect::<Vec<_>>();

        // Test / Assert
        for &left in &symbols {
            for &right in &symbols {
                let (input, position) = match (left, right) {
                    (None, None) => (Vec::new(), 0),
                    (None, Some(right)) => (vec![right], 0),
                    (Some(left), None) => (vec![left], 1),
                    (Some(left), Some(right)) => (vec![left, right], 1),
                };
                let expected = left.is_some_and(is_ascii_word) != right.is_some_and(is_ascii_word);
                assert_eq!(
                    assertion_matches(Assertion::WordBoundary, &input, position, false),
                    expected,
                    "left={left:?}, right={right:?}"
                );
                assert_eq!(
                    assertion_matches(Assertion::NonWordBoundary, &input, position, true),
                    !expected,
                    "left={left:?}, right={right:?}"
                );
            }
        }

        assert!(assertion_matches(Assertion::LineStart, &[0x61], 0, false));
        assert!(assertion_matches(
            Assertion::LineStart,
            &[0x0a, 0x61],
            1,
            true
        ));
        assert!(!assertion_matches(
            Assertion::LineStart,
            &[0x78, 0x61],
            1,
            false
        ));
        assert!(assertion_matches(Assertion::LineEnd, &[0x61], 1, true));
        assert!(assertion_matches(
            Assertion::LineEnd,
            &[0x61, 0x0a],
            1,
            true
        ));
        assert!(!assertion_matches(Assertion::LineEnd, &[0x0a], 0, false));
    }

    #[test]
    fn cache_budgets_preserve_events_and_report_fallback_pressure() -> Result<(), crate::Error> {
        // Prepare
        let patterns = [
            parse(PatternId::new(1), "alpha")?,
            parse(PatternId::new(2), "a(lpha|lps)")?,
            parse(PatternId::new(3), "[a-z]+")?,
            parse(PatternId::new(4), "Ω+")?,
        ];
        let database = nfa::compile(&patterns)?;
        let input = Utf16Text::from("alpha alps ΩΩ alpha alps ΩΩ alpha");
        let (baseline, baseline_stats) = collect_with_budget(&database, &input, 0)?;

        // Test
        let (one_state, one_state_stats) = collect_with_budget(&database, &input, 1)?;
        let (two_states, two_state_stats) = collect_with_budget(&database, &input, 2)?;
        let (full_cache, full_cache_stats) =
            collect_with_budget(&database, &input, DEFAULT_STATE_CACHE_BUDGET)?;

        // Assert
        assert_eq!(one_state, baseline);
        assert_eq!(two_states, baseline);
        assert_eq!(full_cache, baseline);
        assert_eq!(baseline_stats.cache_states, 0);
        assert_eq!(one_state_stats.cache_states, 1);
        assert!(one_state_stats.fallback_transitions > 0);
        assert!(two_state_stats.fallback_transitions > 0);
        assert!(full_cache_stats.cache_states > 2);
        assert!(full_cache_stats.cache_hits > 0);
        assert_eq!(full_cache_stats.fallback_transitions, 0);
        assert!(full_cache_stats.cache_table_bytes <= 4 * 1024 * 1024);
        Ok(())
    }

    #[test]
    fn assertion_patterns_bypass_context_free_cache_keys() -> Result<(), crate::Error> {
        // Prepare
        let patterns = [parse(PatternId::new(1), "^a+")?];
        let database = nfa::compile(&patterns)?;
        let input = Utf16Text::from("aa\na");

        // Test
        let (_, stats) = collect_with_budget(&database, &input, DEFAULT_STATE_CACHE_BUDGET)?;

        // Assert
        assert_eq!(stats.assertion_bypasses, 1);
        assert_eq!(stats.cache_states, 0);
        assert_eq!(stats.cache_hits, 0);
        assert_eq!(stats.cache_misses, 0);
        Ok(())
    }

    #[test]
    fn bounded_generated_composition_agrees_with_tiny_hir_interpreter() -> Result<(), crate::Error>
    {
        // Prepare
        let patterns = generated_patterns();
        let inputs = generated_inputs();

        // Test / Assert
        for (ordinal, source) in patterns.iter().enumerate() {
            let pattern_id = PatternId::new(u32::try_from(ordinal).expect("bounded pattern count"));
            let parsed = parse(pattern_id, source)?;
            let database = nfa::compile(std::slice::from_ref(&parsed))?;
            for input in &inputs {
                let text = Utf16Text::from(input.as_str());
                let mut actual = Vec::new();
                scan(
                    &database,
                    &Prefilter::empty(),
                    &text,
                    DEFAULT_STATE_CACHE_BUDGET,
                    false,
                    false,
                    |event| actual.push((event.span().start(), event.span().end())),
                )?;
                let mut nfa = Vec::new();
                scan(
                    &database,
                    &Prefilter::empty(),
                    &text,
                    0,
                    false,
                    false,
                    |event| {
                        nfa.push((event.span().start(), event.span().end()));
                    },
                )?;

                let mut expected = Vec::new();
                for start in 0..text.units().len() {
                    let longest = interpret(parsed.expression(), text.units(), start)
                        .into_iter()
                        .filter(|&end| end > start)
                        .max();
                    if let Some(end) = longest {
                        expected.push((
                            u64::try_from(start).expect("bounded input"),
                            u64::try_from(end).expect("bounded input"),
                        ));
                    }
                }
                assert_eq!(actual, nfa, "cache/NFA pattern {source:?}, input {input:?}");
                assert_eq!(actual, expected, "pattern {source:?}, input {input:?}");
            }
        }
        Ok(())
    }

    fn collect_with_budget(
        database: &nfa::PatternDatabase,
        input: &Utf16Text,
        budget: usize,
    ) -> Result<CollectedScan, crate::Error> {
        let mut events = Vec::new();
        let stats = scan_with_stats(
            database,
            &Prefilter::empty(),
            input,
            budget,
            false,
            false,
            |event| {
                events.push((
                    event.pattern_id().get(),
                    event.span().start(),
                    event.span().end(),
                ));
            },
        )?;
        events.sort_unstable();
        Ok((events, stats))
    }

    fn interpret(expression: &Hir, input: &[u16], start: usize) -> BTreeSet<usize> {
        interpret_from(expression, input, (start, false))
            .into_iter()
            .map(|(position, _)| position)
            .collect()
    }

    fn interpret_from(
        expression: &Hir,
        input: &[u16],
        state: (usize, bool),
    ) -> BTreeSet<(usize, bool)> {
        let (position, after_current) = state;
        match expression {
            Hir::Never => BTreeSet::new(),
            Hir::Epsilon => BTreeSet::from([state]),
            Hir::Assertion(assertion) => {
                if assertion_matches(*assertion, input, position, after_current) {
                    BTreeSet::from([state])
                } else {
                    BTreeSet::new()
                }
            }
            Hir::Symbol(expected) => input
                .get(position)
                .filter(|&&actual| actual == *expected)
                .map_or_else(BTreeSet::new, |_| BTreeSet::from([(position + 1, true)])),
            Hir::Predicate(predicate) => input
                .get(position)
                .filter(|&&actual| predicate.matches(actual))
                .map_or_else(BTreeSet::new, |_| BTreeSet::from([(position + 1, true)])),
            Hir::Sequence(expressions) => {
                let mut states = BTreeSet::from([state]);
                for item in expressions {
                    states = states
                        .into_iter()
                        .flat_map(|state| interpret_from(item, input, state))
                        .collect();
                }
                states
            }
            Hir::Alternation(expressions) => expressions
                .iter()
                .flat_map(|branch| interpret_from(branch, input, state))
                .collect(),
            Hir::Repeat {
                expression,
                min,
                max,
            } => interpret_repetition(expression, *min, *max, input, state),
        }
    }

    fn interpret_repetition(
        expression: &Hir,
        min: u16,
        max: Option<u16>,
        input: &[u16],
        start: (usize, bool),
    ) -> BTreeSet<(usize, bool)> {
        let mut accepted = BTreeSet::new();
        let mut positions = BTreeSet::from([start]);
        if min == 0 {
            accepted.insert(start);
        }
        let limit = max.map_or_else(
            || {
                usize::from(min)
                    .saturating_add(input.len())
                    .saturating_add(1)
            },
            usize::from,
        );
        for count in 1..=limit {
            positions = positions
                .into_iter()
                .flat_map(|state| interpret_from(expression, input, state))
                .collect();
            if positions.is_empty() {
                break;
            }
            if count >= usize::from(min) {
                accepted.extend(&positions);
            }
        }
        accepted
    }

    fn generated_patterns() -> Vec<String> {
        let atoms = ["a", "b", ".", "[ab]"];
        let mut patterns = atoms.iter().map(ToString::to_string).collect::<Vec<_>>();
        for left in atoms {
            for right in atoms {
                patterns.push(format!("{left}{right}"));
                patterns.push(format!("{left}|{right}"));
                patterns.push(format!("x({left}|{right})"));
                patterns.push(format!("x(|{left}){right}"));
                patterns.push(format!("{left}?{right}"));
                patterns.push(format!("{left}+"));
                patterns.push(format!("({left}|{right}){{1,2}}"));
            }
        }
        patterns.extend(["^a", "a$", r"\ba\b", r"\Ba", r"a\B", "(^a|b$)"].map(ToString::to_string));
        patterns
    }

    fn generated_inputs() -> Vec<String> {
        let alphabet = ['a', 'b', '\n'];
        let mut inputs = vec![String::new()];
        let mut frontier = vec![String::new()];
        for _ in 0..3 {
            frontier = frontier
                .into_iter()
                .flat_map(|prefix| {
                    alphabet.map(move |symbol| {
                        let mut input = prefix.clone();
                        input.push(symbol);
                        input
                    })
                })
                .collect();
            inputs.extend(frontier.iter().cloned());
        }
        inputs
    }
}
