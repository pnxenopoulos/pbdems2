<div align="center">

# pbdems2

[![crates.io](https://img.shields.io/crates/v/pbdems2.svg)](https://crates.io/crates/pbdems2)
[![crates.io Downloads](https://img.shields.io/crates/d/pbdems2.svg)](https://crates.io/crates/pbdems2)
[![docs.rs](https://docs.rs/pbdems2/badge.svg)](https://docs.rs/pbdems2)
[![CI](https://img.shields.io/github/actions/workflow/status/pnxenopoulos/pbdems2/ci.yml?branch=main)](https://github.com/pnxenopoulos/pbdems2/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/pnxenopoulos/pbdems2/blob/main/LICENSE)

</div>

pbdems2 is a game-neutral Rust library for Valve Source 2 demo and replay
files. It implements the shared PBDEMS2 container and entity wire format while
leaving generated protobuf messages and game semantics to consumer crates.

Some projects that use pbdems2 are [Boon (Deadlock)](https://github.com/pnxenopoulos/boon) and [Awpy (CS2)](https://github.com/pnxenopoulos/awpy).

## Responsibilities

pbdems2 owns:

- PBDEMS2 header validation and command framing
- byte- and bit-level Source 2 readers
- Snappy command-body decompression
- flattened serializer graphs and field paths
- configurable field decoding
- entity lifecycle and state
- string tables and instance baselines
- Source 2 cell-coordinate conversion

Game crates own:

- generated protobuf messages
- conversion from protobuf messages into pbdems2 input structures
- game events and domain models
- title-specific constants, datasets, and language bindings

## Game adapters

Wire conventions that differ between games are expressed through a neutral
decode profile. A game adapter defines its own profile instead of adding a
title name to this crate:

    use pbdems2::entity::{
        BareCharEncoding, DecodeProfile, PreciseQAngleMode,
    };

    const PROFILE: DecodeProfile = DecodeProfile::new(
        BareCharEncoding::NullTerminatedString,
        PreciseQAngleMode::Centered,
    )
    .with_ammo_field("m_iClip1");

Additional builder methods configure symbolic array lengths, game-defined
pointer types, and dynamic serializer-array types without hard-coding those
names in pbdems2.

The adapter converts generated messages into the neutral ClassEntry,
FlattenedSerializer, CreateStringTable, UpdateStringTable, and PacketEntities
structures exposed by pbdems2. Generated message types never cross the crate
boundary.

## Neutral parser driver

`Demo` provides strict, allocation-free outer-command iteration and a
header-only `DemoIndex`. `DemoParser` adds signon initialization, tick
callbacks, full-packet seeking, segmented playback, and optional entity-class
filtering while remaining independent of Prost and every game schema.

A consumer implements `DemoAdapter` by matching the neutral outer command,
decoding its own generated protobuf type, and immediately passing neutral
values to `CommandContext`:

    let limits = DecodeLimits::default()
        .with_max_command_body_bytes(32 * 1024 * 1024)
        .with_max_packet_message_bytes(16 * 1024 * 1024);
    let parser = DemoParser::with_limits(&demo_bytes, limits)?;
    let index = parser.index()?;
    let state = parser.try_run_to_end(&mut adapter, 1.0 / 64.0, |tick| {
        consume_tick(tick.tick(), tick.entities())?;
        Ok(())
    })?;

The original `run_to_end`, `run_to_end_filtered`, and `decode_segment` methods
accept infallible callbacks. Their `try_*` counterparts propagate the adapter's
application error type, allowing tick consumers to stop playback on output,
database, cancellation, or other processing failures.

The adapter owns protobuf decoding; `CommandContext` owns validated mutation
of serializers, classes, string tables, entity deltas, and tick interval.
`parse_to_tick` restores the nearest preceding full packet and replays only
the necessary deltas.

## Decode limits

Every untrusted length or repeated count is checked before allocation or
iteration. `DecodeLimits` covers command bodies, decompression, inner packet
messages, string tables and user data, field strings, serializers and symbols,
classes, fixed arrays, entity updates, and field paths. The defaults are large
enough for normal captures and bounded against corrupt length fields; use the
builder methods when a known capture legitimately needs a larger limit.

Limit failures are reported as `Error::LimitExceeded`, allocation failures as
`Error::Allocation`, and command failures retain byte offset, command number,
and tick through `Error::Command`.

## Cargo features

The default `serde` feature implements `Serialize` for `FieldValue` and
`ClassEntry`. Consumers that do not serialize decoded values can keep the
dependency graph smaller:

    pbdems2 = { version = "0.1", default-features = false }

The optional `mmap` feature provides `MappedDemo`, an owning read-only file map
that lends zero-copy `Demo` and `DemoParser` views:

    pbdems2 = { version = "0.1", features = ["mmap"] }

    // SAFETY: Keep the file unchanged and untruncated while `mapped` exists.
    let mapped = unsafe { pbdems2::MappedDemo::open("match.dem")? };
    let parser = mapped.parser()?;

The constructor is intentionally unsafe because every portable file-backed
mmap API requires the caller to prevent in-process or external mutation while
the mapping is borrowed. Consumers that already create a `memmap2::Mmap` can
transfer ownership with `MappedDemo::from_mmap`.

## Using the crate

After the first release:

    [dependencies]
    pbdems2 = "0.1"

The parser crate can preserve its established public paths by re-exporting the
shared implementation:

    pub mod entity {
        pub use pbdems2::entity::*;
    }

Protobuf decoding belongs in the parser crate. Its error type should wrap both
pbdems2::Error and prost::DecodeError.

## Development

The minimum supported Rust version is 1.88.0. CI also tests current stable Rust
on Linux, macOS, and Windows.

Run the same checks used by CI:

    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
    cargo check -p pbdems2 --no-default-features --locked
    cargo nextest run --workspace --all-features --locked --profile ci
    cargo test --workspace --doc --all-features --locked
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features --locked
    cargo deny --all-features check
    cargo package --locked
    cargo bench -p pbdems2-bench --no-run
    cargo coverage
    cargo coverage-report

Property tests exercise arbitrary bit offsets, varints, field paths, command
streams, and strict entity access. Six libFuzzer targets cover command framing,
bit reads, field paths, serializers, string tables, and entity deltas; see
`fuzz/README.md` and run `cargo +nightly fuzz build` to compile all of them.

Coverage uses cargo-llvm-cov with cargo-nextest. CI exports an LCOV artifact and
enforces a 75% line floor against a measured 79.46% baseline. Benchmark and
test-support modules are excluded from that percentage so helpers cannot inflate
the production-code measurement. The floor should be ratcheted upward as field
decoder, serializer, bit-reader, and playback error-path coverage improves.

## Benchmarks

The private pbdems2-bench workspace crate provides deterministic Criterion
benchmarks for I/O, demo framing and decompression, serializers, field paths,
entities, string tables, class lookup, and coordinate conversion.

Run the full suite locally:

    cargo bench -p pbdems2-bench

See crates/pbdems2-bench/README.md for individual targets, filters, and
Criterion baseline comparisons. CI compiles but does not time the benchmarks.

## Performance and safety

The hot path uses direct slot indexing for entities, FxHash maps for repeatedly
queried trusted strings, reusable decode buffers, and Arc-backed immutable
serializer graphs. Serializer symbols, field indices, bit reads, string-table
payloads, and entity indices are bounds checked before allocation or indexing.

Public input structures use constructors and non-exhaustive layouts so future
protocol fields can be added without forcing a major release. CI checks the
public API against the latest crates.io release with cargo-semver-checks once a
published baseline exists.

Release builds use thin LTO and one codegen unit.
