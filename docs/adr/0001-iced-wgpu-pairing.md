# ADR-0001: iced and wgpu pairing

Status: accepted (proposed for ratification by the owner)

Date: 2026-10-05

## Context

Design rule R2 (section 2a) requires the latest stable versions. iced 0.14.0 pins wgpu
27; the latest wgpu line is 30.x. Section 2a.3 lists four options: A) stable iced with
its pinned wgpu, B) fork iced for the newest wgpu now, C) fork later when a needed
feature is blocked, D) drop iced.

Phase 0 (spike) ran on option A: iced 0.14.0 + wgpu 27.0.1. Everything the installer
needs today works through the public iced shader API; nothing in the current roadmap
requires a wgpu feature newer than 27.

## Decision

Stay on option A. Do not fork iced now.

Migration trigger (re-evaluate when any of these happens): a shader feature we need is
only available on a newer wgpu; iced lags two or more wgpu major versions; a blocking
bug in wgpu 27 is fixed only upstream. At that point execute option C per design
2a.3: fork as `orki-iced`, isolate in `orki-gfx`/`orki-ui`, automate the rebase in the
nightly job.

## Consequences

- `orki-gfx` and `orki-ui` keep iced/wgpu imports behind thin wrappers so a later fork
  swap is a mechanical change.
- Nightly dependency-freshness workflow keeps reporting the iced/wgpu delta.
- The "latest wgpu" rule (R2) is waived for this pair; the waiver is tracked here, not
  in dependency-waivers.md, because it is an architectural choice, not a crate ban.
