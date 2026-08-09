#!/usr/bin/env bash

set -Eeuo pipefail

repository_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
plan=${H43_A1_P1_PLAN:-$repository_root/docs/experiments/h43-a1-p1-screen-plan.json}
candidate_root=${H43_A1_P1_CANDIDATE_ROOT:?set H43_A1_P1_CANDIDATE_ROOT to the frozen candidate}
fixture_root=${H43_A1_P1_FIXTURE_ROOT:?set H43_A1_P1_FIXTURE_ROOT to the retained H13 fixtures}
run_root=${H43_A1_P1_RUN_ROOT:?set H43_A1_P1_RUN_ROOT to a fresh evidence directory}

for command in cargo docker jq nvidia-smi python3 sha256sum taskset; do
  command -v "$command" >/dev/null 2>&1 || {
    echo "required H43-A1-P1 command is unavailable: $command" >&2
    exit 2
  }
done

candidate_revision=$(jq -r .candidate_revision "$plan")
candidate_tree=$(jq -r .candidate_tree "$plan")
evidence_id=$(jq -r .evidence_id "$plan")
cpu_set=$(jq -r .cpu_set "$plan")
warmup_pairs=$(jq -r .warmup_pairs_per_order "$plan")
retained_pairs=$(jq -r .retained_pairs_per_order "$plan")

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
while IFS=$'\t' read -r source_path expected_sha; do
  [[ $(sha256sum "$repository_root/$source_path" | awk '{print $1}') == "$expected_sha" ]] || {
    echo "$evidence_id runner source hash mismatch: $source_path" >&2
    exit 2
  }
done < <(jq -r '.runner_sources[] | [.path,.sha256] | @tsv' "$plan")

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

mkdir -p "$run_root"/{artifacts,build,fixtures,host,receipts}
cp "$plan" "$run_root/frozen-plan.json"
while IFS=$'\t' read -r source_file expected_sha; do
  cp "$fixture_root/$source_file" "$run_root/fixtures/$source_file"
  [[ $(sha256sum "$run_root/fixtures/$source_file" | awk '{print $1}') == "$expected_sha" ]] || {
    echo "$evidence_id fixture hash mismatch: $source_file" >&2
    exit 2
  }
done < <(jq -r '.fixtures[] | [.pattern_file,.pattern_sha256] | @tsv' "$plan")

started_utc=$(date -u +%Y-%m-%dT%H:%M:%SZ)
plan_sha256=$(sha256sum "$plan" | awk '{print $1}')
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
  --arg candidate_revision "$candidate_revision" --arg candidate_tree "$candidate_tree" \
  --arg rustc "$(rustc -Vv)" --arg cargo "$(cargo -V)" \
  --argjson docker "$(docker ps -a --no-trunc --format '{{json .}}' | jq -s .)" \
  '{schema_version:1,started_utc:$started_utc,hostname:$hostname,kernel:$kernel,plan_sha256:$plan_sha256,candidate_revision:$candidate_revision,candidate_tree:$candidate_tree,rustc:$rustc,cargo:$cargo,docker:$docker}' \
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
    sleep 0.25
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

checkpoint build-start
probe_root="$run_root/build/prepare-probe"
mkdir -p "$probe_root/src"
cp "$repository_root/tools/h43_a1_p1_prepare_probe.rs" "$probe_root/src/main.rs"
printf '[package]\nname = "h43-a1-p1-prepare-probe"\nversion = "0.0.0"\nedition = "2024"\n\n[workspace]\n\n[dependencies]\nrustmatch = { path = "%s/rustmatch", features = ["benchmark-internals"] }\n' \
  "$candidate_root" > "$probe_root/Cargo.toml"
candidate_target="$run_root/build/candidate-target"
CARGO_TARGET_DIR="$candidate_target" cargo build --offline --release --manifest-path "$probe_root/Cargo.toml" \
  > "$run_root/build/prepare-probe-build.log" 2>&1
probe_binary="$candidate_target/release/h43-a1-p1-prepare-probe"
sha256sum "$probe_binary" > "$run_root/artifacts/prepare-probe-binary-sha256.txt"

checkpoint tests
CARGO_TARGET_DIR="$candidate_target" cargo test --locked --workspace --all-features \
  --manifest-path "$candidate_root/Cargo.toml" > "$run_root/build/cargo-test.log" 2>&1

while IFS=$'\t' read -r fixture_id pattern_file; do
  checkpoint "measure-$fixture_id"
  /usr/bin/time -f '{"max_rss_kb":%M,"elapsed_seconds":%e,"user_seconds":%U,"system_seconds":%S,"major_faults":%F,"minor_faults":%R,"voluntary_context_switches":%w,"involuntary_context_switches":%c}' \
    -o "$run_root/receipts/$fixture_id.time.json" \
    taskset -c "$cpu_set" "$probe_binary" "$run_root/fixtures/$pattern_file" "$fixture_id" \
      "$warmup_pairs" "$retained_pairs" > "$run_root/receipts/$fixture_id.jsonl"
done < <(jq -r '.fixtures[] | [.id,.pattern_file] | @tsv' "$plan")

checkpoint analyze
if [[ -s $run_root/host/docker-events.jsonl || -s $run_root/host/contamination.jsonl ]]; then
  window_error="Docker, GPU, or service contamination occurred during the window"
  exit 3
fi
python3 "$repository_root/tools/h43_a1_p1_analyze.py" --plan "$plan" --run-root "$run_root" \
  --output "$run_root/summary.json" > "$run_root/analyzer-console.json"
find "$run_root" -type f ! -name sha256.txt -print0 | sort -z | xargs -0 sha256sum \
  > "$run_root/sha256.txt"
