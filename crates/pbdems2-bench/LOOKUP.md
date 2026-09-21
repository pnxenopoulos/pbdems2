# Lookup and entity-pipeline investigation

Measured on September 20, 2026, following [OPTIMIZATIONS.md](OPTIMIZATIONS.md).
Ryzen 7 7800X3D, WSL2, Rust 1.96.1, Criterion 0.8.2, cargo-criterion 1.1.0.
The profile, features, and timing settings match the previous investigation.

## What caused the earlier regression?

The earlier byte-copy and entity-count changes did not add work to name lookup.
Matched binaries had the same flat-path instruction structure and function
sizes: 1,863 bytes for `resolve_parts`, 718 for `resolve_field_key`, and 206 for
`get_by_name`. The resolver moved by 304 bytes, changing its alignment.

This points toward generated-code placement, not a new lookup allocation or
different matching rules. It does not establish a specific cache or branch
prediction cause. Hardware-counter tools were unavailable on this machine.

The new cases also exposed avoidable work. Flat queries allocated a temporary
component vector on every lookup. Across 1,000 calls, that meant 1,000
allocations requesting 64,000 bytes. Wide schemas were dominated by candidate
scanning instead. A last-field lookup with send-node prefixes took about
1.43 µs, compared with 30 ns for splitting the query alone.

## Lookup decision

Kept a small allocation-only change: names without dots pass a one-element
stack slice to the existing resolver. Dotted queries and candidate matching
keep their existing implementation. There is no cache, new dependency, or
public API change. Flat queries now make zero allocations in the diagnostic.
Dotted queries still allocate once, with one extra reallocation for the
six-component nested-send-node case.

Two matcher rewrites were rejected. They helped flat scans but made prefix
lookups about 46% slower and nested send-node lookups about 40% slower.
The broader benchmarks caught that trade-off.

| Case | Before | After | Repeat |
| --- | ---: | ---: | ---: |
| `by_name/16` | 47.86 ns | 36.03 ns | 36.94 ns |
| `by_name/64` | 177.55 ns | 168.02 ns | 168.53 ns |
| `by_name/128` | 240.12 ns | 231.44 ns | 224.56 ns |
| `sampled_fields/resolve_per_entity` | 73.37 µs | 66.89 µs | 67.19 µs |
| `sampled_fields/cached_keys` | 1.08 µs | 1.10 µs | 1.09 µs |
| `flat_first` | 18.36 ns | 7.32 ns | 7.33 ns |
| `prefix_last` | 1.43 µs | 1.45 µs | 1.45 µs |
| `nested_depth_4` | 225.36 ns | 222.26 ns | 224.64 ns |
| `nested_send_nodes` | 406.46 ns | 504.64 ns | 407.26 ns |
| `array_element` | 26.67 ns | 48.58 ns | 26.78 ns |
| `array_nested` | 49.17 ns | 80.82 ns | 50.52 ns |
| `array_invalid_index` | 37.85 ns | 58.33 ns | 38.83 ns |

The flat gains repeated. Name-based sampling fell by about 8%, while cached
keys stayed near 1.1 µs for the same 512 reads. Caching keys per serializer
layout remains a much larger consumer-side improvement than this library fix.

The first broad run had anomalously slow nested-send-node and array results.
Those large slowdowns did not repeat with the same binary. Their cause is
unresolved, and the original values remain in the table. A small dotted-path
trade-off remains: the repeated array lookups were about 0.1–1.35 ns slower
and the wide prefix case about 1.2% slower. Do not read this as a universal
lookup speedup.

A third, unchanged-binary check measured 407.83 ns for nested send nodes,
26.92 ns for array elements, 50.43 ns for nested arrays, and 39.07 ns for
invalid indices, close to the repeat above.

## New benchmark coverage

This pass adds 50 cases, bringing the suite to 210 across 11 targets:

- 22 lookup cases separate scanning from query splitting. They cover
  first/middle/last hits, misses, fixed-width names, prefixes, four-level
  nesting, and valid/invalid dynamic-array indices.
- 28 pipeline cases cover mixed bools, signed/unsigned varints, floats, vectors,
  and strings. They compare flat/nested schemas, empty/128-byte strings,
  sparse/dense updates, retained/skipped entities, and isolated decoding stages.

Fixture tests verify independently built field keys, exact decoded values,
untouched fields after sparse updates, lifecycle records, and decoder/skip
cursor agreement. The allocation diagnostic runs separately from timing.

## Mixed-pipeline results

Packet times below are µs per iteration. These compare workloads, not a
before/after parser optimization.

| Schema / string bytes | Create | Retain 16×6 | Skip 16×6 | Retain 512×30 | Skip 512×30 |
| --- | ---: | ---: | ---: | ---: | ---: |
| flat, 0 | 786.939 | 2.149 | 1.493 | 318.148 | 215.010 |
| flat, 128 | 1181.478 | 3.970 | 2.879 | 624.212 | 438.725 |
| nested, 0 | 615.960 | 2.331 | 1.583 | 336.769 | 225.859 |
| nested, 128 | 989.102 | 4.047 | 2.782 | 634.227 | 445.355 |

| Stage | ns per iteration |
| --- | ---: |
| `paths/flat_6` | 48.66 |
| `paths/flat_30` | 219.73 |
| `paths/nested_6` | 50.26 |
| `paths/nested_30` | 221.90 |
| `decode_values/0` | 310.07 |
| `skip_values/0` | 117.41 |
| `decode_values/128` | 892.01 |
| `skip_values/128` | 567.72 |

Changing strings from empty to 128 bytes roughly doubled dense-update time,
including skipped updates. Value decoding rose from 310 to 892 ns per 30
fields, while skipping rose from 117 to 568 ns. The extra nested path step
was much smaller in this fixture. String scanning is worth investigating
before assuming allocation is the only cost.

The existing controls measured 161.95 µs for boolean creates, 133.04 µs for
boolean updates, 2.55 µs for unaligned 64 KiB copies, and 2.13/2.11 ms for
fresh/prepared raw playback. These do not establish a whole-demo speedup.

## Measurement boundaries

These are warm-cache synthetic workloads, not CS2 or Deadlock parsing rates.
No representative demos or game adapters were available for end-to-end
profiling. The desktop VM is unpinned, and small differences need repeat runs.
Confidence intervals describe variation within a run, not all host activity
or code-placement effects.

Mixed packets use 512 entities with 30 fields each. Sparse updates touch
16 entities with six fields each. Fresh creates include allocation and
teardown. Updates reuse state and scratch buffers, clearing tick records each
time. Isolated stages process one entity, so their timings are not directly
additive with packet timings. Decoded stage values are dropped immediately.

Flat and nested schemas carry the same leaf values, but their packed keys and
initial field-map reservations differ. Creation-time differences are not a
clean measure of hierarchy traversal. Paths use sequential increments and one
nested push, not a representative distribution of every Huffman operation.

## Next investigations

1. Measure string scanning across lengths and bit alignments. The existing
   decoder benchmark covers one short string, while the mixed pipeline now
   shows its effect on full updates. Compare bounded bulk scanning with the
   current byte-at-a-time loop, preserving limits and truncation behavior.
2. Separate field-map allocation, insertion, and teardown in fresh creates.
   Vary key shape and capacity before changing storage or reservation rules.
3. Profile Awpy/Boon on representative demos with their dataset callbacks.
   Resolve reusable field keys per serializer layout, then use those profiles
   to choose between string work, field paths, and storage.

No string, field-map, or Huffman redesign is included in this pass.

## Reproduce and validate

The matched baseline uses the new harness with only the single-component
shortcut removed. It retains the earlier byte-copy, entity-count, and
borrowed-component improvements. It is not the old Git HEAD.

Run each revision sequentially with a distinct history ID and output file:

```bash
mkdir -p target/benchmark-results
cargo criterion -p pbdems2-bench --locked \
  --plotting-backend disabled --message-format=json \
  --history-id lookup-check -- \
  'lookup_resolution/|matched_field_lookup/|sampled_fields/|mixed_entities/|mixed_stages/' \
  --warm-up-time 1 --measurement-time 3 --sample-size 50 \
  > target/benchmark-results/lookup-check.jsonl

cargo run -p pbdems2-bench --example lookup_allocations --release --locked
```

No tests or compilation ran concurrently with measurements. Raw results are
under the ignored `target/benchmark-results/lookup-diagnosis/` directory:
`matched-before.jsonl` has 32 baseline cases, `final.jsonl` has 54 cases
including parser controls, `final-repeat.jsonl` has 19 repeated cases, and
`dotted-confirm.jsonl` has four confirmations. `after.jsonl` and
`refined-after.jsonl` preserve the rejected matcher probes.
Compare matching IDs against the explicit baseline, not cargo-criterion's
automatic comparison with whichever experimental build ran immediately before.

Validation passed:

- 229 nextest tests and 4,096 generated lookup-equivalence cases.
- All 210 benchmark smoke cases.
- Formatting, strict Clippy, Rust 1.88, minimal features, rustdoc, and doctests.
  One pre-existing doctest remains intentionally ignored.
- The coverage floor, at 84.56% of workspace lines with the configured
  benchmark/test-helper exclusions.
