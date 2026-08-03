# Security policy

## Supported versions

The latest released minor version receives security fixes. Before the first
crates.io release, `main` is the supported version (i.e., `main` is in development).

## Reporting a vulnerability

Include a small reproducer, the affected version or commit, and your platform
and toolchain.

## Untrusted demos

Treat demos as untrusted input. Keep the default `DecodeLimits` or choose limits
that fit your environment. Higher limits give malicious files more CPU and
memory to work with.

Memory-mapped files must stay unchanged and untruncated while mapped. That is
why `MappedDemo::open` is unsafe. Only map files you can keep stable.
