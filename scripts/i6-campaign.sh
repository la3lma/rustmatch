#!/usr/bin/env bash

set -euo pipefail

repository_root=$(git rev-parse --show-toplevel)
rmatch_repository=${RMATCH_REPOSITORY:-$(dirname "$repository_root")/rmatch}
pattern_source=${PATTERN_SOURCE:-$rmatch_repository/rmatch-tester/corpus/real-words-in-wuthering-heights.txt}
corpus_source=${CORPUS_SOURCE:-$rmatch_repository/rmatch-tester/corpus/wuthr10.txt}
cache_scrub_bytes=${CACHE_SCRUB_BYTES:-268435456}
run_root=${I6_RUN_ROOT:-$repository_root/target/benchmark-results}
revision=$(git -C "$repository_root" rev-parse HEAD)
target_directory=${CARGO_TARGET_DIR:-$repository_root/target}
rust_target=${I6_CARGO_TARGET:-}
build_target_args=()
binary_subdirectory=release
if [[ -n $rust_target ]]; then
  build_target_args=(--target "$rust_target")
  binary_subdirectory="$rust_target/release"
fi

if [[ ${I6_ALLOW_DIRTY:-0} != 1 ]] && [[ -n $(git -C "$repository_root" status --porcelain) ]]; then
  echo "I6 campaign refuses to mislabel a dirty working tree; commit it or set I6_ALLOW_DIRTY=1" >&2
  exit 2
fi
if [[ ! -f $pattern_source ]]; then
  echo "Pattern source does not exist: $pattern_source" >&2
  exit 2
fi
if [[ ! -f $corpus_source ]]; then
  echo "Corpus source does not exist: $corpus_source" >&2
  exit 2
fi
corpus_bytes=${CORPUS_BYTES:-$(wc -c < "$corpus_source" | tr -d ' ')}
if [[ -n ${RUSTMATCH_BENCH_RUNNER:-} ]]; then
  runner_label=$RUSTMATCH_BENCH_RUNNER
else
  logical_cpus=$(getconf _NPROCESSORS_ONLN 2>/dev/null || sysctl -n hw.logicalcpu)
  runner_label="$(hostname) | $(uname -m) | $logical_cpus logical CPUs"
fi

mkdir -p "$run_root"
cargo build --locked --release --package rustmatch-bench "${build_target_args[@]}"
benchmark_binary="$target_directory/$binary_subdirectory/rustmatch-bench"

read -r -a pattern_counts <<< "${PATTERN_COUNTS:-1000 5000 10000}"
receipts=()
for pattern_count in "${pattern_counts[@]}"; do
  receipt="$run_root/wuthering-${pattern_count}-${corpus_bytes}.json"
  RUSTMATCH_BENCH_REVISION="$revision" \
  RUSTMATCH_BENCH_RUNNER="$runner_label" \
    "$benchmark_binary" wuthering-scan \
      "$pattern_source" "$corpus_source" "$pattern_count" "$corpus_bytes" \
      "$cache_scrub_bytes" > "$receipt"
  receipts+=("$receipt")
  "$benchmark_binary" render-table \
    "$run_root/index.html" "${receipts[@]}" >/dev/null
  printf 'I6 scale receipt complete: %s patterns\n' "$pattern_count"
done

printf 'I6 scale report: %s\n' "$run_root/index.html"
