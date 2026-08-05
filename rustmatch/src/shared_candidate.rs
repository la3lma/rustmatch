//! Scan-local candidate discovery shared by large literal partitions.

use std::mem::size_of;
use std::panic;
use std::thread;

use crate::Error;
use crate::prefilter::{
    CandidateBitmap, Prefilter, SharedLiteralFilter, ascii_triple_index, contains_bit,
    prefix_hash_index,
};

#[cfg(not(test))]
const MIN_SHARED_PARTITION_COUNT: usize = 16;
#[cfg(test)]
const MIN_SHARED_PARTITION_COUNT: usize = 2;
#[cfg(not(test))]
const MIN_SHARED_INPUT_UNITS: usize = 32 * 1024 * 1024;
#[cfg(test)]
const MIN_SHARED_INPUT_UNITS: usize = 1024 * 1024;
const DENSITY_SAMPLE_UNITS: usize = 64 * 1024;
const MAX_SHARED_PLAN_SHARDS: usize = 8;
const MIN_LITERAL_UNITS: usize = 3;

pub(crate) fn is_eligible(
    partition_count: usize,
    input_units: usize,
    prefilter_enabled: bool,
    literal_prefilter_enabled: bool,
) -> bool {
    prefilter_enabled
        && literal_prefilter_enabled
        && partition_count >= MIN_SHARED_PARTITION_COUNT
        && input_units >= MIN_SHARED_INPUT_UNITS
}

pub(crate) fn plan(
    partitions: &[&Prefilter],
    input: &[u16],
    prefilter_enabled: bool,
    literal_prefilter_enabled: bool,
) -> Result<Option<SharedCandidatePlan>, Error> {
    if !is_eligible(
        partitions.len(),
        input.len(),
        prefilter_enabled,
        literal_prefilter_enabled,
    ) {
        return Ok(None);
    }
    let filters = partitions
        .iter()
        .map(|prefilter| prefilter.shared_literal_filter())
        .collect::<Option<Vec<_>>>();
    let Some(filters) = filters else {
        return Ok(None);
    };
    let union = UnionLiteralFilter::compile(&filters);

    let sample_len = input.len().min(DENSITY_SAMPLE_UNITS);
    let sample_count = (0..sample_len)
        .filter(|&start| union.allows_start(input, start))
        .count();
    if sample_count.saturating_mul(2) >= sample_len {
        return Ok(None);
    }

    let mut candidates = CandidateBitmap::new(input.len());
    let candidate_words = candidates.words_mut();
    let shard_count = MAX_SHARED_PLAN_SHARDS
        .min(partitions.len())
        .min(candidate_words.len())
        .max(1);
    let words_per_shard = candidate_words.len().div_ceil(shard_count);
    let candidate_count = thread::scope(|scope| {
        let union = &union;
        let mut handles = Vec::with_capacity(shard_count);
        let mut spawn_error = None;
        for (shard_index, words) in candidate_words.chunks_mut(words_per_shard).enumerate() {
            let first_word = shard_index * words_per_shard;
            if let Ok(handle) = thread::Builder::new()
                .name(format!("rustmatch-candidate-{shard_index}"))
                .spawn_scoped(scope, move || {
                    scan_word_slice(union, input, first_word, words)
                })
            {
                handles.push(handle);
            } else {
                spawn_error = Some(Error::WorkerUnavailable);
                break;
            }
        }

        let mut count = 0_usize;
        let mut worker_panic = None;
        for handle in handles {
            match handle.join() {
                Ok(shard_count) => count = count.saturating_add(shard_count),
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
        spawn_error.map_or(Ok(count), Err)
    })?;
    candidates.set_count(candidate_count);
    Ok(Some(SharedCandidatePlan {
        candidates,
        shared_retained_bytes: union.retained_bytes(),
    }))
}

#[derive(Debug)]
pub(crate) struct SharedCandidatePlan {
    candidates: CandidateBitmap,
    shared_retained_bytes: usize,
}

impl SharedCandidatePlan {
    pub(crate) fn partition(&self, index: usize) -> SharedCandidate<'_> {
        SharedCandidate {
            candidates: &self.candidates,
            account_storage: index == 0,
            extra_retained_bytes: usize::from(index == 0) * self.shared_retained_bytes,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct SharedCandidate<'a> {
    candidates: &'a CandidateBitmap,
    account_storage: bool,
    extra_retained_bytes: usize,
}

impl SharedCandidate<'_> {
    pub(crate) fn starts(self) -> impl Iterator<Item = usize> {
        self.candidates.iter()
    }

    pub(crate) const fn accounted_count(self) -> usize {
        if self.account_storage {
            self.candidates.count()
        } else {
            0
        }
    }

    pub(crate) fn accounted_bytes(self) -> usize {
        if self.account_storage {
            self.candidates.retained_bytes()
        } else {
            0
        }
    }

    pub(crate) const fn extra_retained_bytes(self) -> usize {
        self.extra_retained_bytes
    }
}

#[derive(Debug)]
struct UnionLiteralFilter {
    ascii_three_exact: Box<[u64]>,
    ascii_four_hash: Box<[u64]>,
    ascii_five_hash: Box<[u64]>,
    has_three: bool,
    has_four: bool,
    has_five: bool,
}

impl UnionLiteralFilter {
    fn compile(filters: &[SharedLiteralFilter<'_>]) -> Self {
        let first = filters
            .first()
            .copied()
            .expect("an eligible shared plan has partitions");
        let mut union = Self {
            ascii_three_exact: first.ascii_three_exact().into(),
            ascii_four_hash: first.ascii_four_hash().into(),
            ascii_five_hash: first.ascii_five_hash().into(),
            has_three: first.has_three(),
            has_four: first.has_four(),
            has_five: first.has_five(),
        };
        for filter in &filters[1..] {
            union_words(&mut union.ascii_three_exact, filter.ascii_three_exact());
            union_words(&mut union.ascii_four_hash, filter.ascii_four_hash());
            union_words(&mut union.ascii_five_hash, filter.ascii_five_hash());
            union.has_three |= filter.has_three();
            union.has_four |= filter.has_four();
            union.has_five |= filter.has_five();
        }
        union
    }

    fn allows_start(&self, input: &[u16], start: usize) -> bool {
        let Some(triple_end) = start.checked_add(MIN_LITERAL_UNITS) else {
            return false;
        };
        if triple_end > input.len() {
            return false;
        }

        let triple = &input[start..triple_end];
        if triple.iter().any(|&symbol| symbol >= 128) {
            return true;
        }
        if self.has_three
            && contains_bit(
                &self.ascii_three_exact,
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
                &self.ascii_four_hash,
                prefix_hash_index(four, self.ascii_four_hash.len() * 64),
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
                &self.ascii_five_hash,
                prefix_hash_index(five, self.ascii_five_hash.len() * 64),
            )
    }

    fn retained_bytes(&self) -> usize {
        size_of::<Self>()
            + self.ascii_three_exact.len() * size_of::<u64>()
            + self.ascii_four_hash.len() * size_of::<u64>()
            + self.ascii_five_hash.len() * size_of::<u64>()
    }

    const fn supports_avx2_five(&self) -> bool {
        self.has_five && !self.has_three && !self.has_four
    }
}

fn union_words(union: &mut [u64], partition: &[u64]) {
    for (union_word, partition_word) in union.iter_mut().zip(partition) {
        *union_word |= partition_word;
    }
}

#[allow(clippy::inline_always)]
#[allow(clippy::collapsible_if)] // `let` chains are newer than the 1.85.0 MSRV.
#[inline(always)]
fn scan_word_slice(
    filter: &UnionLiteralFilter,
    input: &[u16],
    first_word: usize,
    words: &mut [u64],
) -> usize {
    let first_start = first_word.saturating_mul(64);
    if filter.supports_avx2_five() {
        if let Some(admissions) =
            rustmatch_simd::scan_five_hash_words(input, &filter.ascii_five_hash, first_start, words)
        {
            return admissions;
        }
    }
    scan_word_slice_scalar(filter, input, first_start, words)
}

fn scan_word_slice_scalar(
    filter: &UnionLiteralFilter,
    input: &[u16],
    first_start: usize,
    words: &mut [u64],
) -> usize {
    let end_start = first_start
        .saturating_add(words.len().saturating_mul(64))
        .min(input.len());
    let mut admissions = 0_usize;
    for start in first_start..end_start {
        if !filter.allows_start(input, start) {
            continue;
        }
        let local_start = start - first_start;
        words[local_start / 64] |= 1_u64 << (local_start % 64);
        admissions = admissions.saturating_add(1);
    }
    admissions
}

#[cfg(test)]
mod tests {
    use super::plan;
    use crate::nfa;
    use crate::prefilter::{Prefilter, ScanPlan};
    use crate::{PatternId, parser};

    #[test]
    fn shared_candidates_preserve_each_private_partition_set() -> Result<(), crate::Error> {
        // Prepare
        let patterns = literal_patterns()?;
        let left_database = nfa::compile(&patterns[..256])?;
        let right_database = nfa::compile(&patterns[256..])?;
        let left = Prefilter::compile(&patterns[..256], &left_database);
        let right = Prefilter::compile(&patterns[256..], &right_database);
        let mut input = vec![u16::from(b'x'); 1024 * 1024];
        place(&mut input, 128, "left0255needle");
        place(&mut input, 65_664, "right0511needle");
        place(&mut input, 131_200, "left9999near-miss");

        // Test
        let shared = plan(&[&left, &right], &input, true, true)?
            .expect("a sparse compatible input uses shared candidates");
        let left_private = private_candidates(left.plan(&input, true, true));
        let right_private = private_candidates(right.plan(&input, true, true));
        let left_shared = shared
            .partition(0)
            .starts()
            .filter(|&start| left.shared_allows_start(&input, start))
            .collect::<Vec<_>>();
        let right_shared = shared
            .partition(1)
            .starts()
            .filter(|&start| right.shared_allows_start(&input, start))
            .collect::<Vec<_>>();

        // Assert
        assert_eq!(left_shared, left_private);
        assert_eq!(right_shared, right_private);
        Ok(())
    }

    #[test]
    fn shared_candidates_fall_back_for_a_dense_union_sample() -> Result<(), crate::Error> {
        // Prepare
        let patterns = (0..512_u32)
            .map(|ordinal| parser::parse(PatternId::new(ordinal), "aaa"))
            .collect::<Result<Vec<_>, _>>()?;
        let left_database = nfa::compile(&patterns[..256])?;
        let right_database = nfa::compile(&patterns[256..])?;
        let left = Prefilter::compile(&patterns[..256], &left_database);
        let right = Prefilter::compile(&patterns[256..], &right_database);
        let input = vec![u16::from(b'a'); 1024 * 1024];

        // Test
        let shared = plan(&[&left, &right], &input, true, true)?;

        // Assert
        assert!(shared.is_none());
        Ok(())
    }

    fn literal_patterns() -> Result<Vec<crate::hir::HirPattern>, crate::Error> {
        (0..512_u32)
            .map(|ordinal| {
                let prefix = if ordinal < 256 { "left" } else { "right" };
                parser::parse(
                    PatternId::new(ordinal),
                    &format!("{prefix}{ordinal:04}needle"),
                )
            })
            .collect()
    }

    fn private_candidates(plan: ScanPlan<'_>) -> Vec<usize> {
        match plan {
            ScanPlan::Candidates { candidates, .. } => candidates.iter().collect(),
            _ => panic!("test fixture must use literal candidates"),
        }
    }

    fn place(input: &mut [u16], start: usize, text: &str) {
        let encoded = text.encode_utf16().collect::<Vec<_>>();
        input[start..start + encoded.len()].copy_from_slice(&encoded);
    }
}
