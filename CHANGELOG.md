# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Ed25519 payload signing in `orki-pack` (128-byte `ORKISIG` block at the end of the v1 payload): deterministic signing over the first 56 header bytes plus the manifest, `verify_payload_signature` with key-slot lookup and fail-closed errors (`UnsignedPayload`, `UnknownKeySlot`, `SignatureMismatch`, `BadMagic`), and `append_signature` that flips `FLAG_SIGNED`, extends `payload_len`, refreshes the header CRC, and refuses double signing.
- `orki` CLI: `keygen` (Ed25519 seed/pubkey pair to a text file via `getrandom`) and `sign` (in-place signing followed by a self-check against the signer's own pubkey).
- `orki-stub`: signature gate before any manifest read — one trusted key compiled in via `ORKI_TRUSTED_PUBKEY` (hex, slot 0), unsigned payloads accepted only under `debug_assertions`, verification failures exit with `SIGNATURE_FAILURE` (1621).
- `orki-core`: `SIGNATURE_FAILURE` exit code 1621 per `docs/format.md` decision D1.
- `docs/format.md` ratified by the owner (decisions D1-D5: exit 1621 on ORKI-1002, unsigned packages only under debug assertions, ed25519-dalek, independent chunks in v1.0, uninstaller section reserved).
- Payload format v1 implemented in `orki-pack` (spec in `docs/format.md`): 64-byte `ORKIHDR` payload header, offsets relative to the overlay start, PE-section-based payload location (Authenticode-tolerant), decompression caps (`raw_len + 1`), format hard limits enforced before allocation, unsupported-version rejection, and trailing-bytes tolerance for signature tables. Reader keeps read-only v0 footer compatibility; the writer emits v1 only.
- `orki inspect` reports format version, overlay offset, signature state, and integrity; `orki pack`/`wrap` produce v1 payloads and self-check them before writing; `pack` excludes its own output file from the walk.

- Payload format v1 specification (`docs/format.md`, draft for owner ratification): PE-overlay location tolerant of Authenticode, relative offsets, Ed25519 signature block, embedded config/assets sections, hard limits, and a fail-closed validation pipeline.
- `orki-core`: manifest validation rejects `package.compression = "zstd"` with a clear message per ADR-0002 (codec id stays reserved).

- Baseline report (`docs/reports/baseline.md`) capturing verified build/test state, dependency versions, 100% Rust audit results, and stub sizes; ADR-0001 (iced 0.14 + wgpu 27 pairing, no fork) and ADR-0002 (zstd deferred, codec id reserved).
- `xtask check-history` (also part of `xtask ci`) and a `commit-msg` hook that reject commit messages containing bot footers (`Generated with`, `Co-authored-by` for bot identities); rule documented in `CONTRIBUTING.md`.
- `CONTRIBUTING.md` documenting the English-only language policy for all repository artifacts, the no-comments rule for code files, and the contribution workflow.
- `orki-pack`: `verify_integrity` for full-package verification (footer, manifest placement, chunk reachability, per-chunk BLAKE3 + CRC32, per-file raw-size accounting).
- `orki` CLI: `inspect` command with a fail-fast integrity report (exit 1 on any corruption).
- `orki` CLI: `--stub <path>` flag for `pack` and `wrap` that attaches a real stub binary as a PE overlay, producing a runnable `Setup.exe`; every build now self-checks footer, manifest, and chunk integrity before writing the output.

- Workspace skeleton with pinned toolchain (`rust-toolchain.toml`), static CRT targets for x64 and ARM64, and shared lints.
- `xtask` as the single source of truth for CI and local steps: `fmt`, `check`, `lint`, `test`, `build`, `size-budget`, `audit-imports`, `e2e`, `ci`, `doctor`, including a std-only PE import-table auditor and stub size budgets.
- CI workflows: `ci` (lint, tests on windows-latest and windows-11-arm, stub variant builds with size/import gates, e2e smoke), `nightly` (dependency freshness), `release` (tag builds).
- `orki-core`: typed `orki.toml` manifest (serde + TOML, deny unknown fields), strict relative-path sanitizer (traversal, ADS, reserved device names), `Operation`/`EngineEvent` model, MSI-compatible exit codes, cheap-clone variable store.
- `orki-pack`: `.orkipack` container format with fixed-size footer (magic, version, CRC32), postcard manifest, per-chunk BLAKE3 + CRC32 integrity, overlay attachment on a PE stub.
- `orki-pack`: FastCDC content-defined chunking (16/64/256 KiB bounds) and pure-Rust codecs per chunk: LZMA2 (lzma-rust2, preset 6), Brotli (quality 9), and store, with an auto policy that falls back from LZMA2 to Brotli to store when compression yields no gain.
- `orki-stub`: runtime with GUI/silent/passive/uninstall/repair/modify/list/extract/print-config modes, NSIS- and Tauri-compatible flags (`/S`, `/P`, `/D=`, `/UPDATE`, `/R`), feature variants `full`/`lite`/`headless`.
- `orki` CLI: `wrap` (zero-config payload builder), `pack` (directory to `.orkipack` with recursive walk, deterministic ordering, codec selection), `preview` (manifest summary with per-file chunk and codec report), `init` (template and `--tauri` import with tolerant warnings), `check`, `schema` (JSON Schema for `orki.toml`), `doctor`.
- `orki-tauri`: tolerant `tauri.conf.json` importer and Tauri-updater `latest.json` generator.
- `orki-gfx`: shader uniform ABI contract, quality/backend model, software-adapter detection.
- `orki-platform`: platform trait surface (registry, shortcuts, elevation, locations).
- `orki-net`: URL scheme model, resumable download progress/error types.
- `orki-script` and `orki-plugin`: sandbox limits and WASM capability types.
- `orki-ui`: page navigator and theme token types (UI rendering lands with the iced spike).
- Iced 0.14 spike (phase 0): `orki-ui` shader widget (`Program`/`Primitive`/`Pipeline`) rendering the embedded `aurora.wgsl` via wgpu 27, GUI wiring in `orki-stub` with headless fallback, size budgets measured (full/lite 6.67 MiB, headless 0.34 MiB), allowlist entries for system DLLs imported by wgpu/winit/std.
- Dependency policy enforcement: `deny.toml` bans for C/C++-compiling crates, licenses, sources; README (EN/PL), SECURITY.md, CODEOWNERS, PR template, git hooks wired to `xtask`.
