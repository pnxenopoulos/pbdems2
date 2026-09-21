# Separate-build follow-up

These historical measurements predate the final 0.3.1 safety reconciliation.
See the [0.3.2 release checks](../RELEASE_0.3.2.md) for the final candidate.

The earlier gains hold with each consumer's own release build. This pass adds
CPU profiles, Rust allocation accounting, a larger demo corpus, and focused
Awpy row-construction benchmarks. It makes no further production parser changes.

## Normal release timings

Each consumer was copied into isolated baseline and candidate workspaces.
Their own manifests, lockfiles, features, and release settings were retained.
Only the local pbdems2 override changed dependency resolution. The resulting
lockfiles are byte-identical within each baseline/candidate pair.

The baseline predates the three changes in [OPTIMIZATIONS.md](OPTIMIZATIONS.md):
the prefix decoder, class bindings, and Awpy's owned snapshot columns.
The candidate includes all three. Boon's source is identical in both builds.
These are native-workspace extension builds, not packaged wheels.

Awpy enables Polars dtype-full. Boon keeps its narrower Polars features and
PyO3 multiple-pymethods. Unlike the original diagnostic workspace, these builds
do not unify features across consumers. Timings use uninstrumented release
binaries with optimization level 3, fat LTO, and one codegen unit.

Five alternating fresh-process pairs per case, with one initial warmup per
variant. Seconds are medians. Reductions are median paired time reductions,
so they need not equal the ratio of the two displayed medians.

| Demo / workload | Segments | Baseline → candidate | Reduction |
| --- | ---: | ---: | ---: |
| Cache, every-tick snapshots | 1 | 3.059 → 2.533 s | 17.0% |
| Cache, every-tick snapshots | 4 | 1.770 → 1.215 s | 29.8% |
| Dust2, every-tick snapshots | 1 | 2.937 → 2.462 s | 16.5% |
| Dust2, every-tick snapshots | 4 | 1.759 → 1.193 s | 32.1% |
| Cache, snapshots every 64 ticks | 1 | 1.218 → 1.149 s | 5.4% |
| Dust2, snapshots every 64 ticks | 1 | 1.198 → 1.131 s | 5.7% |
| Cache, events | 1 | 1.224 → 1.165 s | 4.3% |
| Dust2, events | 1 | 1.204 → 1.145 s | 5.0% |
| 100655353, player ticks | 1 | 3.387 → 3.170 s | 6.2% |
| 100655353, player ticks | 4 | 1.501 → 1.412 s | 5.1% |
| 103129247, player ticks | 1 | 2.814 → 2.634 s | 6.5% |
| 103129247, player ticks | 4 | 1.212 → 1.136 s | 6.8% |
| 100655353, ability ticks | 1 | 2.465 → 2.273 s | 7.9% |
| 103129247, ability ticks | 1 | 2.044 → 1.874 s | 8.4% |
| 100655353, combat | 1 | 2.468 → 2.283 s | 7.5% |
| 103129247, combat | 1 | 2.046 → 1.889 s | 7.9% |

Four-segment sampled snapshots improved by 6.1% on Cache and 4.5% on Dust2.
One Dust2 pair was 0.4% slower. Every-tick Awpy pairs varied more widely:
6–21% serial and 20–34% parallel. Treat these as local measurements, not
portable regression thresholds.

## Remaining CPU costs

Fresh symbol-enabled candidate builds use the same native feature sets.
Three dataset calls per recording, 499 Hz CPU-clock sampling, with every
imported timestamp checked against perf. Percentages weight sample periods.

| Candidate workload | Segments | Path decoding¹ | Field resolution¹ | Snapshot rows² | Column growth/merge² | DataFrame² |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Awpy every tick | 1 | 16.3% | 4.0% | 26.4% | 8.0% | 12.0% |
| Awpy every tick | 4 | 15.2% | 3.4% | 26.7% | 8.6% | 9.3% |
| Awpy every 64 ticks | 1 | 29.9% | 6.1% | 1.2% | 0.2% | 1.2% |
| Boon player ticks | 1 | 30.9% | 8.1% | 22.1% | 0.0% | 2.1% |
| Boon player ticks | 4 | 28.0% | 7.7% | 23.2% | 0.0% | 2.1% |
| Boon combat | 1 | 37.3% | 10.8% | n/a | n/a | 0.3% |

¹ Inclusive parser categories overlap. Do not add them.
² Disjoint output stages. Boon's player extraction also fills its columns.

These profiles contain 1,690–4,925 in-window samples each. They use Cache and
100655353, not the whole corpus. Awpy's weapon lookup is the nearest application
frame for about 4.4–4.7% of samples. Name-based field lookup is below 0.1%.
The next core investigation remains field-path metadata traversal. Awpy's
inventory construction is the more focused downstream opportunity.

## Allocation and memory tradeoffs

A separate diagnostic build counts successful Rust allocation requests through
System. Nothing is installed in production. Medians of two fresh calls after
one warmup, with zero accounting underflows and matching output checksums.

| Candidate workload | Rows | Peak live requested bytes, 1 segment | 4 segments |
| --- | ---: | ---: | ---: |
| Awpy every tick | 1,937,140 | 583.7 MiB | 819.3 MiB |
| Boon player ticks | 1,689,484 | 613.4 MiB | 826.4 MiB |

Four segments increased these peaks by about 40% and 35%, respectively.
Awpy's row extraction made 3.11 million allocation requests and 3.50 million
reallocation requests. Column accumulation made only 38 allocations plus
712 reallocations serially. This supports investigating row construction,
but does not identify each allocation's call site.

These numbers exclude input mmap pages, Python's heap, allocator overhead,
fragmentation, and temporary storage inside realloc. They are not total process
RAM. Scope tags do not propagate into internal Polars worker threads. Probe
timings are not used for speed comparisons.

## Broader output validation

All 132 measured baseline/candidate pairs passed row-count, ordered schema,
tick-bound, value-hash, and position-sensitive hash checks. Serial and
four-segment outputs matched too. All 36 CPU/probe output records matched
normal builds. Hash checks are strong smoke tests, not collision-free proofs.

The expanded matrix adds two pairs per case for compatibility, not precise
performance estimates:

| Added demo | Build / mode | Result |
| --- | --- | --- |
| CS2 Mirage | 10847 | All five cases match |
| Deadlock 96850353 | 10854, mode 1 | All four cases match |
| Deadlock 103225230 | 10854, mode 1 | All four cases match |
| Deadlock 97104148 | 10854, Street Brawl | All four cases match |
| Deadlock 70537442 | 10725, Street Brawl | All four cases match |

All three CS2 demos are build 10847. Another CS2 build is still needed.
The existing CI matrix covers Linux, macOS, Windows, and the MSRV. This host
only ran Linux/WSL checks. No remote workflows, commits, or releases were run.

## Focused row-construction experiments

Added 18 Criterion cases and two exact-output tests. These are benchmark-only
controls, not further changes to Awpy or pbdems2. The allocation example uses
the same controls in a separate untimed binary.

Inventory cases start after entity-handle resolution. They use the real weapon
lookup and model the remaining loadout-building work. Criterion typical
estimates, 50 samples, one-second warmup, three-second measurement:

| Loadout | Current | Reserve 64 bytes | Prebound metadata + reserve |
| --- | ---: | ---: | ---: |
| Empty | 18.4 ns | 18.3 ns | 18.0 ns |
| Pistol | 73.3 ns | 66.8 ns | 25.1 ns |
| Full | 212.7 ns | 190.2 ns | 68.2 ns |
| Sparse / unknown slots | 142.4 ns | 132.2 ns | 27.1 ns |

Reservation saves 7–11% on the nonempty cases, but retains more string capacity:
16 → 64 bytes for pistol/sparse, and 104 → 128 bytes for full. Empty and unknown
loadouts still allocate nothing. This is not a good default to ship blindly.
Prebinding suggests lookup headroom, but excludes cache construction and runtime
class-ID lookup. Its 66–81% improvement is not an end-to-end speedup.

Row-dispatch timings from the final full sweep:

| Rows | Fresh per-tick vector | Reused vector | Direct emission |
| --- | ---: | ---: | ---: |
| 10,000 | 0.556 ms | 0.494 ms | 0.404 ms |
| 100,000 | 19.774 ms | 19.250 ms | 18.205 ms |

The 100,000-row case is sensitive to the benchmark process. Pilot full-sweep and
isolated-process runs ranged from roughly 6–20 ms and even reversed the ranking.
The 10,000-row estimates were much steadier. During this audit, the final harness
was tightened to keep synthetic input-buffer destruction outside the timer.
That improves the measurement boundary but does not prove the cause of the
variation or establish cross-process repeatability. Do not turn these larger
synthetic estimates into a promised speedup.

The row-buffer diagnostic processes 100,000 already-built rows, ten per tick,
into the actual production column builder:

| Dispatch | Allocations | Reallocations | Cumulative requested bytes |
| --- | ---: | ---: | ---: |
| Fresh per-tick vector | 10,038 | 20,560 | 137,214,960 |
| Reused per-tick vector | 39 | 562 | 65,542,128 |
| Direct column emission | 38 | 560 | 65,534,960 |

These counts include column growth, but exclude creating the input rows and
converting columns into a DataFrame. Requested bytes are cumulative, not peak
memory. The existing output tests and new dispatch test compare all 38 columns.

The next production experiment should compare removing or reusing Awpy's
temporary per-tick row buffer, with native paired timings and allocation checks.
Fewer requests alone are not enough to retain a change.
Keep the public row-returning API intact. A class-ID weapon metadata cache is a
second candidate, with per-demo invalidation and unknown-class fallback.
For the shared core, investigate path-to-field metadata traversal next.

## Checks

- 239 workspace tests, five exact-output tests, three allocator tests, and ten
  Python harness tests passed.
- Strict Clippy passed for the root workspace and new private Rust targets.
- Rust 1.88 checked every root workspace target with all features.
- Formatting, shell syntax, and whitespace checks passed.
- Both isolated-workspace preparation paths were smoke-tested.

## Reproduce

See [README.md](README.md) for isolated build, timing, CPU-profile, and allocation
commands. The host is a Ryzen 7 7800X3D under WSL2, with Rust 1.96.1,
Python 3.13.14, and Python Polars 1.38.0. Polars and Rayon each use four threads.
Measurements run sequentially with warm file caches and no concurrent builds.

Local evidence is under target/native-profile: paired.jsonl, expanded.jsonl,
summary.json, allocations.jsonl, row-final.jsonl, row-allocations-final.log, profiles/,
and the preserved build snapshots. Expanded demo hashes are saved in
expanded-demo-sha256.txt. Starting Git revisions and the original four-demo
identities are in [RESULTS.md](RESULTS.md).
These are dirty working-tree comparisons, not release tags.
Pilot row measurements remain in row-bench.jsonl, row-confirm.jsonl, and
row-isolated-*.jsonl. They use the earlier owned-input timing boundary and
must not be treated as a before/after optimization comparison with the final run.
