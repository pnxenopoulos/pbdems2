# Field-path captures

Each text file contains a deterministic 512-update reservoir from one full demo.
Only Huffman path operations and their operands are retained. Entity values,
packet payloads, names, and other player data are excluded. Bits before the list
and after its finish opcode are zeroed.

Columns are bit length, path count, a rolling digest of packed decoded paths,
and little-endian hex bytes. The digest checks fixture correctness, not security.
The companion JSON records all opcode counts, path depths, and update sizes,
not just the reservoir. Opcode indices follow `FIELDOP_DESCRIPTORS` in
`src/entity/field_path.rs`. Depth array index zero means a one-component path.

| Capture | Decoded path lists |
| --- | ---: |
| CS2 Cache | 4,305,893 |
| CS2 Dust2 | 4,149,562 |
| Deadlock 100655353 | 10,400,228 |
| Deadlock 103129247 | 8,478,268 |

Capture used the tree decoder and the real-demo Rust control's player-class
filter, including path decoding for skipped entities. The JSON histograms cover
the full pass. Reservoirs approximate its distribution and don't guarantee
coverage of rare operations. Separate unit tests exercise every opcode.

The original demo hashes, builds, and revisions are in
[the profiling report](../../real-world/RESULTS.md). An isolated, untimed copy of
the core used [capture.rs](../../real-world/capture.rs) to record each successful
operation and completed list. It used an LCG-seeded reservoir with seed 0x5eed.
Normal parser builds have no instrumentation or runtime switch.

The Criterion target combines both demos for each game and tests offsets 0, 3,
and 7. Packet cases append 16 zero bytes to model following entity data. Tail
cases end at the final byte. Setup, alignment changes, digest checking, and
allocation of reusable path scratch space happen outside the timer.

Tests validate every fixture at all eight bit alignments with and without
trailing data. These aren't end-to-end parser throughput measurements.
