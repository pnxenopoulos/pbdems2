"""Time actual dataset APIs with fresh caches and emit profiler time windows."""
import argparse
import gc
import importlib.util
import json
import os
from pathlib import Path
import resource
import sys
import time


def workload(demo, game, name):
    if game == "awpy":
        selectors = {
            "snapshot_single": {"ticks": 1000},
            "snapshots_ticks": {"ticks": [1, 1000, 1005]},
            "snapshots_window": {"start_tick": 1000, "end_tick": 1005},
            "snapshots_empty": {"start_tick": -10000, "end_tick": -9999},
        }
        if name in selectors:
            return {"snapshots": demo.snapshots(**selectors[name])}
        if name.startswith("snapshots_"):
            return {"snapshots": demo.snapshots(every=int(name.split("_")[1]))}
        groups = {
            "events": ("kills", "damages", "bomb", "blinds", "shots"),
            "projectiles": ("grenades", "fires", "smokes"),
            "stats": ("stats",),
        }
    else:
        groups = {
            "player_ticks": ("player_ticks",),
            "ability_ticks": ("ability_ticks",),
            "combat": ("kills", "damage", "abilities"),
        }
    if name not in groups:
        raise ValueError(f"unknown {game} workload: {name}")
    names = groups[name]
    demo.load(*names)
    return {name: getattr(demo, name) for name in names}


def summarize(frames, checksum, allow_empty=False):
    result = {}
    for name, frame in frames.items():
        item = {"rows": frame.height, "columns": frame.width,
                "schema": [(name, str(dtype)) for name, dtype in frame.schema.items()]}
        if "tick" in frame.columns:
            item["tick_min"] = frame["tick"].min()
            item["tick_max"] = frame["tick"].max()
        if checksum:
            item["row_hash_sum"] = int(frame.hash_rows(seed=0).sum() or 0)
            item["ordered_row_hash_sum"] = int(
                frame.with_row_index("__profile_row_order").hash_rows(seed=0).sum() or 0)
        result[name] = item
    if not allow_empty and not any(item["rows"] for item in result.values()):
        raise RuntimeError("workload produced no rows")
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--game", choices=("awpy", "boon"), required=True)
    parser.add_argument("--demo", type=Path, required=True)
    parser.add_argument("--extension-dir", type=Path, required=True)
    parser.add_argument("--workload", required=True)
    parser.add_argument("--segments", type=int, default=1)
    parser.add_argument("--repeat", type=int, default=3)
    parser.add_argument("--warmup", type=int, default=1)
    parser.add_argument("--checksum", action="store_true")
    parser.add_argument("--allocations", action="store_true",
                        help="use an isolated allocation-probe build; timings are diagnostic only")
    args = parser.parse_args()
    if args.segments < 1 or args.repeat < 1 or args.warmup < 0:
        parser.error("segments/repeat must be positive and warmup nonnegative")
    os.environ[f"{args.game.upper()}_TICK_SEGMENTS"] = str(args.segments)
    # Avoid unrelated oversubscription. Parser segmentation is controlled above.
    os.environ.setdefault("POLARS_MAX_THREADS", "4")
    os.environ.setdefault("RAYON_NUM_THREADS", "4")
    name = f"_{args.game}"
    library = args.extension_dir.resolve() / f"lib{name}.so"
    spec = importlib.util.spec_from_file_location(name, library)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load extension: {library}")
    extension = importlib.util.module_from_spec(spec)
    sys.modules[name] = extension
    spec.loader.exec_module(extension)
    probe = None
    if args.allocations:
        from allocation_probe import AllocationProbe
        probe = AllocationProbe(library)
    demo_path = str(args.demo.resolve())
    if args.workload == "info":
        demo = extension.Demo(demo_path)
        if args.game == "awpy":
            fields = ("map_name", "build_num", "demo_version_name", "playback_ticks", "playback_time")
            info = {key: value for key, value in demo.header.items() if key in fields}
            info["tick_rate"] = demo.tick_rate
        else:
            fields = ("build", "map_name", "total_ticks", "total_seconds", "game_mode", "match_id")
            info = {field: getattr(demo, field) for field in fields if hasattr(demo, field)}
        print(json.dumps({"game": args.game, "demo": demo_path, "info": info}), flush=True)
        return
    expected = None
    for iteration in range(-args.warmup, args.repeat):
        gc.collect()
        if probe:
            probe.begin()
        init_start = time.monotonic_ns()
        init_cpu = time.process_time_ns()
        demo = extension.Demo(demo_path)
        init_end = time.monotonic_ns()
        init_cpu_end = time.process_time_ns()
        init_allocations = probe.end() if probe else None
        if probe:
            probe.begin()
        work_cpu = time.process_time_ns()
        work_start = time.monotonic_ns()
        frames = workload(demo, args.game, args.workload)
        work_end = time.monotonic_ns()
        cpu_end = time.process_time_ns()
        work_allocations = probe.end() if probe else None
        # Inspect returned data outside both the timer and the profiler window.
        output = summarize(frames, args.checksum, args.workload == "snapshots_empty")
        if expected is not None and output != expected:
            raise RuntimeError(f"output changed across fresh-parser runs: {output} != {expected}")
        expected = output
        record = {
            "game": args.game, "demo": demo_path, "bytes": args.demo.stat().st_size,
            "workload": args.workload, "segments": args.segments, "iteration": iteration,
            "warmup": iteration < 0, "init_start_ns": init_start, "init_end_ns": init_end,
            "work_start_ns": work_start, "work_end_ns": work_end,
            "init_seconds": (init_end - init_start) / 1e9,
            "work_seconds": (work_end - work_start) / 1e9,
            "init_cpu_seconds": (init_cpu_end - init_cpu) / 1e9,
            "work_cpu_seconds": (cpu_end - work_cpu) / 1e9,
            "process_peak_rss_kib": resource.getrusage(resource.RUSAGE_SELF).ru_maxrss,
            "output": output,
        }
        if probe:
            record["init_allocations"] = init_allocations
            record["work_allocations"] = work_allocations
        print(json.dumps(record), flush=True)
        del frames, demo


if __name__ == "__main__":
    main()
