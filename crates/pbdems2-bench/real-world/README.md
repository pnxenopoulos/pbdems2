# Real-demo dataset profiling

This optional Linux harness builds Awpy and Boon's **actual Python extension
sources**, linked to this working copy of pbdems2. It exercises their dataset
callbacks and returns Polars DataFrames. It doesn't replace installed packages
or edit either consumer's source. It builds whatever is in those checkouts.

[RESULTS.md](RESULTS.md) has the first measurements and next optimization targets.
[OPTIMIZATIONS.md](OPTIMIZATIONS.md) records the follow-up experiments and retained changes.
[NATIVE.md](NATIVE.md) checks separate consumer builds, allocation costs, and a
broader demo corpus.

## Build

Expected sibling checkouts are `../awpy-github/awpy` and `../boon`. You'll also
need local demos and a Python environment compatible with both extensions and
Polars. The recorded run used Boon's Python 3.13 environment.

Run from the pbdems2 root:

```bash
export PROFILE_PYTHON="$PWD/../boon/crates/boon-python/.venv/bin/python"
export PROFILE_LIBS="$PWD/target/real-world-profile/build/release"
PYO3_PYTHON="$PROFILE_PYTHON" cargo build \
  --manifest-path crates/pbdems2-bench/real-world/Cargo.toml \
  --workspace --release --locked --target-dir target/real-world-profile/build -j2
```

The private workspace and lockfile keep game protobufs out of pbdems2's normal
dependency graph. Bridge manifests point directly at downstream source files.
Keep their dependency lists in sync when those sources change. Building both
extensions together unifies Cargo features, including Awpy's Polars `dtype-full`
and Boon's PyO3 `multiple-pymethods`. These are diagnostic builds, not stock wheels.
Release optimization matches both consumers, with debug information retained.

## Separate consumer builds

Use `prepare_native.py` to keep each consumer's own lockfile, feature set, and
release settings. It copies tracked and unignored crate files into a new
directory under `target/`, without editing the consumer checkout:

```bash
python3 crates/pbdems2-bench/real-world/prepare_native.py \
  --game awpy --repo ../awpy-github/awpy \
  --output target/native-awpy --kind release
```

Run the printed Cargo commands with `PYO3_PYTHON` set to your profiling Python.
Only the copied lockfile changes to select local pbdems2. Repeat for Boon with
`--game boon --repo ../boon` and a different output directory. Confirm that
`cargo tree` with the printed manifest/config reports the intended local core.
`--core PATH` selects a different core checkout for a baseline.

Save the resulting `lib_awpy.so` and `lib_boon.so` in baseline/candidate
directories for `compare.py`. Keep dependencies and features identical within
each pair. Don't rebuild or overwrite libraries while they are being measured.

`--kind symbols` retains debug information for CPU profiles. Use the ordinary
`release` build for timing. `--kind allocations` installs the private allocation
probe and tags selected snapshot functions in the copied source only.

### Allocation diagnostics

Pass `--allocations` to `datasets.py` with an allocation build. It reports separate
constructor and dataset windows, including allocation/reallocation counts,
requested bytes, and peak live requested bytes. Three probe tests cover
reallocation, concurrent accounting, window resets, and scope restoration.

These are Rust allocation requests from that extension, not Python's heap,
allocator overhead, fragmentation, or mapped demo pages. Reallocation totals
include the full new size. Peak accounting observes allocator-call boundaries,
not temporary storage inside the allocator. Underflows invalidate the result.

Scopes distinguish untagged work, snapshot extraction, column growth/merging,
and DataFrame construction. Boon's extraction scope also fills its columns.
Tags are thread-local and don't automatically propagate into internal Polars
workers. Untagged allocations remain in `other`. Never use these instrumented
runs as performance timings.

## Measure

```bash
"$PROFILE_PYTHON" crates/pbdems2-bench/real-world/datasets.py \
  --game awpy --demo /path/to/cs2.dem --extension-dir "$PROFILE_LIBS" \
  --workload snapshots_1 --segments 1 --repeat 3 --warmup 1 --checksum
```

| Game | Workloads |
| --- | --- |
| Awpy | `snapshots_64`, `snapshots_1`, `events`, `projectiles`, `stats` |
| Boon | `player_ticks`, `ability_ticks`, `combat` |
| Either | `info` for non-player demo metadata |

`events` loads kills, damages, bomb, blinds, and shots together. `combat` loads
kills, damage, and abilities together. Ability ticks are change-only rows.

Every iteration creates a fresh `Demo`, so cached dataset results can't turn
later calls into no-ops. JSON lines contain initialization and dataset-call
times, CPU times, row counts, tick bounds, and monotonic profiler windows.
Construction, output inspection, checksumming, and destruction are outside the
dataset timer. OS file caches stay warm. Peak RSS covers the entire process,
including mapped files and previous iterations, not just dataset allocations.

The runner checks row counts, tick bounds, and ordered schemas within each
process. `--checksum` adds both a sum of Polars row hashes and a position-sensitive
hash sum. Use the same Polars version for comparisons. These detect value and
ordering changes but aren't collision-free proofs.

To compare saved baseline and candidate extension directories in fresh,
alternating processes:

```bash
python3 crates/pbdems2-bench/real-world/compare.py \
  --python "$PROFILE_PYTHON" --baseline /path/to/baseline --candidate "$PROFILE_LIBS" \
  --awpy-demo "$CS2_A" --awpy-demo "$CS2_B" \
  --boon-demo "$DEADLOCK_A" --boon-demo "$DEADLOCK_B" \
  --rounds 5 --segments 1 4 > comparison.jsonl
```

Each directory needs `lib_awpy.so` and/or `lib_boon.so`. Save the baseline before
rebuilding the candidate with the same profile and features. The script fails on
output differences. Repeat `--workload NAME` to narrow the matrix.

Awpy also has `snapshot_single`, `snapshots_ticks`, `snapshots_window`, and
`snapshots_empty` selector checks. Request them explicitly with `--workload`.

For the full matrix, pass two CS2 and two Deadlock demos:

```bash
bash crates/pbdems2-bench/real-world/run-timings.sh \
  "$CS2_A" "$CS2_B" "$DEADLOCK_A" "$DEADLOCK_B" > timings.jsonl
bash crates/pbdems2-bench/real-world/run-segments.sh \
  "$CS2_A" "$CS2_B" "$DEADLOCK_A" "$DEADLOCK_B" > segments.jsonl
```

The scripts run sequentially. Don't build, profile, or run other benchmarks
alongside them. Three measured iterations are a quick investigation, not a
high-confidence regression threshold. Increase repeats for a paired change.
The runner defaults Polars and Rayon to four threads and parser segments to one.

`real-world-control awpy|boon all|players DEMO` is an optional Rust decode-only
control with a counting callback. Its filters and timing boundary differ from
the dataset APIs, so subtracting its time doesn't isolate callback cost.

## Snapshot output benchmark

The private `awpy-output-bench` package compares the original 38-column row-scan
conversion with Awpy's owned column builder. It compiles the production builder
from the sibling checkout. Cases use 10,000, 100,000, and 1,000,000 rows, with
preallocated and growing columns. Row cloning is outside the timer. Consuming
the input rows is inside it, and dropping the returned DataFrame is outside.

```bash
cargo criterion --manifest-path crates/pbdems2-bench/real-world/Cargo.toml \
  -p awpy-output-bench --bench snapshots --locked \
  --target-dir target/real-world-profile/build
cargo test --manifest-path crates/pbdems2-bench/real-world/Cargo.toml \
  -p awpy-output-bench --lib --release --locked \
  --target-dir target/real-world-profile/build
```

Tests compare exact values, column order, types, nulls, empty results, chunk
merging, Unicode, and nonfinite floats. This workspace needs the sibling
checkouts and isn't part of pbdems2's normal dependency graph or CI.

The row_construction target adds 18 profile-guided cases:

- Inventory strings for empty, pistol, full, and sparse loadouts, using Awpy's
  real weapon lookup. Controls try lazy 64-byte reservation and prebound metadata.
- Fresh, reused, and absent per-tick row buffers for 10,000 and 100,000 rows.

Inventory cases model the work after handle resolution. They include dropping
the returned string. Prebinding is outside the timer and excludes runtime cache
lookup, so it is a best-case control, not a measured cache implementation.
Row cases drain a borrowed fixture buffer. Input cloning, fixture-buffer
destruction, output destruction, and DataFrame conversion are outside the timer.
They isolate dispatch into the production column builder, not entity extraction.
Tests check all columns, partial ticks, duplicate grenades, unknown classes,
knife variants, and inventory order.

Run with the same cargo criterion command above and --bench row_construction.
Check allocator-sensitive cases in fresh processes too, using an exact filter
such as awpy_tick_row_dispatch/fresh/100000$. A narrow confidence interval within
one process does not establish repeatability across processes.

For untimed allocation counts and retained inventory capacities:

    cargo run --manifest-path crates/pbdems2-bench/real-world/Cargo.toml \
      -p awpy-output-bench --example row_allocations --release --locked \
      --target-dir target/real-world-profile/build

## Profile

Use `perf`, [Samply](https://github.com/mstange/samply), and `llvm-symbolizer`.
The recorded run used perf 5.15.209, Samply 0.13.1, and LLVM symbolizer 15.
Set these variables to executable paths:

```bash
export PERF=perf SAMPLY=samply LLVM_SYMBOLIZER=llvm-symbolizer
export PROFILE_OUT="$PWD/target/real-world-profile/profiles"
bash crates/pbdems2-bench/real-world/run-profiles.sh \
  "$CS2_A" "$CS2_B" "$DEADLOCK_A" "$DEADLOCK_B"
bash crates/pbdems2-bench/real-world/analyze-profiles.sh
```

Recording uses user-space CPU-clock samples at 499 Hz, the monotonic clock,
and 16 KiB DWARF stacks. Samply unwinds the saved stacks. LLVM resolves symbols
and inline frames in batches. This avoids truncated Rust stacks and slow inline
symbolization seen with the older perf tool alone.

Set `PROFILE_SEGMENTS="1 4"` to include parallel snapshot profiles.
`PROFILE_REPEATS` defaults to three. Each recording's filename includes its
segment count.

The analyzer verifies **every** imported sample timestamp against perf, then
weights samples by their recorded period inside dataset-call windows. Inclusive
categories overlap and must not be added. Disjoint stage estimates separate
snapshot rows, column accumulation, and DataFrame work from playback plus other callbacks.
Folded stacks are also saved for local flamegraph tools. Raw profiles can contain
local paths, so keep them private. Nothing is uploaded or opened in a browser.

```bash
python3 -m unittest discover -s crates/pbdems2-bench/real-world -p 'test_*.py'
cargo fmt --manifest-path crates/pbdems2-bench/real-world/Cargo.toml \
  -p real-world-control -p awpy-output-bench -p pbdems2-allocation-probe --check
```
