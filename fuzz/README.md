# Fuzzing

The fuzz package is a separate Cargo workspace so libFuzzer-only dependencies
never enter the release crate's dependency graph or package archive.

Install nightly Rust and cargo-fuzz, then build every target:

    cargo +nightly install cargo-fuzz
    cargo +nightly fuzz build

Run an individual target until interrupted:

    cargo +nightly fuzz run command_stream

The targets cover outer command framing/decompression, bit primitives, field
paths, flattened serializer validation, string-table updates, and entity delta
decoding. Each target uses deliberately small `DecodeLimits` so malformed
inputs cannot turn into unbounded allocations during a fuzzing campaign.
