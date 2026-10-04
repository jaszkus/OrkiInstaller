# Dependency waivers

Exceptions to the dependency policy (100% pure Rust, latest versions) must be registered here: explicit, justified, and time-bound. A PR that introduces a violation without a matching entry must not be merged.

| Item | Rule | Reason | Expires | Owner |
| ---- | ---- | ------ | ------- | ----- |
| `cargo-fuzz` / libFuzzer (dev tool only) | R1 | libFuzzer is C++, but it is a dev-only fuzzing harness and never reaches shipped binaries | 2027-04-30 | @jaszkus |
| `cc` (build script tool, via `blake3` → `cpufeatures`) | R1 | build-time tool only; compiles no C sources for the workspace and adds nothing to shipped binaries; enforced independently by `xtask audit-imports` | 2027-04-30 | @jaszkus |
| `atomic-polyfill` 1.0.3 (RUSTSEC-2023-0089, via `postcard` → `heapless`) | R2 | unmaintained advisory with no safe upgrade; transitive only, await upstream `postcard`/`heapless` refresh; re-check on every dependabot bump | 2027-01-31 | @jaszkus |
