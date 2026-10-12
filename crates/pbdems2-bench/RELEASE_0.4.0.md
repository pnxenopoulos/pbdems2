# pbdems2 0.4.0 parsing measurements

Initial measurements on October 10, 2026, with Rust 1.96.1, an AMD Ryzen 7 7800X3D,
and Ubuntu under WSL2. Runs used the system allocator, CPU 2, fat LTO,
and one codegen unit. No builds or tests ran during timing passes.

The runner read each demo before its timer. The timer included parser setup,
schema setup, playback, one counting callback per completed tick, and teardown.
It did not create Python tables, decode combat datasets, or calculate stats.
Boon's normal setup also builds its header-only prepared-playback index. This
is an end-to-end state-parsing API comparison, not a decoder-kernel comparison.
Each case discarded one warm-up round, then measured five rounds in rotating
order. Times below are medians in seconds. A lower time is better.

## Inputs

| Demo | Completed ticks | SHA-256 |
| --- | ---: | --- |
| `108575009.dem` | 193317 | `f51e59249b8172f6e06215aa1f0c29e0d68fed46955c66df1d09c68ae560ff05` |
| `111155916.dem` | 188544 | `12640193672457e858a07f085e1c4a943d7ee58c26a2784490d53b4da7913e5a` |
| `ex-mana-vs-bushido-wildcats-m1-cache.dem` | 176163 | `91df5a0121f3ca4557e2af50bc28ef1222f829339afc688377e337567f017a58` |

## Isolate the regressions

All controls used the same runner, release settings, dependency lock, and
adapter code. Separate adapter copies changed dependency constraints and
field-path API calls to compile against each core. No consumer checkout was
reset. The old control used saved pre-fix 0.3.6 source.

The controls added changes in this order:

1. **Checked paths:** wider path components, checked operations, and new keys.
2. **Shape cleanup:** remove descendants when pointers clear or vectors shrink.
3. **Shared walker:** share filtered/full packet walking and entity creation.
4. **Baseline cache:** reuse decoded class baselines.

The path-only control omits required cleanup. It is a diagnostic, not a valid
production alternative. All new controls also use the current schema and
baseline-revision helpers. The three controls that retain both correctness
fixes had matching sampled field fingerprints, entity identities, table state,
and completed tick bounds in all six workloads.

| Demo / classes | Pre-fix | Checked paths | + Cleanup | + Shared walker | + Cache |
| --- | ---: | ---: | ---: | ---: | ---: |
| 108575009.dem / all | 3.132 | 3.259 | 4.002 | 4.030 | 3.150 |
| 108575009.dem / players | 2.240 | 2.362 | 2.437 | 2.420 | 2.412 |
| 111155916.dem / all | 2.963 | 3.111 | 3.800 | 3.798 | 2.941 |
| 111155916.dem / players | 2.115 | 2.226 | 2.299 | 2.298 | 2.289 |
| ex-mana-vs-bushido-wildcats-m1-cache.dem / all | 1.113 | 1.174 | 1.306 | 1.318 | 1.221 |
| ex-mana-vs-bushido-wildcats-m1-cache.dem / players | 1.049 | 1.106 | 1.187 | 1.187 | 1.168 |

The shared walker was approximately neutral. Shape cleanup added the largest
full-parse cost. It scanned stored field maps to remove descendants. Baseline
caching recovered much of that cost for full Deadlock parsing, but helped
player-only parsing much less. Wider paths and their checks also have a cost.
Do not attribute the combined regression to the cache or refactor alone.

## Retained optimization

Compare packed prefixes during shape cleanup instead of unpacking every
stored key. Keep the same removal rule and bounds checks. A differential
property test checks the new predicate against the unpacked rule. Both
Deadlock replays and the CS2 replay kept matching sampled decoded state in
full and filtered passes.

These follow-up pairs compare the cached 0.4.0 control above with packed-prefix
cleanup. They use the same protocol and inputs.

| Demo / classes | Before tuning | Packed prefixes | Time change |
| --- | ---: | ---: | ---: |
| 108575009.dem / all | 3.125 | 3.041 | -2.7% |
| 108575009.dem / players | 2.411 | 2.365 | -1.9% |
| 111155916.dem / all | 2.945 | 2.868 | -2.6% |
| 111155916.dem / players | 2.290 | 2.240 | -2.2% |
| ex-mana-vs-bushido-wildcats-m1-cache.dem / all | 1.219 | 1.164 | -4.5% |
| ex-mana-vs-bushido-wildcats-m1-cache.dem / players | 1.164 | 1.122 | -3.6% |

This recovers part of the cost. Player-only parsing and CS2 parsing still take
more time than the pre-fix control. Keep the correctness fixes. The separate
experiment that moved error formatting to a cold helper had no consistent
benefit and was not retained.

## Follow-up: field-path work

Measured on October 11, 2026, with the same inputs, release settings, and
five-round protocol. These controls start with packed-prefix cleanup above.
Compare columns within this series; do not combine times from separate runs.

Two small changes reduce work without removing checks:

- Store operation indices in the Huffman prefix table. Its size falls from
  4096 to 512 bytes on this target. Handle common increments and finish directly.
- Read a complete field-path integer from one window when at least 35 bits
  remain. Keep the stepwise reader near EOF to preserve errors and cursors.

| Demo / classes | Before | Compact dispatch | + Integer reads | Time change |
| --- | ---: | ---: | ---: | ---: |
| 108575009.dem / all | 3.209 | 3.105 | 3.055 | -4.8% |
| 108575009.dem / players | 2.475 | 2.395 | 2.373 | -4.2% |
| 111155916.dem / all | 2.961 | 2.880 | 2.842 | -4.0% |
| 111155916.dem / players | 2.291 | 2.220 | 2.197 | -4.1% |
| ex-mana-vs-bushido-wildcats-m1-cache.dem / all | 1.198 | 1.168 | 1.159 | -3.3% |
| ex-mana-vs-bushido-wildcats-m1-cache.dem / players | 1.142 | 1.121 | 1.118 | -2.1% |

All six controls kept matching sampled state and completed tick bounds. The
final production runner also matched the combined control in all six passes.
Differential tests compare values, errors, and exact cursors on arbitrary input,
every integer width, all bit alignments, and truncated input. Existing path
bounds, count limits, and stale-field cleanup remain in place.

A separate production recheck measured the remaining regressions against the
saved pre-fix core. Dependency versions matched except for pbdems2. This old
control has different decoded state, so it is cost context, not a valid parser
alternative. The corrected controls above and final runner were checked for
matching sampled state separately.

| Demo / classes | Pre-fix | Before | Production | Gain vs. before | Cost vs. pre-fix |
| --- | ---: | ---: | ---: | ---: | ---: |
| 108575009.dem / players | 2.269 | 2.407 | 2.333 | -3.0% | +2.9% |
| 111155916.dem / players | 2.146 | 2.300 | 2.215 | -3.7% | +3.2% |
| ex-mana-vs-bushido-wildcats-m1-cache.dem / all | 1.129 | 1.182 | 1.170 | -1.1% | +3.6% |
| ex-mana-vs-bushido-wildcats-m1-cache.dem / players | 1.066 | 1.136 | 1.135 | -0.2% | +6.5% |

Gains vary across paired runs. This recheck does not show a clear gain for CS2
player-only parsing. Required path validation and shape cleanup still have a
cost; these changes do not recover all pre-fix performance.

Criterion's twelve real-path cases had median estimates 3.8–11.6% lower.
An initial sequential-path microbenchmark reported a 2.4% slowdown. Three
rotating repeats did not reproduce it; paired medians were 0.3–2.9% lower.
The separate count-limit experiment gave no clear CS2 gain and a small Deadlock
slowdown. It was discarded. The public limits and error handling stay unchanged.

The [follow-up samples](PARSING_FOLLOWUP_0.4.0.json) include individual timings,
input and binary hashes, verification counts, and Criterion estimates.

## CS2 player-only cleanup follow-up

Keep descendant removal outside the common retained-field update loop. Check a
stored key's parent prefix before its depth during removal. The decoder, path
bounds, count limits, and pointer/vector cleanup rules stay unchanged.

A private work counter found about 28 million retained player fields and
1.8 million skipped fields in the Cache replay. These are work counts, not a
sampled CPU profile. The retained-field loop was the main tuning target.

Both production controls used the same source path, locked dependencies,
compiler, system allocator, and fat-LTO release profile. The comparison used
CPU 2, one warm-up pass, and seven rotating measured rounds. Times are median
seconds. Both controls retained player controllers and pawns, consumed skipped
entity updates, updated all string tables, and counted completed ticks.

| CS2 demo | Before | After | Time change |
| --- | ---: | ---: | ---: |
| ex-mana-vs-bushido-wildcats-m1-cache.dem | 1.110 | 1.092 | -1.7% |
| ex-mana-vs-bushido-wildcats-m2-mirage.dem | 1.142 | 1.128 | -1.3% |
| ex-mana-vs-bushido-wildcats-m3-dust2.dem | 1.096 | 1.079 | -1.5% |
| b8-vs-vitality-m1-mirage.dem | 1.024 | 1.004 | -2.0% |
| b8-vs-vitality-m2-dust2.dem | 1.260 | 1.236 | -2.0% |

All five comparisons matched sampled entity identities, stored-field hashes,
string tables, and completed tick bounds. Sample checks do not establish
output equality at every tick. Existing differential and stale-child tests
cover the changed predicate and cleanup behavior.

At this stage, repeated synthetic skip cases showed some cost: about
2–4% for dense flat updates. The vector skip follow-up below addresses these
cases. Costs relative to the pre-fix core remain. That old core has different decoded state and is cost context
only.

The final controls also measured about 0.9% less time for full CS2 parsing and
0.7–1.0% less time for Deadlock player-only parsing. Full Deadlock timings were
within 0.4% of the baseline. In a fresh Cache comparison, the pre-fix core took
1.052 s, the corrected baseline took 1.109 s, and the retained patch took
1.091 s. The remaining cost against the pre-fix core was about 3.7%, down from
5.4%. These three values come from the same rotating comparison.

Shallow-key packing and extra direct operation dispatch gave no reliable gain.
Moving the root check and inlining serializer lookup improved some replay
controls but did not remove the synthetic skip cost. Those experiments and a
larger shape-update refactor were discarded to keep the patch small.

Rust workspace tests, Clippy, formatting, the focused benchmark smoke tests,
and the Sphinx documentation build passed. Boon's Rust checks used the local
pbdems2 dependency.

The [recorded samples](CS2_PLAYERS_0.4.0.json) include the final controls,
discarded experiments, synthetic repeats, hashes, and verification limits.

## Vector skip follow-up

Skip complete, unquantized 3D vectors in one checked 96-bit step. Keep the
original scalar calls for truncated input and other encodings. No public API,
path checks, limits, or retained-field rules change.

The earlier cleanup did not change the skip source. Its dense flat regression
was not stable on recheck. Code generation and measurement variation remain
possible causes; this review does not isolate them. The separate vector change
removes repeated decoder dispatch from the affected mixed workloads.

Two focused rounds reverse the control order. Each uses CPU 2, 30 samples,
one second of warm-up, and two seconds of measurement. Values below are medians
of the two Criterion median estimates.

| Synthetic case | Before | After | Time change |
| --- | ---: | ---: | ---: |
| Dense flat skips, 512 entities × 30 fields | 203.6 µs | 183.1 µs | -10.1% |
| Sparse nested skips, 16 entities × 6 fields | 1.416 µs | 1.300 µs | -8.2% |
| Mixed value skips, 30 fields | 122.3 ns | 85.4 ns | -30.2% |

The broad run reduces all eight mixed skip cases by 4–12%, including long
strings. Scalar skip controls are within 1% of the baseline. Unquantized 3D
vector skips take about 64% less time. Coordinate and quantized 3D vector
controls take about 4% more time (about 0.5 ns) for the extra check. Keep this
tradeoff visible; this is not a claim that every decoder is faster. The next
follow-up addresses the two variable-width controls.

Ten matched replay workloads use the same adapter, lockfile, compiler, source
paths, system allocator, and fat-LTO profile. Five rotating measured rounds
follow one warm-up pass. They cover five CS2 player-only passes, full and
player-only passes for both Deadlock demos, and a full CS2 pass. Timings range
from -0.5% to +0.9% of the baseline. Do not claim a whole-replay speedup.
All ten workloads match sampled entity identities, stored-field hashes,
string tables, and completed tick bounds. Samples do not prove equality at
every tick.

A differential test covers byte alignments, complete values, truncation,
and variable-width fallback errors and cursor positions. Coordinate and
quantized vector cases now remain in the decoder benchmark suite. Broader
helper variants were discarded because they changed other decoder costs.

Rust workspace tests, Clippy, formatting, benchmark smoke tests, and the
Sphinx build passed. Boon checks used the local pbdems2 patch.

See the [recorded samples](SKIP_0.4.0.json) for repeats, discarded experiments,
source and input hashes, verification records, and workspace check results.

## Variable-width vector follow-up

Dispatch coordinate, normal, and quantized 3D components once in a private
helper. Keep the helper outside the common scalar match. Return the vector
result directly. The checked 96-bit path, scalar fallback, error values, and
reader positions remain intact. No public API changes.

These controls start with the retained vector optimization above. Both builds
use the same source paths, compiler, locks, profiles, CPU, and benchmark inputs.
Two focused rounds reverse the control order. Each uses 30 samples, one second
of warm-up, and two seconds of measurement. Values below are medians of the two
Criterion median estimates. Compare within this series.

| Case | Before | After | Time change |
| --- | ---: | ---: | ---: |
| Coordinate vector skip | 15.683 ns | 10.289 ns | -34.4% |
| Quantized vector skip, 12-bit components | 12.915 ns | 7.828 ns | -39.4% |
| Raw vector skip | 3.547 ns | 3.350 ns | -5.5% |
| Dense flat skips, 512 entities × 30 fields | 185.801 µs | 187.222 µs | +0.8% |
| Sparse nested skips, 16 entities × 6 fields | 1.295 µs | 1.299 µs | +0.3% |
| Mixed value skips, 30 fields | 84.864 ns | 86.854 ns | +2.3% |

The earlier mixed entity gains remain. There is a small cost in the isolated
mixed-value control; do not claim that every case improves. The broad scalar
controls range from -0.2% to +1.2%. A dense retained-field control initially
showed a 4% cost. A three-way rotating check did not reproduce it. A final
seven-round interleaved check measured 319.351 µs before and 321.761 µs after
(+0.8%), with variation between runs. Keep all samples when reviewing this cost.

Ten native workloads use five rotating measured rounds after one warm-up pass.
They cover five CS2 player-only passes, both Deadlock demos in full and
player-only modes, and full CS2 parsing. Timing changes range from -1.0% to
+0.9%; this does not show a clear complete-replay speedup. All ten workloads
match sampled entity identities, stored-field hashes, string tables, and
completed tick bounds. Samples do not prove equality at every tick.

Putting all vector branches inside the common skip function cost 16% in the
mixed-value control. An inlining hint also increased string and varint costs.
Those variants were discarded. The final helper keeps the raw-vector fast path.

The differential test now covers every quantized width and flag combination,
normal and coordinate components, nested fallbacks, byte alignments, and
truncated input. It compares exact errors and reader positions. This test
exposed an existing overflow in 31-bit quantized-float setup. Unsigned step
counts fix it; a separate test checks the decoded range endpoints.

Rust workspace tests, Clippy, formatting, benchmark smoke checks, and the Sphinx
build passed. Boon checks used the local pbdems2 patch.

See the [recorded samples](VARIABLE_VECTORS_0.4.0.json) for all controls, source
and binary hashes, discarded experiments, verification records, and checks.

## Match the reference workload

The earlier source2-demo control requested only `ENTITY_STATE`. It updated
`instancebaseline` but skipped other table updates. Boon updated every table
and preserved ordered modifier changes. That was not an equal workload.

The corrected state control requests `ENTITY_STATE | STRING_TABLE_STATE` and
`TICK_END`. Both parsers read the same preloaded bytes and retain all classes.
The Boon state-only control disables ordered modifier copies in a private
adapter copy. The reciprocal entity-only controls skip non-baseline updates
in both parsers. Production Boon keeps all table updates and ordered changes.

Source2-demo used commit `b1b1d504a29267e15b4d925126ddfa1f1e6b746d` with only
the `deadlock` feature. Its default allocator feature was disabled. Rust source,
manifests, and the lockfile matched the pinned GitHub source archive. No Git
commands were used. The controls below use Boon before packed-prefix tuning.

| Demo | State retained | Boon | source2-demo | Reference time change |
| --- | --- | ---: | ---: | ---: |
| 108575009.dem | All entities and tables | 3.138 | 2.395 | -23.7% |
| 108575009.dem | Entities and baseline updates | 3.077 | 2.342 | -23.9% |
| 111155916.dem | All entities and tables | 2.950 | 2.265 | -23.2% |
| 111155916.dem | Entities and baseline updates | 2.903 | 2.204 | -24.1% |

Ordered modifier copies added about 9–17 ms in the separate production controls.
Updating all tables added about 53–62 ms in source2-demo. The workload mismatch
was real, but it did not explain the whole timing gap.

These are entity/table state timings for two replays, not a speed ranking for
Boon's public datasets or stat APIs. The selected parsers also use different
state representations and validation rules. These measurements do not isolate
which representation or check causes the remaining gap.

The separate follow-up below uses production Boon, with ordered modifier
history enabled. It is not the state-only control above. The same five-pair
protocol measured the retained optimization and the reference together.

| Demo | Boon before tuning | Boon after tuning | source2-demo |
| --- | ---: | ---: | ---: |
| 108575009.dem | 3.120 | 3.046 | 2.387 |
| 111155916.dem | 2.924 | 2.858 | 2.243 |

### Output checks and limits

Completed tick counts and bounds matched. At the sampled ticks, entity slots,
serials, classes, common health/Steam ID/pawn-handle values, and modifier and
baseline payload hashes matched. The sampled ticks were 10000, 26147, 40441,
187554, and 200000 where present.

This is not proof that every decoded value is equal. Source2-demo accepts extra
unqualified property aliases and uses different field-value representations.
Empty table payloads, some compressed table keys, and userinfo payload retention
also differed. Modifier payloads were compared by row index; different keys
were not treated as equal. Full output equivalence remains unproved.

## Reproduce

Use [the matched parsing guide](real-world/PARSING.md). The comparison driver
checks sampled state before timing. It rejects changed tick bounds even when
you explicitly allow output differences for a reference-parser comparison.

The [saved samples](PARSING_0.4.0.json) contain all measured times, memory samples,
input hashes, and verification counts. Raw verification reports and diagnostic
builds remain under `target/parser-matched/`; they are not published artifacts.
Peak RSS includes the preloaded demo bytes and all parser allocations. It is
not the size of entity state alone. Five samples on one machine do not establish
a general performance threshold.
