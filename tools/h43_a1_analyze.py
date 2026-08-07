#!/usr/bin/env python3
"""Analyze the H43-A1 same-binary forced-path focused screen."""

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


def timing_summary(run_root: Path, fixture: dict[str, Any], cycles: int) -> dict[str, Any]:
    fixture_id = fixture["id"]
    generic = [
        read_json(run_root / "receipts" / "timing" / f"{fixture_id}-c{cycle:02d}-generic.json")
        for cycle in range(1, cycles + 1)
    ]
    specialized = [
        read_json(
            run_root / "receipts" / "timing" / f"{fixture_id}-c{cycle:02d}-specialized.json"
        )
        for cycle in range(1, cycles + 1)
    ]
    generic_time = [
        read_json(run_root / "receipts" / "timing" / f"{fixture_id}-c{cycle:02d}-generic.time.json")
        for cycle in range(1, cycles + 1)
    ]
    specialized_time = [
        read_json(
            run_root
            / "receipts"
            / "timing"
            / f"{fixture_id}-c{cycle:02d}-specialized.time.json"
        )
        for cycle in range(1, cycles + 1)
    ]

    parity_errors: list[str] = []
    activation_errors: list[str] = []
    expected_activation = int(fixture["expected_specialized_activation"])
    for cycle, (generic_receipt, specialized_receipt) in enumerate(zip(generic, specialized), 1):
        for field in ("matches_per_iteration", "event_digest"):
            if generic_receipt[field] != specialized_receipt[field]:
                parity_errors.append(f"{fixture_id} cycle {cycle} differs in {field}")
        generic_activation = int(generic_receipt["diagnostics"]["assertion_prefix_activations"])
        specialized_activation = int(
            specialized_receipt["diagnostics"]["assertion_prefix_activations"]
        )
        if generic_activation != 0:
            activation_errors.append(f"{fixture_id} cycle {cycle} generic path activated")
        if specialized_activation != expected_activation:
            activation_errors.append(
                f"{fixture_id} cycle {cycle} activation was {specialized_activation}; "
                f"expected {expected_activation}"
            )

    generic_prepare = [int(receipt["prepare_ns"]) for receipt in generic]
    specialized_prepare = [int(receipt["prepare_ns"]) for receipt in specialized]
    generic_scan = [int(receipt["median_scan_ns"]) for receipt in generic]
    specialized_scan = [int(receipt["median_scan_ns"]) for receipt in specialized]
    prepare_ratios = [candidate / baseline for baseline, candidate in zip(generic_prepare, specialized_prepare)]
    scan_ratios = [candidate / baseline for baseline, candidate in zip(generic_scan, specialized_scan)]
    first_ratios = [scan_ratios[index] for index in range(0, cycles, 2)]
    second_ratios = [scan_ratios[index] for index in range(1, cycles, 2)]
    generic_rss = [int(receipt["max_rss_kb"]) * 1024 for receipt in generic_time]
    specialized_rss = [int(receipt["max_rss_kb"]) * 1024 for receipt in specialized_time]
    generic_rss_median = statistics.median(generic_rss)
    specialized_rss_median = statistics.median(specialized_rss)

    return {
        "fixture": fixture_id,
        "role": fixture["role"],
        "cycles": cycles,
        "parity_errors": parity_errors,
        "activation_errors": activation_errors,
        "event_digest": generic[0]["event_digest"],
        "matches_per_iteration": generic[0]["matches_per_iteration"],
        "generic_prepare_median_ns": statistics.median(generic_prepare),
        "specialized_prepare_median_ns": statistics.median(specialized_prepare),
        "prepare_median_delta_percent": 100.0
        * (statistics.median(specialized_prepare) / statistics.median(generic_prepare) - 1.0),
        "prepare_paired_geometric_delta_percent": 100.0 * (geometric_mean(prepare_ratios) - 1.0),
        "generic_scan_median_ns": statistics.median(generic_scan),
        "specialized_scan_median_ns": statistics.median(specialized_scan),
        "scan_median_gain_percent": 100.0
        * (1.0 - statistics.median(specialized_scan) / statistics.median(generic_scan)),
        "scan_paired_geometric_gain_percent": 100.0 * (1.0 - geometric_mean(scan_ratios)),
        "generic_first_scan_gain_percent": 100.0 * (1.0 - geometric_mean(first_ratios)),
        "specialized_first_scan_gain_percent": 100.0 * (1.0 - geometric_mean(second_ratios)),
        "generic_peak_rss_median_bytes": generic_rss_median,
        "specialized_peak_rss_median_bytes": specialized_rss_median,
        "peak_rss_delta_bytes": specialized_rss_median - generic_rss_median,
        "peak_rss_delta_percent": 100.0 * (specialized_rss_median / generic_rss_median - 1.0),
        "specialized_activation": expected_activation,
    }


def resource_summary(run_root: Path, fixture: dict[str, Any]) -> dict[str, Any]:
    fixture_id = fixture["id"]
    generic = read_json(run_root / "receipts" / "resources" / f"{fixture_id}-generic.alloc.json")
    specialized = read_json(
        run_root / "receipts" / "resources" / f"{fixture_id}-specialized.alloc.json"
    )
    return {
        "fixture": fixture_id,
        "generic_allocation_calls": allocation_calls(generic),
        "specialized_allocation_calls": allocation_calls(specialized),
        "extra_allocation_calls": allocation_calls(specialized) - allocation_calls(generic),
        "generic_requested_bytes": int(generic["requested_bytes"]),
        "specialized_requested_bytes": int(specialized["requested_bytes"]),
        "extra_requested_bytes": int(specialized["requested_bytes"])
        - int(generic["requested_bytes"]),
    }


def analyze(plan_path: Path, run_root: Path) -> dict[str, Any]:
    plan = read_json(plan_path)
    thresholds = plan["thresholds"]
    cycles = int(plan["timing_cycles"])
    timing = [timing_summary(run_root, fixture, cycles) for fixture in plan["fixtures"]]
    resources = [resource_summary(run_root, fixture) for fixture in plan["fixtures"]]
    blockers: list[str] = []
    investigations: list[str] = []

    for result in timing:
        blockers.extend(result["parity_errors"])
        blockers.extend(result["activation_errors"])
        if result["role"] == "target":
            if result["scan_paired_geometric_gain_percent"] < thresholds["minimum_target_scan_gain_percent"]:
                blockers.append(
                    f"{result['fixture']} target scan gain was "
                    f"{result['scan_paired_geometric_gain_percent']:.3f}%"
                )
        else:
            for metric in ("scan_paired_geometric_gain_percent",):
                regression = -float(result[metric])
                if regression > thresholds["maximum_guard_regression_percent"]:
                    blockers.append(f"{result['fixture']} scan regressed {regression:.3f}%")
                elif regression > thresholds["investigate_regression_percent_exclusive"]:
                    investigations.append(f"{result['fixture']} scan regressed {regression:.3f}%")
        prepare_regression = float(result["prepare_paired_geometric_delta_percent"])
        if prepare_regression > thresholds["maximum_guard_regression_percent"]:
            blockers.append(f"{result['fixture']} preparation regressed {prepare_regression:.3f}%")
        elif prepare_regression > thresholds["investigate_regression_percent_exclusive"]:
            investigations.append(
                f"{result['fixture']} preparation regressed {prepare_regression:.3f}%"
            )
        rss_delta = float(result["peak_rss_delta_bytes"])
        rss_regression = float(result["peak_rss_delta_percent"])
        if (
            rss_delta > thresholds["peak_rss_absolute_materiality_bytes"]
            and rss_regression > thresholds["maximum_guard_regression_percent"]
        ):
            blockers.append(
                f"{result['fixture']} peak RSS regressed {rss_regression:.3f}% "
                f"({rss_delta:.0f} bytes)"
            )

    for result in resources:
        if result["extra_allocation_calls"] > thresholds["maximum_extra_build_allocation_calls"]:
            blockers.append(
                f"{result['fixture']} added {result['extra_allocation_calls']} build allocations"
            )
        if result["extra_requested_bytes"] > thresholds["maximum_extra_build_requested_bytes"]:
            blockers.append(
                f"{result['fixture']} requested {result['extra_requested_bytes']} extra build bytes"
            )

    disposition = "reject" if blockers else "investigate" if investigations else "authorize-h43-a2"
    return {
        "schema_version": 1,
        "evidence_id": f"{plan['evidence_id']}-summary",
        "claim": plan["claim"],
        "plan_sha256": sha256(plan_path),
        "candidate_revision": plan["candidate_revision"],
        "timing": timing,
        "resources": resources,
        "blockers": blockers,
        "investigations": investigations,
        "automatic_disposition": disposition,
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
