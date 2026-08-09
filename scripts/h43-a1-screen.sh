#!/usr/bin/env bash

set -Eeuo pipefail

repository_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
plan=${H43_A1_PLAN:-$repository_root/docs/experiments/h43-a1-screen-plan.json}
candidate_root=${H43_A1_CANDIDATE_ROOT:?set H43_A1_CANDIDATE_ROOT to the frozen candidate worktree}
fixture_root=${H43_A1_FIXTURE_ROOT:?set H43_A1_FIXTURE_ROOT to the retained H13 fixture directory}
run_root=${H43_A1_RUN_ROOT:?set H43_A1_RUN_ROOT to a fresh evidence directory}

for command in cargo cc docker jq nvidia-smi python3 sha256sum taskset; do
  command -v "$command" >/dev/null 2>&1 || {
    echo "required H43-A1 command is unavailable: $command" >&2
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
candidate_archive=""
candidate_manifest=""

if git -C "$candidate_root" rev-parse --git-dir >/dev/null 2>&1; then
  [[ $(git -C "$candidate_root" rev-parse HEAD) == "$candidate_revision" ]] || {
    echo "$evidence_id candidate revision mismatch" >&2
    exit 2
  }
  [[ $(git -C "$candidate_root" rev-parse HEAD^{tree}) == "$candidate_tree" ]] || {
    echo "$evidence_id candidate tree mismatch" >&2
    exit 2
  }
  [[ -z $(git -C "$candidate_root" status --porcelain) ]] || {
    echo "$evidence_id candidate worktree is dirty" >&2
    exit 2
  }
else
  candidate_archive=${H43_A1_CANDIDATE_ARCHIVE:?set H43_A1_CANDIDATE_ARCHIVE for an archive-backed candidate}
  candidate_manifest=${H43_A1_CANDIDATE_MANIFEST:?set H43_A1_CANDIDATE_MANIFEST for an archive-backed candidate}
  [[ $(git get-tar-commit-id < "$candidate_archive") == "$candidate_revision" ]] || {
    echo "$evidence_id candidate archive revision mismatch" >&2
    exit 2
  }
  (cd "$candidate_root" && sha256sum --check --quiet "$candidate_manifest") || {
    echo "$evidence_id extracted candidate fails its content manifest" >&2
    exit 2
  }
  [[ $(wc -l < "$candidate_manifest") == $(find "$candidate_root" -type f | wc -l) ]] || {
    echo "$evidence_id extracted candidate file count differs from its manifest" >&2
    exit 2
  }
fi

[[ ! -e $run_root ]] || {
  echo "$evidence_id run root already exists: $run_root" >&2
  exit 2
}
[[ -z $(docker ps -q) ]] || {
  echo "$evidence_id requires an empty Docker runtime" >&2
  exit 3
}
gpu_processes=$(nvidia-smi --query-compute-apps=pid --format=csv,noheader 2>/dev/null) || {
  echo "$evidence_id cannot verify GPU state" >&2
  exit 3
}
[[ -z $gpu_processes ]] || {
  echo "$evidence_id requires an idle GPU" >&2
  exit 3
}
for container in llms5090-ollama llms5090-openwebui hive-build-telemetryd-agogo; do
  state=$(docker inspect -f '{{.State.Status}} {{.HostConfig.RestartPolicy.Name}}' "$container")
  [[ $state == "exited no" ]] || {
    echo "$evidence_id managed service $container is not safely disabled: $state" >&2
    exit 3
  }
done

mkdir -p "$run_root"/{artifacts,build,fixtures,host,receipts/{resources,timing}}
cp "$plan" "$run_root/frozen-plan.json"
while IFS=$'\t' read -r source_file expected_sha; do
  cp "$fixture_root/$source_file" "$run_root/fixtures/$source_file"
  [[ $(sha256sum "$run_root/fixtures/$source_file" | awk '{print $1}') == "$expected_sha" ]] || {
    echo "$evidence_id fixture hash mismatch: $source_file" >&2
    exit 2
  }
done < <(jq -r '.fixtures[] | [.pattern_file,.pattern_sha256],[.corpus_file,.corpus_sha256] | @tsv' "$plan" | sort -u)

started_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)
plan_sha256=$(sha256sum "$plan" | awk '{print $1}')
shim_sha256=$(sha256sum "$repository_root/tools/h43_d1_alloc_shim.c" | awk '{print $1}')
candidate_archive_sha256=${candidate_archive:+$(sha256sum "$candidate_archive" | awk '{print $1}')}
candidate_manifest_sha256=${candidate_manifest:+$(sha256sum "$candidate_manifest" | awk '{print $1}')}
window_error=""
guard_pid=""
events_pid=""

write_state() {
  local status=$1
  jq -n --arg status "$status" --arg started_utc "$started_utc" \
    --arg ended_utc "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    --arg candidate_revision "$candidate_revision" --arg plan_sha256 "$plan_sha256" \
    --arg window_error "$window_error" \
    '{schema_version:1,status:$status,started_utc:$started_utc,ended_utc:$ended_utc,candidate_revision:$candidate_revision,plan_sha256:$plan_sha256,window_error:$window_error}' \
    > "$run_root/window-state.json"
}

cleanup() {
  local exit_code=$?
  trap - EXIT
  if [[ -n $guard_pid ]]; then kill "$guard_pid" 2>/dev/null || true; wait "$guard_pid" 2>/dev/null || true; fi
  if [[ -n $events_pid ]]; then kill "$events_pid" 2>/dev/null || true; wait "$events_pid" 2>/dev/null || true; fi
  docker ps -a --no-trunc --format '{{json .}}' | jq -s . > "$run_root/host/docker-after.json"
  nvidia-smi -q > "$run_root/host/gpu-after.txt" 2>&1 || true
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

jq -n --arg started_utc "$started_utc" --arg hostname "$(hostname)" \
  --arg kernel "$(uname -a)" --arg plan_sha256 "$plan_sha256" \
  --arg allocation_shim_sha256 "$shim_sha256" --arg candidate_revision "$candidate_revision" \
  --arg candidate_tree "$candidate_tree" --arg candidate_archive_sha256 "$candidate_archive_sha256" \
  --arg candidate_manifest_sha256 "$candidate_manifest_sha256" --arg rustc "$(rustc -Vv)" \
  --arg cargo "$(cargo -V)" \
  --argjson docker "$(docker ps -a --no-trunc --format '{{json .}}' | jq -s .)" \
  '{schema_version:1,started_utc:$started_utc,hostname:$hostname,kernel:$kernel,plan_sha256:$plan_sha256,allocation_shim_sha256:$allocation_shim_sha256,candidate_revision:$candidate_revision,candidate_tree:$candidate_tree,candidate_archive_sha256:$candidate_archive_sha256,candidate_manifest_sha256:$candidate_manifest_sha256,rustc:$rustc,cargo:$cargo,docker:$docker}' \
  > "$run_root/host/service-snapshot.json"
lscpu > "$run_root/host/lscpu.txt"
nvidia-smi -q > "$run_root/host/gpu-before.txt" 2>&1
docker events --since "$started_utc" --format '{{json .}}' > "$run_root/host/docker-events.jsonl" &
events_pid=$!

guard_loop() {
  while :; do
    observed=$(date -u +%Y-%m-%dT%H:%M:%SZ)
    docker ps --no-trunc --format '{{json .}}' | jq -c --arg observed "$observed" \
      'select(. != null) | {observed_utc:$observed,kind:"docker",detail:.}' \
      >> "$run_root/host/contamination.jsonl"
    nvidia-smi --query-compute-apps=pid,process_name,used_gpu_memory --format=csv,noheader 2>/dev/null | \
      jq -Rcs --arg observed "$observed" \
      'select(length > 0) | {observed_utc:$observed,kind:"gpu",detail:.}' \
      >> "$run_root/host/contamination.jsonl"
    sleep 0.5
  done
}
guard_loop &
guard_pid=$!

checkpoint() {
  local label=$1
  if ! kill -0 "$guard_pid" 2>/dev/null; then
    window_error="environment guard stopped before $label"
    return 3
  fi
  if [[ -s $run_root/host/contamination.jsonl ]]; then
    window_error="environmental contamination observed before $label"
    return 3
  fi
  jq -n --arg observed_utc "$(date -u +%Y-%m-%dT%H:%M:%SZ)" --arg label "$label" \
    --arg load "$(cat /proc/loadavg)" --arg top "$(ps -eo pid,comm,pcpu,pmem --sort=-pcpu | head -8)" \
    '{observed_utc:$observed_utc,label:$label,load:$load,top:$top}' \
    >> "$run_root/host/checkpoints.jsonl"
}

cc -shared -fPIC -O2 -std=c11 -pthread "$repository_root/tools/h43_d1_alloc_shim.c" \
  -o "$run_root/artifacts/h43-a1-alloc-shim.so"
checkpoint build-start
candidate_target="$run_root/build/candidate-target"
CARGO_TARGET_DIR="$candidate_target" cargo build --locked --release \
  --manifest-path "$candidate_root/Cargo.toml" --package rustmatch-bench \
  > "$run_root/build/cargo-build.log" 2>&1
candidate_binary="$candidate_target/release/rustmatch-bench"
sha256sum "$candidate_binary" > "$run_root/artifacts/benchmark-binary-sha256.txt"

probe_root="$run_root/build/prepare-probe"
mkdir -p "$probe_root/src"
cp "$repository_root/tools/h43_a1_prepare_probe.rs" "$probe_root/src/main.rs"
printf '[package]\nname = "h43-a1-prepare-probe"\nversion = "0.0.0"\nedition = "2024"\n\n[workspace]\n\n[dependencies]\nrustmatch = { path = "%s/rustmatch", features = ["benchmark-internals"] }\n' \
  "$candidate_root" > "$probe_root/Cargo.toml"
CARGO_TARGET_DIR="$candidate_target" cargo build --offline --release --manifest-path "$probe_root/Cargo.toml" \
  > "$run_root/build/prepare-probe-build.log" 2>&1
probe_binary="$candidate_target/release/h43-a1-prepare-probe"
sha256sum "$probe_binary" > "$run_root/artifacts/prepare-probe-binary-sha256.txt"

checkpoint tests
CARGO_TARGET_DIR="$candidate_target" cargo test --locked --workspace --all-features \
  --manifest-path "$candidate_root/Cargo.toml" > "$run_root/build/cargo-test.log" 2>&1

run_timing_cell() {
  local fixture_id=$1 pattern_file=$2 corpus_file=$3 cycle=$4 variant=$5 mode=$6
  local stem="$fixture_id-c$(printf '%02d' "$cycle")-$variant"
  checkpoint "$stem"
  RUSTMATCH_BENCH_REVISION="$candidate_revision" RUSTMATCH_BENCH_RUNNER="$runner_label" \
    /usr/bin/time -f '{"max_rss_kb":%M,"elapsed_seconds":%e,"user_seconds":%U,"system_seconds":%S,"major_faults":%F,"minor_faults":%R,"voluntary_context_switches":%w,"involuntary_context_switches":%c}' \
      -o "$run_root/receipts/timing/$stem.time.json" \
      taskset -c "$cpu_set" "$candidate_binary" harness-run \
        "$run_root/fixtures/$pattern_file" "$run_root/fixtures/$corpus_file" \
        "$repeats" "$warmups" "$mode" > "$run_root/receipts/timing/$stem.json"
}

while IFS=$'\t' read -r fixture_id pattern_file corpus_file expected_activation; do
  for cycle in $(seq 1 "$timing_cycles"); do
    if (( cycle % 2 == 1 )); then
      run_timing_cell "$fixture_id" "$pattern_file" "$corpus_file" "$cycle" generic cohort-view-1
      run_timing_cell "$fixture_id" "$pattern_file" "$corpus_file" "$cycle" specialized cohort-assert-1
    else
      run_timing_cell "$fixture_id" "$pattern_file" "$corpus_file" "$cycle" specialized cohort-assert-1
      run_timing_cell "$fixture_id" "$pattern_file" "$corpus_file" "$cycle" generic cohort-view-1
    fi
    generic="$run_root/receipts/timing/$fixture_id-c$(printf '%02d' "$cycle")-generic.json"
    specialized="$run_root/receipts/timing/$fixture_id-c$(printf '%02d' "$cycle")-specialized.json"
    jq -e --slurpfile specialized "$specialized" --arg expected "$expected_activation" \
      '.event_digest == $specialized[0].event_digest and .matches_per_iteration == $specialized[0].matches_per_iteration and .diagnostics.assertion_prefix_activations == 0 and $specialized[0].diagnostics.assertion_prefix_activations == ($expected | tonumber)' \
      "$generic" >/dev/null
  done
done < <(jq -r '.fixtures[] | [.id,.pattern_file,.corpus_file,.expected_specialized_activation] | @tsv' "$plan")

run_resource_cell() {
  local fixture_id=$1 pattern_file=$2 variant=$3
  local stem="$fixture_id-$variant"
  checkpoint "resource-$stem"
  env LD_PRELOAD="$run_root/artifacts/h43-a1-alloc-shim.so" \
    RMATCH_ALLOC_RECEIPT="$run_root/receipts/resources/$stem.alloc.json" \
    taskset -c "$cpu_set" "$probe_binary" "$run_root/fixtures/$pattern_file" "$variant" \
    > "$run_root/receipts/resources/$stem.json"
  jq -e '.allocator == "glibc-interposition-v1"' "$run_root/receipts/resources/$stem.alloc.json" >/dev/null
}

while IFS=$'\t' read -r fixture_id pattern_file; do
  run_resource_cell "$fixture_id" "$pattern_file" generic
  run_resource_cell "$fixture_id" "$pattern_file" specialized
done < <(jq -r '.fixtures[] | [.id,.pattern_file] | @tsv' "$plan")

checkpoint analyze
if [[ -s $run_root/host/docker-events.jsonl || -s $run_root/host/contamination.jsonl ]]; then
  window_error="Docker, GPU, or service contamination occurred during the window"
  exit 3
fi
python3 "$repository_root/tools/h43_a1_analyze.py" --plan "$plan" --run-root "$run_root" \
  --output "$run_root/summary.json" > "$run_root/analyzer-console.json"
find "$run_root" -type f ! -name sha256.txt -print0 | sort -z | xargs -0 sha256sum \
  > "$run_root/sha256.txt"
