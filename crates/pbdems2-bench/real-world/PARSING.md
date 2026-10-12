# Compare Rust parser workloads

Use `compare_parsers.rs` for entity-state parsing. It does not create Python
DataFrames, calculate stats, or parse combat datasets. Use `datasets.py` for
those workloads.

The control reads the demo into memory before the timer. The timer includes
parser construction, schema setup, playback, tick callbacks, and teardown.
Boon's setup includes its header-only prepared-playback index. The control
measures the state-parsing API call, not an isolated decode kernel.
Each process parses once. Both parsers use the system allocator, one thread,
and the same release profile. Do not run builds or tests during measurements.

## Build

Use local Boon, Awpy, and source2-demo checkouts. All downstream dependencies
must select the intended pbdems2 version. A `0.3` dependency does not use a
`0.4` patch. Use a separate adapter copy for migration controls.

From the pbdems2 root, set the three absolute checkout paths. For the recorded
comparison, source2-demo used commit `b1b1d504a29267e15b4d925126ddfa1f1e6b746d`.
Then create an isolated manifest:

```bash
export BOON=/path/to/boon
export AWPY=/path/to/awpy
export SOURCE2=/path/to/source2-demo
mkdir -p target/parser-comparison
cat > target/parser-comparison/Cargo.toml <<TOML
[workspace]
[package]
name = "parser-comparison"
version = "0.0.0"
edition = "2024"
[[bin]]
name = "parser-comparison"
path = "$PWD/crates/pbdems2-bench/real-world/compare_parsers.rs"
[dependencies]
pbdems2 = { path = "$PWD" }
boon_parser = { package = "boon-deadlock", path = "$BOON/crates/boon" }
awpy = { path = "$AWPY/crates/awpy" }
source2-demo = { path = "$SOURCE2/source2-demo", default-features = false, features = ["deadlock"] }
serde_json = "1"
[patch.crates-io]
pbdems2 = { path = "$PWD" }
[profile.release]
lto = "fat"
codegen-units = 1
TOML
cargo build --release --manifest-path target/parser-comparison/Cargo.toml
cargo tree --manifest-path target/parser-comparison/Cargo.toml -i pbdems2
```

Save the binary and lockfile for each control. Use the same lockfile, compiler,
features, profile, and runner source. Change only the core path or the specified
adapter behavior. Do not compare debug and release builds.

## Verify and measure

`boon all` and `awpy all` retain all entities and tables. `players` retains only
player controllers and pawns. `source2 all` requests both `ENTITY_STATE` and
`STRING_TABLE_STATE`. `source2 baseline-only` skips non-baseline table updates;
do not compare that mode with production Boon as if the work were equal.
The source2-demo build above is for Deadlock comparisons.

`--verify` writes sampled entity identities, player properties, table payload
hashes, and a core field-value fingerprint. Samples use ticks 10000, 26147,
40441, 187554, and 200000. Verification runs are separate from timing runs.
Inspect cross-parser differences before you compare times. Full sampled state
must match by default when you compare core variants:

```bash
python3 crates/pbdems2-bench/real-world/compare_parsers.py \
  --case before /absolute/path/to/before boon all \
  --case after /absolute/path/to/after boon all \
  --demo /absolute/path/to/match.dem --rounds 5 --warmup 1 --cpu 2 \
  > comparison.jsonl
```

For CS2 player-only comparisons, use `awpy players` for both cases. Keep the
same adapter, filters, and table handling in both builds.

The driver rotates case order and checks completed tick counts and bounds.
It records input and binary hashes, verification output, each time, and medians.
`--cpu` requires Linux. Omit it where CPU affinity is unavailable.

For different parsers, `--allow-output-differences` permits different sampled
state after inspection. It still rejects different tick counts or bounds.
This option is not proof that the outputs or supported features are equivalent.
Source2-demo uses different property aliases and field representations. Its
current table state also does not replace Boon's ordered modifier history.
Keep those capabilities separate from entity/table state timing claims.

See [the measurements](../RELEASE_0.4.0.md) for the controls, costs, and limits.
