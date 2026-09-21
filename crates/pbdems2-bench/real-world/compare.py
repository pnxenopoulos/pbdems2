"""Alternate fresh-process baseline/candidate dataset runs and check output hashes."""
import argparse
import json
from pathlib import Path
import subprocess
import sys

WORKLOADS = {
    "awpy": ("snapshots_64", "snapshots_1", "events"),
    "boon": ("player_ticks", "ability_ticks", "combat"),
}
EXTRA_WORKLOADS = {
    "awpy": ("snapshot_single", "snapshots_ticks", "snapshots_window", "snapshots_empty"),
    "boon": (),
}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--python", type=Path, required=True)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--awpy-demo", type=Path, action="append", default=[])
    parser.add_argument("--boon-demo", type=Path, action="append", default=[])
    parser.add_argument("--rounds", type=int, default=5)
    parser.add_argument("--segments", type=int, nargs="+", default=[1])
    parser.add_argument("--workload", action="append", default=[])
    args = parser.parse_args()
    if args.rounds < 1 or not args.awpy_demo + args.boon_demo:
        parser.error("provide demos and a positive round count")
    if any(segments < 1 for segments in args.segments):
        parser.error("segment counts must be positive")
    runner = Path(__file__).with_name("datasets.py")
    cases = []
    for game in WORKLOADS:
        for demo in getattr(args, f"{game}_demo"):
            choices = WORKLOADS[game] + EXTRA_WORKLOADS[game] if args.workload else WORKLOADS[game]
            for workload in choices:
                if args.workload and workload not in args.workload:
                    continue
                for segments in args.segments:
                    if segments != 1 and workload not in (
                        "snapshots_64", "snapshots_1", "player_ticks", *EXTRA_WORKLOADS[game]
                    ):
                        continue
                    cases.append((game, demo, workload, segments))
    if not cases:
        parser.error("no workloads matched")
    for game, demo, workload, segments in cases:
        expected = None
        print(f"Comparing {game} {demo.name} {workload} segments={segments}", file=sys.stderr)
        for round_index in range(args.rounds):
            variants = ["baseline", "candidate"]
            if round_index % 2:
                variants.reverse()
            for variant in variants:
                command = [str(args.python), str(runner), "--game", game, "--demo", str(demo),
                           "--extension-dir", str(getattr(args, variant)), "--workload", workload,
                           "--segments", str(segments), "--repeat", "1",
                           "--warmup", "1" if round_index == 0 else "0", "--checksum"]
                result = subprocess.run(command, text=True, capture_output=True)
                if result.returncode:
                    raise RuntimeError(f"dataset process failed ({variant}): {result.stderr}")
                if not result.stdout.strip():
                    raise RuntimeError(f"dataset process returned no records ({variant})")
                for line in result.stdout.splitlines():
                    record = json.loads(line)
                    if expected is not None and record["output"] != expected:
                        raise RuntimeError(f"dataset changed: {game} {demo} {workload} {variant}")
                    expected = record["output"]
                    record["variant"] = variant
                    record["round"] = round_index
                    print(json.dumps(record), flush=True)


if __name__ == "__main__":
    main()
