# pbdems2 benchmarks

This private Criterion crate benchmarks the public pbdems2 API with generated,
repeatable fixtures. It does not need real demo files.

It covers:

- byte and bit readers
- headers, commands, indexes, copies, and Snappy decoding
- field values, serializers, type parsing, and field paths
- entity and class lookup
- string-table creates, updates, snapshots, and lookups
- Source 2 coordinate conversion

Run everything:

```bash
cargo bench -p pbdems2-bench
```

Run one target or filter:

```bash
cargo bench -p pbdems2-bench --bench io
cargo bench -p pbdems2-bench -- serializer
```

Save and compare a Criterion baseline:

```bash
cargo bench -p pbdems2-bench -- --save-baseline before
cargo bench -p pbdems2-bench -- --baseline before
```

CI compiles the benchmarks but does not time them. Shared runners are too noisy
for useful performance comparisons.
