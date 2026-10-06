# M1 report: stub size cost of Ed25519 verification

Owner requirement (2026-10-05 review, Q3 and decision 3): measure the stub cost canonically
with the `release` profile (fat LTO, `codegen-units = 1`), treat any `codegen-units = 16`
number as indicative, attribute the cost, and compare against a smaller implementation (for
example `ed25519-compact`) only if the canonical cost exceeds roughly 100 KiB.

## Method

- Build: `cargo xtask build --variant full --profile <P>` followed by
  `cargo xtask size-budget --variant full --profile <P>`; `release` is the profile
  `.github/workflows/release.yml` uses.
- Baseline: commit `0266103` (PR #11, the commit immediately before the Ed25519 PR #12),
  checked out in a temporary `git worktree` and built with the same profile and a shared
  `CARGO_TARGET_DIR`, so only the changed crates recompile. The baseline tree contains no
  `ed25519` reference at all (verified in its `Cargo.lock` and sources), so the pair isolates
  the Ed25519 change.
- Key presence: the stub's `trusted_keys()` returns `Vec::new()` unless
  `ORKI_TRUSTED_PUBKEY` is set at compile time. Both variants are measured: without the
  variable (what today's CI produces) and with a dummy 32-byte key (the shipping
  configuration). A `touch` on the stub source forces the recompile, because Cargo does not
  track environment variables.
- Link-path proof: the signature error strings (`grep -c ORKI-1002`) are present in the new
  binary and absent in the baseline.
- Section delta: a scratch PE section-header dump (virtual size per section) over the saved
  binaries.
- Isolated probe: a scratch crate with the same profile settings
  (`opt-level = "s"`, `lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, `strip = true`),
  one bin without and one with a reachable `verify_strict` call, to bound the cost of the
  verification path alone.
- No per-crate attribution: `strip = true` in the `release` profile removes symbols, so
  `cargo bloat` cannot attribute code; the commit-pair delta plus the section delta are used
  instead.

## Results (2026-10-06, variant `full`, one machine)

| profile | trusted key | baseline `0266103` | `main` (`e58365b`) | delta |
|---|---|---|---|---|
| release (canonical) | none | 6 252 032 B (5.962 MiB) | 6 254 592 B (5.965 MiB) | +2 560 B |
| release (canonical) | present | 6 252 032 B (5.962 MiB) | 6 319 104 B (6.026 MiB) | **+67 072 B (+65.5 KiB)** |
| ci (thin LTO, cgu=16) | none | 7 201 792 B (6.868 MiB) | 7 270 400 B (6.933 MiB) | +68 608 B |
| ci (thin LTO, cgu=16) | present | 7 201 792 B (6.868 MiB) | 7 270 912 B (6.933 MiB) | +69 120 B |
| isolated probe (release settings) | n/a | 116 224 B | 179 712 B | +63 488 B (+62 KiB) |

Section delta, canonical `release` profile, baseline -> shipping configuration:

| section | delta |
|---|---|
| .text | +45 360 B |
| .rdata | +20 936 B |
| .pdata | +396 B |
| .reloc | -8 B |

Budget: `cargo xtask size-budget --variant full --profile release` reports 5.96 MiB for the
no-key build and 6.03 MiB for the keyed build against the 25 MiB full budget (both pass).
The no-key `release` build reproduces byte-exactly on rebuild (6 254 592 B twice).

## Findings

1. Canonical cost of Ed25519 verification in the shipping configuration: **+65.5 KiB**
   (+67 072 B, +1.07%), of which 45 KiB is code and 21 KiB is constant data. This is below
   the owner's ~100 KiB threshold, so `ed25519-dalek` 3.0.0 stays; no switch to
   `ed25519-compact` is proposed.
2. The key-free `release` build measures only +2 560 B because fat LTO removes the whole
   verification path: with an empty key list the verification call is unreachable code. The
   `ci` profile (thin LTO, `codegen-units = 16`) keeps it, which is why its delta is ~68 KiB
   regardless of the key. **Size gates that build without a key understate the shipping
   cost by 26x**; the release workflow must assert that a key is present (and, after rc2,
   that the `.orkikey` section exists).
3. Today no workflow and no repository file sets `ORKI_TRUSTED_PUBKEY`, so a release stub
   built by the current pipeline cannot accept any signed payload ("key slot is not
   trusted"). Logged in issue #13 with the other rc2 divergences.
4. The previously recorded "+123 KiB (6.03 -> 6.14 MiB at codegen-units = 16)" is **not
   reproducible** under any configuration measured here (the ci deltas are +67 KiB, the
   canonical release delta is +65.5 KiB, and the absolute sizes differ) and is withdrawn;
   the CHANGELOG and `docs/format.md` now quote the canonical numbers.
5. Dependency checks for the recorded decision: `ed25519-dalek` 3.0.0 in `Cargo.lock` is a
   stable release (no pre-release suffix, `curve25519-dalek` 5.0.0), license BSD-3-Clause is
   already in the `deny.toml` allowlist, MSRV 1.85 is below the pinned 1.99 toolchain, and
   `verify_strict` exists in this version. The merged code calls `verify` instead of
   `verify_strict`; that divergence is part of issue #13.

## Caveats

- One machine (Windows 11, i5-7500), one toolchain (rustc 1.99.0), one run per build except
  the no-key `release` build, which reproduced exactly.
- `cargo bloat` is not installed and the `release` profile strips symbols, so attribution is
  by commit pair, section, and isolated probe rather than per crate.
- The dummy trusted key is not a valid curve point; it only forces the code path to be
  linked. The size effect does not depend on the key value.
- The window between `0266103` and `e58365b` contains only PR #12 (Ed25519 signing), so the
  delta is not diluted by unrelated changes.

## Reproduce

```bash
cargo xtask build --variant full --profile release
cargo xtask size-budget --variant full --profile release
touch bins/orki-stub/src/main.rs
ORKI_TRUSTED_PUBKEY=$(printf 'ab%.0s' {1..32}) cargo xtask build --variant full --profile release
```
