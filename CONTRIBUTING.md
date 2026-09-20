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
cargo bench -p pbdems2-bench --no-run
cargo llvm-cov clean --workspace
cargo coverage
cargo coverage-report
```

Add a focused regression test for parser changes. Prefer a small byte fixture
over checking a real demo into Git. If we need a real demo, we can figure it out in your PR.

Add or update a deterministic benchmark for hot-path changes. Use local
Criterion baselines for comparisons. Hosted CI timing is too noisy. Just report the values in your PR.

## Rust conventions

Shared dependency versions and selected Clippy lints live in the workspace
`Cargo.toml`. Inherit them in member crates; enable additional lints only when
these rules improve this codebase.

- Borrow or iterate over data used only for lookup. Allocate when ownership or
  reuse requires it.
- Use the map entry API for cache insertion, and preserve shared `Arc` ownership.
- Propagate malformed-input errors with `?`. Use `expect` only for documented
  internal invariants, never to validate replay data.
- Preserve decode limits, byte order, game profiles, and partial-update semantics.
- Measure changed hot paths against a saved Criterion baseline and verify real
  replay output when changes affect consumers. Avoid unsafe or forced-inlining
  optimizations without evidence.

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
