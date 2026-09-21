# Three-pass optimization results

Follow-up to [the real-demo profiles](RESULTS.md), measured September 20, 2026.
All three experiments produced useful changes. The core parser keeps its public
API, game-neutral dependency graph, and safe decoding checks.

## 1. Field-path decoding

Kept an eight-bit Huffman prefix table. Common codes take one lookup, while long
codes and short input tails use the original tree. The table takes about 4 KiB
on this 64-bit build and initializes once.

Added 2,048 path-only samples from four demos, plus full opcode, depth, and
update-size histograms. Captures contain no field values or player data.
[Fixture notes](../fixtures/field_paths/README.md) explain sampling and limits.

The 12 Criterion cases cover both games, offsets 0/3/7, and packet/short-tail
inputs. Compared with the tree decoder:

| Captured workload | Mean time reduction |
| --- | ---: |
| CS2, packet | 5.7–7.2% |
| CS2, tail | 4.0–7.4% |
| Deadlock, packet | 8.3–15.0% |
| Deadlock, tail | 1.0–3.1% |

The smallest tail changes are close to noise. In five alternating pairs across
all 12 serial dataset workloads, median times fell 1.9–5.8%. Row counts, tick
bounds, and row-hash sums matched. Some individual pairs were slower. No outliers
were removed.

Differential tests compare the original tree loop with the new decoder for every
opcode, all eight alignments, truncated byte boundaries, limits, maximum path
depth, and following data. They check errors, cursor position, and partial output.

## 2. Serializer access

Kept per-entity-container class-ID bindings. Immutable schema identity tokens
invalidate the bindings when either class info or serializers change. Cloned
schemas share identity. Manually inserted entities still have name-based fallback,
and unused missing serializers don't start failing eagerly.

The cache owns shared serializer references, without per-update reference-count
changes, raw pointers, or a global cache. Its slot array costs about 24 bytes per
class-ID slot on this build. Capacity can retain the largest prior schema.

Added 12 Criterion cases spanning 1/16/128 classes, 1/24 changed fields, and
retained/skipped entities. Sparse updates improved 10–16% when retained and
26–30% when skipped. Dense updates improved about 0–5%, with the smallest result
too close to noise to claim a win.

Five paired real-demo runs compared prefix-only against prefix plus bindings:

| Dataset | Before | After | Reduction |
| --- | ---: | ---: | ---: |
| CS2 Cache events | 1.405 s | 1.362 s | 3.1% |
| CS2 Dust2 events | 1.390 s | 1.345 s | 3.3% |
| Deadlock 100655353 combat | 2.764 s | 2.618 s | 5.3% |
| Deadlock 103129247 combat | 2.280 s | 2.216 s | 2.8% |

All 20 pairs improved. Checks included schema and position-sensitive hashes.
Tests cover same-name schema replacement, ID remapping, manual entities,
cloned sessions, empty replacement tables, and delayed missing-name errors.

## 3. Awpy snapshot columns

Awpy now accumulates the same 38 columns as callbacks produce rows, moving owned
names and inventories. It avoids retaining the full-match row vector and then
scanning it once per column. Small per-tick row vectors still exist.

The Rust row-returning API remains available. An additive
`snapshots_query_chunks<C>` API supports per-segment accumulators implementing
`Default + Extend<PlayerState> + Send`. Python selectors and output schema stay
unchanged. Parallel chunks merge in the original segment order.

The private Criterion package compiles Awpy's actual column builder and keeps
the previous conversion as a reference:

| Rows | Original row scans | Preallocated columns | Growing columns |
| ---: | ---: | ---: | ---: |
| 10,000 | 1.492 ms | 1.271 ms | 1.378 ms |
| 100,000 | 21.956 ms | 15.791 ms | 18.612 ms |
| 1,000,000 | 320.856 ms | 184.869 ms | 224.044 ms |

These exclude cloning test input and dropping the resulting DataFrame, but
include consuming the owned rows. Growing columns model an unknown final row
count. The million-row case used 10 samples and five seconds of measurement.
Smaller cases used 50 samples and three seconds. All used a one-second warmup.
The pure-Rust benchmark and Python extensions have different Cargo feature
unions. Compare variants within a run, not absolute timings across harnesses.

Exact-output tests check all columns, types, order, nulls, empty input, chunk
merging, Unicode, and nonfinite floats. Real-demo comparisons additionally check
single-tick, explicit-tick, bounded-range, and empty-range selectors.

Five paired runs isolated this change against the optimized core with the old
Awpy output path:

| Every-tick snapshots | Segments | Before | After | Reduction |
| --- | ---: | ---: | ---: | ---: |
| Cache | 1 | 2.946 s | 2.509 s | 14.8% |
| Dust2 | 1 | 2.809 s | 2.465 s | 12.3% |
| Cache | 4 | 1.750 s | 1.204 s | 31.2% |
| Dust2 | 4 | 1.620 s | 1.173 s | 27.6% |

Sampled snapshots changed only 0.3–1.5% in the medians, with mixed individual
pairs and one 11% slower outlier. Treat those as essentially unchanged.
Both demos matched schema, value hashes, and position-sensitive hashes in
serial and parallel runs. All eight selector checks also matched.

## Combined results

Five fresh-process pairs per case, comparing the starting baseline against all
three retained changes. Times are medians, and reductions are ratios of those
medians. This is a new matched sweep, not a sum of the earlier percentage gains.

| Demo | Dataset | Segments | Before | After | Reduction |
| --- | --- | ---: | ---: | ---: | ---: |
| Cache | Every 64 ticks | 1 | 1.208 s | 1.141 s | 5.5% |
| Cache | Every tick | 1 | 3.031 s | 2.505 s | 17.3% |
| Cache | Events | 1 | 1.223 s | 1.160 s | 5.1% |
| Dust2 | Every 64 ticks | 1 | 1.200 s | 1.130 s | 5.8% |
| Dust2 | Every tick | 1 | 2.935 s | 2.445 s | 16.7% |
| Dust2 | Events | 1 | 1.209 s | 1.140 s | 5.7% |
| Deadlock 100655353 | Player ticks | 1 | 3.346 s | 3.150 s | 5.8% |
| Deadlock 100655353 | Ability changes | 1 | 2.451 s | 2.242 s | 8.5% |
| Deadlock 100655353 | Combat | 1 | 2.485 s | 2.260 s | 9.1% |
| Deadlock 103129247 | Player ticks | 1 | 2.760 s | 2.609 s | 5.5% |
| Deadlock 103129247 | Ability changes | 1 | 2.025 s | 1.865 s | 7.9% |
| Deadlock 103129247 | Combat | 1 | 2.036 s | 1.855 s | 8.9% |
| Cache | Every 64 ticks | 4 | 0.426 s | 0.396 s | 7.1% |
| Cache | Every tick | 4 | 1.813 s | 1.192 s | 34.2% |
| Dust2 | Every 64 ticks | 4 | 0.412 s | 0.399 s | 3.1% |
| Dust2 | Every tick | 4 | 1.634 s | 1.179 s | 27.9% |
| Deadlock 100655353 | Player ticks | 4 | 1.491 s | 1.389 s | 6.8% |
| Deadlock 103129247 | Player ticks | 4 | 1.184 s | 1.180 s | 0.4% |

The last result is unchanged within noise. Its individual pairs ranged from
4.9% faster to 2.5% slower. All pairs in the other 17 cases improved. Absolute
timings drifted between sweeps, which is why each comparison has its own matched
control. No observations were discarded.

All 18 cases had five measured pairs and two warmup records, for 216 records
total. Schemas, row counts, tick bounds, value-hash sums, and position-sensitive
hash sums matched across variants, repetitions, and one/four-segment playback.

## Validation

- 239 workspace tests passed, including the decoder and cache regressions.
- Three exact-output DataFrame tests and seven profiling-harness tests passed.
- Awpy's own workspace passed 108 unit tests with its existing lockfile.
  Three mesh/nav fixture tests remain ignored. These unit tests use its locked
  dependency version, while the paired demos and output tests use local pbdems2.
- Strict Clippy passed for the full pbdems2 workspace and Awpy's Python/output
  bridge targets. Documentation tests and the new benchmark smoke tests passed.
- Rust 1.88 checked the complete workspace, all targets, and all features.
- Eight baseline/candidate selector cases passed on CS2 Cache, covering one
  tick, explicit ticks, a bounded range, and an empty range, with one/four segments.

The new targets add 33 Criterion cases: 12 captured-path cases, 12 serializer
access cases, and nine downstream output cases.

## Reproduction and limits

See [README.md](README.md) for build, Criterion, and paired-comparison commands.
The demo hashes, hardware, and starting revisions are in [RESULTS.md](RESULTS.md).
This run uses that same host, toolchain, release profile, and private workspace.

The baseline is the working tree before these three passes, including the earlier
lookup and byte-copy improvements. It is not a published wheel. Both extensions
use local pbdems2. This pass edits Awpy's snapshot implementation but needs no Boon
source changes. Installed Python packages and consumer lockfiles remain untouched.

Measurements run sequentially with warm file caches, four Polars/Rayon threads,
and fresh Demo instances. Five pairs alternate baseline/candidate order, with
one warmup per variant in the first pair. Constructors, checksum work, and
teardown stay outside the dataset timer. No CPU pinning or host-wide isolation
was used. Four demos from two games aren't a universal performance guarantee.

Schema and position-sensitive row hashes matched where noted. Hashes aren't
collision-free equality proofs. Exact synthetic DataFrame comparisons complement
them. Process peak RSS includes mmap pages, checksums, and warmup history, so it
isn't a measurement of dataset heap usage.

Local raw evidence is under `target/three-pass/`: `field-path-tree.jsonl`,
`field-path-prefix.jsonl`, `serializer-name.jsonl`, `serializer-bound.jsonl`,
`output-bench.jsonl`, `output-million.jsonl`, `pass1-paired.jsonl`,
`pass2-paired.jsonl`, `pass3-paired.jsonl`, `combined-paired.jsonl`, and
`selectors.jsonl`.
These generated files and saved extension binaries are ignored by Git.

## What remains

Re-profile the retained implementation before another optimization pass.
Repeated path-to-field metadata traversal, per-tick snapshot row allocation,
and repeated weapon/name work are candidates, not established next wins.
No metadata cache, SIMD decoder, or additional string-scanning change was added.
