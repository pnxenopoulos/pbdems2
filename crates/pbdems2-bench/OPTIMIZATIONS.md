# Optimization follow-up

Measured on September 20, 2026, after the [first refactor](RESULTS.md).
Same Ryzen 7 7800X3D, WSL2, Rust 1.96.1, Criterion 0.8.2, and cargo-criterion
1.1.0. The bench profile and `serde`/`mmap` features are unchanged.

We added measurements before changing code, then checked the wider workloads.
This pass reached 160 cases across ten targets, with 32 added.
The matched comparison below covers 64 cases, not a remeasurement of all 160.
The later [lookup investigation](LOOKUP.md) adds broader lookup and mixed
entity-pipeline benchmarks and revisits the regression recorded here.

## What changed

- Unaligned `BitReader::read_bytes` combines adjacent bytes using safe slice
  iterators instead of calling the general bit extractor for each output byte.
  Aligned reads still use a direct slice copy. There is no handwritten SIMD.
- `EntityContainer` stores one occupied-slot count. Inserts, replacements,
  removals, and truncation maintain it. Dormant entities still count, skipped
  classes do not, and clones keep independent counts. This adds one `usize`
  per container, not per entity.
- Repeated field lookup now has like-for-like benchmarks and API guidance.
  No hidden name cache was added. Serializer fields are publicly mutable,
  so such a cache would need a clear invalidation contract.

The public API, wire decoding, and lifecycle event semantics are unchanged.
No dependencies were added.

## Results

| Case | Before | After |
| --- | ---: | ---: |
| `bulk_read/offset_3/65536` | 43.960 µs | 2.515 µs |
| `packet_messages/payload_or_copy/4096` | 361.669 µs | 26.903 µs |
| `packet_messages/payload_or_copy/16` | 4.401 µs | 3.015 µs |
| `entity_occupancy/len/dense_16384` | 8.598 µs | 0.420 ns |
| `packet_entities/decode_creates_512x16` | 163.068 µs | 163.263 µs |
| `packet_entities/decode_updates_512x16` | 131.415 µs | 131.148 µs |
| `playback/fresh_run/raw` | 2.127 ms | 2.161 ms |
| `playback/prepared_run/raw` | 2.151 ms | 2.140 ms |

Large unaligned reads were about **17.5× faster**. The independent byte-copy-only
probe measured 2.512 µs, close to the final 2.515 µs result. Extracting 256 packet
payloads of 4 KiB each was **13.4× faster**. Short copies improved too, while the
aligned 64 KiB control stayed around 1.02 µs.

Entity counts are now constant-time loads at roughly the benchmark-loop
overhead. The sub-nanosecond result is not a realistic application latency claim
or a reason to advertise a huge speedup multiplier.

Entity creates and updates changed by less than 0.3%. Fresh playback was 1.6%
slower and prepared playback 0.5% faster. This pass does not establish a
whole-demo parsing speedup.

### Field keys: a consumer optimization

Reading eight fields from 64 entities took **75.26 µs by name**, **1.07 µs with
cached keys**, and **2.26 µs when resolving once per tick**. Building the eight-key
cache cost **1.16 µs** and is excluded from the cached-read case. The fixtures
assert that both access paths return the exact same borrowed values.

This is an advantage of the existing key API, not a new library speedup.
Resolve keys once per serializer layout in Awpy/Boon, then use `field_value`
or typed accessors across entities and ticks. Rebuild those keys if the schema
changes.

The first pair also showed slower name lookups, ranging from 4.9% to 13.4%,
despite unchanged lookup logic. These results remain visible below rather than
being called noise or omitted.

### Repeated check and isolation

A second before/after pair reproduced the slower name lookups: +5.2%, +12.3%,
and +5.3% for 16, 64, and 128 fields. The 64-entity name-based sample was 7.2%
slower. This is a repeatable regression in this build, not dismissed as noise.

We then built each optimization separately, using the same timing settings:

| Case | Neither change | Byte copy only | Count only | Both |
| --- | ---: | ---: | ---: | ---: |
| `matched_field_lookup/by_name/64` | 167.78 ns | 176.75 ns | 177.93 ns | 188.42 ns |
| `matched_field_lookup/cached_key/64` | 1.79 ns | 1.80 ns | 1.78 ns | 1.81 ns |
| `matched_field_lookup/by_name/128` | 233.41 ns | 237.68 ns | 247.67 ns | 245.76 ns |
| `sampled_fields/resolve_per_entity` | 69.658 µs | 72.710 µs | 72.487 µs | 74.706 µs |

Both isolated builds showed some of the slowdown even though the timed lookup
logic is unchanged. Compiler/code-layout effects are a possibility, not a
confirmed cause. A sampling or assembly-level profile is the next step, not
an extra cache or inline annotation guessed into the code.

Both optimizations are retained for their targeted gains, with this trade-off
recorded. Cached-key reads and entity-decode controls were much less affected.
Applications that repeatedly resolve names should remeasure their workload.
The repeat and isolation outputs are
`target/benchmark-results/lookup-{repeat-before,repeat-after,copy-only,count-only}.jsonl`.

## How to reproduce

The baseline is the working tree after the first refactor, with the new
benchmark harness added but before these two production changes. It is not the
older `fbbb008` library. Both runs used the same harness, toolchain, features,
profile, 50 samples, one-second warm-up, and three-second measurement target.
They ran sequentially, with no concurrent tests or compilation.

Run this before and after a change, using a distinct history ID and output file:

```bash
mkdir -p target/benchmark-results
cargo criterion -p pbdems2-bench --locked \
  --plotting-backend disabled --message-format=json \
  --history-id investigation-after -- \
  'entity_occupancy/|matched_field_lookup/|sampled_fields/|bulk_read|packet_messages/|packet_entities/|filtered_entities/|string_table_decode/|playback/(fresh_run|prepared_run)/raw' \
  --warm-up-time 1 --measurement-time 3 --sample-size 50 \
  > target/benchmark-results/investigation-after.jsonl
```

Raw output from this investigation is in
`target/benchmark-results/investigation-{before,after}.jsonl`.
The isolated byte-copy probe is in `byte-copy-probe.jsonl`.
Those generated files are ignored by Git. The summary below keeps the measured
values in the repository.

These are warm-cache synthetic workloads, not CS2 or Deadlock parsing rates.
Field lookup uses flat boolean fields, and playback excludes game protobuf
decoding and dataset output. The machine is an unpinned desktop VM.
Confidence intervals describe variation within a run, not between-run host
activity or code-layout effects. Treat small differences as leads to repeat,
not proof of a lasting improvement or regression.

## Next investigations

Items 1 and 3 are covered by the [next investigation](LOOKUP.md).

1. Profile the repeatable name-lookup regression before changing name lookup
   again. The new matched cases give it a reproducible starting point.
2. Profile Awpy and Boon with representative demos and dataset callbacks.
   Use the new key-cache benchmarks to separate consumer lookup costs from
   entity decoding before adding more parser machinery.
3. Expand entity-delta fixtures to nested paths, mixed field types, and
   low/high update density. The current boolean fixtures are a useful control,
   but not enough to justify a field-path or field-map redesign.
4. Measure sparse traversal in those real workloads. `len` no longer scans,
   but `iter` still does. An occupied-index list would speed some scans while
   adding mutation and ordering costs, so it needs evidence before changing
   the container.

## Validation

- 224 tests passed with nextest. The new randomized byte-copy and occupancy
  checks also passed with 4,096 cases each.
- All 160 benchmark cases passed their untimed smoke tests.
- Formatting, strict Clippy, Rust 1.88, minimal features, rustdoc, and doctests
  passed. One existing doctest is intentionally ignored.
- Coverage passed the existing floor at 83.61% of workspace lines, excluding
  benchmark fixtures and the existing test-helper exclusions.

The byte-copy tests cover every alignment, empty reads, short tails, split
reads, and unchanged cursor/output on bounds errors. Count tests cover slot
replacement, deletion, dormancy, filtering, truncation, rejected mutations,
and independent clones.

## All 64 paired cases

Times are per benchmark iteration, not per entity, field, or byte. Brackets
contain the 95% confidence interval. Negative percentages mean less time.

<details>
<summary>Full measurements</summary>

| Benchmark | Before [95% CI] | After [95% CI] | Time change |
| --- | ---: | ---: | ---: |
| `entity_occupancy/len/empty_64` | 20.57 [20.39, 20.78] ns | 0.421 [0.419, 0.423] ns | -98.0% |
| `entity_occupancy/is_empty/empty_64` | 22.19 [22.10, 22.32] ns | 0.361 [0.358, 0.364] ns | -98.4% |
| `entity_occupancy/len/dense_64` | 20.21 [20.11, 20.34] ns | 0.421 [0.419, 0.424] ns | -97.9% |
| `entity_occupancy/is_empty/dense_64` | 0.644 [0.639, 0.650] ns | 0.359 [0.357, 0.362] ns | -44.2% |
| `entity_occupancy/len/last_only_64` | 20.29 [20.21, 20.40] ns | 0.421 [0.419, 0.423] ns | -97.9% |
| `entity_occupancy/is_empty/last_only_64` | 23.25 [23.14, 23.38] ns | 0.358 [0.355, 0.362] ns | -98.5% |
| `entity_occupancy/len/empty_16384` | 8.577 [8.543, 8.616] µs | 0.418 [0.418, 0.419] ns | -99.995% |
| `entity_occupancy/is_empty/empty_16384` | 8.293 [8.275, 8.315] µs | 0.364 [0.361, 0.367] ns | -99.996% |
| `entity_occupancy/len/dense_16384` | 8.598 [8.577, 8.622] µs | 0.420 [0.419, 0.421] ns | -99.995% |
| `entity_occupancy/is_empty/dense_16384` | 0.638 [0.636, 0.641] ns | 0.361 [0.357, 0.365] ns | -43.4% |
| `entity_occupancy/len/last_only_16384` | 8.622 [8.597, 8.649] µs | 0.422 [0.420, 0.424] ns | -99.995% |
| `entity_occupancy/is_empty/last_only_16384` | 8.371 [8.356, 8.389] µs | 0.356 [0.354, 0.358] ns | -99.996% |
| `matched_field_lookup/by_name/16` | 45.59 [45.39, 45.82] ns | 47.80 [47.61, 47.99] ns | +4.9% |
| `matched_field_lookup/cached_key/16` | 1.77 [1.77, 1.77] ns | 1.81 [1.80, 1.82] ns | +2.2% |
| `matched_field_lookup/by_name/64` | 165.69 [165.11, 166.27] ns | 187.89 [187.47, 188.37] ns | +13.4% |
| `matched_field_lookup/cached_key/64` | 1.77 [1.77, 1.77] ns | 1.81 [1.80, 1.82] ns | +2.2% |
| `matched_field_lookup/by_name/128` | 230.93 [230.55, 231.34] ns | 246.86 [246.36, 247.42] ns | +6.9% |
| `matched_field_lookup/cached_key/128` | 1.78 [1.77, 1.79] ns | 1.79 [1.79, 1.80] ns | +0.7% |
| `sampled_fields/resolve_per_entity` | 69.858 [69.450, 70.349] µs | 75.257 [75.081, 75.431] µs | +7.7% |
| `sampled_fields/cached_keys` | 1.079 [1.077, 1.082] µs | 1.074 [1.070, 1.080] µs | -0.5% |
| `sampled_fields/resolve_once_per_tick` | 2.201 [2.190, 2.216] µs | 2.256 [2.251, 2.260] µs | +2.5% |
| `sampled_fields/build_key_cache` | 1.067 [1.064, 1.070] µs | 1.164 [1.161, 1.168] µs | +9.1% |
| `packet_entities/decode_creates_512x16` | 163.068 [162.834, 163.336] µs | 163.263 [162.904, 163.644] µs | +0.1% |
| `packet_entities/decode_updates_512x16` | 131.415 [131.192, 131.649] µs | 131.148 [130.929, 131.365] µs | -0.2% |
| `filtered_entities/keep_all/64x4` | 5.277 [5.251, 5.304] µs | 5.348 [5.333, 5.366] µs | +1.3% |
| `filtered_entities/skip_all/64x4` | 3.930 [3.914, 3.943] µs | 3.841 [3.829, 3.855] µs | -2.3% |
| `filtered_entities/keep_all/512x16` | 134.030 [132.651, 135.664] µs | 130.797 [130.446, 131.228] µs | -2.4% |
| `filtered_entities/skip_all/512x16` | 99.902 [99.429, 100.530] µs | 98.191 [97.988, 98.429] µs | -1.7% |
| `filtered_entities/keep_all/512x64` | 508.434 [507.295, 509.759] µs | 507.023 [506.218, 507.844] µs | -0.3% |
| `filtered_entities/skip_all/512x64` | 372.348 [370.456, 374.798] µs | 369.553 [368.881, 370.259] µs | -0.8% |
| `packet_messages/headers_only/16` | 1.360 [1.354, 1.368] µs | 1.348 [1.344, 1.352] µs | -0.9% |
| `packet_messages/payload_or_copy/16` | 4.401 [4.390, 4.413] µs | 3.015 [3.009, 3.023] µs | -31.5% |
| `packet_messages/headers_only/256` | 1.612 [1.608, 1.618] µs | 1.620 [1.615, 1.625] µs | +0.5% |
| `packet_messages/payload_or_copy/256` | 26.189 [26.134, 26.248] µs | 4.058 [4.048, 4.070] µs | -84.5% |
| `packet_messages/headers_only/4096` | 2.240 [2.236, 2.245] µs | 2.233 [2.227, 2.239] µs | -0.3% |
| `packet_messages/payload_or_copy/4096` | 361.669 [360.504, 363.046] µs | 26.903 [26.805, 27.004] µs | -92.6% |
| `bulk_read/offset_0/64` | 4.44 [4.42, 4.45] ns | 2.45 [2.43, 2.46] ns | -44.8% |
| `bulk_read/offset_1/64` | 57.01 [56.86, 57.16] ns | 3.40 [3.39, 3.41] ns | -94.0% |
| `bulk_read/offset_3/64` | 57.04 [56.90, 57.18] ns | 3.43 [3.42, 3.45] ns | -94.0% |
| `bulk_read/offset_7/64` | 57.46 [57.10, 57.89] ns | 3.41 [3.40, 3.42] ns | -94.1% |
| `bulk_read/offset_0/4096` | 38.44 [38.17, 38.77] ns | 37.17 [37.10, 37.23] ns | -3.3% |
| `bulk_read/offset_1/4096` | 2.769 [2.756, 2.786] µs | 148.00 [147.53, 148.49] ns | -94.7% |
| `bulk_read/offset_3/4096` | 2.793 [2.777, 2.809] µs | 148.53 [147.77, 149.35] ns | -94.7% |
| `bulk_read/offset_7/4096` | 2.776 [2.748, 2.825] µs | 147.73 [147.32, 148.14] ns | -94.7% |
| `bulk_read/offset_0/65536` | 1.016 [1.012, 1.020] µs | 1.018 [1.014, 1.024] µs | +0.2% |
| `bulk_read/offset_1/65536` | 43.826 [43.714, 43.949] µs | 2.493 [2.478, 2.508] µs | -94.3% |
| `bulk_read/offset_3/65536` | 43.960 [43.773, 44.241] µs | 2.515 [2.499, 2.530] µs | -94.3% |
| `bulk_read/offset_7/65536` | 44.412 [43.930, 44.939] µs | 2.495 [2.477, 2.516] µs | -94.4% |
| `bulk_read_tails/0` | 3.55 [3.54, 3.56] ns | 0.704 [0.701, 0.708] ns | -80.2% |
| `bulk_read_tails/1` | 4.65 [4.62, 4.68] ns | 1.20 [1.19, 1.20] ns | -74.3% |
| `bulk_read_tails/7` | 18.67 [18.45, 19.00] ns | 3.28 [3.27, 3.29] ns | -82.4% |
| `bulk_read_tails/8` | 19.28 [19.11, 19.54] ns | 1.48 [1.48, 1.49] ns | -92.3% |
| `bulk_read_tails/9` | 19.84 [19.79, 19.90] ns | 1.92 [1.92, 1.92] ns | -90.3% |
| `bulk_read_tails/15` | 24.02 [23.88, 24.19] ns | 3.84 [3.83, 3.84] ns | -84.0% |
| `bulk_read_tails/16` | 24.31 [24.26, 24.36] ns | 1.93 [1.93, 1.94] ns | -92.1% |
| `bulk_read_tails/17` | 24.91 [24.85, 24.99] ns | 2.25 [2.24, 2.27] ns | -91.0% |
| `bulk_read_tails/63` | 55.96 [55.78, 56.15] ns | 5.82 [5.79, 5.84] ns | -89.6% |
| `bulk_read_tails/65` | 57.06 [56.92, 57.24] ns | 2.97 [2.96, 2.97] ns | -94.8% |
| `playback/fresh_run/raw` | 2.127 [2.123, 2.131] ms | 2.161 [2.151, 2.173] ms | +1.6% |
| `playback/prepared_run/raw` | 2.151 [2.139, 2.166] ms | 2.140 [2.136, 2.144] ms | -0.5% |
| `string_table_decode/create/1` | 110.94 [110.32, 111.76] ns | 110.46 [110.01, 110.97] ns | -0.4% |
| `string_table_decode/create/32` | 1.354 [1.350, 1.359] µs | 1.352 [1.348, 1.357] µs | -0.2% |
| `string_table_decode/create/256` | 9.205 [9.183, 9.235] µs | 9.228 [9.192, 9.272] µs | +0.2% |
| `string_table_decode/update_256` | 9.204 [9.177, 9.232] µs | 9.203 [9.176, 9.234] µs | -0.0% |

</details>
