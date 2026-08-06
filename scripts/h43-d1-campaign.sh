#!/usr/bin/env bash

set -euo pipefail

repository_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
plan=${H43_D1_PLAN:-$repository_root/docs/experiments/h43-d1-plan.json}
baseline_root=${H43_D1_BASELINE_ROOT:?set H43_D1_BASELINE_ROOT to the isolated pre-K1 worktree}
candidate_root=${H43_D1_CANDIDATE_ROOT:?set H43_D1_CANDIDATE_ROOT to the isolated E1 worktree}
run_root=${H43_D1_RUN_ROOT:?set H43_D1_RUN_ROOT to a fresh evidence directory}

for command in cargo cc docker jq nvidia-smi python3 sha256sum taskset; do
  command -v "$command" >/dev/null 2>&1 || {
    echo "required H43-D1 command is unavailable: $command" >&2
    exit 2
  }
done

baseline_revision=$(jq -r .baseline_revision "$plan")
candidate_revision=$(jq -r .candidate_revision "$plan")
evidence_id=$(jq -r .evidence_id "$plan")
cpu_set=$(jq -r .cpu_set "$plan")
timing_cycles=$(jq -r .timing_cycles "$plan")
warmups=$(jq -r .warmup_scans "$plan")
repeats=$(jq -r .retained_scans "$plan")
resource_cycles=$(jq -r .resource_cycles "$plan")
compile_cycles=$(jq -r .compile_cycles "$plan")
runner_label="$(hostname) | $(uname -m) | cpu-set $cpu_set"

if [[ -e $run_root ]]; then
  echo "$evidence_id run root already exists: $run_root" >&2
  exit 2
fi
mkdir -p "$run_root"/{activation,artifacts,build,fixtures,host,receipts/{timing,resources}}

verify_source() {
  local label=$1
  local root=$2
  local expected=$3
  local actual
  actual=$(git -C "$root" rev-parse HEAD)
  if [[ $actual != "$expected" ]]; then
    echo "$label revision mismatch: expected $expected, found $actual" >&2
    exit 2
  fi
  if [[ -n $(git -C "$root" status --porcelain) ]]; then
    echo "$label worktree is dirty: $root" >&2
    exit 2
  fi
}

verify_source baseline "$baseline_root" "$baseline_revision"
verify_source candidate "$candidate_root" "$candidate_revision"

if [[ -n $(docker ps -q) ]]; then
  echo "$evidence_id requires an empty Docker runtime" >&2
  docker ps --no-trunc >&2
  exit 3
fi
if [[ -n $(nvidia-smi --query-compute-apps=pid --format=csv,noheader 2>/dev/null) ]]; then
  echo "$evidence_id requires an idle GPU" >&2
  nvidia-smi >&2
  exit 3
fi

started_epoch=$(date +%s)
started_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)
plan_sha256=$(sha256sum "$plan" | awk '{print $1}')
generator_sha256=$(sha256sum "$repository_root/tools/h43_d1_fixtures.py" | awk '{print $1}')
shim_sha256=$(sha256sum "$repository_root/tools/h43_d1_alloc_shim.c" | awk '{print $1}')

{
  printf '{\n'
  printf '  "schema_version": 1,\n'
  printf '  "started_utc": "%s",\n' "$started_utc"
  printf '  "hostname": "%s",\n' "$(hostname)"
  printf '  "kernel": %s,\n' "$(uname -a | jq -R .)"
  printf '  "plan_sha256": "%s",\n' "$plan_sha256"
  printf '  "fixture_generator_sha256": "%s",\n' "$generator_sha256"
  printf '  "allocation_shim_sha256": "%s",\n' "$shim_sha256"
  printf '  "baseline_revision": "%s",\n' "$baseline_revision"
  printf '  "candidate_revision": "%s",\n' "$candidate_revision"
  printf '  "rustc": %s,\n' "$(rustc -Vv | jq -Rs .)"
  printf '  "cargo": %s,\n' "$(cargo -V | jq -R .)"
  printf '  "cpu": %s,\n' "$(lscpu | jq -Rs .)"
  printf '  "docker": %s,\n' "$(docker ps -a --no-trunc --format '{{json .}}' | jq -s .)"
  printf '  "gpu": %s\n' "$(nvidia-smi -q 2>/dev/null | jq -Rs .)"
  printf '}\n'
} > "$run_root/host/service-snapshot.json"

python3 "$repository_root/tools/h43_d1_fixtures.py" \
  --plan "$plan" --output "$run_root/fixtures" \
  > "$run_root/fixtures/generator-console.json"
cc -shared -fPIC -O2 -std=c11 -pthread \
  "$repository_root/tools/h43_d1_alloc_shim.c" \
  -o "$run_root/artifacts/h43-d1-alloc-shim.so"

guard_failure="$run_root/host/contamination.jsonl"
guard_loop() {
  while :; do
    local observed
    observed=$(date -u +%Y-%m-%dT%H:%M:%SZ)
    docker ps --no-trunc --format '{{json .}}' | \
      jq -c --arg observed "$observed" 'select(. != null) | {observed_utc:$observed,kind:"docker",detail:.}' \
      >> "$guard_failure"
    nvidia-smi --query-compute-apps=pid,process_name,used_gpu_memory \
      --format=csv,noheader 2>/dev/null | \
      jq -Rcs --arg observed "$observed" 'select(length > 0) | {observed_utc:$observed,kind:"gpu",detail:.}' \
      >> "$guard_failure"
    sleep 1
  done
}
guard_loop &
guard_pid=$!
cleanup_guard() {
  kill "$guard_pid" 2>/dev/null || true
  wait "$guard_pid" 2>/dev/null || true
}
trap cleanup_guard EXIT INT TERM

host_checkpoint() {
  local label=$1
  if [[ -s $guard_failure ]]; then
    echo "$evidence_id contamination guard fired before $label" >&2
    exit 3
  fi
  {
    printf '{"observed_utc":"%s","label":%s,"load":%s,"top":%s}\n' \
      "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
      "$(jq -Rn --arg value "$label" '$value')" \
      "$(cat /proc/loadavg | jq -R .)" \
      "$(ps -eo pid,comm,pcpu,pmem --sort=-pcpu | head -8 | jq -Rs .)"
  } >> "$run_root/host/checkpoints.jsonl"
}

baseline_target="$run_root/build/baseline-target"
candidate_target="$run_root/build/candidate-target"
host_checkpoint build-start
CARGO_TARGET_DIR="$baseline_target" cargo build --locked --release \
  --manifest-path "$baseline_root/Cargo.toml" --package rustmatch-bench
CARGO_TARGET_DIR="$candidate_target" cargo build --locked --release \
  --manifest-path "$candidate_root/Cargo.toml" --package rustmatch-bench
baseline_binary="$baseline_target/release/rustmatch-bench"
candidate_binary="$candidate_target/release/rustmatch-bench"

sha256sum "$baseline_binary" "$candidate_binary" \
  > "$run_root/artifacts/benchmark-binary-sha256.txt"
file "$baseline_binary" "$candidate_binary" > "$run_root/artifacts/benchmark-binary-file.txt"
stat --printf='%n %s\n' "$baseline_binary" "$candidate_binary" \
  > "$run_root/artifacts/benchmark-binary-bytes.txt"
size "$baseline_binary" "$candidate_binary" > "$run_root/artifacts/benchmark-binary-size.txt"

mapfile -t fixture_ids < <(jq -r '.fixtures[].id' "$plan")
mapfile -t workers < <(jq -r '.worker_counts[]' "$plan")

for fixture in "${fixture_ids[@]}"; do
  RUSTMATCH_BENCH_REVISION="$candidate_revision" \
  RUSTMATCH_BENCH_RUNNER="$runner_label" \
    "$candidate_binary" cohort-report "$run_root/fixtures/$fixture.tsv" \
    > "$run_root/activation/$fixture.json"
  expected=$(jq -r --arg fixture "$fixture" '.fixtures[] | select(.id == $fixture) | .expected_cohorts' "$plan")
  actual=$(jq -r .cohort_count "$run_root/activation/$fixture.json")
  if [[ $actual != "$expected" ]]; then
    echo "$fixture activated $actual cohorts; expected $expected" >&2
    exit 4
  fi
done

run_timing_cell() {
  local lane=$1 fixture=$2 worker=$3 cycle=$4 variant=$5 binary=$6 revision=$7 mode=$8
  local stem="$lane-$fixture-w$worker-c$(printf '%02d' "$cycle")-$variant"
  host_checkpoint "$stem"
  RUSTMATCH_BENCH_REVISION="$revision" \
  RUSTMATCH_BENCH_RUNNER="$runner_label" \
    /usr/bin/time -f '{"max_rss_kb":%M,"elapsed_seconds":%e,"user_seconds":%U,"system_seconds":%S,"major_faults":%F,"minor_faults":%R,"voluntary_context_switches":%w,"involuntary_context_switches":%c}' \
      -o "$run_root/receipts/timing/$stem.time.json" \
      taskset -c "$cpu_set" "$binary" harness-run \
        "$run_root/fixtures/$fixture.tsv" "$run_root/fixtures/$fixture.txt" \
        "$repeats" "$warmups" "$mode" \
        > "$run_root/receipts/timing/$stem.json"
}

for fixture in "${fixture_ids[@]}"; do
  for worker in "${workers[@]}"; do
    for cycle in $(seq 1 "$timing_cycles"); do
      if (( cycle % 2 == 1 )); then
        run_timing_cell inactive "$fixture" "$worker" "$cycle" baseline \
          "$baseline_binary" "$baseline_revision" "$worker"
        run_timing_cell inactive "$fixture" "$worker" "$cycle" candidate \
          "$candidate_binary" "$candidate_revision" "$worker"
        run_timing_cell cohort "$fixture" "$worker" "$cycle" ordinary \
          "$candidate_binary" "$candidate_revision" "$worker"
        run_timing_cell cohort "$fixture" "$worker" "$cycle" cohort \
          "$candidate_binary" "$candidate_revision" "cohort-$worker"
      else
        run_timing_cell inactive "$fixture" "$worker" "$cycle" candidate \
          "$candidate_binary" "$candidate_revision" "$worker"
        run_timing_cell inactive "$fixture" "$worker" "$cycle" baseline \
          "$baseline_binary" "$baseline_revision" "$worker"
        run_timing_cell cohort "$fixture" "$worker" "$cycle" cohort \
          "$candidate_binary" "$candidate_revision" "cohort-$worker"
        run_timing_cell cohort "$fixture" "$worker" "$cycle" ordinary \
          "$candidate_binary" "$candidate_revision" "$worker"
      fi
    done
  done
done

run_resource_cell() {
  local fixture=$1 worker=$2 cycle=$3 variant=$4 binary=$5 revision=$6 mode=$7
  local stem="$fixture-w$worker-c$(printf '%02d' "$cycle")-$variant"
  local allocation="$run_root/receipts/resources/$stem.alloc.json"
  host_checkpoint "resource-$stem"
  RUSTMATCH_BENCH_REVISION="$revision" \
  RUSTMATCH_BENCH_RUNNER="$runner_label" \
    /usr/bin/time -f '{"max_rss_kb":%M,"elapsed_seconds":%e,"user_seconds":%U,"system_seconds":%S,"major_faults":%F,"minor_faults":%R,"voluntary_context_switches":%w,"involuntary_context_switches":%c}' \
      -o "$run_root/receipts/resources/$stem.time.json" \
      env LD_PRELOAD="$run_root/artifacts/h43-d1-alloc-shim.so" \
        RMATCH_ALLOC_RECEIPT="$allocation" \
        taskset -c "$cpu_set" "$binary" harness-run \
          "$run_root/fixtures/$fixture.tsv" "$run_root/fixtures/$fixture.txt" \
          "$repeats" "$warmups" "$mode" \
          > "$run_root/receipts/resources/$stem.json"
  jq -e '.allocator == "glibc-interposition-v1"' "$allocation" >/dev/null
}

for fixture in "${fixture_ids[@]}"; do
  for worker in "${workers[@]}"; do
    for cycle in $(seq 1 "$resource_cycles"); do
      run_resource_cell "$fixture" "$worker" "$cycle" baseline \
        "$baseline_binary" "$baseline_revision" "$worker"
      run_resource_cell "$fixture" "$worker" "$cycle" ordinary \
        "$candidate_binary" "$candidate_revision" "$worker"
      run_resource_cell "$fixture" "$worker" "$cycle" cohort \
        "$candidate_binary" "$candidate_revision" "cohort-$worker"
    done
  done
done

measure_compile() {
  local variant=$1 root=$2 target=$3
  mkdir -p "$run_root/build/compile-$variant"
  CARGO_TARGET_DIR="$target" cargo build --locked --release \
    --manifest-path "$root/Cargo.toml" --package rustmatch \
    --features benchmark-internals >/dev/null
  for cycle in $(seq 1 "$compile_cycles"); do
    # A metadata-only touch forces the measured crate to rebuild while retaining
    # already-built dependencies and leaving the source identity unchanged.
    touch "$root/rustmatch/src/lib.rs"
    host_checkpoint "compile-$variant-$cycle"
    /usr/bin/time -f '{"max_rss_kb":%M,"elapsed_seconds":%e,"user_seconds":%U,"system_seconds":%S,"major_faults":%F,"minor_faults":%R,"voluntary_context_switches":%w,"involuntary_context_switches":%c}' \
      -o "$run_root/build/compile-$variant/cycle-$(printf '%02d' "$cycle").json" \
      env CARGO_TARGET_DIR="$target" cargo build --locked --release \
        --manifest-path "$root/Cargo.toml" --package rustmatch \
        --features benchmark-internals >/dev/null
  done
  find "$target/release/deps" -maxdepth 1 -name 'librustmatch-*.rlib' \
    -printf '%f %s\n' | sort > "$run_root/artifacts/$variant-feature-rlib-bytes.txt"
}

measure_compile baseline "$baseline_root" "$run_root/build/baseline-compile-target"
measure_compile candidate "$candidate_root" "$run_root/build/candidate-compile-target"

for variant in baseline candidate; do
  if [[ $variant == baseline ]]; then root=$baseline_root; else root=$candidate_root; fi
  target="$run_root/build/$variant-default-target"
  CARGO_TARGET_DIR="$target" cargo build --locked --release \
    --manifest-path "$root/Cargo.toml" --package rustmatch >/dev/null
  find "$target/release/deps" -maxdepth 1 -name 'librustmatch-*.rlib' \
    -printf '%f %s\n' | sort > "$run_root/artifacts/$variant-default-rlib-bytes.txt"
done

cleanup_guard
trap - EXIT INT TERM
ended_epoch=$(date +%s)
docker events --since "$started_epoch" --until "$ended_epoch" --format '{{json .}}' \
  > "$run_root/host/docker-events.jsonl"
if [[ -s $guard_failure || -s $run_root/host/docker-events.jsonl ]]; then
  echo "$evidence_id window rejected by contamination evidence" >&2
  exit 3
fi

{
  printf '{"schema_version":1,"evidence_id":"%s-window","status":"pass",' "$evidence_id"
  printf '"started_utc":"%s","ended_utc":"%s",' "$started_utc" "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
  printf '"plan_sha256":"%s","baseline_revision":"%s","candidate_revision":"%s",' \
    "$plan_sha256" "$baseline_revision" "$candidate_revision"
  printf '"timing_receipts":%s,"resource_receipts":%s}\n' \
    "$(find "$run_root/receipts/timing" -name '*.json' ! -name '*.time.json' | wc -l)" \
    "$(find "$run_root/receipts/resources" -name '*.json' ! -name '*.time.json' ! -name '*.alloc.json' | wc -l)"
} > "$run_root/window-state.json"

sha256sum "$run_root/window-state.json" "$run_root/fixtures/manifest.json" \
  > "$run_root/manifest-sha256.txt"
echo "$evidence_id guarded campaign complete: $run_root"
