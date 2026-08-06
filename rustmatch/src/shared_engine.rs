//! Semantic verification for scan-local shared candidate starts.

use crate::nfa::PatternDatabase;
use crate::prefilter::{Prefilter, PrefilterBypass, PrefilterPath};
use crate::shared_candidate::SharedCandidate;
use crate::{Error, Match, Utf16Text};

use super::{ScanStats, scan_without_assertions_dispatch};

pub(crate) fn scan_with_shared_candidates_and_stats(
    database: &PatternDatabase,
    prefilter: &Prefilter,
    input: &Utf16Text,
    state_cache_budget: usize,
    shared: SharedCandidate<'_>,
    mut sink: impl FnMut(Match),
) -> Result<ScanStats, Error> {
    let units = input.units();
    let accounted_count = shared.accounted_count();
    let mut metrics = ScanStats {
        prefilter_path: PrefilterPath::Literal,
        prefilter_bypass: PrefilterBypass::None,
        prefilter_retained_bytes: prefilter
            .retained_bytes()
            .saturating_add(shared.extra_retained_bytes()),
        prefilter_candidate_bytes: shared.accounted_bytes(),
        prefilter_admissions: accounted_count as u64,
        prefilter_candidate_starts: accounted_count,
        ..ScanStats::default()
    };

    let mut starts_scanned = 0_usize;
    let starts = shared
        .starts()
        .filter(|&start| prefilter.shared_allows_start(units, start))
        .inspect(|_| starts_scanned += 1);
    scan_without_assertions_dispatch(
        database,
        database.root(),
        input,
        state_cache_budget,
        starts,
        &mut metrics,
        &mut sink,
    )?;
    metrics.prefilter_starts_scanned = starts_scanned;
    metrics.prefilter_starts_skipped = units.len().saturating_sub(starts_scanned);
    Ok(metrics)
}
