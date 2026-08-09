//! Benchmark-only cohort construction and mixed-cohort scheduling.

use std::any::Any;
use std::panic;
use std::thread;

use crate::shared_candidate;
use crate::{Error, Utf16Text};

use super::{
    CohortLayout, Matcher, MatcherBuilder, MatcherCohortDiagnostics, MatcherPartition,
    ParallelScanOutput, PartitionScanOutput, compile_partitions, scan_partition,
    scan_partition_shared,
};

pub(super) fn build_matcher(builder: MatcherBuilder) -> Result<Matcher, Error> {
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

pub(super) fn allocate_cohort_partitions(
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

pub(super) fn execution_diagnostics(matcher: &Matcher) -> MatcherCohortDiagnostics {
    let split_partition = matcher
        .cohort_layout
        .split_partition
        .unwrap_or(matcher.partitions.len());
    let assertion_free_retained_bytes = if matcher.cohort_layout.assertion_free_pattern_count > 0 {
        matcher.partitions[..split_partition]
            .iter()
            .map(MatcherPartition::retained_bytes)
            .sum()
    } else {
        0
    };
    let assertion_bearing_start = if matcher.cohort_layout.split_partition.is_some() {
        split_partition
    } else {
        0
    };
    let assertion_bearing_retained_bytes =
        if matcher.cohort_layout.assertion_bearing_pattern_count > 0 {
            matcher.partitions[assertion_bearing_start..]
                .iter()
                .map(MatcherPartition::retained_bytes)
                .sum()
        } else {
            0
        };
    MatcherCohortDiagnostics {
        enabled: matcher.cohort_layout.enabled,
        cohort_count: matcher.cohort_layout.cohort_count(),
        assertion_free_pattern_count: matcher.cohort_layout.assertion_free_pattern_count,
        assertion_bearing_pattern_count: matcher.cohort_layout.assertion_bearing_pattern_count,
        assertion_free_partition_count: matcher.cohort_layout.assertion_free_partition_count,
        assertion_bearing_partition_count: matcher.cohort_layout.assertion_bearing_partition_count,
        total_partition_count: matcher.partitions.len(),
        total_cache_budget: matcher
            .partitions
            .iter()
            .map(|partition| partition.state_cache_budget)
            .sum(),
        assertion_free_retained_bytes,
        assertion_bearing_retained_bytes,
        spawned_workers: spawned_worker_count(matcher),
        buffers_before_delivery: matcher.partitions.len() > 1,
    }
}

pub(super) fn spawned_worker_count(matcher: &Matcher) -> usize {
    matcher.cohort_layout.split_partition.map_or_else(
        || matcher.partitions.len().saturating_sub(1),
        |split_partition| {
            if matcher.requested_worker_count == 1 {
                split_partition.saturating_sub(1)
                    + matcher
                        .partitions
                        .len()
                        .saturating_sub(split_partition)
                        .saturating_sub(1)
            } else {
                matcher.partitions.len().saturating_sub(1)
            }
        },
    )
}

pub(super) fn scan_parallel_with_hooks<BeforeSpawn, BeforeScan>(
    matcher: &Matcher,
    input: &Utf16Text,
    before_spawn: &BeforeSpawn,
    before_scan: &BeforeScan,
) -> Result<ParallelScanOutput, Error>
where
    BeforeSpawn: Fn(usize) -> Result<(), Error>,
    BeforeScan: Fn(usize) + Sync,
{
    let split_partition = matcher
        .cohort_layout
        .split_partition
        .expect("cohort scheduler requires a mixed layout");
    if matcher.requested_worker_count == 1 {
        let mut outputs = scan_partition_group_with_hooks(
            matcher,
            &matcher.partitions[..split_partition],
            0,
            input,
            before_spawn,
            before_scan,
        )?;
        outputs.extend(scan_partition_group_with_hooks(
            matcher,
            &matcher.partitions[split_partition..],
            split_partition,
            input,
            before_spawn,
            before_scan,
        )?);
        return Ok(ParallelScanOutput::new(outputs));
    }

    scan_parallel_cohorts_with_hooks(matcher, split_partition, input, before_spawn, before_scan)
}

fn scan_parallel_cohorts_with_hooks<BeforeSpawn, BeforeScan>(
    matcher: &Matcher,
    split_partition: usize,
    input: &Utf16Text,
    before_spawn: &BeforeSpawn,
    before_scan: &BeforeScan,
) -> Result<ParallelScanOutput, Error>
where
    BeforeSpawn: Fn(usize) -> Result<(), Error>,
    BeforeScan: Fn(usize) + Sync,
{
    let assertion_free = &matcher.partitions[..split_partition];
    let assertion_bearing = &matcher.partitions[split_partition..];
    let assertion_free_plan = shared_candidate_plan(matcher, assertion_free, input)?;
    let assertion_bearing_plan = shared_candidate_plan(matcher, assertion_bearing, input)?;
    let partitions = thread::scope(|scope| {
        let mut handles = Vec::with_capacity(matcher.partitions.len().saturating_sub(1));
        let mut spawn_error = None;
        for (partition_index, partition) in matcher.partitions.iter().enumerate().skip(1) {
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
                            matcher.prefilter_enabled,
                            matcher.literal_prefilter_enabled,
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
                    Some(shared) => scan_partition_shared(&matcher.partitions[0], input, shared),
                    None => scan_partition(
                        &matcher.partitions[0],
                        input,
                        matcher.prefilter_enabled,
                        matcher.literal_prefilter_enabled,
                    ),
                },
            )
        } else {
            None
        };
        let mut outputs: Vec<Option<PartitionScanOutput>> =
            (0..matcher.partitions.len()).map(|_| None).collect();
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
    matcher: &Matcher,
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
    let shared_plan = shared_candidate_plan(matcher, partitions, input)?;
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
                            matcher.prefilter_enabled,
                            matcher.literal_prefilter_enabled,
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
                    matcher.prefilter_enabled,
                    matcher.literal_prefilter_enabled,
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
        Ok(outputs
            .into_iter()
            .map(|output| output.expect("every successful partition returned output"))
            .collect::<Vec<_>>())
    })
}

fn shared_candidate_plan(
    matcher: &Matcher,
    partitions: &[MatcherPartition],
    input: &Utf16Text,
) -> Result<Option<shared_candidate::SharedCandidatePlan>, Error> {
    if !shared_candidate::is_eligible(
        partitions.len(),
        input.units().len(),
        matcher.prefilter_enabled,
        matcher.literal_prefilter_enabled,
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
        matcher.prefilter_enabled,
        matcher.literal_prefilter_enabled,
    )
}
