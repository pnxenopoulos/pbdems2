# Security policy

## Supported versions

Security fixes are provided for the latest released minor version of pbdems2.
Until the first crates.io release, the `main` branch is the supported version.

## Reporting a vulnerability

Please use GitHub's private vulnerability reporting for the pbdems2 repository.
Do not open a public issue for a parser crash, denial-of-service vector, memory
exhaustion issue, or other vulnerability until a fix is available. Include the
smallest reproducing input you can share, the affected version or commit, and
the platform/toolchain used.

## Untrusted demo data

Demo files are untrusted input. Applications should retain the default
`DecodeLimits` or choose explicit limits appropriate for their environment.
Raising limits may increase the CPU and memory available to a malicious input.
The fuzz targets in `fuzz/` exercise the primary decode boundaries with reduced
limits and AddressSanitizer.

File-backed memory maps require the underlying file to remain unchanged and
untruncated for the mapping's lifetime. `MappedDemo::open` is therefore unsafe
and delegates that invariant to its caller; applications should map immutable
or otherwise protected demo files.
