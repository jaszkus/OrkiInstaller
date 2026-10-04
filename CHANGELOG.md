# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Workspace skeleton with pinned toolchain (`rust-toolchain.toml`), static CRT targets for x64 and ARM64, and shared lints.
- `xtask` as the single source of truth for CI and local steps: `fmt`, `check`, `lint`, `test`, `build`, `size-budget`, `audit-imports`, `e2e`, `ci`, `doctor`, including a std-only PE import-table auditor and stub size budgets.
- CI workflows: `ci` (lint, tests on windows-latest and windows-11-arm, stub variant builds with size/import gates, e2e smoke), `nightly` (dependency freshness), `release` (tag builds).
- `orki-core`: typed `orki.toml` manifest (serde + TOML, deny unknown fields), strict relative-path sanitizer (traversal, ADS, reserved device names), `Operation`/`EngineEvent` model, MSI-compatible exit codes, cheap-clone variable store.
- `orki-pack`: `.orkipack` container format with fixed-size footer (magic, version, CRC32), postcard manifest, per-chunk BLAKE3 + CRC32 integrity, overlay attachment on a PE stub.
- `orki-stub`: runtime with GUI/silent/passive/uninstall/repair/modify/list/extract/print-config modes, NSIS- and Tauri-compatible flags (`/S`, `/P`, `/D=`, `/UPDATE`, `/R`), feature variants `full`/`lite`/`headless`.
- `orki` CLI: `wrap` (zero-config payload builder), `init` (template and `--tauri` import with tolerant warnings), `check`, `schema` (JSON Schema for `orki.toml`), `doctor`.
- `orki-tauri`: tolerant `tauri.conf.json` importer and Tauri-updater `latest.json` generator.
- `orki-gfx`: shader uniform ABI contract, quality/backend model, software-adapter detection.
- `orki-platform`: platform trait surface (registry, shortcuts, elevation, locations).
- `orki-net`: URL scheme model, resumable download progress/error types.
- `orki-script` and `orki-plugin`: sandbox limits and WASM capability types.
- `orki-ui`: page navigator and theme token types (UI rendering lands with the iced spike).
- Dependency policy enforcement: `deny.toml` bans for C/C++-compiling crates, licenses, sources; README (EN/PL), SECURITY.md, CODEOWNERS, PR template, git hooks wired to `xtask`.
