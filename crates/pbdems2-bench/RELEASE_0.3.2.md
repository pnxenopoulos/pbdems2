# 0.3.2 release checks

This pass compares the release candidate with published 0.3.1, not the older
working-tree baseline used in the profiling reports. It keeps 0.3.1's field-path
bounds checks and serializer lookup guards alongside the new optimizations.

## Checks

- 247 workspace tests passed with all features. Five doctests passed, one ignored.
- All 220 library tests passed with default features disabled.
- All 237 benchmark smoke cases passed.
- Formatting, strict Clippy, rustdoc, minimal features, Rust 1.88.0, cargo-deny,
  package verification, and the CLI version smoke test passed on Linux.
- cargo-semver-checks 0.50.0 passed 223 checks against 0.3.1 with a patch bump.
  Another 31 checks were skipped.
- Workspace line coverage was 85.63%, above the 75% gate.
- actionlint 1.7.12 accepted the workflows. macOS and Windows still need GitHub CI.

## Focused timing check

Criterion 0.8.2 through cargo-criterion 1.1.0, rustc 1.96.1, Linux under WSL2,
Ryzen 7 7800X3D. Both versions used the current benchmark sources, matching
dependency versions, thin LTO, and one codegen unit. The baseline harness lived
in an isolated workspace pointed at the cached published 0.3.1 source.

Each of 26 cases used 20 samples, a 0.5-second warmup, and a 1-second measurement.
The two versions ran sequentially, without other builds or tests. These are short
regression-screening runs, not end-to-end speed claims. Values are Criterion's
typical estimates, rounded below. Ranges span alignments 0, 3, and 7.
Differences of a few percent need a longer repeated run before drawing conclusions.

| Case | 0.3.1 | 0.3.2 | Time change |
| --- | ---: | ---: | ---: |
| Flat lookup, first field | 15.90 ns | 7.34 ns | -54% |
| Flat lookup, last field | 1.140 us | 0.348 us | -69% |
| Send-node prefix, last field | 4.473 us | 1.704 us | -62% |
| Nested lookup, depth four | 678 ns | 236 ns | -65% |
| Array element | 25.89 ns | 26.71 ns | +3% |
| Invalid array index | 26.83 ns | 39.11 ns | +46% |
| Small nested-array schema | 75.34 ns | 87.82 ns | +17% |
| Format nested-array name | 60.73 ns | 59.38 ns | -2% |
| Maximum-depth push/pop | 185.99 ns | 180.03 ns | -3% |
| CS2 paths with packet tail | 93.9–99.0 us | 85.0–85.4 us | -10% to -14% |
| CS2 paths near EOF | 100.4–104.4 us | 91.5–92.1 us | -8% to -12% |
| Deadlock paths with packet tail | 80.9–84.5 us | 75.0–75.9 us | -7% to -11% |
| Deadlock paths near EOF | 87.8–89.9 us | 88.5–90.3 us | -1% to +3% |

Name lookup has a tradeoff. Single-component queries allocate nothing. Dotted
queries collect components once, avoiding repeated splitting while scanning
fields. Unlike 0.3.1's borrowed split iterator, this allocates a temporary vector.
The allocation diagnostic confirms one allocation per dotted query, plus one
growth for the longer nested-send-node case. Large schema scans get much faster,
but two small array cases cost about 12 ns more. These are recorded regressions,
not blanket performance improvements. A small-buffer lookup is a possible
follow-up, with these same cases as its acceptance tests. Cached field keys
continue to avoid name resolution during repeated sampling.

Raw logs, JSON measurements, and the allocation diagnostic are under the ignored
target/release-0.3.2 directory. The restored nested-array and maximum-depth cases
remain in the serializers benchmark target for future comparisons.

## Downstream output checks

Awpy and Boon both built against 0.3.2 in isolated copies of their own workspaces.
Their repositories were not edited. The comparison used the saved candidate
extensions from [NATIVE.md](real-world/NATIVE.md), before this release's safety
reconciliation. This is an output regression check, not a new end-to-end timing
comparison with published 0.3.1.

The CS2 Cache demo exercised snapshots every tick and every 64 ticks, plus
events. Deadlock demos 100655353 and the older 70537442 exercised player ticks,
ability ticks, and combat. Snapshot/player-tick cases ran with one and four
segments. All 13 paired cases passed, including warmups: 52 records matched in
schema, row count, tick bounds, and value/order hashes. Outputs also matched
across segment counts. The ten Python harness tests passed.

The release CLI validated framing and decompression of all three demos too.
