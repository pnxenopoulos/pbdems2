# pbdems2 0.4.1 property filtering — 2026-10-15

Compare the saved 0.4.0 decoder with 0.4.1 on a Ryzen 7 7800X3D under WSL2,
Rust 1.96.1. Both binaries use the same bench profile. Tests added to the saved
baseline do not change runtime code. No tests or builds ran during timing.

## Full decoding

Pin each run to CPU 2. Use 50 Criterion samples, 0.5-second warmup, and
1.5-second measurement. Repeat each case in both build orders. Values are the
median of run medians. The small create case uses a longer eight-run repeat.

| Case | Before (µs) | After (µs) | Change |
| --- | ---: | ---: | ---: |
| Nested updates, empty strings | 330.995 | 325.809 | -1.6% |
| Nested updates, 128-byte strings | 629.600 | 633.370 | +0.6% |
| Flat creates, empty strings | 780.281 | 777.395 | -0.4% |
| Flat creates, 128-byte strings | 1186.697 | 1178.226 | -0.7% |
| Updates, 512 × 16 fields | 131.898 | 132.149 | +0.2% |
| Creates, 512 × 16 fields | 167.975 | 168.628 | +0.4% |

No material full-decoding regression is established. Full decoding keeps a
separate specialized field loop without per-field selection checks. An early
candidate regressed by 6–7% because LLVM outlined map insertion and exported the
field resolver. It was rejected. The filter compiler now stays behind a
non-generic function, and the projected loop uses a separate insertion form.

Skip validation now enforces string/binary limits and 32-bit varint widths.
The skip-only control changes from 86.292 to 78.439 ns for empty strings and
529.170 to 533.364 ns (+0.8%) for 128-byte strings. Fast paths keep the uncommon
string-limit loop out of small packets. Boundary tests compare errors and cursor
positions with decoding.

## Selected properties

All cases consume identical packets with 512 entities and 30 mixed fields.
Selection keeps two scalars. The unlisted case has a filter for another class
and keeps all fields. Flat and nested schemas cover empty and 128-byte strings.
Use 50 samples, 0.5-second warmup, and two-second measurement on CPU 2.
These synthetic results are not whole-demo or Python API timings.

| Layout | String bytes | Full (µs) | Selected (µs) | Unlisted (µs) |
| --- | ---: | ---: | ---: | ---: |
| flat | 0 | 321.399 | 237.844 | 321.182 |
| flat | 128 | 624.995 | 470.507 | 631.415 |
| nested | 0 | 329.412 | 247.231 | 333.229 |
| nested | 128 | 633.778 | 473.251 | 639.198 |

## Real demos and correctness

Awpy's complete five-dataset combat request is 9.1–9.5% faster on one core and
4.6–6.7% faster on four, across FURIA Ancient and Mirage. Construction and output
table creation are timed. Seven fresh-process samples per build alternate order;
every column and value matches. Five other CS2 demos also pass exact comparisons.
See Awpy's Python benchmark report for the full workload and control tables.

Boon's sampled health and nested ability values match across 1,938,183 fields in
two demos. Direct filtered seeks match unfiltered seeks at both checked ticks.
Boon's production code is unchanged. Its callers can use nested subtree selection
rather than the small CS2 combat selection.

The workspace tests, all 256 benchmark smoke cases, Clippy, and format checks
pass. Small regression cases
cover immutable selections, baseline isolation, partial updates, clearing and
shrinking children, dynamic indices above 255, handle reuse, schema replacement,
seeks, segments, checkpoints, concurrent sessions, limits, and truncated input.

Raw samples and saved binaries remain in Awpy's ignored `target/property-filter/`.
Repeat synthetic cases with:

```bash
cargo bench -p pbdems2-bench --locked --bench entity_pipeline -- projected_entities
cargo bench -p pbdems2-bench --locked --bench entity_pipeline -- mixed_entities
cargo bench -p pbdems2-bench --locked --bench entities -- packet_entities
```
