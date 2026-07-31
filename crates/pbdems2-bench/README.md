# pbdems2 benchmarks

This private workspace crate contains deterministic Criterion benchmarks for
the public pbdems2 API. It is excluded from publishing and does not require
real demo files.

Coverage:

- aligned, unaligned, fixed-width, variable-width, and Source 2 bit reads
- byte-reader fixed-width, varint, and borrowed-slice reads
- demo header validation, command framing, full-stream iteration, seek-index
  construction, copying, and Snappy decompression
- field-value decoding and skip paths for scalar, string, vector, and angle types
- serializer construction, type parsing, field resolution, and field paths
- dense and sparse entity lookup, iteration, typed fields, and class lookup
- string-table creation, updates, snapshots, and table lookup
- Source 2 coordinate conversion

Run every benchmark:

    cargo bench -p pbdems2-bench

Run one benchmark target or filter:

    cargo bench -p pbdems2-bench --bench io
    cargo bench -p pbdems2-bench -- serializer

Record and compare Criterion baselines:

    cargo bench -p pbdems2-bench -- --save-baseline before
    cargo bench -p pbdems2-bench -- --baseline before

CI compiles every benchmark but does not execute timing measurements, since
shared hosted runners are unsuitable for performance comparisons.
