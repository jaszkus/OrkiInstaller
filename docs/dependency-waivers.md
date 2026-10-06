# Dependency waivers

Exceptions to the dependency policy (100% pure Rust, latest versions) must be registered here: explicit, justified, and time-bound. A PR that introduces a violation without a matching entry must not be merged.

| Item | Rule | Reason | Expires | Owner |
| ---- | ---- | ------ | ------- | ----- |
| `cargo-fuzz` / libFuzzer (dev tool only) | R1 | libFuzzer is C++, but it is a dev-only fuzzing harness and never reaches shipped binaries | 2027-04-30 | @jaszkus |
| `cc` (build script tool, via `blake3` → `cpufeatures`) | R1 | build-time tool only; compiles no C sources for the workspace and adds nothing to shipped binaries; enforced independently by `xtask audit-imports` | 2027-04-30 | @jaszkus |
| `atomic-polyfill` 1.0.3 (RUSTSEC-2023-0089, via `postcard` → `heapless`) | R2 | unmaintained advisory with no safe upgrade; transitive only, await upstream `postcard`/`heapless` refresh; re-check on every dependabot bump | 2027-01-31 | @jaszkus |
| `paste` 1.0.15 (RUSTSEC-2024-0436, via `wgpu` → `wgpu-types`) | R2 | unmaintained advisory with no safe upgrade; proc-macro codegen only, no runtime code of its own beyond expanded output; re-check on every `wgpu` bump | 2027-04-30 | @jaszkus |
| `ttf-parser` 0.25.1 (RUSTSEC-2026-0192, via `iced` → `cosmic-text` → `fontdb`) | R2 | unmaintained advisory; alternative `skrifa` exists but is not wired into `iced 0.14`; re-check on every `iced` bump | 2027-04-30 | @jaszkus |
| XZ Utils `xz` CLI 5.8.3 (dev tool only) | R3 | generates the LZMA2 decoder vectors and the `bcj-x86` pre-filtered bytes of `docs/examples/v1-example-2.bin`; reference encoder only, never linked into shipped binaries; its output is committed and re-verified in CI | 2027-04-30 | @jaszkus |
| Brotli `brotli` CLI 1.2.0 (dev tool only) | R3 | generates the Brotli decoder vectors in `docs/examples/`; reference encoder only, never linked into shipped binaries | 2027-04-30 | @jaszkus |
| Python 3.13 with `blake3`/`brotli` wheels (dev tool only) | R3 | runs the `docs/examples` checkers (reader, negative suite, vector verification) locally and in CI; test tooling only, never shipped | 2027-04-30 | @jaszkus |
