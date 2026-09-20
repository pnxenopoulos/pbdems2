# Changelog

Notable pbdems2 changes live here.

## Unreleased

- Resolve serializer names with borrowed iterators and build dotted names in one
  string. Unrepresentable nested paths and invalid packed depths return `None`.
- Reuse field-cache entries without repeated map lookups, and share dynamic-array
  serializer construction while preserving decoder metadata.
- Centralize shared workspace dependencies and selected Rust lint conventions.

- Return parse errors for field-path operations that exceed seven nesting levels,
  pop past the root, or increment a nonexistent parent, instead of panicking on
  malformed entity updates.

## v0.3.0

- Added ordered per-tick entity lifecycle records for creation, updates,
  reactivation, leaving the PVS, deletion, and slot replacement.
- Added compact deletion tombstones with entity index, serial, class ID, and a
  shared class name. Replacements report the old deletion before the new
  creation.
- Added `EntityId { index, serial }` with helpers on entities and lifecycle
  records for reliable keys across slot reuse.
- Preserved entity serial numbers from the wire instead of leaving them at
  zero.
- Aligned filtered and unfiltered lifecycle behavior. Skipped classes stay
  silent, while tracked entities keep complete transition records.
- Defined callback clearing, sign-on baseline, and full-packet keyframe
  behavior without changing the existing `updated_indices` API.
- Added optional Serde serialization for entity identities and lifecycle
  records.
- Added focused lifecycle, identity, filtering, ordering, callback, and Serde
  tests, plus lifecycle-aware packet-entity benchmarks.

## v0.2.2

- Added a multi-page PBDEMS2 format and parser architecture guide to the
  docs.rs API documentation, covering command and packet framing, serializers,
  string tables, entity lifecycles and handles, adapters, seeking, and prepared
  playback.
- Added `PreparedPlayback::segment_plan`, which partitions a demo into a
  bounded, never-empty set of independently decodable ranges at full-packet
  keyframes. Planning accounts for the post-signon baseline and tail intervals
  and centralizes boundary arithmetic for parallel consumers.
- Added `PacketMessageFrame::payload_or_copy`, which borrows aligned protobuf
  payloads and otherwise reconstructs them in a caller-owned reusable scratch
  buffer.

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
