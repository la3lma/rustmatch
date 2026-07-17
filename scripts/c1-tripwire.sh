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
if [[ ${C1_ALLOW_DIRTY:-0} != 1 ]] && [[ -n $(git -C "$repository_root" status --porcelain) ]]; then
  echo "C1 refuses to mislabel a dirty working tree; commit it or set C1_ALLOW_DIRTY=1" >&2
  exit 2
fi

run_root=${C1_RUN_ROOT:-$repository_root/target/c1-tripwire}
mkdir -p "$run_root"

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
  CARGO_TARGET_DIR="$base_target" \
    cargo build --locked --release --package rustmatch-bench
)
(
  cd "$repository_root"
  CARGO_TARGET_DIR="$candidate_target" \
    cargo build --locked --release --package rustmatch-bench
)

measure_base() {
  RUSTMATCH_BENCH_REVISION="$base_revision" \
  RUSTMATCH_BENCH_RUNNER="$runner_label" \
    "$base_target/release/rustmatch-bench" literal-tripwire \
    > "$receipts/base.json"
}

measure_candidate() {
  RUSTMATCH_BENCH_REVISION="$candidate_revision" \
  RUSTMATCH_BENCH_RUNNER="$runner_label" \
    "$candidate_target/release/rustmatch-bench" literal-tripwire \
    > "$receipts/candidate.json"
}

compare() {
  "$candidate_target/release/rustmatch-bench" compare-tripwire \
    "$receipts/base.json" \
    "$receipts/candidate.json" \
    | tee "$receipts/comparison.json"
}

measure_base
measure_candidate
if ! compare; then
  echo "C1 timing threshold crossed; rerunning in reverse order" >&2
  measure_candidate
  measure_base
  compare
fi

cat "$receipts/base.json"
cat "$receipts/candidate.json"
printf 'C1 receipts: %s\n' "$receipts"
