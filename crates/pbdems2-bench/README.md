# pbdems2 benchmarks

This private crate benchmarks pbdems2's public API with generated fixtures and
checked-in path-only captures. No game protobufs or demo downloads are needed.

| Target | Coverage |
| --- | --- |
| `io` | Fixed-width reads, bit widths, varints, coordinates |
| `demo` | Headers, command iteration, seek indexes, copies, Snappy size and entropy |
| `packet` | Inner framing, borrowed/copied payloads, bulk reads at four alignments, short tails |
| `decoders` | Decoding and skipping numeric, string, vector, and angle fields |
| `serializers` | Schema parsing, field lookup, type parsing, packed and Huffman paths |
| `entities` | Dense/sparse lookup, field access, creates, updates, class filtering |
| `string_tables` | Creates, updates, snapshots, prefix history, compressed user data |
| `playback` | Populated signon, fresh/prepared runs, filtering, segments, seeking |
| `input` | Heap and mmap scans, file-read/map setup, warm-cache 16 MiB input |
| `access` | Counts, name/key sampling, lookup hits/misses, prefixes, nesting, arrays |
| `entity_pipeline` | Mixed values, nested paths, sparse/dense deltas, retained/skipped updates |
| `field_paths` | Real-demo opcode mixes, nesting, alignment, and short tails |
| `serializer_access` | Sparse/dense retained and skipped updates across 1–128 classes |

Run from the workspace root:

```bash
cargo install cargo-criterion --version 1.1.0 --locked
cargo criterion -p pbdems2-bench --locked

# One target, or a name filter
cargo criterion -p pbdems2-bench --bench packet --locked
cargo criterion -p pbdems2-bench --locked -- 'playback|packet_entities'

# Shorter measured run, also used for the checked-in results
mkdir -p target/benchmark-results
cargo criterion -p pbdems2-bench --locked --message-format=json \
  --history-id my-change -- --warm-up-time 1 --measurement-time 3 --sample-size 50 \
  > target/benchmark-results/my-change.jsonl
```

[cargo-criterion](https://bheisler.github.io/criterion.rs/book/cargo_criterion/cargo_criterion.html)
keeps history and HTML reports under `target/criterion`. Use a distinct
`--history-id` for each revision. Run timings sequentially on an idle machine
with the same toolchain and power settings.

For an optimization comparison, add the benchmark first and run both revisions
with that same harness, profile, features, and arguments. Keep setup and
iteration settings identical. Don't run tests or compilation in parallel with
measurements.

`cargo bench -p pbdems2-bench` still works. Its `--save-baseline` and
`--baseline` options belong to that runner, not cargo-criterion.

Fixture construction is outside the timer unless it is part of the API cost
being measured. Owned parser inputs use batched setup. Packet-entity updates
reuse scratch buffers and clear tick changes on every iteration. Whole playback
includes session setup and teardown.

These are microbenchmarks, not full CS2 or Deadlock parsing rates. Playback uses
a small adapter with raw entity deltas and excludes protobuf decoding and
dataset output. The mmap tests use warm file caches and do not measure cold
disk speed or peak memory. The original packet-entity fixtures use boolean
fields. `entity_pipeline` adds bools, signed/unsigned varints, floats, vectors,
and strings, with the same values in flat and nested schemas. Its paths cover
sequential increments and one nested push, not the full field-path opcode mix.

The mixed pipeline separates path decoding and value decoding/skipping from
whole packets. Updates reuse populated state and scratch buffers. Fresh creates
include state allocation and teardown. Stage timings use one entity, while
packet timings use up to 512, so they are not directly additive.

The `access` target compares the same borrowed values for names and cached
keys. Its sampling workload reads eight fields from each of 64 entities.
Cache construction is measured separately. Reusing keys excludes that one-time
cost, while `resolve_once_per_tick` includes resolution using a reused buffer.
Keys belong to a particular serializer layout, so consumers must rebuild them
when that layout changes.

Lookup cases separate first/middle/last hits, misses, send-node prefixes, nested
schemas, and array indices. The split-only control measures query allocation
without scanning fields. Count allocations separately from timing:

```bash
cargo run -p pbdems2-bench --example lookup_allocations --release --locked
```

This single-threaded diagnostic excludes fixture setup. Requested bytes include
reallocation requests, not peak memory.

CI runs `cargo bench -p pbdems2-bench --locked -- --test` to execute every
benchmark once without timing. Unit tests check the shared fixtures separately.
See [RESULTS.md](RESULTS.md) for the first refactor and
[OPTIMIZATIONS.md](OPTIMIZATIONS.md) for the first follow-up.
[LOOKUP.md](LOOKUP.md) covers the lookup investigation and mixed pipeline.
[RELEASE_0.3.2.md](RELEASE_0.3.2.md) checks the final candidate against published
0.3.1, including retained safety guards and small lookup tradeoffs.

For actual Awpy and Boon dataset workloads, see the optional
[real-demo profiling harness](real-world/README.md) and its
[results](real-world/RESULTS.md). It uses sibling checkouts and local demos,
outside this crate's microbenchmarks and CI. The
[follow-up report](real-world/OPTIMIZATIONS.md) covers the prefix decoder,
class bindings, and Awpy column output.
The [separate-build follow-up](real-world/NATIVE.md) verifies those changes with
native feature sets, allocation diagnostics, and a wider demo corpus.
