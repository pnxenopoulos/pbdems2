# Allocator and reader experiments

Measured October 7, 2026, on Linux/WSL with an AMD Ryzen 7 7800X3D and Rust 1.96.1.

The system allocator remains the default. The private `real-world-control` executable
has an optional `mimalloc` feature. The published parser has no new dependencies.

## Allocator results

Five measured pairs per case, after one warmup per variant. Each pair uses fresh
processes and alternates execution order. Runs are sequential, with warm file
caches. The release builds use fat LTO and one codegen unit. They link the same
local pbdems2 and consumer sources. Mimalloc 0.1.52 uses its v3 implementation.

Each timer includes file mapping, parser setup, full playback, and parser teardown.
The callback counts ticks. These are decode controls, not Python dataset timings.

| Demo | Classes | System | Mimalloc | Time reduction | Extra peak RSS |
| --- | --- | ---: | ---: | ---: | ---: |
| 108575009 | all | 3.159 s | 3.089 s | 2.2% | 27.7 MiB |
| 108575009 | players | 2.255 s | 2.193 s | 2.7% | 22.9 MiB |
| 111155916 | all | 3.108 s | 2.922 s | 6.0% | 29.6 MiB |
| 111155916 | players | 2.149 s | 2.082 s | 3.1% | 23.4 MiB |
| cache | all | 1.178 s | 1.107 s | 6.0% | 12.9 MiB |
| cache | players | 1.050 s | 1.038 s | 1.2% | 14.5 MiB |
| dust2 | all | 1.175 s | 1.081 s | 8.0% | 15.0 MiB |
| dust2 | players | 1.042 s | 1.037 s | 0.5% | 16.5 MiB |

Time reductions are ratios of medians. Individual pairs vary: some Deadlock
runs are slower with mimalloc. Small differences do not establish a gain. RSS
includes mapped demo pages and allocator overhead. It is not live heap size.
These results support further dataset tests, not a default allocator change.

## Validation

Both allocators complete all four demos with the same tick counts. Separate
checks sample every 256th tick and hash the updated entities' class IDs, serials,
active flags, field keys, and values. Those hashes match across allocators.
These checks do not compare Python datasets or prove equality at every tick.

The benchmark builds with and without `mimalloc`. Formatting and strict Clippy
pass. Workspace tests pass (261 passed, one ignored). Production decoder code
is unchanged.

## Reader experiments

- A safe cached 64-bit window increases packet-path time by 14–20%. Several
  fixed-width read cases take about twice as long. Short tails change little.
- A four-bit lookahead for field-path integer prefixes increases captured-path
  time by 4–8%.

Both prototypes were removed. Captures cover CS2 and Deadlock, bit offsets
0/3/7, and packet and short-tail inputs. Each case uses a 0.5-second warmup
and a one-second measurement. The cache uses 15 samples; lookahead uses 30.
The existing checks pass for both prototypes. A differential lookahead check
also preserves values, errors, cursor position, and following reads at all
eight alignments and every byte truncation.

## Existing design

- Entity fields already use `FxHashMap`, backed by Rust's flat SwissTable.
  Entity handles use a dense slot array. See [entities.rs](../../../src/entity/entities.rs).
- Huffman field-path operations already use function pointers and a prefix
  lookup table. See [field_path.rs](../../../src/entity/field_path.rs).
- Boon exports Polars columns through `pyo3-polars` FFI. This shares column
  buffers; creating and filling those columns still costs time and allocations.

## Reproduce

From the pbdems2 root, with the sibling Boon and Awpy checkouts:

```bash
manifest=crates/pbdems2-bench/real-world/Cargo.toml
cargo build --manifest-path "$manifest" -p real-world-control --release --locked \
  --target-dir target/allocator-system
cargo build --manifest-path "$manifest" -p real-world-control --release --locked \
  --features mimalloc --target-dir target/allocator-mimalloc

demo=../boon/108575009.dem
/usr/bin/time -f "peak RSS: %M KiB" \
  target/allocator-system/release/real-world-control boon all "$demo"
/usr/bin/time -f "peak RSS: %M KiB" \
  target/allocator-mimalloc/release/real-world-control boon all "$demo"
```

Use `players` for the class filter, or `awpy` with a CS2 demo. Warm up each
variant, then alternate process order for at least five pairs. Do not compile
or run other benchmarks during measurement.

Raw timings, prototype binaries, source copies, and metadata are under
`target/parser-experiments/`. This directory is ignored by Git.

## Inputs

Starting revisions:

- `pbdems2`: `98d7aa14fc69d748fd18dc416db1cc786b66cba1`
- `boon`: `6880ea8e44a12b622f5dad1cf43d35cb7fb525fb`
- `awpy`: `d9295ae5525fa142ee75701be1b335e4fffe6097`

Demo SHA-256 hashes:

- `108575009.dem`: `f51e59249b8172f6e06215aa1f0c29e0d68fed46955c66df1d09c68ae560ff05`
- `111155916.dem`: `12640193672457e858a07f085e1c4a943d7ee58c26a2784490d53b4da7913e5a`
- `ex-mana-vs-bushido-wildcats-m1-cache.dem`: `91df5a0121f3ca4557e2af50bc28ef1222f829339afc688377e337567f017a58`
- `ex-mana-vs-bushido-wildcats-m3-dust2.dem`: `19971b90e67da96ff805db5cffd99961ae49a8845214c058a34ff7feb22f9b9c`
