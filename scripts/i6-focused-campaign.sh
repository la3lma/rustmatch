#!/usr/bin/env bash

set -euo pipefail

repository_root=$(git rev-parse --show-toplevel)
base_ref=${BASE_SHA:?BASE_SHA must name the frozen I6 baseline revision}
candidate_ref=${CANDIDATE_SHA:-HEAD}
base_revision=$(git -C "$repository_root" rev-parse "${base_ref}^{commit}")
candidate_revision=$(git -C "$repository_root" rev-parse "${candidate_ref}^{commit}")
source_revision=$(git -C "$repository_root" rev-parse HEAD)
runner_label=${RUSTMATCH_BENCH_RUNNER:-$(hostname) | $(uname -m)}
run_root=${I6_RUN_ROOT:-$repository_root/target/i6-focused-campaign}
rust_target=${I6_CARGO_TARGET:-}
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
if [[ ${I6_ALLOW_DIRTY:-0} != 1 ]] && [[ -n $(git -C "$repository_root" status --porcelain) ]]; then
  echo "I6 refuses to mislabel a dirty working tree; commit it or set I6_ALLOW_DIRTY=1" >&2
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

scenarios=(
  "literal-sparse 64 1048576"
  "literal-sparse 256 2097152"
  "mixed-sparse 64 1048576"
  "literal-dense 64 1048576"
  "assertion-bypass 64 1048576"
)

measure() {
  local binary=$1
  local revision=$2
  local output=$3
  local scenario=$4
  local pattern_count=$5
  local corpus_bytes=$6
  RUSTMATCH_BENCH_REVISION="$revision" RUSTMATCH_BENCH_RUNNER="$runner_label" \
    "$binary" i6-scan "$scenario" "$pattern_count" "$corpus_bytes" > "$output"
}

for scenario_specification in "${scenarios[@]}"; do
  read -r scenario pattern_count corpus_bytes <<< "$scenario_specification"
  stem="${scenario}-${pattern_count}-${corpus_bytes}"
  base_receipt="$receipts/${stem}-base.json"
  candidate_receipt="$receipts/${stem}-candidate.json"
  comparison_receipt="$receipts/${stem}-comparison.json"

  measure "$base_target/$binary_subdirectory/rustmatch-bench" "$base_revision" "$base_receipt" \
    "$scenario" "$pattern_count" "$corpus_bytes"
  measure "$candidate_target/$binary_subdirectory/rustmatch-bench" "$candidate_revision" \
    "$candidate_receipt" "$scenario" "$pattern_count" "$corpus_bytes"
  if ! "$candidate_target/$binary_subdirectory/rustmatch-bench" compare-i6 \
    "$base_receipt" "$candidate_receipt" > "$comparison_receipt"; then
    echo "I6 threshold crossed for $stem; repeating in reverse order" >&2
    measure "$candidate_target/$binary_subdirectory/rustmatch-bench" "$candidate_revision" \
      "$candidate_receipt" "$scenario" "$pattern_count" "$corpus_bytes"
    measure "$base_target/$binary_subdirectory/rustmatch-bench" "$base_revision" "$base_receipt" \
      "$scenario" "$pattern_count" "$corpus_bytes"
    "$candidate_target/$binary_subdirectory/rustmatch-bench" compare-i6 \
      "$base_receipt" "$candidate_receipt" > "$comparison_receipt"
  fi
  cat "$comparison_receipt"
done

printf 'I6 focused receipts: %s\n' "$receipts"
