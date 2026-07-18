#!/usr/bin/env bash

set -euo pipefail

repository_root=$(git rev-parse --show-toplevel)
candidate_ref=${CANDIDATE_SHA:-HEAD}
candidate_revision=$(git -C "$repository_root" rev-parse "${candidate_ref}^{commit}")
source_revision=$(git -C "$repository_root" rev-parse HEAD)
runner_label=${RUSTMATCH_BENCH_RUNNER:-$(hostname) | $(uname -m)}
run_root=${I7_RUN_ROOT:-$repository_root/target/i7-focused-campaign}
cache_scrub_bytes=${CACHE_SCRUB_BYTES:-268435456}
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

target_directory=${CARGO_TARGET_DIR:-$repository_root/target}
receipts="$run_root/receipts"
mkdir -p "$receipts"
cargo build --locked --release --package rustmatch-bench "${build_target_args[@]}"
benchmark_binary="$target_directory/$binary_subdirectory/rustmatch-bench"
file "$benchmark_binary"

scenarios=(
  "literal-sparse 1000 8388608"
  "literal-sparse 5000 8388608"
  "literal-dense 1000 8388608"
  "mixed-sparse 1000 8388608"
  "assertion-bypass 1000 8388608"
  "mixed-unfilterable 1000 8388608"
  "short-literal 5000 16384"
)

measure() {
  local mode=$1
  local output=$2
  local scenario=$3
  local pattern_count=$4
  local corpus_bytes=$5
  RUSTMATCH_BENCH_PREFILTER="$mode" \
  RUSTMATCH_BENCH_REVISION="$candidate_revision" \
  RUSTMATCH_BENCH_RUNNER="$runner_label" \
    "$benchmark_binary" i7-scan "$scenario" "$pattern_count" "$corpus_bytes" \
      "$cache_scrub_bytes" > "$output"
}

for scenario_specification in "${scenarios[@]}"; do
  read -r scenario pattern_count corpus_bytes <<< "$scenario_specification"
  stem="${scenario}-${pattern_count}-${corpus_bytes}"
  disabled_receipt="$receipts/${stem}-disabled.json"
  enabled_receipt="$receipts/${stem}-enabled.json"
  comparison_receipt="$receipts/${stem}-comparison.json"

  measure off "$disabled_receipt" "$scenario" "$pattern_count" "$corpus_bytes"
  measure on "$enabled_receipt" "$scenario" "$pattern_count" "$corpus_bytes"
  if ! "$benchmark_binary" compare-i7 "$disabled_receipt" "$enabled_receipt" \
    > "$comparison_receipt"; then
    echo "I7 threshold crossed for $stem; repeating in reverse order" >&2
    measure on "$enabled_receipt" "$scenario" "$pattern_count" "$corpus_bytes"
    measure off "$disabled_receipt" "$scenario" "$pattern_count" "$corpus_bytes"
    "$benchmark_binary" compare-i7 "$disabled_receipt" "$enabled_receipt" \
      > "$comparison_receipt"
  fi
  cat "$comparison_receipt"
done

printf 'I7 focused receipts: %s\n' "$receipts"
