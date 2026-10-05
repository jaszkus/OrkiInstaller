# OrkiInstaller payload format v1

Status: **draft for owner ratification** (M1.1). Implementation follows only after this
document is approved. Version 0 (the current prototype in `orki-pack`) is documented at
the bottom for reference; v1 supersedes it.

Source of truth for intent: `OrkiInstaller.md` section 5. This document freezes the
byte-level contract. Any change after ratification requires a new ADR and a format
version bump.

## Goals

1. Survive Authenticode signing **after** packaging: the signature is appended to the
   PE by `signtool` and must not invalidate payload location or integrity.
2. Authenticate before use: an Ed25519 signature over the manifest root, with the
   public key baked into the stub at packaging time. CRC32 and BLAKE3 are integrity
   checks, not trust anchors.
3. Stream: read and extract without loading the whole installer into RAM.
4. Fail closed: unknown format versions, out-of-range counts, oversized allocations,
   and decompression bombs are rejected before any allocation or disk write.
5. Stay delta-ready: content-defined chunks remain the deduplication unit.

## Container layout

```
+---------------------------------------------+
| PE stub (orki-stub.exe, signed as a whole)  |
+---------------------------------------------+ <- overlay_start = end of last PE section (raw)
| PAYLOAD HEADER            64 bytes, fixed   |
| STRING TABLE              (optional, align) |
| MANIFEST                  postcard, versioned|
| FILE TABLE                inside manifest   |
| CHUNKS                    data chunks       |
| ASSETS                    themes/fonts/i18n |
| UNINSTALLER               small PE, optional|
| SIGNATURE BLOCK           128 bytes, fixed  |
+---------------------------------------------+ <- payload_end = overlay_start + payload_len
| [ Authenticode certificate table ]          |   appended by signtool AFTER packaging
| [ padding: anything ]                       |   ignored by the reader
+---------------------------------------------+
```

The reader locates the payload through the PE header (end of the last section's raw
data), never through the file end. The Authenticode table and any trailing padding are
therefore invisible to the format. A payload packed without a stub is a valid degenerate
file whose "overlay" starts at offset 0 (the CLI `pack`/`wrap` output keeps working).

### Payload header (64 bytes, little-endian)

| offset | size | field |
|---|---|---|
| 0 | 8 | magic `ORKIHDR` |
| 1.. | | |
| 8 | 4 | format_version (u32) = 1 |
| 12 | 4 | header_size (u32) = 64 |
| 16 | 8 | payload_len (u64): overlay_start .. overlay_start + payload_len = payload_end |
| 24 | 8 | manifest_offset (u64, relative to overlay_start) |
| 32 | 8 | manifest_len (u64) |
| 40 | 4 | manifest_crc32 (CRC32 of the postcard bytes) |
| 44 | 4 | flags (bit 0: signed, bit 1: has string table, bit 2: has assets, bit 3: has uninstaller) |
| 48 | 8 | string_table_offset (u64, relative to overlay_start; 0 when absent) |
| 56 | 4 | header_crc32 (CRC32 over bytes 0..56) |
| 60 | 4 | reserved (0) |

Notes: offsets inside the payload are **relative to overlay_start**, never to the file.
The string table embeds interned strings (design 18b.4) in a later minor version; v1.0
writes zero length and readers must skip it.

### Signature block (128 bytes, at payload_end - 128, relative)

| offset | size | field |
|---|---|---|
| 0 | 8 | magic `ORKISIG` |
| 8 | 1 | algorithm = 1 (Ed25519) |
| 9 | 1 | key_slot (0-255, which baked-in public key signed this payload) |
| 10 | 2 | reserved |
| 12 | 64 | Ed25519 signature over: payload header bytes 0..56 + manifest bytes |
| 76 | 4 | reserved |
| 80 | 44 | reserved (0) |

The signed message is `header[0..56] || manifest_bytes`. This binds the manifest
(offsets, hashes, codec choices, app metadata) and the header fields together. Chunk
bytes are covered transitively through BLAKE3 hashes in the manifest (Merkle-style
single-level tree; a deep tree is deferred until delta updates need one).

### Manifest (postcard)

```text
PackManifest v2 {
  schema: u32 = 2,
  app_id, app_name, app_version,
  signer_key_slot: u8,
  config_offset: u64, config_len: u64,      embedded compiled orki.toml (postcard)
  assets: Vec<AssetEntry>,                  typed assets (theme, icon, license, i18n, shader)
  files: Vec<FileEntry>,                    application files
  chunks: Vec<Chunk>,                       deduplicated by content hash
}
FileEntry { path: u32 (string table index), offset: u64, size: u64, chunk_start: u32, chunk_count: u32 }
AssetEntry { kind: u8, path: u32, offset: u64, size: u64, chunk_start: u32, chunk_count: u32 }
Chunk { codec: u8, comp_len: u32, raw_len: u32, blake3: [u8; 32], crc32: u32 }
```

String table: v1.0 writes `flags.has_string_table = 0` and stores `path` as
`u32::MAX`-free plain indices into an empty table is not allowed; instead v1.0 embeds
`path` as a postcard `String` directly in `FileEntry` and the `u32` index form activates
in v1.1 with the string table. This keeps v1.0 self-contained while reserving the wire
shape.

Chunk deduplication: identical BLAKE3 hashes reuse one chunk; `chunk_start` ranges of
different files may point at the same chunk index. A reader verifies every referenced
chunk once and counts it per reference for accounting.

### Validation pipeline (in order, fail closed)

1. Locate PE overlay start via section headers. If the file is not a PE (payload-only
   artifacts), overlay_start = 0.
2. Read the 64-byte payload header at overlay_start; verify magic, format_version == 1
   (reject >1 and <1 with `ORKI-1002`), header CRC32.
3. Bounds-check `payload_len` against the actual file (payload_end must be
   `<= file_len`; a signature is allowed to sit exactly at payload_end).
4. Read manifest at `[overlay_start + manifest_offset, +manifest_len)`; verify CRC32.
5. If `flags.signed`: read the signature block, verify Ed25519 over
   `header[0..56] || manifest` against the public key for `key_slot` **before any
   extraction**. Unsigned payloads are rejected in release builds; a `--unsigned-dev`
   escape hatch exists for local dev builds only (debug_assertions).
6. Walk the manifest and validate structural invariants: file/asset offsets sorted,
   non-overlapping, inside payload; chunk indices in range; `raw_len` sum per file ==
   `size`; dedup consistency. **No allocation happens before this step.**
7. Extract or verify on demand, chunk by chunk, with a decompression limit of
   `raw_len + 1` bytes (rejecting bombs), re-checking BLAKE3 + CRC32 per chunk.

### Hard limits (constants in orki-pack, all enforced)

| limit | value |
|---|---|
| max files | 100_000 |
| max chunks | 400_000 |
| max assets | 10_000 |
| max path length | 1024 UTF-16 units |
| max raw chunk | 16 MiB |
| max file size | 4 GiB |
| max total raw size | 64 GiB |
| max manifest_len | 256 MiB |
| max decompression output | raw_len + 1 |
| max signature block | 128 B fixed |

## CLI behavior

- `orki pack` builds a v1 payload; `--sign <keyfile>` produces a signed payload
  (Ed25519 via a pure-Rust crate; keygen helper `orki keygen`).
- `orki-stub` refuses to run a payload whose signature does not verify (exit
  `ORKI-1002` mapping to MSI 1621? decision below).
- `orki inspect` reports: format version, signed/not, key slot, integrity, limits hit.
- `orki preview` gains `--json` output for tooling.

## Open questions for the owner (block implementation)

1. **Exit code for signature failure**: propose MSI 1621 (ERROR_INSTALL_LOG_FAILURE)
   is not semantically right; better: define `ORKI-1003` -> exit 1621 as "signature
   verification failed". ratify or propose different mapping.
2. **Unsigned dev payloads**: OK to gate on `debug_assertions` in the stub?
3. **Ed25519 crate choice**: `ed25519-dalek` 2.x (pure Rust, zeroize; no ring) —
   confirm. It pulls `curve25519-dalek`; verify licenses (BSD-3) are acceptable for
   the deny allowlist.
4. **Solid blocks** (D4 from the review): v1.0 ships independent chunks (simplest,
   delta-ready). A "solid group" of chunks compressed as one LZMA2 stream can be added
   in v1.1 without breaking readers if encoded as a new codec id. Agree?
5. **Uninstaller section**: v1.0 reserves the flag and writes nothing; the standalone
   `orki-uninstall.exe` remains a build-time artifact copied next to the receipt (M4).
   Agree?

## Version 0 (current prototype) — kept for reference

64-byte footer at file end, absolute offsets from file start (base = stub length),
manifest-only CRC32 integrity, no signature, no config/assets sections, whole-file
reads. v0 readers/writers are removed in the same PR that lands v1; the CLI gains a
`--v0` migration flag only if the owner asks for one (default: no).

## Acceptance criteria for the M1 implementation PR

- Spec (this document) ratified by the owner.
- Golden-file test: a committed binary blob built by the writer, read back byte-exact by
  the reader (format drift detection).
- Tests: signed/unsigned, tampered chunk/manifest/header, unknown format version,
  Authenticode-table tolerance (appended bytes after payload_end), decompression bomb,
  oversized allocations rejected pre-allocation, cross-platform determinism (Windows +
  Linux CI produce identical payloads).
- Fuzz targets for `read_manifest`, `read_footer`/header, `extract_file`,
  `verify_integrity` in nightly CI.
- `docs/reports/` gains an M1 report with compression measurements (per-chunk vs solid
  probe) on a reference Tauri app payload.
