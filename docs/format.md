# OrkiInstaller payload format v1

Status: **rc2 — draft for owner ratification** (2026-10-05). The 2026-10-05 ratification of
the earlier draft is withdrawn; that draft had security gaps (unauthenticated payload
regions, downgrade via the signed flag, decompression before authentication) and internal
contradictions. Implementation is frozen at step 2 (Ed25519 signing over header+manifest,
already merged in PR #12) and must not advance until this revision is ratified.

Scope of this revision (owner review 2026-10-05):

- A1-A6: full-payload byte coverage, signed-flag downgrade removed, compressed-bytes hash
  before decompression, domain-separated signature over BLAKE3 digests, allocation
  discipline, key table as a PE section plus packaging order.
- B1-B8: 8-byte magics, flat file-to-chunk references, string table in v1.0, single
  version field, error catalog, mutually derived limits, section order for early UI,
  canonical reserved bytes.
- C1-C8: file metadata, per-file component targeting, asset kinds, codec parameters,
  payload fingerprint, encryption reservation, unsigned diagnostic policy, repair/modify
  payload source.
- Q1-Q5: exit codes, dev-build policy, Ed25519 crate conditions, block/chunk data model,
  uninstaller wire shape.

Source of truth for intent: `OrkiInstaller — dokument projektowy.md` section 5 (referenced
elsewhere as the design document). Any change after ratification requires a new ADR and a
format version bump.

## Goals

1. Survive Authenticode signing **after** packaging: the certificate table is appended to
   the PE by an external tool and must not invalidate payload location or integrity.
2. Authenticate the entire payload before any use: every payload byte is either inside the
   signed region or covered by a hash that is inside the signed region; sections must
   cover the payload with no gaps.
3. Authenticate before decompress: the decoder never sees bytes that were not verified
   against a hash carried in the signed region.
4. Stream: read and extract without loading the whole installer into RAM.
5. Fail closed: unknown format versions, out-of-range counts, oversized allocations, and
   decompression bombs are rejected before any allocation or disk write.

Non-goals for v1.0: encryption (flag reserved, C6), deep Merkle trees (single-level hash
list, revisited with delta updates), deduplication inside the installer payload (B2).

## Container layout

```
+---------------------------------------------+
| PE stub (orki-stub.exe)                     |
|   resources customized FIRST                |
|   key table patched SECOND (.orkikey)       |
+---------------------------------------------+ <- overlay_start = end of last PE section (raw)
| PAYLOAD HEADER            64 bytes, fixed   |
| STRING TABLE              aligned           |
| MANIFEST                  postcard          |
| CONFIG                    compiled orki.toml|
| ASSETS                    themes/fonts/i18n |
| UNINSTALLER               small PE, optional|
| BLOCKS (chunk data)       codec per block   |
| SIGNATURE BLOCK           128 bytes, fixed  |
+---------------------------------------------+ <- payload_end = overlay_start + payload_len
| [ Authenticode certificate table ]          |   appended by an external signing tool AFTER packaging
| [ padding: anything ]                       |   ignored by the reader
+---------------------------------------------+
```

Section order is normative and ascending by offset: header, string table, manifest, config,
assets, uninstaller, blocks, signature block. Early sections let the UI start after reading
the first few MiB; the builder is two-pass and absorbs the layout cost. The string table
always starts at `overlay_start + HEADER_SIZE` and is self-describing (count + blob length,
below); manifest, config, uninstaller offsets and lengths live in the signed manifest; the
blocks region offset lives in the header, its internal layout in the manifest.

The reader locates the payload through the PE header (end of the last section's raw data),
never through the file end. The Authenticode table and any trailing padding are invisible
to the format. A payload packed without a stub is a valid degenerate file whose overlay
starts at offset 0.

### Packaging order (normative)

1. Customize stub resources (icon, version info, embedded UI assets) with a pure-Rust PE
   resource editor; this rewrites the file and may drop any existing overlay.
2. Patch the key table into the `.orkikey` section.
3. Append the payload (header through signature block).
4. Authenticode-sign last; the certificate table lands after `payload_end` and covers the
   overlay in its own digest.

Editing resources or the key table after step 3 invalidates the PE layout the payload
offsets were computed from; `orki pack` must refuse such inputs and the acceptance tests
demonstrate the failure (section 5).

### Payload header (64 bytes, little-endian)

| offset | size | field |
|---|---|---|
| 0 | 8 | magic `ORKIHDR\0` (exactly 8 bytes) |
| 8 | 4 | format_version (u32) = 1 |
| 12 | 4 | header_size (u32) = 64 |
| 16 | 8 | payload_len (u64): overlay_start .. overlay_start + payload_len = payload_end |
| 24 | 4 | file_count (u32) |
| 28 | 4 | block_count (u32) |
| 32 | 8 | manifest_offset (u64, relative to overlay_start) |
| 40 | 4 | manifest_len (u32) |
| 44 | 4 | flags (bit 0: signature present, informational only — see "Signed flag") |
| 48 | 8 | blocks_offset (u64, relative to overlay_start) |
| 56 | 4 | header_crc32 (CRC32 over bytes 0..56) |
| 60 | 4 | reserved (must be 0) |

Notes: offsets inside the payload are **relative to overlay_start**, never to the file.
Reserved fields are validated as zero (canonical form; a reader rejects non-zero reserved
bytes instead of skipping them). The signed flag is informational; it must never drive a
security decision (see "Signed flag"). File and block counts live in the header so a reader
can bound-check the manifest before deserializing it.

### Signed region and coverage (A1)

The signature message is `DOMAIN_TAG_ORKI_PAYLOAD_V1 || header[0..56] || sig_digest` (A4),
where `sig_digest` is the BLAKE3 hash of the payload bytes taken **linearly and
streamingly** across `header[56..64]`, the string table, the manifest, the config, the
asset region, the uninstaller, and the blocks region — i.e. every byte from
`overlay_start + 56` to `overlay_start + payload_len - SIGNATURE_BLOCK_SIZE`. No byte in
that interval can change without breaking the signature; the validator additionally checks
that the section table (header offsets plus manifest offsets/lengths) exactly partitions
the interval with no gaps, no overlaps, and no extension into the signature block, so the
digest cannot be satisfied by re-labelling bytes between sections. Header bytes 56..64
carry no semantics beyond the header CRC and reserved-zero, both validated locally; the
CRC field is excluded from the signed material deliberately (it is derivable).

A fixed domain-separation label prefixes the digest before signing, so a signature made in
another context (remote manifest 14a, updater artifacts) cannot be replayed as a payload
signature and vice versa. The digest is computed streaming; the full payload is never held
in memory to sign it.

### Signed flag (A2)

`flags.bit 0` records whether a signature block is present. It is metadata for tooling
(`orki inspect`), never an authorization input. Whether signatures are required is decided
by the stub's own build (key table present or not, A6/Q2) — a data byte in untrusted input
cannot downgrade the policy. A payload with no signature block runs only on dev stubs
(Q2); a release stub treats it exactly like a bad signature.

### Signature block (128 bytes, at payload_end - 128, relative)

| offset | size | field |
|---|---|---|
| 0 | 8 | magic `ORKISIG\0` (exactly 8 bytes) |
| 8 | 1 | algorithm = 1 (Ed25519) |
| 9 | 1 | key_slot (0-3) |
| 10 | 2 | reserved (0) |
| 12 | 64 | Ed25519 signature over `DOMAIN_TAG_ORKI_PAYLOAD_V1` + header prefix + `sig_digest` |
| 76 | 52 | reserved (0) |

The algorithm field makes ECDSA P-256 addable later without a format bump (Q3).

### String table (B3, in v1.0)

First section, at `overlay_start + HEADER_SIZE`, always present (count may be 0):

| offset | size | field |
|---|---|---|
| 0 | 8 | magic `ORKISTR\0` |
| 8 | 4 | count (u32) |
| 12 | 4 | blob_len (u32) |
| 16 | count*4 | offsets (u32) into blob, entry 0 = empty string |
| ... | blob_len | UTF-8 blob |

Total section size is `16 + count*4 + blob_len`. Manifest entries reference strings by
`u32` index; `app_id`/`app_name`/`app_version` stay inline postcard strings (three fields,
always present), while paths, components, and targets use the table. The table lives inside
the signed region so file paths are authenticated (A1). The design document's interning
goals (18b) are served at the format level from day one; there is no v1.0/v1.1 type
switch.

### Manifest (postcard)

```text
PackManifest v1 {
  schema: u32 = 1,
  app_id: str, app_name: str, app_version: str,
  signer_key_slot: u8,
  payload_fingerprint: [u8; 32],            BLAKE3 of sig_digest (C5)
  config_offset: u64, config_len: u32, config_blake3: [u8; 32],
  uninstaller_offset: u64, uninstaller_len: u32, uninstaller_blake3: [u8; 32],
  assets: Vec<AssetEntry>,
  files: Vec<FileEntry>,
}
FileEntry {
  path: u32 (string table index),
  component: u32 (string table index; empty = default),
  target: u32 (string table index; install-dir-relative destination),
  arch_mask: u16 (bit 0: x64, bit 1: arm64; 0 = all),
  entry_kind: u8 (0 = file, 1 = empty directory),
  attrs: u8 (bit 0: read-only, bit 1: hidden, bit 2: system),
  mtime: u64 (seconds since 1970 UTC; build-normalized, see C1),
  pe_version: Option<(u16, u16, u16, u16)>,
  size: u64 (0 for directories),
  block_refs: Vec<BlockRef>,
}
BlockRef { index: u32, raw_offset: u64, raw_len: u32 }
AssetEntry {
  kind: u8 (see asset kinds), path: u32, target: u32,
  offset: u64, len: u32, blake3: [u8; 32],
}
```

There is exactly one version field: `format_version` in the header and `schema` in the
manifest are the same counter expressed once (B4); the manifest `schema` value is
deserialized and checked for equality with the header value.

File-to-block references are a flat per-file list (B2). `raw_offset` locates the file's
bytes inside the decompressed block stream; blocks are ordered by payload position, a file
may start at any offset inside a block, and deduplication is not expressed inside the
installer payload (the delta artifact of phase 3 addresses chunks independently).

Block metadata (B1/Q4 model):

```text
Block {
  codec: u8,                 0 store, 1 lzma2, 2 brotli, 3 zstd (reserved, rejected)
  filter: u8,                0 none, 1 bcj-x86, 2 bcj-arm64 (PE-only pre-filter)
  comp_len: u32,
  raw_len: u32,
  comp_blake3: [u8; 32],     verified BEFORE decompression (A3)
  raw_blake3: [u8; 32],      defense in depth, delta input, diagnostics
}
```

A block is the compression unit (target 4-8 MiB raw; codec parameters outside the block,
C4). FastCDC chunks inside a block are the raw-byte hashing unit for the phase-3 delta
artifact and diagnostics; v1.0 manifests do not carry per-chunk records. CRC32 per chunk
is dropped (B8): BLAKE3 covers integrity.

Asset kinds (C3): 0 theme, 1 font, 2 icon, 3 license, 4 i18n, 5 shader, 6 image,
7 script, 8 plugin, 9 uninstaller (only valid in the uninstaller section; rejected
elsewhere). Unknown kinds are rejected.

Codec parameters (C4): LZMA2 dictionary size is recorded per payload in the manifest
(`lzma2_dict_size: u32`, field added to `PackManifest`), bounded by `LZMA2_MAX_DICT`
(below); Brotli large windows are disabled at encode time and rejected at decode time
(`lgwin <= 24`); `store` has no parameters. A decoder must be able to reconstruct every
block from the manifest alone.

Filter allowlist (C4): v1.0 has exactly two pre-filters, `bcj-x86` (id 1) and `bcj-arm64`
(id 2), both length-preserving and applied only to PE input, selected by the PE `Machine`
field (0x8664 -> bcj-x86, 0xAA64 -> bcj-arm64). Filter id 0 means no pre-filter. A filter id
outside the allowlist, a filter applied to a non-PE file, or a filtered stream whose output
length differs from the block's `raw_len` is ORKI-1001. The remaining `lzma-rust2` filters
(ARM, ARM-Thumb, PPC, SPARC, IA64, RISC-V, BCJ2, delta) are deliberately not part of the
format: each is attack surface with no payload benefit here.

### Validation pipeline (in order, fail closed)

1. Locate PE overlay start via section headers. Non-PE files are payload-only artifacts
   with overlay_start = 0.
2. Read the 64-byte payload header; verify magic, format_version == 1 (any other value is
   `ORKI-1003`), reserved bytes zero, header CRC32.
3. Bound-check `payload_len` against the actual file; verify the block/file counts against
   the hard limits and against `manifest_len` using minimal per-entry sizes, before any
   allocation sized by untrusted data.
4. Allocation discipline (A5): before authentication, the only allocations are bounded by
   compile-time constants (header, signature block, per-entry bounds checks) — phrased
   precisely: **allocations before authentication are capped by fixed limits**, never by
   untrusted lengths alone.
5. Verify the signature on raw bytes: compute `sig_digest` streaming over the regions
   (string table, manifest, config, assets, uninstaller, blocks); the per-block
   `comp_blake3` values live inside the manifest and are therefore covered by the same
   digest. Then verify Ed25519 over the domain-tagged digest. This happens before manifest
   deserialization, before decompression, and before any disk write.
6. Deserialize the manifest (postcard) after authentication; check `schema == 1`, path and
   component indices in range, structural invariants: sections partition the payload with
   no gaps (A1), block references in range, per-file raw accounting equals `size`,
   `comp_len <= raw_len + COMPRESSION_SLACK`.
7. Extract or verify on demand: for each block, verify `comp_blake3` on the compressed
   bytes (already covered by the signature re-check, enforced again at read time), then
   decompress with a hard output cap of `raw_len + 1` (bomb rejection), then verify
   `raw_blake3`.

### Signed-flag downgrade and unsigned diagnostic policy (A2, C7)

- Release stub (key table present): a missing or invalid signature is
  `ORKI-1002`/exit 1625. There is no flag, environment variable, or byte that changes this.
- Dev stub (built with the `dev-unsigned` Cargo feature, default off): unsigned payloads
  run with a visible warning banner in the GUI and a warning line in `--print-config`.
  No runtime flag exists.
- Without a valid signature, `--list` and `--print-config` may run in a diagnostic mode on
  any stub when explicitly enabled at build time for CI tooling; `--extract` and
  installation never do. The default build omits even the diagnostic mode.

### Key table (A6)

Stored in a dedicated PE section `.orkikey`, read at runtime by scanning the stub's own
section headers (never through a Rust `static`: const-folding, LTO, and `gc-sections` can
eliminate or inline statics). Capacity: 4 slots.

```text
KeyEntry (40 bytes) {
  key_id: [u8; 8],          BLAKE3(pubkey)[0..8], lookup handle
  pubkey: [u8; 32],
}
Section header: magic `ORKIKEY\0`, count u32, entries..., CRC32.
```

Verification resolves `key_slot` from the signature block against this table and verifies
the signature; no key material, no verification.

Key entries carry no validity window. The installer's clock is untrusted input, and an
installer signed in 2026 must not stop working in 2031, so time-based expiry would buy no
security while creating a real availability failure; expiry belongs to the update channel
(14a), which can refuse new releases without invalidating installers that are already
signed. Rotation is a stub rebuild: the new installer embeds a new or replaced `.orkikey`
entry and is signed with the matching payload key. A key baked into an already-distributed
installer cannot be revoked there, so the mitigations are one payload key per release line,
key separation (below), offline custody of the private key, and a revocation list on the
update channel for software the updater can still reach. Reading the section at runtime
(instead of a compiled-in constant) also makes the verification path reachable by
construction, so the linker cannot drop it: a stub with an empty key table is a
template/dev build that refuses every payload (fail closed), and a stub with a key table
always links the verification code. Key separation (Q3): the payload key, the Tauri updater
key, and the remote-manifest key (14a) are distinct keys; nothing shares material across
those roles.

Private keys never enter the repository. `orki sign` accepts `--key <file>`,
`--key env:VAR`, `--key stdin`, or `--key exec:<command>` (KMS/HSM integration); the file
form exists for local development only.

### Ed25519 (Q3)

`ed25519-dalek` 3.x (already merged in PR #12; the review's 2.x reference is superseded by
what shipped). Conditions:

- Verification uses `verify_strict`.
- Licenses: `ed25519-dalek` and `curve25519-dalek` are BSD-3-Clause; add BSD-3-Clause to
  the `deny.toml` allowlist and audit transitive licenses.
- The stub links only the verification path; keygen and signing live in `orki` (CLI).
- Measured stub cost: **+65.5 KiB** on the canonical `release` profile with a trusted key
  present (5.96 -> 6.03 MiB, variant `full`), i.e. below the ~100 KiB threshold, so no
  smaller implementation is proposed. A key-free build measures only +2.5 KiB because the
  linker drops the unreachable verification path, so the size gate must build with a key
  present (`docs/reports/m1-stub-size.md`).
- Deterministic Ed25519 is acceptable for payload signing; the signing key file format is
  the seed only (`seed: <hex>`), never an expanded key.

### CLI surface (Q3, design section 7)

Payload signing lives under distinct names so that `orki sign` stays reserved for
external/updater signing (Authenticode, Tauri updater artifacts): `orki keygen` creates a
signing key pair, `orki pack --sign-key <source>` signs the payload it just wrote (source is
`file:`, `env:VAR`, `stdin`, or `exec:<command>` for KMS/HSM), and `orki verify <file>`
re-runs the whole validation pipeline including the signature. `orki sign` is not part of
payload packaging.

### Exit codes and error catalog (Q1, B5)

| Orki code | meaning | exit |
|---|---|---|
| ORKI-1001 | corrupted or inconsistent payload (CRC, hash, limits, structure) | 1620 |
| ORKI-1002 | signature missing, invalid, or key unknown/untrusted | 1625 |
| ORKI-1003 | unsupported format version | 1620 |

1621 (ERROR_INSTALL_UI_FAILURE) and 1622 (ERROR_INSTALL_LOG_FAILURE) are pre-existing MSI
semantics and must not be reused. The stub's `List` and default dispatch map ORKI-1002 to
1625. The design document's section 21.3 catalog gains ORKI-1003 accordingly. (Note: the
merged PR #12 implementation still uses 1621; the implementation PR for this spec updates
it as part of step 3.)

### Unsigned dev builds and release CI (Q2)

- The stub is built with the `dev-unsigned` Cargo feature (default off) to accept
  unsigned payloads; the feature also forces the visible warning banner.
- `orki pack` without a signing key fails unless `--dev` is passed; `--dev` implies the
  output is invalid for release distribution.
- Release CI asserts: stub has a key table, stub was built without `dev-unsigned`, release
  artifacts are signed end-to-end.

### Hard limits (constants in orki-pack, mutually derived, all enforced)

| limit | value | derivation |
|---|---|---|
| max files | 100_000 | header file_count bound |
| max blocks | 200_000 | 8 GiB / 40 KiB min raw block |
| max assets | 10_000 | |
| max path length | 1024 UTF-8 bytes | single unit for all path limits (B6); the 255 UTF-16 component cap stays with the sanitizer |
| max raw block | 16 MiB | |
| max file size | 4 GiB | |
| max total raw size | 8 GiB | 200_000 blocks x 40 KiB average bound (B6) |
| max manifest_len | 64 MiB | ~35 MB at 100k files + 800k refs by estimate; 64 MiB is headroom |
| max string table | 64 MiB | shares the manifest bound |
| LZMA2 dict | LZMA2_MAX_DICT = 32 MiB | decoder-side cap; default dict = block size rounded up to a supported value; final value fixed by the block-size study |
| Brotli window | lgwin <= 24 | large windows disabled |
| block slack | comp_len <= raw_len + 1 KiB | incompressible input stored, not expanded |
| decompression output | raw_len + 1 | bomb rejection |
| signature block | 128 B fixed | |

Block `comp_len` sums must not exceed the blocks region length; the partition check (A1)
enforces it structurally.

### Decoder memory budget (P0-3)

Extraction is a streaming pipeline with a hard process-wide budget of 200 MiB, independent
of payload size. Per-worker cost is dominated by the codec window, so parallelism is derived
from the payload and never fixed:

```text
per_worker = dict_size + filter_buffers + 64 KiB range-decoder buffer   (LZMA2)
workers    = max(1, min(logical_cores, floor((budget - fixed_overhead) / per_worker)))
```

The packer's default dictionary equals the block size rounded up to the next supported value
(a dictionary larger than the block it decodes buys nothing) and never exceeds
`LZMA2_MAX_DICT`. With 8 MiB blocks and a 200 MiB budget this is nowhere near the worker
limit, so the parallel path is not memory-bound; a payload that forces `workers = 1` still
decodes correctly, it only loses parallelism. The block-size study in
`docs/reports/m1-compression.md` fixes the final default and confirms the budget on real
payloads.

### Manifest budget (A5)

The manifest carries explicit counts in the header (file_count, block_count). Before
deserializing, the reader checks `manifest_len` against
`file_count * MIN_FILE_ENTRY + block_count * MIN_BLOCK_ENTRY + FIXED_OVERHEAD`; a violation
is ORKI-1001. Postcard deserialization therefore cannot be coaxed into allocations
disproportionate to a bounded input. Serde/postcard have no built-in multiplicity limits —
the bound comes from this pre-check, not from the deserializer.

### Uninstaller (Q5, C8)

The uninstaller is part of the payload: a stub-only, independently signed PE embedded as
its own section with a BLAKE3 hash in the signed manifest (`uninstaller_*` fields). The
wire shape exists in v1.0; M4 decides when installers start embedding it. Repair and modify
need a payload; the resolution: the installer is cached at install time (copy of
`Setup.exe`) and repair/modify run from the cache, falling back to a user-provided
`Setup.exe` path. The cache location follows the install scope: user scope uses
`%LOCALAPPDATA%\Orki\<app_id>\cache\`, machine scope uses
`%ProgramData%\Orki\<app_id>\cache\` with an ACL limited to administrators. The cached
copy is untrusted input and is re-verified in full (signature over the payload, section
coverage, per-block hashes) before it is used; a failing copy is never repaired silently but
reported and replaced from a user-provided installer. Caching can be disabled per package
(no cache directory is created), which doubles the repair cost but keeps no payload on disk.
Receipt paths are untrusted input for the elevated uninstaller: it re-sanitizes every path
and constrains writes to the install directory; machine-scope receipts live under an ACL
limited to administrators.

### Encryption (C6)

`flags` bit 1 is reserved for encryption (XChaCha20-Poly1305 per the design document,
section 5). v1.0 readers reject every flag bit other than bit 0 (the informational
signature flag), and v1.0 writers never set bit 1.

### Payload fingerprint (C5)

`payload_fingerprint` = BLAKE3 of `sig_digest`, carried in the signed manifest. Receipts,
repair, and the remote manifest (14a) reference an installed payload by this fingerprint.

## Threat model

What Ed25519 here protects: the payload plan (files, registry writes, hooks, shortcuts),
all payload bytes (via the coverage rule), and the binding between header, manifest, and
content digests — against anyone without a trusted private key, including a compromised
distribution host (the host can refuse service, not substitute content).

What Ed25519 here does not protect: the stub binary itself before Authenticode (a rebuilt
stub with an attacker's key table and attacker's payload is self-consistent; only
Authenticode ties the stub to the publisher), the Authenticode signature's own claims
(Authenticode certifies the publisher, not payload integrity; it also breaks if anyone
edits bytes after signing), endpoints after verification (a verified installer can still
be a valid installer of malicious content if the key itself is compromised — key
ceremony, rotation via validity windows, and revocation flags are the operational answer),
and the debug/dev channel, which is trust-free by design and must never ship.

What Authenticode adds: publisher identity and SmartScreen reputation, tamper evidence for
the whole file as distributed. The two are complementary; neither substitutes the other.
The `.orkikey` table binds the payload key to a specific stub build; rotating the payload
key means rebuilding and re-signing the installer, which the validity windows in the key
table are designed to make a scheduled, non-emergency operation.

## Version 0 (historical)

The pre-M1 prototype format (64-byte footer, absolute offsets, CRC-only integrity) was
removed from the writer in PR #11; no migration path is provided.

## Acceptance criteria for the v1 implementation PRs

- Spec (this document) ratified by the owner **before** implementation resumes; steps
  3 (streaming) and 4 (fuzz targets) wait for rc2 sign-off. Step 2 (Ed25519 over
  header+manifest) is already merged and will be reworked to the domain-tagged digest
  scheme in the implementation PR.
- Golden files, committed as binaries and read back byte-exact: unsigned, signed, and
  signed-with-appended-certificate-table variants.
- Coverage test: every single-byte flip anywhere in the payload (config, string table,
  uninstaller, blocks, padding excluded) is detected by verification.
- Downgrade test: a cleared signed flag does not enable installation on a release stub;
  the decision follows the key table, not the flag.
- Key table test: the stub reads a value patched after compilation (not a folded static).
- Decompression bomb, oversized counts, overlapping ranges, `comp_len` beyond payload:
  rejected pre-allocation.
- PE locator fuzz and edge tests: unsorted sections, `PointerToRawData = 0`,
  `SizeOfRawData` beyond file size, no sections, pre-existing certificate table in the stub.
- Resource-after-payload test: editing stub resources after payload append corrupts the
  file, and `orki pack` refuses the sequence (packaging order is enforced by tooling).
- Real Authenticode test certificate in CI (signtool or equivalent): the payload reads
  back and the Windows signature verifies; re-sign and dual-sign cycles keep the overlay
  readable. The release signing stack (for example Azure Trusted Signing) is validated
  against the same tests before M2.
- Determinism: identical payload bytes on Windows and Linux with multithreaded compression
  (ordering is canonical, compression is per-block deterministic).
- Fuzz targets for header, string table, manifest, block decoding, and the PE locator run
  in nightly CI.
- `docs/reports/` gains M1 reports for the solid-vs-independent measurement
  (`docs/reports/m1-compression.md`) and the Ed25519 stub-size measurement
  (`docs/reports/m1-stub-size.md`).
- Cross-domain replay test: a signature produced for the remote-manifest context fails
  payload verification and vice versa.

## Compression study

The block-vs-chunk question is settled by measurement, not by estimate: independent 64 KiB
and 256 KiB chunks against 8 MiB solid blocks (with and without the BCJ x86 pre-filter),
measured on a real 515 MiB Electron-style payload with the production codec parameters.
Methodology, environment, corpus fingerprint, results, and the decision thresholds live in
`docs/reports/m1-compression.md`; numbers stay out of this specification so that no
re-measurement can invalidate it. Measured margins: 12.62% (dictionary-sized blocks) and
14.05% (BCJ x86) against the **shipped v0 packer** measured on the same corpus, 14.0-15.4%
against fixed 64 KiB chunks, and 8.48% / 9.97% against the best hypothetical independent
configuration (256 KiB auto-chunks) — that last case is 0.03 points below the 10% target and
is an improvement over production that nothing ships. BCJ x86 gains a further 2.12% on the
PE subset (72% of the corpus) at no measurable time cost, so a default PE-only pre-filter
meets its adoption criterion on this corpus. Blocks are the v1.0 compression
unit (B1/Q4) on these grounds: (a) the reference baseline is the shipped 64 KiB chunking,
where the win is above 14%; (b) 256 KiB chunks buy ratio by giving up delta granularity and
decode parallelism, which the design needs elsewhere; (c) corpora with many small files,
not measured yet, favour solid blocking further; and (d) the two-level model degenerates to
the one-level model when a block holds a single chunk, so the decision is reversible in the
direction that matters. The margin is re-checked on corpora 2 and 3 and after the block-size
sweep; the default block size, dictionary rule, grouping policy, filter allowlist, and
Brotli-versus-LZMA2 policy are fixed in the compression decisions section of that report and
enter this specification as decisions, not as measurements.

BCJ availability: `lzma-rust2` 0.21 ships x86/ARM/ARM64/ARM-Thumb/PPC/SPARC/IA64/RISC-V
BCJ filters plus BCJ2 and delta (`lzma_rust2::filter::bcj::BcjWriter`/`BcjReader`), so the
block model carries a pre-filter without a new dependency.

Stub size: verification-only Ed25519 costs **+65.5 KiB** on the canonical `release` profile
with a trusted key present, leaving the stub at 6.03 MiB of the 25 MiB full-variant budget;
`docs/reports/m1-stub-size.md` carries the measurement, the key-free case that the linker
strips, and the dependency checks. No crate switch is proposed.
