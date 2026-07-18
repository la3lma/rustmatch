#!/usr/bin/env bash

set -euo pipefail

repository_root=$(git rev-parse --show-toplevel)
base_ref=${BASE_SHA:-284571996b343041d374f2620905f32d63dbafdc}
candidate_ref=${CANDIDATE_SHA:-HEAD}
base_revision=$(git -C "$repository_root" rev-parse "${base_ref}^{commit}")
candidate_revision=$(git -C "$repository_root" rev-parse "${candidate_ref}^{commit}")
source_revision=$(git -C "$repository_root" rev-parse HEAD)
runner_label=${RUSTMATCH_BENCH_RUNNER:-$(hostname) | $(uname -m)}
run_root=${I7_WUTHERING_RUN_ROOT:-$repository_root/target/i7-wuthering-campaign}
cache_scrub_bytes=${CACHE_SCRUB_BYTES:-268435456}
pattern_source="$repository_root/benchmarks/fixtures/wuthering-heights/real-words-in-wuthering-heights.txt"
corpus_source="$repository_root/benchmarks/fixtures/wuthering-heights/wuthr10.txt"
corpus_bytes=${CORPUS_BYTES:-8388608}
rust_target=${I7_CARGO_TARGET:-}
build_target_args=()
binary_subdirectory=release
if [[ -n $rust_target ]]; then
  build_target_args=(--target "$rust_target")
  binary_subdirectory="$rust_target/release"
fi

if [[ $candidate_revision != "$source_revision" ]]; then
  echo "CANDIDATE_SHA must identify the checked-out source revision" >&2
  exit 2
fi
if [[ ${I7_ALLOW_DIRTY:-0} != 1 ]] && [[ -n $(git -C "$repository_root" status --porcelain) ]]; then
  echo "I7 refuses to mislabel a dirty working tree; commit it or set I7_ALLOW_DIRTY=1" >&2
  exit 2
fi

base_source="$run_root/base-source"
base_target="$run_root/base-target"
candidate_target="$run_root/candidate-target"
receipts="$run_root/receipts"
mkdir -p "$receipts"

cleanup() {
  if [[ -d $base_source ]]; then
    git -C "$repository_root" worktree remove --force "$base_source" >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

git -C "$repository_root" worktree add --detach "$base_source" "$base_revision"
(
  cd "$base_source"
  CARGO_TARGET_DIR="$base_target" cargo build --locked --release \
    --package rustmatch-bench "${build_target_args[@]}"
)
(
  cd "$repository_root"
  CARGO_TARGET_DIR="$candidate_target" cargo build --locked --release \
    --package rustmatch-bench "${build_target_args[@]}"
)
base_binary="$base_target/$binary_subdirectory/rustmatch-bench"
candidate_binary="$candidate_target/$binary_subdirectory/rustmatch-bench"
file "$base_binary"
file "$candidate_binary"

measure() {
  local binary=$1
  local mode=$2
  local revision=$3
  local pattern_count=$4
  local output=$5
  RUSTMATCH_BENCH_PREFILTER="$mode" \
  RUSTMATCH_BENCH_WARMUP_ITERATIONS=3 \
  RUSTMATCH_BENCH_MEASURED_ITERATIONS=7 \
  RUSTMATCH_BENCH_REVISION="$revision" \
  RUSTMATCH_BENCH_RUNNER="$runner_label" \
    "$binary" wuthering-scan "$pattern_source" "$corpus_source" \
      "$pattern_count" "$corpus_bytes" "$cache_scrub_bytes" > "$output"
}

read -r -a pattern_counts <<< "${PATTERN_COUNTS:-1000 5000 10000}"
report_receipts=()
for pattern_count in "${pattern_counts[@]}"; do
  stem="wuthering-${pattern_count}-${corpus_bytes}"
  disabled_receipt="$receipts/${stem}-disabled.json"
  enabled_receipt="$receipts/${stem}-enabled.json"
  comparison_receipt="$receipts/${stem}-comparison.json"

  measure "$candidate_binary" off "$candidate_revision" "$pattern_count" "$disabled_receipt"
  measure "$candidate_binary" on "$candidate_revision" "$pattern_count" "$enabled_receipt"
  if ! "$candidate_binary" compare-i7-wuthering "$disabled_receipt" "$enabled_receipt" \
    > "$comparison_receipt"; then
    echo "I7 Wuthering threshold crossed for $pattern_count patterns; repeating in reverse order" >&2
    measure "$candidate_binary" on "$candidate_revision" "$pattern_count" "$enabled_receipt"
    measure "$candidate_binary" off "$candidate_revision" "$pattern_count" "$disabled_receipt"
    "$candidate_binary" compare-i7-wuthering "$disabled_receipt" "$enabled_receipt" \
      > "$comparison_receipt"
  fi
  cat "$comparison_receipt"
  report_receipts+=("$disabled_receipt" "$enabled_receipt")
done

"$candidate_binary" render-table "$run_root/index.html" "${report_receipts[@]}" >/dev/null

base_build_receipt="$receipts/wuthering-10000-${corpus_bytes}-frozen-base.json"
candidate_build_receipt="$receipts/wuthering-10000-${corpus_bytes}-enabled.json"
measure "$base_binary" off "$base_revision" 10000 "$base_build_receipt"
if [[ ! -f $candidate_build_receipt ]]; then
  measure "$candidate_binary" on "$candidate_revision" 10000 "$candidate_build_receipt"
fi
base_compile_ns=$(jq -r '.median_compile_ns' "$base_build_receipt")
candidate_compile_ns=$(jq -r '.median_compile_ns' "$candidate_build_receipt")
compile_regression_ns=$((candidate_compile_ns - base_compile_ns))
if (( compile_regression_ns < 0 )); then
  compile_regression_ns=0
fi
if (( compile_regression_ns > 25000000 )); then
  echo "I7 frozen-base build gate failed: regression=${compile_regression_ns}ns" >&2
  exit 1
fi
printf '{"schema_version":1,"evidence_id":"I7-P1","comparison":"frozen-base-build-v1","baseline_revision":"%s","candidate_revision":"%s","baseline_median_compile_ns":%s,"candidate_median_compile_ns":%s,"compile_regression_ns":%s,"status":"pass"}\n' \
  "$base_revision" "$candidate_revision" "$base_compile_ns" "$candidate_compile_ns" \
  "$compile_regression_ns" > "$receipts/frozen-base-build-comparison.json"
cat "$receipts/frozen-base-build-comparison.json"

printf 'I7 Wuthering receipts: %s\n' "$receipts"
printf 'I7 Wuthering report: %s\n' "$run_root/index.html"
