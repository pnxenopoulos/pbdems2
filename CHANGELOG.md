# Changelog

Notable pbdems2 changes live here.

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
