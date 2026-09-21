"""Summarize perf samples using Samply's unwinder and LLVM inline symbols."""
import argparse
from collections import Counter, defaultdict
import gzip
import json
from pathlib import Path
import re
import subprocess

HEADER = re.compile(r"^\s*\S+\s+\d+/\d+\s+(\d+)\.(\d+):\s+(\d+)\s+cpu-clock:u:")
GROUPS = {
    "field_paths": ("pbdems2::entity::field_path::",),
    "field_path_resolution": ("entities::resolve_field_path",),
    "value_decode_skip": ("field_decoder::Decoder::decode", "field_decoder::Decoder::skip"),
    "wire_string_scan": ("BitReader::read_string", "BitReader::skip_string"),
    "name_resolution": ("Serializer::resolve_field_key", "Serializer::resolve_parts"),
    "cached_field_reads": ("Entity::field_value", "Entity::get_i64", "Entity::get_u32",
                           "Entity::get_u64", "Entity::get_f32", "Entity::get_bool",
                           "Entity::get_qangle", "Entity::get_vector3", "Entity::get_handle",
                           "Entity::get_bytes", "Entity::get_str", "Entity::get_string"),
    "entity_updates": ("Entity::apply_update", "Entity::skip_update",
                       "EntityContainer::handle_packet_entities", "EntityContainer::handle_update"),
    "snappy": ("snap::decompress::",),
    "awpy_snapshot_rows": ("::player_states",),
    "awpy_snapshot_columns": ("::snapshot_columns::",),
    "boon_snapshot_rows": ("::snapshots::PtCols::collect_tick",
                           "::snapshots::SegSnap::collect_tick", "::snapshots::SegSnap::update"),
    "dataframe": ("polars_core::", "polars_arrow::", "polars_compute::",
                  "polars_ops::", "pyo3_polars::", "into_dataframe", "states_to_frame",
                  "SnapshotColumns::into_frame"),
    "allocation": ("malloc", "cfree", "realloc", "__rust_alloc", "__rust_dealloc",
                   "alloc::alloc::", "alloc::raw_vec::"),
}


def perf_periods(text):
    result = {}
    for line in text.splitlines():
        if match := HEADER.match(line):
            sec, frac, period = match.groups()
            timestamp = int(sec) * 1_000_000_000 + int(frac.ljust(9, "0"))
            if timestamp in result:
                raise ValueError("ambiguous duplicate sample timestamp")
            result[timestamp] = int(period)
    if not result:
        raise ValueError("no cpu-clock:u samples found")
    return result


def json_values(text):
    """LLVM emits one JSON object per input address (or an array for CLI args)."""
    decoder = json.JSONDecoder()
    while text := text.lstrip():
        value, end = decoder.raw_decode(text)
        yield from value if isinstance(value, list) else [value]
        text = text[end:]


def symbolize(profile, executable):
    addresses = defaultdict(set)
    for thread in profile["threads"]:
        for frame, address in enumerate(thread["frameTable"]["address"]):
            func = thread["frameTable"]["func"][frame]
            resource = thread["funcTable"]["resource"][func]
            if resource >= 0:
                lib = thread["resourceTable"]["lib"][resource]
                if lib is not None and address >= 0:
                    addresses[lib].add(address)
    symbols = {}
    for lib, values in addresses.items():
        path = profile["libs"][lib]["path"]
        if not Path(path).is_file():
            continue
        # One invocation per ELF, not per address. Retain inline call chains.
        result = subprocess.run(
            [str(executable), f"--obj={path}", "--output-style=JSON", "--inlines", "--demangle"],
            input="".join(f"0x{address:x}\n" for address in sorted(values)),
            text=True, capture_output=True, check=True,
        )
        for entry in json_values(result.stdout):
            frames = []
            for symbol in entry["Symbol"]:
                name = re.sub(r"::h[0-9a-f]{16}$", "", symbol["FunctionName"])
                frames.append((name or "[unknown]", symbol["FileName"], symbol["Line"]))
            symbols[(lib, int(entry["Address"], 16))] = frames
    return symbols


def frame_symbols(thread, frame, profile, symbols):
    table = thread["frameTable"]
    func = table["func"][frame]
    resource = thread["funcTable"]["resource"][func]
    if resource >= 0:
        lib = thread["resourceTable"]["lib"][resource]
        if (lib, table["address"][frame]) in symbols:
            return symbols[(lib, table["address"][frame])]
    name = thread["stringArray"][thread["funcTable"]["name"][func]]
    return [(name if not name.startswith("0x") else "[unknown]", "", 0)]


def summarize(profile, periods, windows, symbols):
    origin = min(periods)
    total = 0
    count = 0
    unknown = 0
    groups = Counter()
    phases = Counter()
    leaf = Counter()
    inclusive = Counter()
    application_leaf = Counter()
    depths = Counter()
    folded = Counter()
    seen = set()
    for thread in profile["threads"]:
        table = thread["samples"]
        frame_cache = {}
        for time_ms, stack in zip(table["time"], table["stack"]):
            timestamp = origin + round(time_ms * 1_000_000)
            # Samply imports perf with its first sample as the zero point.
            # Verify every timestamp against perf instead of assuming alignment.
            if timestamp not in periods or timestamp in seen:
                raise ValueError(f"Samply/perf sample mismatch: {timestamp}")
            seen.add(timestamp)
            if not any(start <= timestamp < end for start, end in windows):
                continue
            period = periods[timestamp]
            total += period
            count += 1
            frames = []
            depth = 0
            while stack is not None:
                frame = thread["stackTable"]["frame"][stack]
                if frame not in frame_cache:
                    frame_cache[frame] = frame_symbols(thread, frame, profile, symbols)
                frames.extend(frame_cache[frame])
                stack = thread["stackTable"]["prefix"][stack]
                depth += 1
            depths[depth] += 1
            names = [name for name, _, _ in frames]
            if not names or names[0] == "[unknown]":
                unknown += period
            leaf[names[0] if names else "[missing]"] += period
            application = next((name for name in names if name.startswith(
                ("pbdems2::", "awpy::", "boon_parser::", "_awpy::", "_boon::"))), "[other]")
            application_leaf[application] += period
            for name in set(names):
                inclusive[name] += period
            matched = {group for group, patterns in GROUPS.items()
                       if any(pattern in name for name in names for pattern in patterns)}
            for group in matched:
                groups[group] += period
            # Disjoint stage estimates. Callback/DF samples outrank parser ancestors.
            if "dataframe" in matched:
                phase = "dataframe"
            elif "awpy_snapshot_columns" in matched:
                phase = "snapshot_columns"
            elif matched & {"awpy_snapshot_rows", "boon_snapshot_rows"}:
                phase = "snapshot_rows"
            elif any(name.startswith("pbdems2::") for name in names):
                phase = "parser_and_other_callbacks"
            else:
                phase = "other"
            phases[phase] += period
            folded[";".join(reversed(names))] += period
    if seen != set(periods):
        raise ValueError("Samply dropped or added samples relative to perf")
    if not count:
        raise ValueError("no samples matched the dataset windows")
    pct = lambda value: round(100 * value / total, 3)
    ranked = lambda values: [{"symbol": k, "percent": pct(v)} for k, v in values.most_common(40)]
    return {
        "samples": count, "sampled_cpu_seconds": total / 1e9,
        "unknown_leaf_percent": pct(unknown),
        "inclusive_percent": {k: pct(groups[k]) for k in GROUPS},
        "disjoint_stage_percent": {k: pct(v) for k, v in phases.items()},
        "top_leaf_percent": ranked(leaf), "top_inclusive_percent": ranked(inclusive),
        "nearest_application_frame_percent": ranked(application_leaf),
        "physical_stack_depth_counts": dict(sorted(depths.items())),
    }, folded


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", type=Path, required=True)
    parser.add_argument("--perf-data", type=Path, required=True)
    parser.add_argument("--perf", type=Path, default=Path("perf"))
    parser.add_argument("--symbolizer", type=Path, default=Path("llvm-symbolizer"))
    parser.add_argument("--windows", type=Path, required=True)
    parser.add_argument("--phase", choices=("init", "work"), default="work")
    parser.add_argument("--folded", type=Path)
    args = parser.parse_args()
    opener = gzip.open if args.profile.suffix == ".gz" else open
    with opener(args.profile, "rt") as stream:
        profile = json.load(stream)
    raw = subprocess.run([str(args.perf), "script", "--no-inline", "-G", "--ns",
                          "-i", str(args.perf_data), "-F", "comm,pid,tid,time,period,event"],
                         text=True, capture_output=True, check=True)
    periods = perf_periods(raw.stdout)
    records = [json.loads(line) for line in args.windows.read_text().splitlines()]
    windows = [(r[f"{args.phase}_start_ns"], r[f"{args.phase}_end_ns"])
               for r in records if not r["warmup"]]
    result, stacks = summarize(profile, periods, windows, symbolize(profile, args.symbolizer))
    if args.folded:
        args.folded.write_text("".join(f"{stack} {period}\n" for stack, period in stacks.items()))
    print(json.dumps(result, indent=2))


if __name__ == "__main__":
    main()
