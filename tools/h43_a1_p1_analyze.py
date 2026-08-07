#!/usr/bin/env python3
"""Analyze the H43-A1-P1 phase-local preparation discriminator."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import random
import statistics
from pathlib import Path
from typing import Any


def read_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def read_jsonl(path: Path) -> list[dict[str, Any]]:
    return [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines() if line]


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def geometric_mean(values: list[float]) -> float:
    if not values or any(value <= 0 for value in values):
        raise ValueError("geometric means require positive values")
    return math.exp(statistics.fmean(math.log(value) for value in values))


def bootstrap_delta_interval(ratios: list[float], seed: int) -> list[float]:
    generator = random.Random(seed)
    effects = []
    for _ in range(20_000):
        sample = [ratios[generator.randrange(len(ratios))] for _ in ratios]
        effects.append(100.0 * (geometric_mean(sample) - 1.0))
    effects.sort()
    return [effects[499], effects[19_499]]


STRUCTURE_FIELDS = (
    "state_count",
    "edge_count",
    "predicate_count",
    "terminal_count",
    "pattern_count",
    "assertion_free_pattern_count",
    "assertion_bearing_pattern_count",
    "database_retained_bytes",
    "prefilter_retained_bytes",
    "assertion_prefix_available",
)


def fixture_summary(
    run_root: Path, fixture: dict[str, Any], plan: dict[str, Any]
) -> dict[str, Any]:
    fixture_id = fixture["id"]
    records = read_jsonl(run_root / "receipts" / f"{fixture_id}.jsonl")
    expected_pairs = int(plan["retained_pairs_per_order"])
    integrity_errors: list[str] = []
    ratios_by_order: dict[str, list[float]] = {"ab": [], "ba": []}
    allocation_deltas: list[int] = []
    requested_byte_deltas: list[int] = []
    structures: dict[str, set[tuple[Any, ...]]] = {"generic": set(), "specialized": set()}

    expected_records = expected_pairs * 4
    if len(records) != expected_records:
        integrity_errors.append(
            f"{fixture_id} has {len(records)} records; expected {expected_records}"
        )

    for order in ("ab", "ba"):
        for pair in range(1, expected_pairs + 1):
            adjacent = [
                record
                for record in records
                if record.get("order") == order and int(record.get("pair", -1)) == pair
            ]
            adjacent.sort(key=lambda record: int(record["position"]))
            expected_variants = ["generic", "specialized"] if order == "ab" else ["specialized", "generic"]
            actual_variants = [record.get("variant") for record in adjacent]
            if actual_variants != expected_variants:
                integrity_errors.append(
                    f"{fixture_id} {order} pair {pair} variants were {actual_variants}"
                )
                continue
            by_variant = {record["variant"]: record for record in adjacent}
            generic = by_variant["generic"]
            specialized = by_variant["specialized"]
            ratios_by_order[order].append(
                int(specialized["elapsed_ns"]) / int(generic["elapsed_ns"])
            )
            allocation_deltas.append(
                int(specialized["allocation_calls"]) - int(generic["allocation_calls"])
            )
            requested_byte_deltas.append(
                int(specialized["requested_bytes"]) - int(generic["requested_bytes"])
            )

    for record in records:
        variant = str(record["variant"])
        structures[variant].add(tuple(record[field] for field in STRUCTURE_FIELDS))
        if variant == "generic" and bool(record["assertion_specialization_enabled"]):
            integrity_errors.append(f"{fixture_id} generic record enables specialization")
        if variant == "specialized" and not bool(record["assertion_specialization_enabled"]):
            integrity_errors.append(f"{fixture_id} specialized record disables specialization")

    if len(structures["generic"]) != 1 or len(structures["specialized"]) != 1:
        integrity_errors.append(f"{fixture_id} retained structure varied across builds")
    elif structures["generic"] != structures["specialized"]:
        integrity_errors.append(f"{fixture_id} generic and specialized retained structure differs")

    pooled = ratios_by_order["ab"] + ratios_by_order["ba"]
    delta_by_order = {
        order: 100.0 * (geometric_mean(ratios) - 1.0)
        for order, ratios in ratios_by_order.items()
        if ratios
    }
    slower_by_order = {
        order: sum(ratio > 1.0 for ratio in ratios)
        for order, ratios in ratios_by_order.items()
    }
    pooled_delta = 100.0 * (geometric_mean(pooled) - 1.0) if pooled else math.nan
    thresholds = plan["thresholds"]
    stable_regression = (
        not integrity_errors
        and pooled_delta > float(thresholds["close_regression_percent"])
        and all(
            delta_by_order.get(order, -math.inf)
            > float(thresholds["close_regression_percent"])
            for order in ("ab", "ba")
        )
        and all(
            slower_by_order.get(order, 0)
            >= int(thresholds["directional_burden_per_order"])
            for order in ("ab", "ba")
        )
    )
    allocation_equivalent = all(delta == 0 for delta in allocation_deltas)
    requested_bytes_equivalent = all(delta == 0 for delta in requested_byte_deltas)

    generic_times = [
        int(record["elapsed_ns"]) for record in records if record["variant"] == "generic"
    ]
    specialized_times = [
        int(record["elapsed_ns"])
        for record in records
        if record["variant"] == "specialized"
    ]
    return {
        "fixture": fixture_id,
        "role": fixture["role"],
        "records": len(records),
        "pairs_per_order": expected_pairs,
        "integrity_errors": sorted(set(integrity_errors)),
        "generic_median_ns": statistics.median(generic_times) if generic_times else None,
        "specialized_median_ns": statistics.median(specialized_times)
        if specialized_times
        else None,
        "pooled_paired_geometric_delta_percent": pooled_delta,
        "paired_bootstrap_95_percent_delta_interval": bootstrap_delta_interval(
            pooled, 43 + int(fixture["seed_offset"])
        )
        if pooled
        else None,
        "order_delta_percent": delta_by_order,
        "specialized_slower_pairs_by_order": slower_by_order,
        "allocation_equivalent_every_pair": allocation_equivalent,
        "requested_bytes_equivalent_every_pair": requested_bytes_equivalent,
        "allocation_delta_range": [min(allocation_deltas), max(allocation_deltas)]
        if allocation_deltas
        else None,
        "requested_byte_delta_range": [min(requested_byte_deltas), max(requested_byte_deltas)]
        if requested_byte_deltas
        else None,
        "retained_structure_equivalent": not any(
            "structure" in error for error in integrity_errors
        ),
        "stable_greater_than_three_percent_regression": stable_regression,
    }


def analyze(plan_path: Path, run_root: Path) -> dict[str, Any]:
    plan = read_json(plan_path)
    fixtures = [fixture_summary(run_root, fixture, plan) for fixture in plan["fixtures"]]
    integrity_errors = [
        error for fixture in fixtures for error in fixture["integrity_errors"]
    ]
    resource_investigations = [
        fixture["fixture"]
        for fixture in fixtures
        if not fixture["allocation_equivalent_every_pair"]
        or not fixture["requested_bytes_equivalent_every_pair"]
    ]
    stable_regressions = [
        fixture["fixture"]
        for fixture in fixtures
        if fixture["stable_greater_than_three_percent_regression"]
    ]

    if integrity_errors:
        window_disposition = "reject-invalid-discriminator"
        conservative_disposition = "no-decision"
        experimental_disposition = "no-decision"
    else:
        window_disposition = "valid"
        conservative_disposition = (
            "close-h43-a2" if stable_regressions else "authorize-h43-a2"
        )
        experimental_disposition = (
            "investigate-resource-difference"
            if resource_investigations
            else "authorize-x1-adr"
        )

    return {
        "schema_version": 1,
        "evidence_id": f"{plan['evidence_id']}-summary",
        "claim": plan["claim"],
        "plan_sha256": sha256(plan_path),
        "candidate_revision": plan["candidate_revision"],
        "fixtures": fixtures,
        "integrity_errors": integrity_errors,
        "stable_regressions": stable_regressions,
        "resource_investigations": resource_investigations,
        "window_disposition": window_disposition,
        "conservative_disposition": conservative_disposition,
        "experimental_disposition": experimental_disposition,
        "historical_h43_a1_disposition": "unchanged-machine-reject",
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--plan", required=True, type=Path)
    parser.add_argument("--run-root", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    arguments = parser.parse_args()
    result = analyze(arguments.plan, arguments.run_root)
    arguments.output.write_text(
        json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
