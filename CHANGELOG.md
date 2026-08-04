# Changelog

Notable pbdems2 changes live here.

## v0.2.1

- Restored Source 2's last-definition-wins behavior for duplicate flattened
  serializer names. Valid Counter-Strike 2 and Deadlock demos contain these
  duplicates, so rejecting them prevented both games from decoding signon.

## v0.2.0

- Added non-borrowing `PreparedPlayback` seeds that decode signon and build the
  header-only index once for repeated playback, seeking, and segments.
- Added consuming `PlaybackSession` runs with independent parser and adapter
  state, including parallel-session support when checkpoint state is
  `Send + Sync`.
- Added `CheckpointAdapter` so games preserve semantic signon state without
  cloning large scratch buffers or per-tick output.
- Added adapter-aware tick callbacks for game-specific state such as Boon event
  batches while keeping protobuf and event types outside the core crate.
- Added prepared/cold parity, filtering, seeking, segment, callback-failure,
  identity, isolation, and concurrency tests.
- Added prepared-demo identity and decode-limit validation, retained the first
  synchronization boundary in indexes, and documented full-packet segment
  correctness requirements.

## v0.1.0

- Split the shared Source 2 parser into a standalone, game-neutral crate.
- Added PBDEMS2 headers, command framing, packet framing, Snappy decoding, I/O,
  serializers, fields, entities, string tables, and coordinates.
- Kept generated protobufs and Prost out of the crate. Games use
  `DecodeProfile` and `DemoAdapter` instead.
- Added playback setup, tick callbacks, fallible callbacks, filtering,
  full-packet seeking, segmented parsing, and structured errors.
- Added decode limits and validated public constructors for untrusted input.
- Added optional Serde support and zero-copy memory-mapped input.
- Added the `pbdems2` inspector CLI with JSON output and native release builds.
- Added Criterion benchmarks, Proptest coverage, and cargo-llvm-cov with a 75%
  line floor.
- Added Rustfmt, Clippy, rustdoc, cargo-nextest, cargo-deny, MSRV, package, and
  semver checks in CI.
- Added manual crates.io publishing and GitHub releases for Linux, macOS, and
  Windows CLI archives.
- Reduced entity-create allocations by sharing class names with `Arc<str>`.
- Documented the public API and examples for the main entry points.
