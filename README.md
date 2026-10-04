# OrkiInstaller (orki)

Rust-native installer builder with an iced + wgpu UI — an NSIS alternative, designed Tauri v2-first.

## Status

Bootstrap phase (roadmap phase 0). The workspace builds and tests green; the packing engine, UI, and shader system are under active development.

- [x] Workspace skeleton, pinned toolchain, CI
- [x] `orki-core`: typed manifest (`orki.toml`), path sanitization, operation/transaction model, MSI exit codes
- [x] `orki-pack`: `.orkipack` container format (footer, manifest, chunk integrity with BLAKE3 + CRC32)
- [x] `orki-stub`: runtime with silent/passive/uninstall modes and NSIS/Tauri-compatible flags
- [x] `orki` CLI: `wrap`, `init` (incl. `--tauri`), `check`, `schema`, `doctor`
- [ ] iced + wgpu UI and shader system (phase 0 spike)
- [ ] Transactional engine: plan → validate → execute → rollback (phase 1)
- [ ] Delta updates, Rhai/WASM scripting (phase 3)

## Requirements

- Rust stable (see `rust-toolchain.toml` for the pinned version)
- Windows 10/11 (x64 or ARM64) — MSVC Build Tools with Windows SDK
- `cargo-deny` and `cargo-nextest` (optional locally, enforced in CI)

## Getting started

```sh
cargo xtask check      # fmt + check + clippy (fast loop)
cargo xtask ci         # everything CI runs
cargo run -p orki-cli -- wrap my-app.exe -o Setup.exe
cargo run -p orki-cli -- init --tauri src-tauri/tauri.conf.json
cargo run -p orki-cli -- check
cargo run -p orki-cli -- schema > orki.schema.json
```

## Variants

`orki-stub` builds with one of three feature variants:

| Variant | Contents |
| --- | --- |
| `full` | iced + wgpu + shaders + Rhai + WASM (default) |
| `lite` | iced + tiny-skia software renderer, no shaders/WASM |
| `headless` | engine only, no UI |

```sh
cargo xtask build --variant full --profile ci
cargo xtask size-budget --variant full --profile ci
cargo xtask audit-imports --variant full --profile ci
```

## Dependency policy

100% pure Rust: no `*-sys` crates compiling C/C++, no `cc`/`cmake`/`bindgen`/`pkg-config` in the tree (OS API FFI via `windows`/`windows-sys` is the only exception). All dependencies stay on the latest stable release; exceptions require a dated entry in `docs/dependency-waivers.md`. Enforced in CI with `cargo deny check` plus an import-table audit of every produced stub.

## CI

GitHub Actions (`.github/workflows/ci.yml`): lint, tests on `windows-latest` + `windows-11-arm`, builds of all three stub variants with size budgets and import audits, and an e2e smoke run. `nightly` checks dependency freshness; `release` builds on `v*` tags.

## License

Dual-licensed under MIT or Apache-2.0 (see `LICENSE-MIT`, `LICENSE-APACHE`).
