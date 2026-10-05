# ADR-0002: zstd codec deferred

Status: accepted (proposed for ratification by the owner)

Date: 2026-10-05

## Context

The payload format reserves codec id 3 for zstd (design section 5) and the manifest
already declares `Compression::Zstd`, but no implementation exists: `CodecPolicy` has
no zstd variant, the packer cannot encode it, the stub cannot decode it, and
`orki check` does not reject it. The 100% Rust policy (2a.5) restricts the choice:
`ruzstd` is a decoder only; pure-Rust zstd encoders are immature. The default codecs
(LZMA2 for ratio, Brotli for web assets and speed) are implemented and verified.

## Decision

Defer zstd. `orki check` treats `compression = "zstd"` as an error with a clear message
until an implementation lands. The codec id 3 stays reserved in the format so a future
implementation remains wire-compatible. Revisit when a pure-Rust zstd encoder reaches
production quality or when a measured workload shows LZMA2+Brotli losing to zstd on
decode speed by a margin that matters (milestone M1.5 benchmark).

## Consequences

- Manifest validation rejects `zstd` today (P1-1 acceptance closed by rejection).
- Format documentation lists codec 3 as reserved, not implemented.
- No new dependency is added.
