# Fuzzing

The fuzz package is a separate Cargo workspace so libFuzzer-only dependencies
never enter the release crate's dependency graph or package archive.

Install nightly Rust and cargo-fuzz, then build every target:

    cargo +nightly install cargo-fuzz
    cargo +nightly fuzz build

Run an individual target until interrupted:

    cargo +nightly fuzz run command_stream

The `+nightly` prefix is required: `rust-toolchain.toml` pins the stable MSRV
for normal builds, and a toolchain file outranks `rustup default`, so without it
cargo-fuzz fails on `-Zsanitizer=address`.

If cargo-fuzz was installed as a prebuilt binary rather than built from source,
it is musl-linked and defaults to a musl target that cannot be instrumented.
Pass the glibc host triple explicitly, the way CI does:

    cargo +nightly fuzz build --target x86_64-unknown-linux-gnu

The targets cover outer command framing/decompression, bit primitives, field
paths, flattened serializer validation, string-table updates, and entity delta
decoding. Each target uses deliberately small `DecodeLimits` so malformed
inputs cannot turn into unbounded allocations during a fuzzing campaign.
