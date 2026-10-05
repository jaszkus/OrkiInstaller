# Baseline report

Generated: 2026-10-05. Scope: full local verification of the repository as merged on
`main` (squash commits #1-#4 bootstrap, #5 phase-0 spike, #7 pack/preview, #8 inspect and
stub overlay). This report answers the questions left open by the external static review.

## Toolchain and platform

- Host: Windows x64 (`x86_64-pc-windows-msvc`), Git Bash.
- Rust: pinned by `rust-toolchain.toml` (1.99.0 at review time), Edition 2024 workspace.
- Local gate: `cargo xtask ci` (fmt check, clippy `-D warnings`, `cargo deny`, workspace
  tests, three stub variants with size budgets and PE import audit, e2e smoke) — **green
  at the time of this report**; CI on GitHub (`windows-latest`, `windows-11-arm`) is
  green on every merged PR (7 required checks each).

## Dependency versions (from Cargo.lock)

| crate | version | notes |
|---|---|---|
| iced | 0.14.0 | stable iced, default wgpu backend |
| wgpu | 27.0.1 | naga 27.0.3 in lockstep |
| naga | 27.0.3 | |
| blake3 | 1.8.7 | no features enabled, pure-Rust neutral backend |
| crc32fast | 1.5.2 | |
| fastcdc | 5.0.0 | v2020 chunker in use |
| lzma-rust2 | 0.21.0 | LZMA2 raw streams, dict pinned on both sides |
| brotli | 9.0.0 | q9 encoder, Decompressor reader |
| postcard | 1.1.3 | manifest encoding |
| toml | 1.1.6+spec-1.1.0 | manifest parsing |
| serde | 1.0.229 | |

The iced 0.14 <-> wgpu 27 pairing is a deliberate phase-0 decision (doc section 2a.3,
option C deferred). ADR-0001 records the current position and the migration trigger.

## 100% Rust audit

- `cargo tree` normal edges: **zero** hits for `cc`, `cmake`, `bindgen`, `pkg-config`,
  `ring`, `aws-lc*`, and no `*-sys` crates that compile C. Present `-sys` crates are
  declarations-only FFI (`windows-sys` 0.52/0.59/0.61, `renderdoc-sys`), consistent with
  the policy (they build no C sources). `deny.toml` bans remain active and green.
- `xtask audit-imports` (PE import table): full/lite import only allowlisted system
  DLLs; headless variant imports 3 DLLs. No CRT, no third-party native libraries.

## Stub sizes (release `ci` profile, after decoder work)

| variant | size | budget |
|---|---|---|
| full | 6.86 MiB | 25 MiB |
| lite | 6.86 MiB | 15 MiB (identical to full on the spike; separation pending) |
| headless | 0.53 MiB | 10 MiB |

## Verified findings from the external review

Confirmed in code: footer read from the last 64 bytes (P0-1), manifest read from
absolute offsets (P0-1), `deny_unknown_fields` on the Tauri importer config (P0-8),
silent/passive/uninstall/repair/modify all route to GUI (P0-7), `Compression::Zstd`
present in the manifest while `orki-pack` cannot encode or decode it (P1-1),
`package.level` ignored (P1-1), `shader_progress_clamped` test without assertions
(P2-1). Also confirmed: `verify_integrity` exists and covers chunks/manifest/footer;
`PackBuilder` output is deterministic (tested); full self-check runs after every
`pack`/`wrap`.

Not confirmed: the review suspected a meaningless assertion in the `orki-gfx` layout
test. The `palette[9 - 2]` assertion checks the guard `index < 8` (no-op write), but it
is written unclearly and is fixed in this milestone for readability.

## Fixed in this milestone (M0.5)

- `orki-ui`: `shader_progress_clamped` now asserts the clamp behavior.
- `orki-gfx`: layout test rewritten to assert the palette-index guard directly.
- Anti-regression gates: `.githooks/commit-msg` and `xtask check-history` reject commit
  messages containing bot footers (`Generated with`, `Co-authored-by` pointing at bot
  identities such as `Codebuff`); rule documented in `CONTRIBUTING.md`.

## Known gaps (input to the milestone plan)

Ordered per the review: payload format v1 relative offsets + Authenticode tolerance
(P0-1), Ed25519 signature root and strict version rejection (P0-2), streaming API
(P0-3), hostile-payload hardening + fuzz targets (P0-4), safe extraction and case
handling (P0-5), config/assets sections (P0-6), silent path through a real engine
(P0-7), tolerant Tauri import with real-world fixtures (P0-8), shader animation state
and uniform slots (P0-9). Zstd decision tracked as ADR-0002 (deferred).

## Startup time

Not yet measured with a formal harness; the GUI smoke test shows the iced window alive
15 s+ with zero GPU fallback. A measured number lands with the phase-0/1 report
(milestone M5 budget). No claim is made here.
