# Contributing

Thank you for improving pbdems2.

## Before opening a pull request

Install the pinned Rust toolchain plus cargo-nextest and cargo-deny, then run:

    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
    cargo check -p pbdems2 --no-default-features --locked
    cargo nextest run --workspace --all-features --locked --profile ci
    cargo test --workspace --doc --all-features --locked
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features --locked
    cargo deny --all-features check
    cargo package --locked
    cargo bench -p pbdems2-bench --no-run
    cargo +nightly fuzz build
    cargo llvm-cov clean --workspace
    cargo coverage
    cargo coverage-report

Add focused regression tests for parser changes. Demo fixtures should remain
outside Git when a minimal byte sequence can reproduce the behavior.

Performance-sensitive changes should update or extend the benchmark crate
with deterministic fixtures. Record local Criterion baselines when comparing
implementations; do not treat hosted CI timings as performance evidence.

## Repository boundary

Keep PBDEMS2 framing, I/O, serializers, field decoding, string tables,
entities, and other game-neutral Source 2 mechanics here.

Generated protobufs, protobuf conversion, events, domain models, game
constants, and language bindings belong in consumer repositories such as Awpy
or Boon.

When a wire convention differs by game, expose a neutral policy option on
DecodeProfile and test each behavior. Do not add game names, generated message
types, or copied game modules to pbdems2.

## Compatibility

pbdems2 follows semantic versioning. Note intentional public API changes in
CHANGELOG.md and add migration guidance when appropriate. Prefer private fields
plus constructors and accessors for public input types, and mark extensible
enums and data-transfer structs `#[non_exhaustive]` before their first release.

CI runs cargo-semver-checks against the latest published release. Run the same
check locally for public API changes when a crates.io baseline exists.
