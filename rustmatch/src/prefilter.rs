//! Conservative candidate-start acceleration ahead of the semantic engine.

use std::mem::size_of;

use crate::hir::{Hir, HirPattern};
use crate::nfa::{EdgeKind, PatternDatabase, StateId};

const MIN_LITERAL_PATTERN_COUNT: usize = 256;
const MIN_LITERAL_INPUT_UNITS: usize = 1024 * 1024;
const MIN_LITERAL_UNITS: usize = 3;
const MAX_LITERAL_UNITS: usize = 32;
const DENSITY_SAMPLE_UNITS: usize = 64 * 1024;
const ASCII_TRIPLE_WORDS: usize = 128 * 128 * 128 / 64;
const FOUR_PREFIX_HASH_WORDS: usize = (1 << 19) / 64;
const FIVE_PREFIX_HASH_WORDS: usize = (1 << 20) / 64;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum PrefilterPath {
    #[default]
    AllStarts,
    StartTable,
    Literal,
    #[cfg(feature = "benchmark-internals")]
    MixedParallel,
}

#[cfg(feature = "benchmark-internals")]
impl PrefilterPath {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::AllStarts => "all-starts",
            Self::StartTable => "start-table",
            Self::Literal => "literal-prefilter",
            Self::MixedParallel => "mixed-parallel",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum PrefilterBypass {
    #[default]
    None,
    Disabled,
    LiteralDisabled,
    Assertions,
    PatternCount,
    InputSize,
    Unfilterable,
    DenseSample,
    #[cfg(feature = "benchmark-internals")]
    MixedParallel,
}

#[cfg(feature = "benchmark-internals")]
impl PrefilterBypass {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Disabled => "disabled",
            Self::LiteralDisabled => "literal-disabled",
            Self::Assertions => "assertions",
            Self::PatternCount => "pattern-count",
            Self::InputSize => "input-size",
            Self::Unfilterable => "unfilterable",
            Self::DenseSample => "dense-sample",
            Self::MixedParallel => "mixed-parallel",
        }
    }
}

#[derive(Debug)]
pub(crate) struct Prefilter {
    start_table: Option<StartTable>,
    literal: Option<LiteralPrefilter>,
    literal_unavailable: PrefilterBypass,
    retained_bytes: usize,
}

impl Prefilter {
    #[cfg(test)]
    pub(crate) const fn empty() -> Self {
        Self {
            start_table: None,
            literal: None,
            literal_unavailable: PrefilterBypass::Disabled,
            retained_bytes: 0,
        }
    }

    pub(crate) fn compile(patterns: &[HirPattern], database: &PatternDatabase) -> Self {
        if database.uses_assertions() {
            return Self {
                start_table: None,
                literal: None,
                literal_unavailable: PrefilterBypass::Assertions,
                retained_bytes: 0,
            };
        }

        let start_table = StartTable::compile(database);
        let (literal, literal_unavailable) = if patterns.len() < MIN_LITERAL_PATTERN_COUNT {
            (None, PrefilterBypass::PatternCount)
        } else {
            let hints = patterns
                .iter()
                .enumerate()
                .map(|(ordinal, pattern)| {
                    NecessaryLiteral::from_pattern(ordinal, pattern.expression())
                })
                .collect::<Option<Vec<_>>>();
            match hints {
                Some(hints) => (
                    Some(LiteralPrefilter::compile(&hints)),
                    PrefilterBypass::None,
                ),
                None => (None, PrefilterBypass::Unfilterable),
            }
        };
        let retained_bytes = start_table.as_ref().map_or(0, StartTable::retained_bytes)
            + literal.as_ref().map_or(0, LiteralPrefilter::retained_bytes);
        Self {
            start_table,
            literal,
            literal_unavailable,
            retained_bytes,
        }
    }

    pub(crate) fn plan<'a>(
        &'a self,
        input: &[u16],
        enabled: bool,
        literal_enabled: bool,
    ) -> ScanPlan<'a> {
        if !enabled {
            return ScanPlan::All {
                bypass: PrefilterBypass::Disabled,
                retained_bytes: self.retained_bytes,
            };
        }
        if !literal_enabled {
            return self.start_or_all(PrefilterBypass::LiteralDisabled);
        }

        let Some(filter) = &self.literal else {
            return self.start_or_all(self.literal_unavailable);
        };
        if input.len() < MIN_LITERAL_INPUT_UNITS {
            return self.start_or_all(PrefilterBypass::InputSize);
        }

        let sample_len = input.len().min(DENSITY_SAMPLE_UNITS);
        let mut sample = CandidateBitmap::new(sample_len);
        let mut admissions = 0_u64;
        filter.scan_starts(input, 0..sample_len, &mut sample, &mut admissions);
        if sample.count().saturating_mul(2) >= sample_len {
            return self.start_or_all(PrefilterBypass::DenseSample);
        }

        let mut candidates = CandidateBitmap::new(input.len());
        candidates.copy_prefix(&sample);
        filter.scan_starts(
            input,
            sample_len..input.len(),
            &mut candidates,
            &mut admissions,
        );
        ScanPlan::Candidates {
            candidates,
            admissions,
            retained_bytes: self.retained_bytes,
        }
    }

    fn start_or_all(&self, bypass: PrefilterBypass) -> ScanPlan<'_> {
        match &self.start_table {
            Some(table) => ScanPlan::StartTable {
                table,
                bypass,
                retained_bytes: self.retained_bytes,
            },
            None => ScanPlan::All {
                bypass,
                retained_bytes: self.retained_bytes,
            },
        }
    }

    pub(crate) fn shared_literal_filter(&self) -> Option<SharedLiteralFilter<'_>> {
        self.literal.as_ref().map(SharedLiteralFilter)
    }

    pub(crate) fn shared_allows_start(&self, input: &[u16], start: usize) -> bool {
        start
            .checked_add(MIN_LITERAL_UNITS)
            .is_some_and(|end| end <= input.len())
            && self
                .literal
                .as_ref()
                .is_some_and(|filter| filter.prefix_allows(input, start))
    }

    pub(crate) const fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }
}

#[derive(Clone, Copy)]
pub(crate) struct SharedLiteralFilter<'a>(&'a LiteralPrefilter);

impl<'a> SharedLiteralFilter<'a> {
    pub(crate) fn ascii_three_exact(self) -> &'a [u64] {
        self.0.ascii_three_exact.as_ref()
    }

    pub(crate) fn ascii_four_hash(self) -> &'a [u64] {
        self.0.ascii_four_hash.as_ref()
    }

    pub(crate) fn ascii_five_hash(self) -> &'a [u64] {
        self.0.ascii_five_hash.as_ref()
    }

    pub(crate) const fn has_three(self) -> bool {
        self.0.has_three
    }

    pub(crate) const fn has_four(self) -> bool {
        self.0.has_four
    }

    pub(crate) const fn has_five(self) -> bool {
        self.0.has_five
    }
}

pub(crate) enum ScanPlan<'a> {
    All {
        bypass: PrefilterBypass,
        retained_bytes: usize,
    },
    StartTable {
        table: &'a StartTable,
        bypass: PrefilterBypass,
        retained_bytes: usize,
    },
    Candidates {
        candidates: CandidateBitmap,
        admissions: u64,
        retained_bytes: usize,
    },
}

impl ScanPlan<'_> {
    pub(crate) const fn path(&self) -> PrefilterPath {
        match self {
            Self::All { .. } => PrefilterPath::AllStarts,
            Self::StartTable { .. } => PrefilterPath::StartTable,
            Self::Candidates { .. } => PrefilterPath::Literal,
        }
    }

    pub(crate) const fn bypass(&self) -> PrefilterBypass {
        match self {
            Self::All { bypass, .. } | Self::StartTable { bypass, .. } => *bypass,
            Self::Candidates { .. } => PrefilterBypass::None,
        }
    }

    pub(crate) const fn admissions(&self) -> u64 {
        match self {
            Self::Candidates { admissions, .. } => *admissions,
            Self::All { .. } | Self::StartTable { .. } => 0,
        }
    }

    pub(crate) const fn candidate_count(&self) -> usize {
        match self {
            Self::Candidates { candidates, .. } => candidates.count(),
            Self::All { .. } | Self::StartTable { .. } => 0,
        }
    }

    pub(crate) const fn retained_bytes(&self) -> usize {
        match self {
            Self::All { retained_bytes, .. }
            | Self::StartTable { retained_bytes, .. }
            | Self::Candidates { retained_bytes, .. } => *retained_bytes,
        }
    }

    pub(crate) fn candidate_bytes(&self) -> usize {
        match self {
            Self::Candidates { candidates, .. } => candidates.retained_bytes(),
            Self::All { .. } | Self::StartTable { .. } => 0,
        }
    }
}

#[derive(Debug)]
pub(crate) struct StartTable {
    first_ascii: [u64; 2],
    pair_ascii: Box<[u64; 256]>,
}

impl StartTable {
    fn compile(database: &PatternDatabase) -> Option<Self> {
        if database.uses_assertions() {
            return None;
        }

        let mut closure = ClosureScratch::new(database.state_count());
        let root = closure.epsilon_closure(database, &[database.root()]);
        let mut first_ascii = [0_u64; 2];
        let mut pair_ascii = Box::new([0_u64; 256]);
        for first in 0_u16..128 {
            let first_states = closure.transition(database, &root, first);
            if first_states.is_empty() {
                continue;
            }
            set_bit(&mut first_ascii, usize::from(first));
            let accepts_after_first = first_states
                .iter()
                .any(|&state| !database.terminals_at(state).is_empty());
            for second in 0_u16..128 {
                if accepts_after_first
                    || !closure
                        .transition(database, &first_states, second)
                        .is_empty()
                {
                    let pair = usize::from(first) * 128 + usize::from(second);
                    set_bit(pair_ascii.as_mut(), pair);
                }
            }
        }
        Some(Self {
            first_ascii,
            pair_ascii,
        })
    }

    pub(crate) fn allows(&self, input: &[u16], start: usize) -> bool {
        let first = input[start];
        if first >= 128 {
            return true;
        }
        if !contains_bit(&self.first_ascii, usize::from(first)) {
            return false;
        }
        let Some(&second) = input.get(start + 1) else {
            return true;
        };
        if second >= 128 {
            return true;
        }
        let pair = usize::from(first) * 128 + usize::from(second);
        contains_bit(self.pair_ascii.as_ref(), pair)
    }

    fn retained_bytes(&self) -> usize {
        std::mem::size_of_val(self) + size_of::<[u64; 256]>()
    }
}

struct ClosureScratch {
    seen: Vec<u32>,
    generation: u32,
    stack: Vec<StateId>,
}

impl ClosureScratch {
    fn new(state_count: usize) -> Self {
        Self {
            seen: vec![0; state_count],
            generation: 0,
            stack: Vec::new(),
        }
    }

    fn epsilon_closure(&mut self, database: &PatternDatabase, seeds: &[StateId]) -> Vec<StateId> {
        let generation = self.next_generation();
        let mut output = Vec::new();
        self.stack.extend(seeds.iter().copied());
        while let Some(state) = self.stack.pop() {
            if self.seen[state.index()] == generation {
                continue;
            }
            self.seen[state.index()] = generation;
            output.push(state);
            for edge in database.edges_from(state) {
                if edge.kind == EdgeKind::Epsilon {
                    self.stack.push(edge.target);
                }
            }
        }
        output.sort_unstable();
        output
    }

    fn transition(
        &mut self,
        database: &PatternDatabase,
        source: &[StateId],
        symbol: u16,
    ) -> Vec<StateId> {
        let mut seeds = Vec::new();
        for &state in source {
            for edge in database.edges_from(state) {
                if database.edge_matches(edge.kind, symbol) {
                    seeds.push(edge.target);
                }
            }
        }
        self.epsilon_closure(database, &seeds)
    }

    fn next_generation(&mut self) -> u32 {
        self.generation = self.generation.wrapping_add(1);
        if self.generation == 0 {
            self.seen.fill(0);
            self.generation = 1;
        }
        self.generation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct NecessaryLiteral {
    ordinal: usize,
    offset: usize,
    units: Box<[u16]>,
}

impl NecessaryLiteral {
    fn from_pattern(ordinal: usize, expression: &Hir) -> Option<Self> {
        let mut units = necessary_prefix(expression);
        units.truncate(MAX_LITERAL_UNITS);
        (units.len() >= MIN_LITERAL_UNITS).then(|| Self {
            ordinal,
            offset: 0,
            units: units.into_boxed_slice(),
        })
    }
}

fn necessary_prefix(expression: &Hir) -> Vec<u16> {
    match expression {
        Hir::Symbol(symbol) => vec![*symbol],
        Hir::Sequence(expressions) => {
            let mut prefix = Vec::new();
            for expression in expressions {
                append_bounded(&mut prefix, &necessary_prefix(expression));
                if exact_literal(expression).is_none() || prefix.len() == MAX_LITERAL_UNITS {
                    break;
                }
            }
            prefix
        }
        Hir::Alternation(expressions) => {
            let mut expressions = expressions.iter();
            let Some(first) = expressions.next() else {
                return Vec::new();
            };
            let mut prefix = necessary_prefix(first);
            for expression in expressions {
                let candidate = necessary_prefix(expression);
                let shared = prefix
                    .iter()
                    .zip(candidate.iter())
                    .take_while(|(left, right)| left == right)
                    .count();
                prefix.truncate(shared);
                if prefix.is_empty() {
                    break;
                }
            }
            prefix
        }
        Hir::Repeat {
            expression, min, ..
        } if *min > 0 => {
            if let Some(literal) = exact_literal(expression) {
                let mut prefix = Vec::new();
                for _ in 0..*min {
                    append_bounded(&mut prefix, &literal);
                    if prefix.len() == MAX_LITERAL_UNITS {
                        break;
                    }
                }
                prefix
            } else {
                necessary_prefix(expression)
            }
        }
        Hir::Never | Hir::Epsilon | Hir::Assertion(_) | Hir::Predicate(_) | Hir::Repeat { .. } => {
            Vec::new()
        }
    }
}

fn exact_literal(expression: &Hir) -> Option<Vec<u16>> {
    match expression {
        Hir::Epsilon => Some(Vec::new()),
        Hir::Symbol(symbol) => Some(vec![*symbol]),
        Hir::Sequence(expressions) => {
            let mut literal = Vec::new();
            for expression in expressions {
                append_bounded(&mut literal, &exact_literal(expression)?);
            }
            Some(literal)
        }
        Hir::Alternation(expressions) => {
            let mut expressions = expressions.iter();
            let first = exact_literal(expressions.next()?)?;
            expressions
                .all(|expression| exact_literal(expression).as_ref() == Some(&first))
                .then_some(first)
        }
        Hir::Repeat {
            expression,
            min,
            max: Some(max),
        } if min == max => {
            let unit = exact_literal(expression)?;
            let mut literal = Vec::new();
            for _ in 0..*min {
                append_bounded(&mut literal, &unit);
            }
            Some(literal)
        }
        Hir::Never | Hir::Assertion(_) | Hir::Predicate(_) | Hir::Repeat { .. } => None,
    }
}

fn append_bounded(output: &mut Vec<u16>, suffix: &[u16]) {
    let available = MAX_LITERAL_UNITS.saturating_sub(output.len());
    output.extend(suffix.iter().copied().take(available));
}

#[derive(Debug)]
struct LiteralPrefilter {
    ascii_three_exact: Box<[u64]>,
    ascii_four_hash: Box<[u64]>,
    ascii_five_hash: Box<[u64]>,
    has_three: bool,
    has_four: bool,
    has_five: bool,
    prefix_mappings: Box<[(u64, usize)]>,
}

impl LiteralPrefilter {
    fn compile(hints: &[NecessaryLiteral]) -> Self {
        let mut ascii_three_exact = vec![0_u64; ASCII_TRIPLE_WORDS].into_boxed_slice();
        let mut ascii_four_hash = vec![0_u64; FOUR_PREFIX_HASH_WORDS].into_boxed_slice();
        let mut ascii_five_hash = vec![0_u64; FIVE_PREFIX_HASH_WORDS].into_boxed_slice();
        let mut has_three = false;
        let mut has_four = false;
        let mut has_five = false;
        let mut prefix_mappings = Vec::with_capacity(hints.len());
        for hint in hints {
            debug_assert_eq!(hint.offset, 0);
            let triple = &hint.units[..MIN_LITERAL_UNITS];
            prefix_mappings.push((prefix_key(triple[0], triple[1], triple[2]), hint.ordinal));
            match hint.units.len() {
                MIN_LITERAL_UNITS => {
                    has_three = true;
                    if triple.iter().all(|&symbol| symbol < 128) {
                        set_bit(
                            ascii_three_exact.as_mut(),
                            ascii_triple_index(triple[0], triple[1], triple[2]),
                        );
                    }
                }
                4 => {
                    has_four = true;
                    if hint.units[..4].iter().all(|&symbol| symbol < 128) {
                        set_bit(
                            ascii_four_hash.as_mut(),
                            prefix_hash_index(&hint.units[..4], FOUR_PREFIX_HASH_WORDS * 64),
                        );
                    }
                }
                _ => {
                    has_five = true;
                    if hint.units[..5].iter().all(|&symbol| symbol < 128) {
                        set_bit(
                            ascii_five_hash.as_mut(),
                            prefix_hash_index(&hint.units[..5], FIVE_PREFIX_HASH_WORDS * 64),
                        );
                    }
                }
            }
        }
        prefix_mappings.sort_unstable();
        Self {
            ascii_three_exact,
            ascii_four_hash,
            ascii_five_hash,
            has_three,
            has_four,
            has_five,
            prefix_mappings: prefix_mappings.into_boxed_slice(),
        }
    }

    fn scan_starts(
        &self,
        input: &[u16],
        starts: std::ops::Range<usize>,
        candidates: &mut CandidateBitmap,
        admissions: &mut u64,
    ) {
        for start in starts {
            let Some(triple_end) = start.checked_add(MIN_LITERAL_UNITS) else {
                continue;
            };
            if triple_end > input.len() || !self.prefix_allows(input, start) {
                continue;
            }
            *admissions = admissions.saturating_add(1);
            candidates.insert(start);
        }
    }

    fn prefix_allows(&self, input: &[u16], start: usize) -> bool {
        let triple = &input[start..start + 3];
        if triple.iter().any(|&symbol| symbol >= 128) {
            return true;
        }
        if self.has_three
            && contains_bit(
                self.ascii_three_exact.as_ref(),
                ascii_triple_index(triple[0], triple[1], triple[2]),
            )
        {
            return true;
        }

        let Some(four) = input.get(start..start + 4) else {
            return false;
        };
        if four[3] >= 128 {
            return true;
        }
        if self.has_four
            && contains_bit(
                self.ascii_four_hash.as_ref(),
                prefix_hash_index(four, FOUR_PREFIX_HASH_WORDS * 64),
            )
        {
            return true;
        }

        let Some(five) = input.get(start..start + 5) else {
            return false;
        };
        if five[4] >= 128 {
            return true;
        }
        self.has_five
            && contains_bit(
                self.ascii_five_hash.as_ref(),
                prefix_hash_index(five, FIVE_PREFIX_HASH_WORDS * 64),
            )
    }

    fn retained_bytes(&self) -> usize {
        size_of::<Self>()
            + self.ascii_three_exact.len() * size_of::<u64>()
            + self.ascii_four_hash.len() * size_of::<u64>()
            + self.ascii_five_hash.len() * size_of::<u64>()
            + self.prefix_mappings.len() * size_of::<(u64, usize)>()
    }

    #[cfg(test)]
    fn patterns_for_prefix(&self, prefix: &[u16]) -> Vec<usize> {
        let key = prefix_key(prefix[0], prefix[1], prefix[2]);
        self.prefix_mappings
            .iter()
            .filter_map(|&(candidate, ordinal)| (candidate == key).then_some(ordinal))
            .collect()
    }
}

pub(crate) fn ascii_triple_index(first: u16, second: u16, third: u16) -> usize {
    (usize::from(first) << 14) | (usize::from(second) << 7) | usize::from(third)
}

fn prefix_key(first: u16, second: u16, third: u16) -> u64 {
    u64::from(first) << 32 | u64::from(second) << 16 | u64::from(third)
}

pub(crate) fn prefix_hash_index(prefix: &[u16], bit_count: usize) -> usize {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for &symbol in prefix {
        hash ^= u64::from(symbol);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    let masked = hash & u64::try_from(bit_count - 1).expect("filter size fits u64");
    usize::try_from(masked).expect("masked filter index fits usize")
}

#[derive(Debug)]
pub(crate) struct CandidateBitmap {
    words: Vec<u64>,
    start_count: usize,
    bit_len: usize,
}

impl CandidateBitmap {
    pub(crate) fn new(bit_len: usize) -> Self {
        Self {
            words: vec![0; bit_len.div_ceil(64)],
            start_count: 0,
            bit_len,
        }
    }

    fn insert(&mut self, start: usize) {
        debug_assert!(start < self.bit_len);
        let word = start / 64;
        let bit = 1_u64 << (start % 64);
        if self.words[word] & bit == 0 {
            self.words[word] |= bit;
            self.start_count += 1;
        }
    }

    fn copy_prefix(&mut self, prefix: &Self) {
        self.words[..prefix.words.len()].copy_from_slice(&prefix.words);
        self.start_count = prefix.start_count;
    }

    pub(crate) const fn count(&self) -> usize {
        self.start_count
    }

    pub(crate) fn iter(&self) -> CandidateIter<'_> {
        CandidateIter {
            words: self.words.iter().enumerate(),
            current: 0,
            word_base: 0,
        }
    }

    pub(crate) fn words_mut(&mut self) -> &mut [u64] {
        &mut self.words
    }

    pub(crate) const fn set_count(&mut self, start_count: usize) {
        self.start_count = start_count;
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        self.words.capacity() * size_of::<u64>()
    }
}

pub(crate) struct CandidateIter<'a> {
    words: std::iter::Enumerate<std::slice::Iter<'a, u64>>,
    current: u64,
    word_base: usize,
}

impl Iterator for CandidateIter<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if self.current != 0 {
                let bit = self.current.trailing_zeros() as usize;
                self.current &= self.current - 1;
                return Some(self.word_base + bit);
            }
            let (word_index, word) = self.words.next()?;
            self.current = *word;
            self.word_base = word_index * 64;
        }
    }
}

fn set_bit(words: &mut [u64], bit: usize) {
    words[bit / 64] |= 1_u64 << (bit % 64);
}

pub(crate) fn contains_bit(words: &[u64], bit: usize) -> bool {
    words[bit / 64] & (1_u64 << (bit % 64)) != 0
}

#[cfg(test)]
mod tests {
    use super::{
        CandidateBitmap, LiteralPrefilter, MIN_LITERAL_INPUT_UNITS, NecessaryLiteral, Prefilter,
        PrefilterBypass, PrefilterPath, necessary_prefix,
    };
    use crate::hir::Hir;
    use crate::{MatcherBuilder, PatternId, Utf16Text, nfa, parser};

    type TestEvent = (u32, u64, u64);
    type TestScan = Result<(Vec<TestEvent>, crate::engine::ScanStats), crate::Error>;

    #[test]
    fn necessary_prefix_is_structural_and_conservative() {
        // Prepare
        let common_alternation = Hir::alternation(vec![literal("foobar"), literal("foobaz")]);
        let optional_prefix = Hir::sequence(vec![
            Hir::repeat(literal("foo"), 0, Some(1)),
            literal("bar"),
        ]);
        let repeated_prefix = Hir::repeat(literal("ab"), 2, Some(4));

        // Test
        let common = necessary_prefix(&common_alternation);
        let optional = necessary_prefix(&optional_prefix);
        let repeated = necessary_prefix(&repeated_prefix);

        // Assert
        assert_eq!(common, utf16("fooba"));
        assert!(optional.is_empty());
        assert_eq!(repeated, utf16("abab"));
    }

    #[test]
    fn literal_prefilter_retains_overlapping_prefix_candidates_and_pattern_mapping() {
        // Prepare
        let hints = vec![hint(0, "her"), hint(1, "sher")];
        let filter = LiteralPrefilter::compile(&hints);
        let input = utf16("ushers");
        let mut candidates = CandidateBitmap::new(input.len());
        let mut admissions = 0;

        // Test
        filter.scan_starts(&input, 0..input.len(), &mut candidates, &mut admissions);

        // Assert
        assert_eq!(candidates.iter().collect::<Vec<_>>(), vec![1, 2]);
        assert_eq!(admissions, 2);
        assert_eq!(filter.patterns_for_prefix(&utf16("her")), vec![0]);
        assert_eq!(filter.patterns_for_prefix(&utf16("she")), vec![1]);
    }

    #[test]
    fn start_table_rejects_impossible_ascii_pairs_conservatively() -> Result<(), crate::Error> {
        // Prepare
        let patterns = vec![
            parser::parse(PatternId::new(1), "ab")?,
            parser::parse(PatternId::new(2), "cd")?,
        ];
        let database = nfa::compile(&patterns)?;
        let prefilter = Prefilter::compile(&patterns, &database);
        let table = prefilter
            .start_table
            .as_ref()
            .expect("assertion-free table");

        // Test and assert
        assert!(table.allows(&utf16("ab"), 0));
        assert!(table.allows(&utf16("cd"), 0));
        assert!(!table.allows(&utf16("ax"), 0));
        assert!(!table.allows(&utf16("z"), 0));
        assert!(table.allows(&utf16("a"), 0));
        Ok(())
    }

    #[test]
    fn assertion_pattern_sets_bypass_every_prefilter_layer() -> Result<(), crate::Error> {
        // Prepare
        let patterns = vec![parser::parse(PatternId::new(1), "^needle")?];
        let database = nfa::compile(&patterns)?;
        let prefilter = Prefilter::compile(&patterns, &database);

        // Test
        let plan = prefilter.plan(&utf16("needle"), true, true);

        // Assert
        assert_eq!(plan.path(), PrefilterPath::AllStarts);
        assert_eq!(plan.bypass(), PrefilterBypass::Assertions);
        Ok(())
    }

    #[test]
    fn dense_sample_bypasses_the_full_literal_filter() -> Result<(), crate::Error> {
        // Prepare
        let mut patterns = Vec::new();
        for ordinal in 0..256_u32 {
            patterns.push(parser::parse(PatternId::new(ordinal), "aaa")?);
        }
        let database = nfa::compile(&patterns)?;
        let prefilter = Prefilter::compile(&patterns, &database);
        let input = vec![u16::from(b'a'); 1024 * 1024];

        // Test
        let plan = prefilter.plan(&input, true, true);

        // Assert
        assert_eq!(plan.path(), PrefilterPath::StartTable);
        assert_eq!(plan.bypass(), PrefilterBypass::DenseSample);
        Ok(())
    }

    #[test]
    fn nullable_patterns_preserve_every_consuming_match() -> Result<(), crate::Error> {
        // Prepare
        let patterns = vec![parser::parse(PatternId::new(1), "a*")?];
        let database = nfa::compile(&patterns)?;
        let prefilter = Prefilter::compile(&patterns, &database);
        let input = Utf16Text::from("baa");

        // Test
        let (accelerated, diagnostics) = scan_events(&database, &prefilter, &input, true, true)?;
        let (baseline, _) = scan_events(&database, &prefilter, &input, false, false)?;

        // Assert
        assert_eq!(accelerated, baseline);
        assert_eq!(accelerated, vec![(1, 1, 3), (1, 2, 3)]);
        assert_eq!(diagnostics.prefilter_path, PrefilterPath::StartTable);
        assert_eq!(diagnostics.prefilter_bypass, PrefilterBypass::PatternCount);
        Ok(())
    }

    #[test]
    fn full_literal_filter_preserves_mixed_pattern_semantics() -> Result<(), crate::Error> {
        // Prepare
        let sources = literal_filter_pattern_sources();
        let patterns = sources
            .iter()
            .enumerate()
            .map(|(ordinal, source)| {
                parser::parse(
                    PatternId::new(u32::try_from(ordinal).expect("fixture ordinal fits u32")),
                    source,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let database = nfa::compile(&patterns)?;
        let prefilter = Prefilter::compile(&patterns, &database);
        let mut input = vec![u16::from(b'x'); MIN_LITERAL_INPUT_UNITS];
        place(&mut input, 0, "cat");
        place(&mut input, 100, "dogs");
        place(&mut input, 200, "horse");
        place(&mut input, 300, "foobar");
        place(&mut input, 400, "prefixabababab");
        place(&mut input, 500, "startend");
        place(&mut input, 600, "åpen");
        place(&mut input, 700, "needle42");
        place(&mut input, 800, "token0255");
        let final_start = input.len() - "horse".len();
        place(&mut input, final_start, "horse");
        let input = Utf16Text::from_units(input);

        // Test
        let (accelerated, diagnostics) = scan_events(&database, &prefilter, &input, true, true)?;
        let (baseline, _) = scan_events(&database, &prefilter, &input, false, false)?;

        // Assert
        assert_eq!(accelerated, baseline);
        assert_eq!(diagnostics.prefilter_path, PrefilterPath::Literal);
        assert_eq!(diagnostics.prefilter_bypass, PrefilterBypass::None);
        assert!(diagnostics.prefilter_candidate_starts > 0);
        assert!(diagnostics.prefilter_starts_skipped > input.as_units().len() * 9 / 10);
        assert!(accelerated.contains(&(3, 300, 306)));
        assert!(accelerated.contains(&(4, 400, 414)));
        assert!(accelerated.contains(&(5, 500, 508)));
        assert!(accelerated.contains(&(6, 600, 604)));
        assert!(accelerated.contains(&(7, 700, 708)));
        assert!(accelerated.contains(&(
            2,
            u64::try_from(final_start).expect("fixture position fits u64"),
            u64::try_from(input.as_units().len()).expect("fixture length fits u64"),
        )));
        Ok(())
    }

    #[test]
    fn unfilterable_pattern_sets_use_the_conservative_start_table() -> Result<(), crate::Error> {
        // Prepare
        let mut patterns = (0..255_u32)
            .map(|ordinal| parser::parse(PatternId::new(ordinal), &format!("literal{ordinal:04}")))
            .collect::<Result<Vec<_>, _>>()?;
        patterns.push(parser::parse(PatternId::new(255), "(?i)needle")?);
        let database = nfa::compile(&patterns)?;
        let prefilter = Prefilter::compile(&patterns, &database);
        let mut input = vec![u16::from(b'x'); MIN_LITERAL_INPUT_UNITS];
        place(&mut input, 64, "NEEDLE");
        place(&mut input, 128, "literal0254");
        let input = Utf16Text::from_units(input);

        // Test
        let (accelerated, diagnostics) = scan_events(&database, &prefilter, &input, true, true)?;
        let (baseline, _) = scan_events(&database, &prefilter, &input, false, false)?;

        // Assert
        assert_eq!(accelerated, baseline);
        assert_eq!(diagnostics.prefilter_path, PrefilterPath::StartTable);
        assert_eq!(diagnostics.prefilter_bypass, PrefilterBypass::Unfilterable);
        assert!(diagnostics.prefilter_starts_skipped > input.as_units().len() * 9 / 10);
        assert!(accelerated.contains(&(255, 64, 70)));
        assert!(accelerated.contains(&(254, 128, 139)));
        Ok(())
    }

    #[test]
    fn generated_filterable_sets_match_the_all_start_reference() -> Result<(), crate::Error> {
        for round in 0..4_usize {
            // Prepare
            let pairs = generated_pattern_pairs(round);
            let patterns = pairs
                .iter()
                .enumerate()
                .map(|(ordinal, (pattern, _))| {
                    parser::parse(
                        PatternId::new(u32::try_from(ordinal).expect("generated ordinal fits u32")),
                        pattern,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            let database = nfa::compile(&patterns)?;
            let prefilter = Prefilter::compile(&patterns, &database);
            let mut input = vec![u16::from(b'x'); MIN_LITERAL_INPUT_UNITS];
            for (ordinal, (_, matched_text)) in pairs.iter().take(16).enumerate() {
                let start = (ordinal + 1) * 32_768 + round * 257;
                place(&mut input, start, matched_text);
            }
            let input = Utf16Text::from_units(input);

            // Test
            let (accelerated, diagnostics) =
                scan_events(&database, &prefilter, &input, true, true)?;
            let (baseline, _) = scan_events(&database, &prefilter, &input, false, false)?;

            // Assert
            assert_eq!(accelerated, baseline, "generated round {round}");
            assert_eq!(diagnostics.prefilter_path, PrefilterPath::Literal);
            assert!(diagnostics.prefilter_starts_skipped > input.as_units().len() * 9 / 10);
        }
        Ok(())
    }

    #[test]
    fn hidden_disable_control_preserves_the_all_start_baseline() -> Result<(), crate::Error> {
        // Prepare
        let patterns = (0..256_u32)
            .map(|ordinal| parser::parse(PatternId::new(ordinal), &format!("needle{ordinal:04}")))
            .collect::<Result<Vec<_>, _>>()?;
        let database = nfa::compile(&patterns)?;
        let prefilter = Prefilter::compile(&patterns, &database);

        // Test
        let plan = prefilter.plan(&vec![u16::from(b'x'); 1024 * 1024], false, true);

        // Assert
        assert_eq!(plan.path(), PrefilterPath::AllStarts);
        assert_eq!(plan.bypass(), PrefilterBypass::Disabled);
        Ok(())
    }

    #[test]
    fn candidate_bitmap_iterates_in_start_order_without_duplicates() {
        // Prepare
        let mut bitmap = CandidateBitmap::new(130);
        bitmap.insert(129);
        bitmap.insert(1);
        bitmap.insert(64);
        bitmap.insert(1);

        // Test
        let starts = bitmap.iter().collect::<Vec<_>>();

        // Assert
        assert_eq!(starts, vec![1, 64, 129]);
        assert_eq!(bitmap.count(), 3);
    }

    #[test]
    fn public_patterns_still_compile_without_exposing_prefilter_types() -> Result<(), crate::Error>
    {
        // Prepare
        let mut builder = MatcherBuilder::new();
        builder.add(PatternId::new(7), "needle")?;

        // Test
        let matcher = builder.build()?;
        let mut spans = Vec::new();
        matcher.scan(&Utf16Text::from("a needle"), |event| {
            spans.push((event.span().start(), event.span().end()));
        })?;

        // Assert
        assert_eq!(spans, vec![(2, 8)]);
        Ok(())
    }

    fn literal(value: &str) -> Hir {
        Hir::sequence(utf16(value).into_iter().map(Hir::Symbol).collect())
    }

    fn hint(ordinal: usize, value: &str) -> NecessaryLiteral {
        NecessaryLiteral {
            ordinal,
            offset: 0,
            units: utf16(value).into_boxed_slice(),
        }
    }

    fn literal_filter_pattern_sources() -> Vec<String> {
        let mut sources = vec![
            "cat".to_owned(),
            "dogs".to_owned(),
            "horse".to_owned(),
            "fooba(?:r|z)".to_owned(),
            "prefix(?:ab){2,4}".to_owned(),
            "start(?:xy)?end".to_owned(),
            "åpen".to_owned(),
            "needle[0-9]+".to_owned(),
        ];
        sources.extend((sources.len()..256).map(|ordinal| format!("token{ordinal:04}")));
        sources
    }

    fn generated_pattern_pairs(round: usize) -> Vec<(String, String)> {
        (0..256)
            .map(|ordinal| {
                let prefix = format!("g{round:02}{ordinal:04}");
                match (ordinal + round) % 4 {
                    0 => (format!("{prefix}(?:ab|ac)"), format!("{prefix}ac")),
                    1 => (format!("{prefix}x{{1,3}}z"), format!("{prefix}xxxz")),
                    2 => (format!("{prefix}(?:uv)?w"), format!("{prefix}uvw")),
                    _ => (format!("{prefix}[a-c]+q"), format!("{prefix}abcq")),
                }
            })
            .collect()
    }

    fn place(input: &mut [u16], start: usize, value: &str) {
        let units = utf16(value);
        input[start..start + units.len()].copy_from_slice(&units);
    }

    fn scan_events(
        database: &crate::nfa::PatternDatabase,
        prefilter: &Prefilter,
        input: &Utf16Text,
        prefilter_enabled: bool,
        literal_prefilter_enabled: bool,
    ) -> TestScan {
        let mut events = Vec::new();
        let diagnostics = crate::engine::scan_with_stats(
            database,
            prefilter,
            input,
            crate::engine::DEFAULT_STATE_CACHE_BUDGET,
            prefilter_enabled,
            literal_prefilter_enabled,
            |matched| {
                events.push((
                    matched.pattern_id().get(),
                    matched.span().start(),
                    matched.span().end(),
                ));
            },
        )?;
        Ok((events, diagnostics))
    }

    fn utf16(value: &str) -> Vec<u16> {
        value.encode_utf16().collect()
    }
}
