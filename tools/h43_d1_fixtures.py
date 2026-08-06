#!/usr/bin/env python3
"""Generate deterministic H43-D1 pattern and ASCII corpus fixtures."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_exact_corpus(path: Path, size: int, markers: list[bytes]) -> None:
    filler = b"ordinary ascii filler with no numbered marker tokens\n"
    chunks: list[bytes] = []
    target_spacing = max(1, size // (len(markers) + 1))
    current = 0
    for marker in markers:
        target = min(size, current + target_spacing)
        while current + len(filler) <= target:
            chunks.append(filler)
            current += len(filler)
        if current + len(marker) <= size:
            chunks.append(marker)
            current += len(marker)
    while current + len(filler) <= size:
        chunks.append(filler)
        current += len(filler)
    chunks.append(b"~" * (size - current))
    data = b"".join(chunks)
    if len(data) != size or not data.isascii():
        raise ValueError(f"fixture {path} did not reach its exact ASCII size")
    path.write_bytes(data)


def pattern_rows(expressions: list[str]) -> bytes:
    return "".join(f"{index}\t{expression}\n" for index, expression in enumerate(expressions, 1)).encode("ascii")


def generate(output: Path, plan_path: Path) -> dict[str, object]:
    plan = json.loads(plan_path.read_text(encoding="utf-8"))
    output.mkdir(parents=True, exist_ok=True)
    manifest: dict[str, object] = {
        "schema_version": 1,
        "evidence_id": f"{plan['evidence_id']}-fixtures",
        "plan_sha256": hashlib.sha256(plan_path.read_bytes()).hexdigest(),
        "fixtures": [],
    }

    free = [f"free{index:04d}" for index in range(64)]
    asserted = [f"^assert{index:04d}$" for index in range(64)]
    definitions = {
        "assertion-free": (
            free,
            [f"xx free{index:04d} yy\n".encode("ascii") for index in range(64)],
        ),
        "assertion-only": (
            asserted,
            [f"assert{index:04d}\n".encode("ascii") for index in range(64)],
        ),
        "mixed-balanced": (
            free[:32] + asserted[:32],
            [f"xx free{index:04d} yy\n".encode("ascii") for index in range(32)]
            + [f"assert{index:04d}\n".encode("ascii") for index in range(32)],
        ),
        "mixed-output": (
            ["a+" for _ in range(8)] + ["^a+$" for _ in range(8)],
            [b"a" * 63 + b"\n" for _ in range(256)],
        ),
    }

    for fixture in plan["fixtures"]:
        fixture_id = fixture["id"]
        expressions, markers = definitions[fixture_id]
        if len(expressions) != fixture["pattern_count"]:
            raise ValueError(f"{fixture_id} pattern count disagrees with the frozen plan")
        pattern_path = output / f"{fixture_id}.tsv"
        corpus_path = output / f"{fixture_id}.txt"
        pattern_path.write_bytes(pattern_rows(expressions))
        write_exact_corpus(corpus_path, fixture["corpus_bytes"], markers)
        manifest["fixtures"].append(
            {
                "id": fixture_id,
                "expected_cohorts": fixture["expected_cohorts"],
                "pattern_count": len(expressions),
                "pattern_path": pattern_path.name,
                "pattern_sha256": sha256(pattern_path),
                "corpus_bytes": corpus_path.stat().st_size,
                "corpus_path": corpus_path.name,
                "corpus_sha256": sha256(corpus_path),
            }
        )

    manifest_path = output / "manifest.json"
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    return manifest


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--plan", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    arguments = parser.parse_args()
    print(json.dumps(generate(arguments.output, arguments.plan), sort_keys=True))


if __name__ == "__main__":
    main()
