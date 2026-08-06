#!/usr/bin/env python3
"""Analyze the focused H43-V1 shared-storage feasibility screen."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import statistics
from pathlib import Path
from typing import Any


def read_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def geometric_mean(values: list[float]) -> float:
    if not values or any(value <= 0 for value in values):
        raise ValueError("geometric means require positive samples")
    return math.exp(sum(math.log(value) for value in values) / len(values))


def allocation_calls(receipt: dict[str, Any]) -> int:
    return sum(
        int(receipt[name])
        for name in ("malloc_calls", "calloc_calls", "aligned_calls", "realloc_calls")
    )


def timing_summary(run_root: Path, fixture: str, cycles: int) -> dict[str, Any]:
    ordinary = [
        read_json(run_root / "receipts" / "timing" / f"{fixture}-c{cycle:02d}-ordinary.json")
        for cycle in range(1, cycles + 1)
    ]
    shared = [
        read_json(run_root / "receipts" / "timing" / f"{fixture}-c{cycle:02d}-shared.json")
        for cycle in range(1, cycles + 1)
    ]
    parity_errors: list[str] = []
    for cycle, (ordinary_receipt, shared_receipt) in enumerate(zip(ordinary, shared), 1):
        for field in ("matches_per_iteration", "event_digest"):
            if ordinary_receipt[field] != shared_receipt[field]:
                parity_errors.append(f"{fixture} cycle {cycle} differs in {field}")

    ordinary_prepare = [int(receipt["prepare_ns"]) for receipt in ordinary]
    shared_prepare = [int(receipt["prepare_ns"]) for receipt in shared]
    ordinary_scan = [int(receipt["median_scan_ns"]) for receipt in ordinary]
    shared_scan = [int(receipt["median_scan_ns"]) for receipt in shared]
    ordinary_prepare_median = statistics.median(ordinary_prepare)
    shared_prepare_median = statistics.median(shared_prepare)
    ordinary_scan_median = statistics.median(ordinary_scan)
    shared_scan_median = statistics.median(shared_scan)
    prepare_pairs = [candidate / baseline for baseline, candidate in zip(ordinary_prepare, shared_prepare)]
    scan_pairs = [candidate / baseline for baseline, candidate in zip(ordinary_scan, shared_scan)]

    return {
        "fixture": fixture,
        "cycles": cycles,
        "parity_errors": parity_errors,
        "event_digest": ordinary[0]["event_digest"],
        "matches_per_iteration": ordinary[0]["matches_per_iteration"],
        "ordinary_prepare_ns": ordinary_prepare,
        "shared_prepare_ns": shared_prepare,
        "ordinary_prepare_median_ns": ordinary_prepare_median,
        "shared_prepare_median_ns": shared_prepare_median,
        "prepare_median_delta_percent": 100.0 * (shared_prepare_median / ordinary_prepare_median - 1.0),
        "prepare_paired_geometric_delta_percent": 100.0 * (geometric_mean(prepare_pairs) - 1.0),
        "ordinary_scan_ns": ordinary_scan,
        "shared_scan_ns": shared_scan,
        "ordinary_scan_median_ns": ordinary_scan_median,
        "shared_scan_median_ns": shared_scan_median,
        "scan_median_gain_percent": 100.0 * (1.0 - shared_scan_median / ordinary_scan_median),
        "scan_paired_geometric_gain_percent": 100.0 * (1.0 - geometric_mean(scan_pairs)),
        "ordinary_database_retained_bytes": ordinary[0]["diagnostics"]["database_retained_bytes"],
        "shared_database_retained_bytes": shared[0]["diagnostics"]["database_retained_bytes"],
        "ordinary_prefilter_retained_bytes": ordinary[0]["diagnostics"]["prefilter_retained_bytes"],
        "shared_prefilter_retained_bytes": shared[0]["diagnostics"]["prefilter_retained_bytes"],
    }


def resource_summary(run_root: Path, fixture: str) -> dict[str, Any]:
    ordinary = read_json(run_root / "receipts" / "resources" / f"{fixture}-ordinary.alloc.json")
    shared = read_json(run_root / "receipts" / "resources" / f"{fixture}-shared.alloc.json")
    ordinary_calls = allocation_calls(ordinary)
    shared_calls = allocation_calls(shared)
    return {
        "fixture": fixture,
        "ordinary_allocation_calls": ordinary_calls,
        "shared_allocation_calls": shared_calls,
        "extra_allocation_calls": shared_calls - ordinary_calls,
        "ordinary_requested_bytes": int(ordinary["requested_bytes"]),
        "shared_requested_bytes": int(shared["requested_bytes"]),
        "extra_requested_bytes": int(shared["requested_bytes"]) - int(ordinary["requested_bytes"]),
        "ordinary_peak_live_usable_bytes": int(ordinary["peak_live_usable_bytes"]),
        "shared_peak_live_usable_bytes": int(shared["peak_live_usable_bytes"]),
    }


def analyze(plan_path: Path, run_root: Path) -> dict[str, Any]:
    plan = read_json(plan_path)
    fixtures = [fixture["id"] for fixture in plan["fixtures"]]
    timing = [timing_summary(run_root, fixture, int(plan["timing_cycles"])) for fixture in fixtures]
    resources = [resource_summary(run_root, fixture) for fixture in fixtures]
    thresholds = plan["thresholds"]
    blockers: list[str] = []

    for result in timing:
        blockers.extend(result["parity_errors"])
        if result["prepare_median_delta_percent"] > thresholds["maximum_preparation_regression_percent"]:
            blockers.append(
                f"{result['fixture']} preparation regressed "
                f"{result['prepare_median_delta_percent']:.3f}%"
            )
    for result in resources:
        if result["extra_allocation_calls"] > thresholds["maximum_extra_allocation_calls"]:
            blockers.append(
                f"{result['fixture']} added {result['extra_allocation_calls']} allocation calls"
            )
        if result["extra_requested_bytes"] > thresholds["maximum_extra_requested_bytes"]:
            blockers.append(
                f"{result['fixture']} requested {result['extra_requested_bytes']} extra bytes"
            )
    if all(
        result["scan_median_gain_percent"] < thresholds["minimum_useful_scan_gain_percent"]
        for result in timing
    ):
        blockers.append("both mixed fixtures retained less than the required 5% scan gain")

    return {
        "schema_version": 1,
        "evidence_id": f"{plan['evidence_id']}-summary",
        "claim": plan["claim"],
        "plan_sha256": sha256(plan_path),
        "candidate_revision": plan["candidate_revision"],
        "preproved_local_evidence": plan["preproved_local_evidence"],
        "timing": timing,
        "resources": resources,
        "blockers": blockers,
        "automatic_disposition": "authorize-runtime-spine" if not blockers else "reject",
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--plan", required=True, type=Path)
    parser.add_argument("--run-root", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    arguments = parser.parse_args()
    result = analyze(arguments.plan, arguments.run_root)
    arguments.output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
