use std::alloc::{GlobalAlloc, Layout, System};
use std::env;
use std::error::Error;
use std::fs;
use std::hint::black_box;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use rustmatch::{MatcherBuilder, PatternId, SharedCohortDiagnostics};

struct CountingAllocator;

static ALLOCATION_CALLS: AtomicU64 = AtomicU64::new(0);
static REQUESTED_BYTES: AtomicU64 = AtomicU64::new(0);

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATION_CALLS.fetch_add(1, Ordering::Relaxed);
        REQUESTED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        ALLOCATION_CALLS.fetch_add(1, Ordering::Relaxed);
        REQUESTED_BYTES.fetch_add(layout.size() as u64, Ordering::Relaxed);
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        unsafe { System.dealloc(pointer, layout) };
    }

    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATION_CALLS.fetch_add(1, Ordering::Relaxed);
        REQUESTED_BYTES.fetch_add(new_size as u64, Ordering::Relaxed);
        unsafe { System.realloc(pointer, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: CountingAllocator = CountingAllocator;

#[derive(Clone, Copy)]
struct AllocationSnapshot {
    calls: u64,
    requested_bytes: u64,
}

impl AllocationSnapshot {
    fn capture() -> Self {
        Self {
            calls: ALLOCATION_CALLS.load(Ordering::Relaxed),
            requested_bytes: REQUESTED_BYTES.load(Ordering::Relaxed),
        }
    }

    fn difference(self, earlier: Self) -> Self {
        Self {
            calls: self.calls - earlier.calls,
            requested_bytes: self.requested_bytes - earlier.requested_bytes,
        }
    }
}

struct PatternSpec {
    id: u32,
    expression: Box<str>,
}

#[derive(Clone, Copy)]
enum Variant {
    Generic,
    Specialized,
}

impl Variant {
    const fn name(self) -> &'static str {
        match self {
            Self::Generic => "generic",
            Self::Specialized => "specialized",
        }
    }
}

#[derive(Clone, Copy)]
enum PairOrder {
    Ab,
    Ba,
}

impl PairOrder {
    const fn name(self) -> &'static str {
        match self {
            Self::Ab => "ab",
            Self::Ba => "ba",
        }
    }

    const fn variants(self) -> [Variant; 2] {
        match self {
            Self::Ab => [Variant::Generic, Variant::Specialized],
            Self::Ba => [Variant::Specialized, Variant::Generic],
        }
    }
}

struct BuildSample {
    elapsed: Duration,
    allocations: AllocationSnapshot,
    diagnostics: SharedCohortDiagnostics,
}

struct Record {
    order: PairOrder,
    pair: usize,
    position: usize,
    variant: Variant,
    sample: BuildSample,
}

fn parse_patterns(path: &Path) -> Result<Vec<PatternSpec>, Box<dyn Error>> {
    let source = fs::read_to_string(path)?;
    source
        .lines()
        .enumerate()
        .map(|(line_index, line)| {
            let (id, expression) = line
                .split_once('\t')
                .ok_or_else(|| format!("pattern line {} has no tab", line_index + 1))?;
            Ok(PatternSpec {
                id: id.parse()?,
                expression: expression.into(),
            })
        })
        .collect()
}

fn prepare_builder(patterns: &[PatternSpec]) -> Result<MatcherBuilder, Box<dyn Error>> {
    let mut builder = MatcherBuilder::new();
    for pattern in patterns {
        builder.add(PatternId::new(pattern.id), &pattern.expression)?;
    }
    Ok(builder)
}

fn build_once(patterns: &[PatternSpec], variant: Variant) -> Result<BuildSample, Box<dyn Error>> {
    // Registration and regular-expression parsing are intentionally outside
    // the timed and allocation-difference interval.
    let builder = prepare_builder(patterns)?;
    let allocation_start = AllocationSnapshot::capture();
    let started = Instant::now();
    let matcher = match variant {
        Variant::Generic => builder.build_shared_cohort_diagnostic()?,
        Variant::Specialized => builder.build_shared_cohort_assertion_diagnostic()?,
    };
    let ended = Instant::now();
    let allocation_end = AllocationSnapshot::capture();
    let diagnostics = matcher.structure_diagnostics();
    black_box(&matcher);
    drop(matcher);
    Ok(BuildSample {
        elapsed: ended.duration_since(started),
        allocations: allocation_end.difference(allocation_start),
        diagnostics,
    })
}

fn run_pair(patterns: &[PatternSpec], order: PairOrder) -> Result<(), Box<dyn Error>> {
    for variant in order.variants() {
        black_box(build_once(patterns, variant)?);
    }
    Ok(())
}

fn json_bool(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

fn print_record(fixture: &str, record: &Record) {
    let diagnostics = record.sample.diagnostics;
    println!(
        concat!(
            "{{\"schema_version\":1,\"fixture\":\"{}\",\"order\":\"{}\",",
            "\"pair\":{},\"position\":{},\"variant\":\"{}\",\"elapsed_ns\":{},",
            "\"allocation_calls\":{},\"requested_bytes\":{},\"state_count\":{},",
            "\"edge_count\":{},\"predicate_count\":{},\"terminal_count\":{},",
            "\"pattern_count\":{},\"assertion_free_pattern_count\":{},",
            "\"assertion_bearing_pattern_count\":{},\"database_retained_bytes\":{},",
            "\"prefilter_retained_bytes\":{},\"assertion_prefix_available\":{},",
            "\"assertion_specialization_enabled\":{}}}"
        ),
        fixture,
        record.order.name(),
        record.pair,
        record.position,
        record.variant.name(),
        record.sample.elapsed.as_nanos(),
        record.sample.allocations.calls,
        record.sample.allocations.requested_bytes,
        diagnostics.state_count(),
        diagnostics.edge_count(),
        diagnostics.predicate_count(),
        diagnostics.terminal_count(),
        diagnostics.pattern_count(),
        diagnostics.assertion_free_pattern_count(),
        diagnostics.assertion_bearing_pattern_count(),
        diagnostics.database_retained_bytes(),
        diagnostics.prefilter_retained_bytes(),
        json_bool(diagnostics.assertion_prefix_available()),
        json_bool(diagnostics.assertion_specialization_enabled()),
    );
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut arguments = env::args_os().skip(1);
    let pattern_path = arguments.next().ok_or("missing pattern path")?;
    let fixture = arguments
        .next()
        .ok_or("missing fixture ID")?
        .into_string()
        .map_err(|_| "fixture ID is not UTF-8")?;
    let warmup_pairs_per_order: usize = arguments
        .next()
        .ok_or("missing warmup-pair count")?
        .into_string()
        .map_err(|_| "warmup-pair count is not UTF-8")?
        .parse()?;
    let retained_pairs_per_order: usize = arguments
        .next()
        .ok_or("missing retained-pair count")?
        .into_string()
        .map_err(|_| "retained-pair count is not UTF-8")?
        .parse()?;
    if arguments.next().is_some() {
        return Err("unexpected preparation-probe argument".into());
    }

    let patterns = parse_patterns(Path::new(&pattern_path))?;
    if patterns.is_empty() {
        return Err("pattern fixture is empty".into());
    }

    for _ in 0..warmup_pairs_per_order {
        run_pair(&patterns, PairOrder::Ab)?;
        run_pair(&patterns, PairOrder::Ba)?;
    }

    let mut records = Vec::with_capacity(retained_pairs_per_order * 4);
    for pair in 1..=retained_pairs_per_order {
        for order in [PairOrder::Ab, PairOrder::Ba] {
            for (position, variant) in order.variants().into_iter().enumerate() {
                records.push(Record {
                    order,
                    pair,
                    position: position + 1,
                    variant,
                    sample: build_once(&patterns, variant)?,
                });
            }
        }
    }

    for record in &records {
        print_record(&fixture, record);
    }
    Ok(())
}
