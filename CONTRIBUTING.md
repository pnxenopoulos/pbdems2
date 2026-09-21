# Contributing

Thank you for considering for helping with pbdems2.

## Before opening a pull request

Install the pinned Rust toolchain, cargo-nextest, and cargo-deny. Then run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo check -p pbdems2 --no-default-features --locked
cargo nextest run --workspace --all-features --locked --profile ci
cargo test --workspace --doc --all-features --locked
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features --locked
cargo deny --all-features check
cargo package --locked
cargo bench -p pbdems2-bench --locked -- --test
cargo llvm-cov clean --workspace
cargo coverage
cargo coverage-report
```

Add a focused regression test for parser changes. Prefer a small byte fixture
over checking a real demo into Git. If we need a real demo, we can figure it out in your PR.

Add or update a deterministic benchmark for hot-path changes. Use
`cargo criterion -p pbdems2-bench --locked` for local measurements and report
the values in your PR. See the [benchmark guide](crates/pbdems2-bench/README.md)
for setup, filters, and comparisons. Hosted CI only smoke-tests the suite.

## Rust conventions

Shared dependency versions and selected Clippy lints live in the workspace
Cargo.toml. Inherit them in member crates. Use borrowed data for lookups, the map
entry API for cache insertion, and errors for malformed replay input.
Preserve decode limits, game profiles, and partial-update behavior when optimizing.

## Release checklist

1. Reconcile with the latest published version and keep its regression tests.
2. Update the library and CLI versions together, refresh Cargo.lock, and add the
   matching version heading in CHANGELOG.md.
3. Run the checks above and verify API compatibility against the published crate.
   A dirty working tree can use cargo package --locked --allow-dirty locally.
4. Merge to main and wait for CI Check on that exact commit, including macOS and
   Windows. Then dispatch Release pbdems2 from main.

Benchmark results are not release guarantees. Keep game-specific changes and
their release notes in the consumer repositories.

## Scope

pbdems2 owns game-neutral Source 2 code such as framing, I/O, serializers,
fields, string tables, and entities.

Generated protobufs, events, game constants, domain models, and language
bindings stay in Awpy, Boon, or another consumer. Game-specific wire behavior
should be a neutral `DecodeProfile` option, not a game name in this crate.
Basically, do not put anything game-specific in pbdems2!

## Compatibility

Note public API changes in `CHANGELOG.md` and add migration notes when useful.
Prefer private fields with constructors and accessors. Mark extensible public
types `#[non_exhaustive]` before release.
