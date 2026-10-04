# Dependency waivers

Exceptions to the dependency policy (100% pure Rust, latest versions) must be registered here: explicit, justified, and time-bound. A PR that introduces a violation without a matching entry must not be merged.

| Crate | Rule broken | Reason | Expires | Owner |
| ----- | ----------- | ------ | ------- | ----- |
| `cargo-fuzz` / libFuzzer (dev tool only) | R1 | libFuzzer is C++, but it is a dev-only fuzzing harness and never reaches shipped binaries | 2027-04-30 | @jaszkus |

No runtime or build-time C/C++ dependencies are currently waived.
