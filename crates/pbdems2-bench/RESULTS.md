# Benchmark results

This is the first refactor's baseline. See [OPTIMIZATIONS.md](OPTIMIZATIONS.md)
for the follow-up investigation of byte copies, entity counts, and field keys.

Measured on September 20, 2026, on an AMD Ryzen 7 7800X3D under WSL2
(Linux 6.18.33.2). Rust 1.96.1, Criterion 0.8.2, cargo-criterion 1.1.0.
The bench profile uses optimization level 3, thin LTO, one codegen unit, and
debug symbols. Both `serde` and `mmap` were enabled.

This run covered 128 cases across nine targets. Each used 50 samples, a one-second
warm-up, and a three-second target measurement window. Intervals below are
Criterion's 95% confidence intervals for its typical time estimate. Times are
per complete benchmark iteration, not per field, entity, or byte.

## What changed

The Rust-skills pass removed per-candidate temporary vectors from field-name
lookup, built returned names directly into one string, made string-table
scratch allocation proportional to the user data actually present, and reused
cached baseline allocations. Filtered entity updates now share the normal
update implementation. Reader bounds checks reject oversized requests safely.

The public API and decoded output are unchanged. These changes add no runtime
dependencies. The benchmark crate adds `tempfile` for private mmap fixtures.

## Matched before/after comparison

The original library at `fbbb008` and this refactor were run sequentially using
the **same new benchmark harness, features, profile, toolchain, and runner
options**, with plot generation disabled. Negative percentages mean less time.

| Benchmark | Before | After | Time change |
| --- | ---: | ---: | ---: |
| `packet_entities/decode_creates_512x16` | 192.96 µs | 191.46 µs | -0.8% |
| `packet_entities/decode_updates_512x16` | 146.67 µs | 149.40 µs | +1.9% |
| `playback/fresh_run/raw` | 2.427 ms | 2.464 ms | +1.5% |
| `playback/prepared_run/raw` | 2.367 ms | 2.365 ms | -0.1% |
| `serializers/parse/16` | 2.42 µs | 2.50 µs | +3.5% |
| `serializers/parse/128` | 21.81 µs | 22.82 µs | +4.6% |
| `serializers/parse/512` | 85.94 µs | 87.06 µs | +1.3% |
| `serializers/container_lookup` | 5.43 ns | 5.82 ns | +7.1% |
| `serializers/resolve_first_field` | 37.40 ns | 24.76 ns | -33.8% |
| `serializers/resolve_last_of_128_fields` | 1.79 µs | 287.10 ns | -84.0% |
| `serializers/field_name_from_key` | 37.85 ns | 21.46 ns | -43.3% |
| `string_table_decode/create/1` | 1.34 µs | 126.78 ns | -90.5% |
| `string_table_decode/create/32` | 2.97 µs | 1.47 µs | -50.7% |
| `string_table_decode/create/256` | 12.63 µs | 9.48 µs | -25.0% |
| `string_table_decode/update_256` | 12.81 µs | 9.73 µs | -24.1% |

The clearest gains are last-field lookup (6.2× faster), constructing a field
name (43% less time), and string-table creation (25–91% less time).
Prepared playback was essentially unchanged. Entity updates were 1.9% slower,
and parsing a 128-field schema was 4.6% slower. Container lookup increased by
about 0.38 ns. Those smaller regressions remain in the table rather than being
reported as wins.

These are single paired runs on an unpinned desktop VM. Confidence intervals
describe variation within a run, not thermal changes, host activity, allocator
state, or code layout between runs. An initial comparison against the older
harness showed much larger differences even in unchanged code, so it was
superseded by this matched comparison. Repeat the matched runs before treating
small differences as lasting regressions.

## Workload results

- A 16,384-command header scan took 154 µs. Building its seek index took 169 µs.
- Applying 512 updates with 16 boolean fields each took 149 µs.
- Prepared playback of 128 ticks with 64 entities and 16 fields took 2.37 ms.
  This includes cloning the populated session, tick callbacks, and teardown.
- A populated session clone took 9.08 µs. Seeking to tick 126 from the preceding
  keyframe took 297 µs.
- For warm-cache 16 MiB input, reading into a Vec and decoding took 993 µs.
  Opening an mmap and decoding took 395 µs. Scanning existing heap and mapped
  buffers took 364 µs and 350 µs respectively.
- Copying an aligned 64 KiB payload took 1.24 µs. The unaligned copies took
  53.5–53.8 µs.
- Counting entities across 16,384 slots took about 9.7–9.9 µs. Sparse and dense
  iteration both took about 11.7 µs because both scan every slot.

## Next optimizations to investigate

The first three candidates below are covered in the [follow-up](OPTIMIZATIONS.md).
The descriptions here refer to the pre-investigation implementation.

1. **Unaligned bulk reads.** `BitReader::read_bytes` currently extracts a byte
   at a time using the general bit-reading helper. A safe implementation that
   combines adjacent bytes, or processes larger chunks, is worth measuring.
   The roughly 43× aligned/unaligned gap is the strongest microbenchmark lead.
   Verify every bit offset and short tail against the existing reference tests.
2. **Cache entity counts.** `EntityContainer::len` scans the slot array.
   Maintaining an occupied-slot count would make it constant time. It must stay
   correct on replacement, deletion, skipped classes, and slot truncation.
   Consider a separate occupied-index list only if real sparse iteration is hot.
3. **Resolve field keys once.** Resolving and looking up a name among 64 fields
   took 199 ns, compared with roughly 5–6 ns for a pre-resolved typed lookup.
   These cases use different field/value shapes, so that ratio is directional,
   not a like-for-like speedup. Awpy and Boon should cache keys after signon.
   A library name index could help repeated dynamic queries, at a memory cost.
4. **Profile complete game parses before deeper decoder work.** Use real CS2
   and Deadlock demos to see how much time goes to protobuf decoding, field
   paths, allocation, decompression, and dataset construction. The synthetic
   suite can then reproduce the expensive cases without bundling demo files.

These are candidates, not promised end-to-end speedups.

## Scope and reproduction

The fixtures are deterministic synthetic data. Packet entities use boolean
fields, and the field-path decoder cases use sequential increments. Separate
decoder cases cover numeric, string, vector, and angle fields. Playback's
adapter reads raw neutral entity deltas, with a full packet every 16 ticks.
It does not measure game protobufs or dataset output.

The mmap cases use private temporary files and warm caches. They do not
measure cold disk I/O, peak memory, or multi-gigabyte demos. Sequential segment
benchmarks include separate session setup and are not parallel speedup tests.

The complete survey generated HTML reports. The matched 15-case comparison
disabled plots for both versions. The table below uses the matched after-run
where available and the survey elsewhere. It should not be treated as a single
end-to-end throughput measurement.

See [README.md](README.md) for installation and full-suite commands. The
comparison filter was:

```bash
cargo criterion -p pbdems2-bench --locked --plotting-backend disabled \
  --message-format=json --history-id my-revision -- \
  'packet_entities/|serializers/|string_table_decode/|playback/(fresh_run|prepared_run)/raw' \
  --warm-up-time 1 --measurement-time 3 --sample-size 50
```

Local raw samples and estimates are in `target/benchmark-results/after.jsonl`,
`control-before.jsonl`, and `control-after.jsonl`. The initial survey's HTML
reports are under `target/criterion/reports`. Generated files stay out of Git.
To compare another revision, use the same harness and options in both checkouts
and run them sequentially.

## All 128 measurements

| Benchmark | Typical time | 95% interval |
| --- | ---: | ---: |
| `field_decode/bool` | 10.27 ns | 10.24–10.31 ns |
| `field_decode/signed_varint` | 11.49 ns | 11.45–11.53 ns |
| `field_decode/unsigned_varint` | 11.74 ns | 11.70–11.77 ns |
| `field_decode/float_no_scale` | 11.13 ns | 11.11–11.15 ns |
| `field_decode/quantized_float_12_bit` | 15.17 ns | 15.09–15.25 ns |
| `field_decode/string` | 34.66 ns | 34.46–34.87 ns |
| `field_decode/vector3` | 24.66 ns | 24.56–24.76 ns |
| `field_decode/qangle_12_bit` | 22.63 ns | 22.37–22.95 ns |
| `field_decode/qangle_precise` | 28.04 ns | 27.70–28.44 ns |
| `field_skip/bool` | 4.08 ns | 4.05–4.11 ns |
| `field_skip/signed_varint` | 4.22 ns | 4.20–4.25 ns |
| `field_skip/unsigned_varint` | 4.56 ns | 4.53–4.59 ns |
| `field_skip/float_no_scale` | 4.08 ns | 4.06–4.10 ns |
| `field_skip/quantized_float_12_bit` | 4.95 ns | 4.92–4.98 ns |
| `field_skip/string` | 12.22 ns | 12.15–12.28 ns |
| `field_skip/vector3` | 11.39 ns | 11.32–11.47 ns |
| `field_skip/qangle_12_bit` | 4.22 ns | 4.16–4.29 ns |
| `field_skip/qangle_precise` | 5.52 ns | 5.48–5.58 ns |
| `demo_framing/verify_header` | 0.65 ns | 0.65–0.65 ns |
| `demo_framing/read_command_headers` | 48.79 µs | 48.39–49.20 µs |
| `demo_framing/iterate_complete_demo` | 153.75 µs | 152.61–155.11 µs |
| `demo_framing/build_seek_index` | 168.69 µs | 167.30–170.01 µs |
| `command_body/copy_uncompressed_64_kib` | 1.07 µs | 1.07–1.07 µs |
| `command_body/decompress_snappy_64_kib` | 3.76 µs | 3.74–3.78 µs |
| `command_body_scaling/repeated/64` | 92.87 ns | 92.25–93.71 ns |
| `command_body_scaling/varied/64` | 13.94 ns | 13.83–14.05 ns |
| `command_body_scaling/repeated/4096` | 1.77 µs | 1.76–1.78 µs |
| `command_body_scaling/varied/4096` | 88.14 ns | 87.56–88.71 ns |
| `command_body_scaling/repeated/65536` | 27.09 µs | 27.04–27.14 µs |
| `command_body_scaling/varied/65536` | 1.63 µs | 1.62–1.63 µs |
| `entity_container/indexed_lookup_dense` | 14.82 µs | 14.78–14.87 µs |
| `entity_container/handle_lookup_dense` | 12.68 µs | 12.63–12.73 µs |
| `entity_container/iterate_dense` | 11.67 µs | 11.62–11.73 µs |
| `entity_container/iterate_sparse_one_in_eight` | 11.73 µs | 11.68–11.78 µs |
| `entity_container/len_dense` | 9.70 µs | 9.65–9.75 µs |
| `entity_container/len_sparse` | 9.89 µs | 9.81–10.00 µs |
| `entity_fields/typed_integer_lookup` | 5.68 ns | 5.66–5.70 ns |
| `entity_fields/typed_vector_lookup` | 5.29 ns | 5.27–5.30 ns |
| `entity_fields/resolve_and_lookup_by_name` | 198.98 ns | 197.39–200.40 ns |
| `entity_fields/world_position` | 14.52 ns | 14.45–14.59 ns |
| `class_and_position/build_class_info_1024` | 38.57 µs | 38.32–38.79 µs |
| `class_and_position/class_id_lookup_1024` | 1.13 µs | 1.12–1.14 µs |
| `class_and_position/cell_to_world` | 11.25 µs | 11.22–11.28 µs |
| `entity_handle_mask/0x3fff` | 0.67 ns | 0.67–0.67 ns |
| `packet_entities/decode_creates_512x16` | 191.46 µs | 189.15–193.99 µs |
| `packet_entities/decode_updates_512x16` | 149.40 µs | 148.17–150.80 µs |
| `filtered_entities/keep_all/64x4` | 5.97 µs | 5.93–6.01 µs |
| `filtered_entities/skip_all/64x4` | 4.42 µs | 4.37–4.46 µs |
| `filtered_entities/keep_all/512x16` | 150.17 µs | 149.38–151.01 µs |
| `filtered_entities/skip_all/512x16` | 107.38 µs | 106.97–107.86 µs |
| `filtered_entities/keep_all/512x64` | 583.30 µs | 580.32–586.35 µs |
| `filtered_entities/skip_all/512x64` | 411.11 µs | 409.10–413.34 µs |
| `input_warm_cache/heap_headers_4096` | 76.22 µs | 75.59–76.71 µs |
| `input_warm_cache/mmap_headers_4096` | 82.13 µs | 81.41–82.65 µs |
| `input_warm_cache/heap_decode_16_mib` | 363.95 µs | 361.85–366.55 µs |
| `input_warm_cache/mmap_decode_16_mib` | 349.71 µs | 347.86–351.49 µs |
| `input_warm_cache/read_file_and_decode_16_mib` | 993.23 µs | 986.17–999.12 µs |
| `input_warm_cache/open_mmap_and_decode_16_mib` | 395.34 µs | 393.06–397.55 µs |
| `bit_reader/read_bool` | 256.35 µs | 255.18–257.51 µs |
| `bit_reader/read_u8_aligned` | 15.84 µs | 15.77–15.91 µs |
| `bit_reader/read_u8_unaligned` | 52.06 µs | 51.85–52.29 µs |
| `bit_reader/read_bits/1` | 619.72 µs | 616.94–622.77 µs |
| `bit_reader/read_bits/6` | 104.00 µs | 103.37–104.63 µs |
| `bit_reader/read_bits/17` | 36.94 µs | 36.74–37.14 µs |
| `bit_reader/read_bits/32` | 19.62 µs | 19.50–19.76 µs |
| `bit_reader/read_bits/57` | 11.26 µs | 11.11–11.46 µs |
| `bit_reader/read_bits/64` | 9.78 µs | 9.74–9.82 µs |
| `bit_reader/read_uvarint32` | 49.19 µs | 49.02–49.37 µs |
| `bit_reader/read_bitcoord_zero` | 103.77 µs | 103.13–104.37 µs |
| `byte_reader/read_u32` | 20.52 µs | 20.33–20.76 µs |
| `byte_reader/read_64_byte_slices` | 747.55 ns | 744.17–750.86 ns |
| `byte_reader/read_uvarint32` | 30.21 µs | 29.56–30.89 µs |
| `packet_messages/headers_only/16` | 1.81 µs | 1.79–1.83 µs |
| `packet_messages/payload_or_copy/16` | 5.47 µs | 5.44–5.52 µs |
| `packet_messages/headers_only/256` | 1.97 µs | 1.95–1.99 µs |
| `packet_messages/payload_or_copy/256` | 32.73 µs | 32.50–32.93 µs |
| `packet_messages/headers_only/4096` | 2.53 µs | 2.52–2.54 µs |
| `packet_messages/payload_or_copy/4096` | 438.68 µs | 436.93–440.54 µs |
| `bulk_read/offset_0/64` | 5.82 ns | 5.76–5.88 ns |
| `bulk_read/offset_1/64` | 70.44 ns | 69.85–71.09 ns |
| `bulk_read/offset_3/64` | 71.41 ns | 70.87–71.94 ns |
| `bulk_read/offset_7/64` | 72.12 ns | 71.60–72.70 ns |
| `bulk_read/offset_0/4096` | 43.65 ns | 43.36–43.93 ns |
| `bulk_read/offset_1/4096` | 3.41 µs | 3.39–3.43 µs |
| `bulk_read/offset_3/4096` | 3.36 µs | 3.35–3.38 µs |
| `bulk_read/offset_7/4096` | 3.36 µs | 3.34–3.37 µs |
| `bulk_read/offset_0/65536` | 1.24 µs | 1.23–1.24 µs |
| `bulk_read/offset_1/65536` | 53.74 µs | 53.45–54.04 µs |
| `bulk_read/offset_3/65536` | 53.84 µs | 53.48–54.23 µs |
| `bulk_read/offset_7/65536` | 53.53 µs | 52.02–54.95 µs |
| `playback/fresh_run/raw` | 2.464 ms | 2.432–2.503 ms |
| `playback/prepared_run/raw` | 2.365 ms | 2.323–2.397 ms |
| `playback/filtered_run/raw` | 2.420 ms | 2.407–2.435 ms |
| `playback/four_segments_sequential/raw` | 2.465 ms | 2.457–2.473 ms |
| `playback/fresh_run/snappy` | 2.474 ms | 2.448–2.509 ms |
| `playback/prepared_run/snappy` | 2.420 ms | 2.412–2.429 ms |
| `playback/filtered_run/snappy` | 2.421 ms | 2.414–2.428 ms |
| `playback/four_segments_sequential/snappy` | 2.462 ms | 2.451–2.473 ms |
| `prepared_playback/prepare_signon_and_index` | 30.77 µs | 30.51–31.03 µs |
| `prepared_playback/clone_populated_session` | 9.08 µs | 9.05–9.12 µs |
| `prepared_playback/seek_to_tick_126` | 296.90 µs | 295.78–298.14 µs |
| `serializers/parse/16` | 2.50 µs | 2.45–2.55 µs |
| `serializers/parse/128` | 22.82 µs | 22.58–23.09 µs |
| `serializers/parse/512` | 87.06 µs | 86.19–88.01 µs |
| `serializers/container_lookup` | 5.82 ns | 5.78–5.86 ns |
| `serializers/resolve_first_field` | 24.76 ns | 24.56–24.98 ns |
| `serializers/resolve_last_of_128_fields` | 287.10 ns | 284.43–289.69 ns |
| `serializers/field_name_from_key` | 21.46 ns | 21.30–21.63 ns |
| `field_path/pack` | 2.64 µs | 2.61–2.68 µs |
| `field_path/unpack` | 467.80 ns | 463.29–473.00 ns |
| `field_path_decode/1` | 12.36 ns | 12.32–12.40 ns |
| `field_path_decode/16` | 127.45 ns | 127.14–127.80 ns |
| `field_path_decode/64` | 490.71 ns | 489.61–491.73 ns |
| `field_path_decode/256` | 1.95 µs | 1.95–1.95 µs |
| `field_type_parse/uint32` | 22.04 ns | 21.95–22.13 ns |
| `field_type_parse/Vector[3]` | 30.31 ns | 30.06–30.55 ns |
| `field_type_parse/CHandle<CBaseEntity>` | 76.01 ns | 75.17–76.75 ns |
| `field_type_parse/CUtlVector<CHandle<CBaseEntity>>` | 123.63 ns | 122.59–124.75 ns |
| `string_table_decode/create/1` | 126.78 ns | 122.19–131.74 ns |
| `string_table_decode/create/32` | 1.47 µs | 1.45–1.48 µs |
| `string_table_decode/create/256` | 9.48 µs | 9.34–9.65 µs |
| `string_table_decode/update_256` | 9.73 µs | 9.55–9.96 µs |
| `string_table_container/full_snapshot_1024` | 9.70 µs | 9.57–9.81 µs |
| `string_table_container/lookup_last_of_64` | 126.62 ns | 126.17–127.12 ns |
| `string_table_user_data/raw/64` | 8.00 µs | 7.91–8.09 µs |
| `string_table_user_data/snappy/64` | 10.52 µs | 10.43–10.62 µs |
| `string_table_user_data/raw/4096` | 228.51 µs | 227.13–229.73 µs |
| `string_table_user_data/snappy/4096` | 146.94 µs | 145.89–147.84 µs |
