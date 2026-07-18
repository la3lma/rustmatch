#!/usr/bin/env bash

set -euo pipefail

repository_root=$(git rev-parse --show-toplevel)
base_ref=${BASE_SHA:?BASE_SHA must name the baseline revision}
candidate_ref=${CANDIDATE_SHA:-HEAD}
base_revision=$(git -C "$repository_root" rev-parse "${base_ref}^{commit}")
candidate_revision=$(git -C "$repository_root" rev-parse "${candidate_ref}^{commit}")
source_revision=$(git -C "$repository_root" rev-parse HEAD)
runner_label=${RUSTMATCH_BENCH_RUNNER:-local-unidentified}

if [[ $candidate_revision != "$source_revision" ]]; then
  echo "CANDIDATE_SHA must identify the checked-out source revision" >&2
  exit 2
fi
if [[ ${C2_ALLOW_DIRTY:-0} != 1 ]] && [[ -n $(git -C "$repository_root" status --porcelain) ]]; then
  echo "C2 refuses to mislabel a dirty working tree; commit it or set C2_ALLOW_DIRTY=1" >&2
  exit 2
fi

run_root=${C2_RUN_ROOT:-$repository_root/target/c2-wuthering-tripwire}
base_source="$run_root/base-source"
base_target="$run_root/base-target"
candidate_target="$run_root/candidate-target"
receipts="$run_root/receipts"
pattern_source="$repository_root/benchmarks/fixtures/wuthering-heights/real-words-in-wuthering-heights.txt"
corpus_source="$repository_root/benchmarks/fixtures/wuthering-heights/wuthr10.txt"
pattern_count=${C2_PATTERN_COUNT:-5000}
corpus_bytes=${C2_CORPUS_BYTES:-8388608}
cache_scrub_bytes=${C2_CACHE_SCRUB_BYTES:-67108864}
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
  CARGO_TARGET_DIR="$base_target" cargo build --locked --release --package rustmatch-bench
)
(
  cd "$repository_root"
  CARGO_TARGET_DIR="$candidate_target" cargo build --locked --release --package rustmatch-bench
)

measure() {
  local binary=$1
  local revision=$2
  local output=$3
  RUSTMATCH_BENCH_REVISION="$revision" \
  RUSTMATCH_BENCH_RUNNER="$runner_label" \
    "$binary" wuthering-scan "$pattern_source" "$corpus_source" \
      "$pattern_count" "$corpus_bytes" "$cache_scrub_bytes" > "$output"
}

measure_base() {
  measure "$base_target/release/rustmatch-bench" "$base_revision" "$receipts/base.json"
}

measure_candidate() {
  measure "$candidate_target/release/rustmatch-bench" "$candidate_revision" \
    "$receipts/candidate.json"
}

compare() {
  "$candidate_target/release/rustmatch-bench" compare-wuthering-tripwire \
    "$receipts/base.json" "$receipts/candidate.json" | tee "$receipts/comparison.json"
}

measure_base
measure_candidate
if ! compare; then
  echo "C2 timing threshold crossed; rerunning in reverse order" >&2
  measure_candidate
  measure_base
  compare
fi

cat "$receipts/base.json"
cat "$receipts/candidate.json"
printf 'C2 receipts: %s\n' "$receipts"
