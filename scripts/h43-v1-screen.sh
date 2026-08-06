#!/usr/bin/env bash

set -Eeuo pipefail

repository_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
plan=${H43_V1_PLAN:-$repository_root/docs/experiments/h43-v1-screen-plan.json}
candidate_root=${H43_V1_CANDIDATE_ROOT:?set H43_V1_CANDIDATE_ROOT to the frozen candidate worktree}
run_root=${H43_V1_RUN_ROOT:?set H43_V1_RUN_ROOT to a fresh evidence directory}

for command in cargo cc docker jq nvidia-smi python3 sha256sum tar taskset; do
  command -v "$command" >/dev/null 2>&1 || {
    echo "required H43-V1 command is unavailable: $command" >&2
    exit 2
  }
done

candidate_revision=$(jq -r .candidate_revision "$plan")
candidate_tree=$(jq -r .candidate_tree "$plan")
evidence_id=$(jq -r .evidence_id "$plan")
cpu_set=$(jq -r .cpu_set "$plan")
timing_cycles=$(jq -r .timing_cycles "$plan")
warmups=$(jq -r .warmup_scans "$plan")
repeats=$(jq -r .retained_scans "$plan")
runner_label="$(hostname) | $(uname -m) | cpu-set $cpu_set"

if git -C "$candidate_root" rev-parse --git-dir >/dev/null 2>&1; then
  if [[ $(git -C "$candidate_root" rev-parse HEAD) != "$candidate_revision" ]]; then
    echo "$evidence_id candidate revision mismatch" >&2
    exit 2
  fi
  if [[ $(git -C "$candidate_root" rev-parse HEAD^{tree}) != "$candidate_tree" ]]; then
    echo "$evidence_id candidate tree mismatch" >&2
    exit 2
  fi
  if [[ -n $(git -C "$candidate_root" status --porcelain) ]]; then
    echo "$evidence_id candidate worktree is dirty" >&2
    exit 2
  fi
else
  candidate_archive=${H43_V1_CANDIDATE_ARCHIVE:?set H43_V1_CANDIDATE_ARCHIVE for an archive-backed candidate}
  archive_revision=$(git get-tar-commit-id < "$candidate_archive")
  if [[ $archive_revision != "$candidate_revision" ]]; then
    echo "$evidence_id candidate archive revision mismatch" >&2
    exit 2
  fi
  if ! tar --compare --file "$candidate_archive" --directory "$candidate_root" >/dev/null; then
    echo "$evidence_id extracted candidate differs from its retained Git archive" >&2
    exit 2
  fi
fi
if [[ -e $run_root ]]; then
  echo "$evidence_id run root already exists: $run_root" >&2
  exit 2
fi
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
for container in llms5090-ollama llms5090-openwebui hive-build-telemetryd-agogo; do
  state=$(docker inspect -f '{{.State.Status}} {{.HostConfig.RestartPolicy.Name}}' "$container")
  if [[ $state != "exited no" ]]; then
    echo "$evidence_id managed service $container is not safely disabled: $state" >&2
    exit 3
  fi
done

mkdir -p "$run_root"/{artifacts,build,fixtures,host,receipts/{resources,timing}}
started_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)
plan_sha256=$(sha256sum "$plan" | awk '{print $1}')
generator_sha256=$(sha256sum "$repository_root/tools/h43_d1_fixtures.py" | awk '{print $1}')
shim_sha256=$(sha256sum "$repository_root/tools/h43_d1_alloc_shim.c" | awk '{print $1}')
window_error=""
guard_pid=""
events_pid=""

write_state() {
  local status=$1
  jq -n \
    --arg status "$status" \
    --arg started_utc "$started_utc" \
    --arg ended_utc "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    --arg candidate_revision "$candidate_revision" \
    --arg plan_sha256 "$plan_sha256" \
    --arg window_error "$window_error" \
    '{schema_version:1,status:$status,started_utc:$started_utc,ended_utc:$ended_utc,candidate_revision:$candidate_revision,plan_sha256:$plan_sha256,window_error:$window_error}' \
    > "$run_root/window-state.json"
}

cleanup() {
  local exit_code=$?
  trap - EXIT
  if [[ -n $guard_pid ]]; then
    kill "$guard_pid" 2>/dev/null || true
    wait "$guard_pid" 2>/dev/null || true
  fi
  if [[ -n $events_pid ]]; then
    kill "$events_pid" 2>/dev/null || true
    wait "$events_pid" 2>/dev/null || true
  fi
  docker ps -a --no-trunc --format '{{json .}}' | jq -s . > "$run_root/host/docker-after.json"
  nvidia-smi -q 2>/dev/null > "$run_root/host/gpu-after.txt" || true
  if (( exit_code == 0 )); then
    write_state closed-cleanly
  else
    [[ -n $window_error ]] || window_error="runner exited with status $exit_code"
    write_state rejected
  fi
  exit "$exit_code"
}
record_error() {
  local exit_code=$?
  window_error="command failed at line ${BASH_LINENO[0]} with status $exit_code"
  return "$exit_code"
}
trap record_error ERR
trap cleanup EXIT

jq -n \
  --arg started_utc "$started_utc" \
  --arg hostname "$(hostname)" \
  --arg kernel "$(uname -a)" \
  --arg plan_sha256 "$plan_sha256" \
  --arg fixture_generator_sha256 "$generator_sha256" \
  --arg allocation_shim_sha256 "$shim_sha256" \
  --arg candidate_revision "$candidate_revision" \
  --arg rustc "$(rustc -Vv)" \
  --arg cargo "$(cargo -V)" \
  --argjson docker "$(docker ps -a --no-trunc --format '{{json .}}' | jq -s .)" \
  '{schema_version:1,started_utc:$started_utc,hostname:$hostname,kernel:$kernel,plan_sha256:$plan_sha256,fixture_generator_sha256:$fixture_generator_sha256,allocation_shim_sha256:$allocation_shim_sha256,candidate_revision:$candidate_revision,rustc:$rustc,cargo:$cargo,docker:$docker}' \
  > "$run_root/host/service-snapshot.json"
lscpu > "$run_root/host/lscpu.txt"
nvidia-smi -q 2>/dev/null > "$run_root/host/gpu-before.txt" || true
docker events --since "$(date -u +%Y-%m-%dT%H:%M:%SZ)" --format '{{json .}}' > "$run_root/host/docker-events.jsonl" &
events_pid=$!

guard_loop() {
  while :; do
    observed=$(date -u +%Y-%m-%dT%H:%M:%SZ)
    docker ps --no-trunc --format '{{json .}}' | \
      jq -c --arg observed "$observed" 'select(. != null) | {observed_utc:$observed,kind:"docker",detail:.}' \
      >> "$run_root/host/contamination.jsonl"
    nvidia-smi --query-compute-apps=pid,process_name,used_gpu_memory --format=csv,noheader 2>/dev/null | \
      jq -Rcs --arg observed "$observed" 'select(length > 0) | {observed_utc:$observed,kind:"gpu",detail:.}' \
      >> "$run_root/host/contamination.jsonl"
    sleep 0.5
  done
}
guard_loop &
guard_pid=$!

checkpoint() {
  local label=$1
  if [[ -s $run_root/host/contamination.jsonl ]]; then
    window_error="environmental contamination observed before $label"
    return 3
  fi
  jq -n \
    --arg observed_utc "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    --arg label "$label" \
    --arg load "$(cat /proc/loadavg)" \
    --arg top "$(ps -eo pid,comm,pcpu,pmem --sort=-pcpu | head -8)" \
    '{observed_utc:$observed_utc,label:$label,load:$load,top:$top}' \
    >> "$run_root/host/checkpoints.jsonl"
}

python3 "$repository_root/tools/h43_d1_fixtures.py" --plan "$plan" --output "$run_root/fixtures" \
  > "$run_root/fixtures/generator-console.json"
cc -shared -fPIC -O2 -std=c11 -pthread "$repository_root/tools/h43_d1_alloc_shim.c" \
  -o "$run_root/artifacts/h43-v1-alloc-shim.so"

checkpoint build-start
candidate_target="$run_root/build/candidate-target"
CARGO_TARGET_DIR="$candidate_target" cargo build --locked --release \
  --manifest-path "$candidate_root/Cargo.toml" --package rustmatch-bench \
  > "$run_root/build/cargo-build.log" 2>&1
candidate_binary="$candidate_target/release/rustmatch-bench"
sha256sum "$candidate_binary" > "$run_root/artifacts/benchmark-binary-sha256.txt"
file "$candidate_binary" > "$run_root/artifacts/benchmark-binary-file.txt"

run_timing_cell() {
  local fixture=$1 cycle=$2 variant=$3 mode=$4
  local stem="$fixture-c$(printf '%02d' "$cycle")-$variant"
  checkpoint "$stem"
  RUSTMATCH_BENCH_REVISION="$candidate_revision" RUSTMATCH_BENCH_RUNNER="$runner_label" \
    /usr/bin/time -f '{"max_rss_kb":%M,"elapsed_seconds":%e,"user_seconds":%U,"system_seconds":%S,"major_faults":%F,"minor_faults":%R,"voluntary_context_switches":%w,"involuntary_context_switches":%c}' \
      -o "$run_root/receipts/timing/$stem.time.json" \
      taskset -c "$cpu_set" "$candidate_binary" harness-run \
        "$run_root/fixtures/$fixture.tsv" "$run_root/fixtures/$fixture.txt" \
        "$repeats" "$warmups" "$mode" \
        > "$run_root/receipts/timing/$stem.json"
}

mapfile -t fixture_ids < <(jq -r '.fixtures[].id' "$plan")
for fixture in "${fixture_ids[@]}"; do
  for cycle in $(seq 1 "$timing_cycles"); do
    if (( cycle % 2 == 1 )); then
      run_timing_cell "$fixture" "$cycle" ordinary 1
      run_timing_cell "$fixture" "$cycle" shared cohort-view-1
    else
      run_timing_cell "$fixture" "$cycle" shared cohort-view-1
      run_timing_cell "$fixture" "$cycle" ordinary 1
    fi
    jq -e --slurpfile shared "$run_root/receipts/timing/$fixture-c$(printf '%02d' "$cycle")-shared.json" \
      '.event_digest == $shared[0].event_digest and .matches_per_iteration == $shared[0].matches_per_iteration' \
      "$run_root/receipts/timing/$fixture-c$(printf '%02d' "$cycle")-ordinary.json" >/dev/null
  done
done

run_resource_cell() {
  local fixture=$1 variant=$2 mode=$3
  local stem="$fixture-$variant"
  checkpoint "resource-$stem"
  RUSTMATCH_BENCH_REVISION="$candidate_revision" RUSTMATCH_BENCH_RUNNER="$runner_label" \
    env LD_PRELOAD="$run_root/artifacts/h43-v1-alloc-shim.so" \
      RMATCH_ALLOC_RECEIPT="$run_root/receipts/resources/$stem.alloc.json" \
      taskset -c "$cpu_set" "$candidate_binary" harness-run \
        "$run_root/fixtures/$fixture.tsv" "$run_root/fixtures/$fixture.txt" \
        "$repeats" "$warmups" "$mode" \
        > "$run_root/receipts/resources/$stem.json"
  jq -e '.allocator == "glibc-interposition-v1"' "$run_root/receipts/resources/$stem.alloc.json" >/dev/null
}

for fixture in "${fixture_ids[@]}"; do
  run_resource_cell "$fixture" ordinary 1
  run_resource_cell "$fixture" shared cohort-view-1
done

checkpoint analyze
if [[ -s $run_root/host/docker-events.jsonl || -s $run_root/host/contamination.jsonl ]]; then
  window_error="Docker, GPU, or service contamination occurred during the window"
  exit 3
fi
python3 "$repository_root/tools/h43_v1_analyze.py" \
  --plan "$plan" --run-root "$run_root" --output "$run_root/summary.json" \
  > "$run_root/analyzer-console.json"
find "$run_root" -type f ! -name sha256.txt -print0 | sort -z | xargs -0 sha256sum \
  > "$run_root/sha256.txt"
