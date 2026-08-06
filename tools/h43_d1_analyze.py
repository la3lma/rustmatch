#!/usr/bin/env python3
"""Validate and summarize a completed H43-D1 guarded campaign."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import re
import statistics
from pathlib import Path
from typing import Any


TIMING_PATTERN = re.compile(
    r"^(inactive|cohort)-(.+)-w(\d+)-c(\d+)-(baseline|candidate|ordinary|cohort)\.json$"
)
RESOURCE_PATTERN = re.compile(
    r"^(.+)-w(\d+)-c(\d+)-(baseline|ordinary|cohort)\.json$"
)


def read_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError(f"{path} is not a JSON object")
    return value


def geometric_mean(values: list[float]) -> float:
    if not values or any(value <= 0 for value in values):
        raise ValueError("geometric mean requires positive values")
    return math.exp(sum(math.log(value) for value in values) / len(values))


def delta_basis_points(ratio: float) -> int:
    return round((ratio - 1.0) * 10_000)


def parse_size(path: Path) -> int:
    rows = [line.split() for line in path.read_text(encoding="utf-8").splitlines() if line.strip()]
    if not rows:
        raise ValueError(f"{path} contains no artifact sizes")
    return max(int(row[-1]) for row in rows)


def timing_summary(
    root: Path,
    plan: dict[str, Any],
) -> tuple[list[dict[str, Any]], list[str]]:
    grouped: dict[tuple[str, str, int, str], dict[int, dict[str, Any]]] = {}
    expected_cycles = int(plan["timing_cycles"])
    baseline_revision = plan["baseline_revision"]
    candidate_revision = plan["candidate_revision"]
    correctness_errors: list[str] = []

    for path in sorted((root / "receipts/timing").glob("*.json")):
        if path.name.endswith(".time.json"):
            continue
        match = TIMING_PATTERN.match(path.name)
        if match is None:
            raise ValueError(f"unrecognized timing receipt name: {path.name}")
        lane, fixture, worker_source, cycle_source, variant = match.groups()
        worker = int(worker_source)
        cycle = int(cycle_source)
        receipt = read_json(path)
        expected_revision = baseline_revision if variant == "baseline" else candidate_revision
        if receipt.get("revision") != expected_revision or receipt.get("correctness") != "pass":
            correctness_errors.append(f"{path.name}: invalid revision or correctness")
        grouped.setdefault((lane, fixture, worker, variant), {})[cycle] = receipt

    comparisons: list[dict[str, Any]] = []
    for fixture in [item["id"] for item in plan["fixtures"]]:
        for worker in plan["worker_counts"]:
            for lane, first, second in (
                ("inactive", "baseline", "candidate"),
                ("cohort", "ordinary", "cohort"),
            ):
                first_rows = grouped.get((lane, fixture, worker, first), {})
                second_rows = grouped.get((lane, fixture, worker, second), {})
                expected = set(range(1, expected_cycles + 1))
                if set(first_rows) != expected or set(second_rows) != expected:
                    raise ValueError(f"incomplete {lane}/{fixture}/w{worker} timing cycles")
                for cycle in expected:
                    left = first_rows[cycle]
                    right = second_rows[cycle]
                    identity = (left["matches_per_iteration"], left["event_digest"])
                    if identity != (right["matches_per_iteration"], right["event_digest"]):
                        correctness_errors.append(
                            f"{lane}/{fixture}/w{worker}/c{cycle}: event identity mismatch"
                        )
                for metric in ("prepare_ns", "median_scan_ns"):
                    ratios = [
                        float(second_rows[cycle][metric]) / float(first_rows[cycle][metric])
                        for cycle in sorted(expected)
                    ]
                    ratio = geometric_mean(ratios)
                    comparisons.append(
                        {
                            "lane": lane,
                            "fixture": fixture,
                            "worker_count": worker,
                            "metric": metric,
                            "baseline_variant": first,
                            "candidate_variant": second,
                            "cycles": expected_cycles,
                            "baseline_median": statistics.median(
                                int(first_rows[cycle][metric]) for cycle in expected
                            ),
                            "candidate_median": statistics.median(
                                int(second_rows[cycle][metric]) for cycle in expected
                            ),
                            "geometric_ratio": ratio,
                            "delta_basis_points": delta_basis_points(ratio),
                            "candidate_slower_cycles": sum(value > 1.0 for value in ratios),
                            "candidate_faster_cycles": sum(value < 1.0 for value in ratios),
                        }
                    )
    return comparisons, correctness_errors


def resource_summary(root: Path, plan: dict[str, Any]) -> list[dict[str, Any]]:
    grouped: dict[tuple[str, int, str], list[dict[str, int]]] = {}
    for path in sorted((root / "receipts/resources").glob("*.json")):
        if path.name.endswith((".time.json", ".alloc.json")):
            continue
        match = RESOURCE_PATTERN.match(path.name)
        if match is None:
            raise ValueError(f"unrecognized resource receipt name: {path.name}")
        fixture, worker_source, _cycle, variant = match.groups()
        worker = int(worker_source)
        stem = path.name.removesuffix(".json")
        time_receipt = read_json(path.with_name(stem + ".time.json"))
        allocation = read_json(path.with_name(stem + ".alloc.json"))
        harness = read_json(path)
        grouped.setdefault((fixture, worker, variant), []).append(
            {
                "max_rss_bytes": int(time_receipt["max_rss_kb"]) * 1024,
                "allocation_calls": int(allocation["malloc_calls"])
                + int(allocation["calloc_calls"])
                + int(allocation["aligned_calls"])
                + int(allocation["realloc_calls"]),
                "requested_bytes": int(allocation["requested_bytes"]),
                "peak_live_usable_bytes": int(allocation["peak_live_usable_bytes"]),
                "database_retained_bytes": int(harness["diagnostics"]["database_retained_bytes"]),
                "cache_table_bytes": int(harness["diagnostics"]["cache_table_bytes"]),
                "prefilter_retained_bytes": int(harness["diagnostics"]["prefilter_retained_bytes"]),
                "buffered_event_bytes": int(harness["diagnostics"]["buffered_event_bytes"]),
            }
        )

    rows: list[dict[str, Any]] = []
    metrics = (
        "max_rss_bytes",
        "allocation_calls",
        "requested_bytes",
        "peak_live_usable_bytes",
        "database_retained_bytes",
        "cache_table_bytes",
        "prefilter_retained_bytes",
        "buffered_event_bytes",
    )
    for fixture in [item["id"] for item in plan["fixtures"]]:
        for worker in plan["worker_counts"]:
            for lane, first, second in (
                ("inactive", "baseline", "ordinary"),
                ("cohort", "ordinary", "cohort"),
            ):
                left = grouped[(fixture, worker, first)]
                right = grouped[(fixture, worker, second)]
                for metric in metrics:
                    baseline_value = statistics.median(row[metric] for row in left)
                    candidate_value = statistics.median(row[metric] for row in right)
                    ratio = None if baseline_value == 0 else candidate_value / baseline_value
                    rows.append(
                        {
                            "lane": lane,
                            "fixture": fixture,
                            "worker_count": worker,
                            "metric": metric,
                            "baseline_variant": first,
                            "candidate_variant": second,
                            "baseline_median": baseline_value,
                            "candidate_median": candidate_value,
                            "delta_bytes_or_calls": candidate_value - baseline_value,
                            "delta_basis_points": None
                            if ratio is None
                            else delta_basis_points(ratio),
                        }
                    )
    return rows


def build_summary(root: Path, plan: dict[str, Any]) -> dict[str, Any]:
    times: dict[str, list[float]] = {}
    for variant in ("baseline", "candidate"):
        times[variant] = [
            float(read_json(path)["elapsed_seconds"])
            for path in sorted((root / f"build/compile-{variant}").glob("cycle-*.json"))
        ]
        if len(times[variant]) != int(plan["compile_cycles"]):
            raise ValueError(f"incomplete {variant} compile-time receipts")
    compile_ratio = geometric_mean(
        [candidate / baseline for baseline, candidate in zip(times["baseline"], times["candidate"])]
    )
    artifacts = {}
    for feature in ("feature", "default"):
        baseline = parse_size(root / f"artifacts/baseline-{feature}-rlib-bytes.txt")
        candidate = parse_size(root / f"artifacts/candidate-{feature}-rlib-bytes.txt")
        artifacts[feature] = {
            "baseline_bytes": baseline,
            "candidate_bytes": candidate,
            "delta_bytes": candidate - baseline,
            "delta_basis_points": delta_basis_points(candidate / baseline),
        }
    return {
        "compile": {
            "baseline_seconds": times["baseline"],
            "candidate_seconds": times["candidate"],
            "baseline_median_seconds": statistics.median(times["baseline"]),
            "candidate_median_seconds": statistics.median(times["candidate"]),
            "geometric_ratio": compile_ratio,
            "delta_basis_points": delta_basis_points(compile_ratio),
        },
        "rlib_artifacts": artifacts,
    }


def classify(
    timing: list[dict[str, Any]],
    resources: list[dict[str, Any]],
    correctness_errors: list[str],
    plan: dict[str, Any],
) -> tuple[str, list[dict[str, Any]]]:
    thresholds = plan["thresholds"]
    signals: list[dict[str, Any]] = []
    if correctness_errors:
        return "reject", [{"kind": "correctness", "detail": item} for item in correctness_errors]
    for row in timing:
        delta = row["delta_basis_points"]
        directional = row["candidate_slower_cycles"] >= thresholds["directional_burden_required"]
        if delta > thresholds["block_regression_basis_points"] and directional:
            signals.append({"severity": "block", **row})
        elif delta > thresholds["investigate_regression_basis_points"] and directional:
            signals.append({"severity": "investigate", **row})
    for row in resources:
        if row["metric"] != "max_rss_bytes" or row["delta_basis_points"] is None:
            continue
        material = row["delta_bytes_or_calls"] > thresholds["rss_absolute_materiality_bytes"]
        if row["delta_basis_points"] > thresholds["block_regression_basis_points"] and material:
            signals.append({"severity": "block", **row})
        elif row["delta_basis_points"] > thresholds["investigate_regression_basis_points"] and material:
            signals.append({"severity": "investigate", **row})
    if any(signal["severity"] == "block" for signal in signals):
        return "rework", signals
    if signals:
        return "investigate", signals
    return "authorize", signals


def analyze(root: Path, plan_path: Path) -> dict[str, Any]:
    plan = read_json(plan_path)
    state = read_json(root / "window-state.json")
    if state.get("status") != "pass":
        raise ValueError("guarded window did not pass")
    plan_sha256 = hashlib.sha256(plan_path.read_bytes()).hexdigest()
    if state.get("plan_sha256") != plan_sha256:
        raise ValueError("window state is not bound to this plan")
    activation = {
        fixture["id"]: read_json(root / f"activation/{fixture['id']}.json")["cohort_count"]
        for fixture in plan["fixtures"]
    }
    for fixture in plan["fixtures"]:
        if activation[fixture["id"]] != fixture["expected_cohorts"]:
            raise ValueError(f"activation mismatch for {fixture['id']}")
    timing, correctness_errors = timing_summary(root, plan)
    resources = resource_summary(root, plan)
    builds = build_summary(root, plan)
    decision, signals = classify(timing, resources, correctness_errors, plan)
    summary = {
        "schema_version": 1,
        "evidence_id": "H43-D1-summary",
        "plan_sha256": plan_sha256,
        "window_state_sha256": hashlib.sha256((root / "window-state.json").read_bytes()).hexdigest(),
        "baseline_revision": plan["baseline_revision"],
        "candidate_revision": plan["candidate_revision"],
        "activation": activation,
        "timing_comparisons": timing,
        "resource_comparisons": resources,
        "build_metrics": builds,
        "correctness_errors": correctness_errors,
        "automatic_disposition": decision,
        "signals": signals,
    }
    (root / "summary.json").write_text(
        json.dumps(summary, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    return summary


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--plan", required=True, type=Path)
    parser.add_argument("--run-root", required=True, type=Path)
    arguments = parser.parse_args()
    summary = analyze(arguments.run_root, arguments.plan)
    print(json.dumps({"automatic_disposition": summary["automatic_disposition"], "signals": len(summary["signals"])}))


if __name__ == "__main__":
    main()
