"""Compare Rust parsing controls with separate output checks and timing passes."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import statistics
import subprocess
from pathlib import Path


def run(case: list[str], demo: Path, *, verify: bool = False) -> dict:
    label, runner, parser, mode = case
    command = [runner, parser, mode, str(demo)]
    if verify:
        command.append("--verify")
    result = subprocess.run(command, check=True, capture_output=True, text=True)
    row = json.loads(result.stdout)
    if row["verification"] != verify:
        raise ValueError(f"{label}: unexpected verification mode")
    return row


def tick_bounds(row: dict) -> tuple[int, int, int]:
    return row["first_tick"], row["last_tick"], row["ticks"]


def validate(reference: dict, row: dict, *, allow_differences: bool = False) -> None:
    if tick_bounds(row) != tick_bounds(reference):
        raise ValueError("completed ticks differ")
    if not allow_differences and row["samples"] != reference["samples"]:
        raise ValueError("sampled state differs; inspect the verification reports")


def file_hash(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--case",
        action="append",
        nargs=4,
        required=True,
        metavar=("LABEL", "RUNNER", "PARSER", "MODE"),
    )
    parser.add_argument("--demo", type=Path, action="append", required=True)
    parser.add_argument("--rounds", type=int, default=5)
    parser.add_argument("--warmup", type=int, default=1)
    parser.add_argument("--cpu", type=int)
    parser.add_argument(
        "--allow-output-differences",
        action="store_true",
        help="Retain tick checks, but allow different sampled state. Inspect it first.",
    )
    args = parser.parse_args()
    if args.rounds < 1 or args.warmup < 0:
        parser.error("rounds must be positive; warmup must be non-negative")
    if len({case[0] for case in args.case}) != len(args.case):
        parser.error("case labels must be unique")
    if args.cpu is not None:
        os.sched_setaffinity(0, {args.cpu})
    print(
        json.dumps(
            {
                "kind": "metadata",
                "platform": platform.platform(),
                "cpu": args.cpu,
                "rounds": args.rounds,
                "warmup": args.warmup,
                "allow_output_differences": args.allow_output_differences,
                "binaries": {case[0]: file_hash(Path(case[1])) for case in args.case},
                "demos": {str(demo): file_hash(demo) for demo in args.demo},
            }
        ),
        flush=True,
    )
    for demo in args.demo:
        reference = None
        for case in args.case:
            row = run(case, demo, verify=True)
            if not row["samples"]:
                raise ValueError(f"{demo}: no verification sample tick was reached")
            if reference is not None:
                validate(
                    reference, row, allow_differences=args.allow_output_differences
                )
            reference = row
            print(
                json.dumps(
                    {"kind": "verification", "label": case[0], "demo": str(demo), **row}
                ),
                flush=True,
            )
        assert reference is not None
        samples = {case[0]: [] for case in args.case}
        for round_number in range(args.warmup + args.rounds):
            offset = round_number % len(args.case)
            order = args.case[offset:] + args.case[:offset]
            for case in order:
                row = run(case, demo)
                if tick_bounds(row) != tick_bounds(reference):
                    raise ValueError(
                        f"{case[0]}: completed ticks changed during timing"
                    )
                warmup = round_number < args.warmup
                if not warmup:
                    samples[case[0]].append(row["seconds"])
                print(
                    json.dumps(
                        {
                            "kind": "timing",
                            "label": case[0],
                            "demo": str(demo),
                            "round": round_number,
                            "warmup": warmup,
                            **row,
                        }
                    ),
                    flush=True,
                )
        for label, values in samples.items():
            print(
                json.dumps(
                    {
                        "kind": "summary",
                        "label": label,
                        "demo": str(demo),
                        "median_seconds": statistics.median(values),
                        "seconds": values,
                    }
                ),
                flush=True,
            )


if __name__ == "__main__":
    main()
