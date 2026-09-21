# Real-demo profiles

Measured on September 20, 2026. **Field-path decoding is the strongest shared
optimization target.** Name lookup and wire-string scanning were both below
0.1% of sampled CPU time in every tested workload. The synthetic long-string
stress case doesn't describe these four demos.

## Workloads and timings

These calls run real downstream dataset callbacks and build the returned Python
Polars DataFrames. Each measurement uses a fresh `Demo`, warm file caches, one
parser segment, one warmup, and three timed iterations. Times below exclude
constructor and teardown, but include dataset initialization and DataFrame output.

| Demo | Dataset | Rows | Median seconds | Min–max |
| --- | --- | ---: | ---: | ---: |
| CS2 Cache | Snapshots every 64 ticks | 30,273 | 1.223 | 1.222–1.333 |
| CS2 Cache | Snapshots every tick | 1,937,140 | 3.636 | 3.622–3.720 |
| CS2 Cache | Events | 4,293 | 1.396 | 1.396–1.414 |
| CS2 Dust2 | Snapshots every 64 ticks | 28,248 | 1.373 | 1.367–1.379 |
| CS2 Dust2 | Snapshots every tick | 1,807,311 | 3.482 | 3.167–3.493 |
| CS2 Dust2 | Events | 3,594 | 1.384 | 1.372–1.388 |
| Deadlock 100655353 | Player ticks | 1,689,484 | 3.928 | 3.907–4.036 |
| Deadlock 100655353 | Ability changes | 13,898 | 2.916 | 2.865–2.959 |
| Deadlock 100655353 | Combat | 79,573 | 2.893 | 2.888–2.902 |
| Deadlock 103129247 | Player ticks | 1,400,932 | 3.306 | 3.284–3.368 |
| Deadlock 103129247 | Ability changes | 12,603 | 2.366 | 2.352–2.382 |
| Deadlock 103129247 | Combat | 67,464 | 2.433 | 2.411–2.450 |

Event and combat row totals combine their constituent tables. CS2 snapshots
have 38 columns, Deadlock player ticks have 60, and ability changes have nine.
All repeated runs produced stable row counts and tick bounds. Median Awpy
construction took less than 0.2 ms. Boon's medians were about 27–32 ms.

## Where the CPU goes

Ranges below cover the two demos for each workload. These are **inclusive CPU
sample shares**, not wall-time measurements. Don't add columns together.

| Workload | Field-path decoding | Path-to-field resolution | Value decode/skip | Snapshot callbacks | DataFrame construction |
| --- | ---: | ---: | ---: | ---: | ---: |
| CS2 every 64 ticks | 27.4–29.2% | 5.4–5.7% | 12.7–13.2% | 1.1–1.6% | 0.9–1.1% |
| CS2 every tick | 13.4–13.6% | 2.9% | 6.0–6.2% | 23.4–25.4% | 23.0–23.9% |
| CS2 events | 29.6–30.1% | 5.8–6.3% | 12.6–12.9% | n/a | 0.7–0.8% |
| Deadlock player ticks | 27.3–28.5% | 7.4–8.0% | 11.2–11.4% | 22.6–22.7% | 1.9–2.0% |
| Deadlock ability changes | 36.2–37.1% | 9.6–10.2% | 13.5–14.0% | n/a | 0.3% |
| Deadlock combat | 35.5–35.9% | 9.8–10.5% | 13.6–14.1% | n/a | 0.4% |

`n/a` means this isn't a snapshot workload, not that callbacks are free.
Entity-update processing, including its path and value decoding, accounts for
roughly 70–76% of CS2 sampled/event workloads and 65–84% of Deadlock workloads.
Snappy accounts for about 2–8% across the matrix.

Both consumers already cache field keys. Name resolution has at most one sample
in any recording, about 0–0.05%. Wire-string scanning also stays at 0–0.05%.
No samples does **not** mean zero cost. Cached typed field reads are different:
they account for roughly 9% in every-tick player datasets, nested within callback
time. Changing lookup-by-name again is unlikely to improve these workloads.

Awpy's every-tick output is a separate opportunity. `player_states` builds owned
rows, then `states_to_frame` scans them into columns and clones inventory data.
Boon's player snapshots accumulate columns directly. Their schemas differ, so
the 24% versus 2% conversion shares are a lead to investigate, not a controlled
comparison or a promised speedup.

## Four-segment playback

A separate unprofiled sweep compared one and four segments, with one warmup and
three measured iterations each. Checksumming ran outside the timer. Row counts,
tick bounds, and order-independent row-hash sums matched across segment counts
for all six cases. This doesn't check row ordering or prove collision-free equality.

| Workload | Serial median | Four segments | Speedup |
| --- | ---: | ---: | ---: |
| CS2 Cache, every 64 ticks | 1.381 s | 0.431 s | 3.20× |
| CS2 Dust2, every 64 ticks | 1.381 s | 0.438 s | 3.15× |
| CS2 Cache, every tick | 3.553 s | 1.871 s | 1.90× |
| CS2 Dust2, every tick | 3.579 s | 1.823 s | 1.96× |
| Deadlock 100655353, player ticks | 4.005 s | 1.579 s | 2.54× |
| Deadlock 103129247, player ticks | 3.310 s | 1.156 s | 2.86× |

Total process CPU time rises by about 8–17% with four segments. This is a
wall-time win, not less overall work. CPU attribution above is from serial
profiles only. The weaker scaling of CS2 every-tick output is consistent with
its large post-playback conversion cost, but this isn't a parallel stack profile.

## Next experiments

1. **Field-path decoding first.** The current implementation walks a boxed
   Huffman tree one bit at a time. Capture representative opcode mixes and
   path depths in an untimed diagnostic, then add Criterion fixtures for those
   distributions. Compare a compact table or prefix lookup with the current
   decoder. Include truncated tails, every opcode, nesting, and decode limits.
   Existing sequential/nested synthetic paths don't cover the real opcode mix.
2. **Serializer access and path resolution next.** `SerializerContainer::get`
   accounts for roughly 3–6% as the nearest application frame. Investigate
   class-ID-to-serializer caching and repeated path metadata traversal with
   schema-change/reset tests. This is separate from dotted field-name lookup.
3. **Awpy column builders.** Benchmark direct column accumulation against
   `Vec<PlayerState>` plus `states_to_frame`, keeping the same 38-column output.
   Also inspect per-tick inventory/name cloning and repeated weapon metadata
   lookup. Keep public snapshot behavior unchanged.

Keep the existing name and string benchmarks as regression/stress checks, but
don't prioritize new string-scanning code based on the synthetic cases alone.
Any candidate should pass both Criterion comparisons and this real-demo matrix.
No production optimization was made during this investigation.

## Reproduction details and limits

- AMD Ryzen 7 7800X3D, WSL2 Ubuntu, kernel 6.18.33.2, 16 logical CPUs, about
  15 GiB RAM available to WSL. Rust 1.96.1, CPython 3.13.14, Python Polars 1.38.0.
- Release `opt-level=3`, fat LTO, one codegen unit, debug information retained.
  Polars and Rayon defaulted to four threads. No CPU pinning or cold-disk tests.
- Local pbdems2 0.3.0 working tree at `fbbb008d8044`, including the preceding
  uncommitted optimizations. Awpy at `1abb386c2e88`, Boon at `5470cb4ae0cd`.
  Existing downstream edits were preserved. Neither downstream checkout nor
  installed Python package was modified.
- Both consumers normally lock published pbdems2 0.3.1. The private workspace
  patches that dependency to the local working tree. These measurements are
  **not** a before/after comparison against their installed versions.
- The private lockfile pins this diagnostic build. Building both extensions in
  one workspace unifies Polars `dtype-full` and PyO3 `multiple-pymethods` features.
  This differs from building stock Boon alone. Confirm promising changes with
  each consumer's own release build before making release-performance claims.
- Twelve serial CPU profiles, three fresh calls each, 2,102–5,499 in-window
  samples per profile. User-space software CPU sampling at 499 Hz. Samply
  unwound DWARF stacks, LLVM expanded inline frames, and every timestamp was
  checked against perf before selecting dataset-call windows.
- About 6.7–16.7% of leaf symbols remain unresolved. Caller chains are retained,
  so stage attribution includes those samples. Allocation shares are sampling
  estimates, not allocation counts. There are no hardware cache-miss counters.
- Four complete demos cover two CS2 maps and two Deadlock matches, not every
  build, mode, dataset, or machine. Awpy projectiles/stats and Boon world/trooper
  datasets were not included. Three timing samples give exploratory medians
  and ranges, not formal confidence intervals.

| Input | Bytes | Build | Last tick | Duration |
| --- | ---: | ---: | ---: | ---: |
| `ex-mana-vs-bushido-wildcats-m1-cache.dem` | 346,840,106 | 10847 | 176,167 | 2,752.61 s |
| `ex-mana-vs-bushido-wildcats-m3-dust2.dem` | 330,283,089 | 10847 | 164,319 | 2,567.48 s |
| `100655353.dem` | 559,474,986 | 10854 | 156,547 | 2,446.05 s |
| `103129247.dem` | 458,141,549 | 10854 | 130,852 | 2,044.56 s |

SHA-256, in the same order:

```text
91df5a0121f3ca4557e2af50bc28ef1222f829339afc688377e337567f017a58
19971b90e67da96ff805db5cffd99961ae49a8845214c058a34ff7feb22f9b9c
36a0929c5ea68ec31e804dea30c97b941b9148a92ae86f1ce7eeed2f39e51cb6
608316f569eda999e334adfb77b93cc74550c61fc23e9a7bcdbf805abd7ed76f
```

The [harness instructions](README.md) reproduce the runs. Local timing JSON,
recordings, analyzed profiles, and folded stacks are under
`target/real-world-profile/`. Demo bytes and raw profiles aren't checked in.

Validation: both extensions built successfully, all twelve dataset workloads
completed, all six serial/parallel checksum comparisons matched, six analyzer
tests passed, and the existing workspace's 229 nextest tests passed. Rust
formatting, shell syntax, locked workspace metadata, and whitespace checks passed.
