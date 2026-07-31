# Changelog

All notable changes to pbdems2 will be documented here.

## v0.1.0

- Prepared the initial standalone pbdems2 crate.
- Consolidated Source 2 command framing, I/O, serializers, field decoding,
  entities, string tables, errors, and coordinate conversion.
- Replaced title-named decoder dialects with a game-supplied DecodeProfile.
- Removed generated CS2 and Deadlock protobuf crates and the Prost dependency.
- Added strict formatting, Clippy, rustdoc, cargo-nextest, cargo-deny, MSRV,
  cross-platform CI, package verification, and trusted crates.io publishing.
- Added a private Criterion benchmark crate covering the core parsing hot paths.
- Added a protobuf-independent command iterator, header-only seek index, and
  parser driver with signon initialization, full-packet seeking, tick callbacks,
  segmented playback, and entity-class filtering.
- Added configurable resource limits across command, serializer, string-table,
  field, and entity decoding, with structured allocation and command errors.
- Hardened the public API with validated constructors, private container state,
  strict typed and UTF-8-preserving entity accessors, non-exhaustive extensible
  types, and cargo-semver-checks release/CI gates.
- Made Serde support a default-but-optional feature and verified the minimal
  feature set in CI.
- Added Proptest invariants plus six cargo-fuzz targets and a bounded scheduled
  fuzzing workflow.
- Added fallible `try_run_to_end`, `try_run_to_end_filtered`, and
  `try_decode_segment` callbacks that preserve application errors and stop
  playback immediately.
- Added optional zero-copy large-file support through an owning `MappedDemo`
  wrapper around `memmap2`.
- Added cargo-llvm-cov aliases, LCOV CI artifacts, and a ratchetable 75% line
  coverage floor.
- Added behavior-focused entity lifecycle, filtered parsing, baseline, string
  table, and field-path test suites. The field-path suite drives every opcode
  through the real Huffman decoder; the string-table suite covers history,
  sparse, fixed-width, variable-width, and compressed payloads. Production
  line coverage is 79.46%, excluding benchmark and test-support code.
- Documented every public item, denied `missing_docs` in both library crates,
  and added runnable examples on the crate root, `Demo`, `DecodeLimits`, and
  `MappedDemo`.
- Labelled feature-gated items on docs.rs with `doc(cfg(...))`.
- Unpinned the development toolchain from the MSRV so CI genuinely tests
  current stable on Linux, macOS, and Windows, and forced each test-matrix row
  to its intended toolchain.
- Fixed the fuzz workflow, which silently ran the pinned stable toolchain and a
  musl target instead of nightly and the glibc host.
- Made releases manual and main-only: publish is dispatched by hand, gated on
  CI, and tags the released commit afterwards.
- Added crates.io, docs.rs, CI, and license badges to the README.
