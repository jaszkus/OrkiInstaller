# OrkiInstaller payload format v1

Status: **rc2 — draft for owner ratification** (2026-10-05). The 2026-10-05 ratification of
the earlier draft is withdrawn; that draft had security gaps (unauthenticated payload
regions, downgrade via the signed flag, decompression before authentication) and internal
contradictions. Implementation is frozen at step 2 (Ed25519 signing over header+manifest,
already merged in PR #12) and must not advance until this revision is ratified. Ratification
records an exact commit: the owner comments `RATIFIED @ <SHA>` on the PR and merges it, and
the decision log gains a row quoting that SHA in the follow-up docs commit, because adding
the row earlier would change the SHA being ratified.

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
list, revisited with delta updates), deduplication inside the installer payload (B2), and
per-chunk (CDC) records. The manifest carries no chunk table (D13): integrity inside a block
is block-level (`comp_blake3` before decompression, `raw_blake3` after it), and delta
readiness lives in the chunk-addressed artifact of phase 3, not in the installer payload.
"Normative content vs packer policy" below separates what a reader enforces from what the
packer chooses.

## Normative content vs packer policy

This document defines only what a **reader** must enforce: field layouts and magics, the
filter allowlist, the hard limits (LZMA2_MAX_DICT, maximum block size, the extraction memory
budget, manifest and string-table bounds), the error catalog, and the validation order.
Changing any of those requires an ADR and a format-version bump.

Packer policy is deliberately **not** normative: the default block size, stream grouping and
ordering, whether a filter is applied by default, to which architectures, and which blocks
are filtered (a block that mixes PE and non-PE bytes is not filtered), the
Brotli-versus-LZMA2 choice, the compression preset per profile, and the store-detection
heuristic. Policy lives in `docs/reports/m1-compression.md` and can change without
re-ratifying this document as long as every payload it produces satisfies the rules above; a
reader never rejects a payload for having been produced with different policy values.

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
  config_offset: u64, config_len: u32, config_blake3: [u8; 32],
  uninstaller_offset: u64, uninstaller_len: u32, uninstaller_blake3: [u8; 32],
  assets: Vec<AssetEntry>,
  blocks: Vec<Block>,
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
artifact and diagnostics; v1.0 manifests carry no per-chunk records and no chunk table
(D13), so a reader never derives integrity from chunk boundaries and cannot be asked to
re-chunk a payload. CRC32 per chunk is dropped (B8): BLAKE3 covers integrity.

Hash and filter semantics (normative, and the reason the two hashes are not redundant):

- `comp_blake3` covers the stored bytes of the block and is verified before any
decompression (A3).
- `raw_len` is the length of the block's raw byte range, and `raw_blake3` covers that range
(i.e. the bytes the file records address). For `filter = 0` the codec output *is* the raw
range, so a reader can verify `raw_blake3` immediately after decompression; for
`filter != 0` the codec output is the filtered stream and the raw range only exists after
the inverse filter, so a reader decompresses, applies the inverse filter, and then verifies
`raw_blake3`. Every allowlisted filter is length-preserving, so the decompressed length, the
inverse-filtered length, and `raw_len` are all equal.
- A filter is applied per block to that block's own raw bytes, with the filter position base
at 0; blocks decode independently, so a file may cross a block boundary and the blocks it
occupies are filtered or not filtered independently of each other.
- A filter may be applied to any block, whatever its content: every allowlisted filter is a
length-preserving bijection over arbitrary bytes, so it is safe without the reader knowing
what the bytes mean. Whether a block is filtered is therefore a packer decision and never a
reader check (D15): the packer filters only blocks whose bytes belong exclusively to PE
input, selected by the PE `Machine` field, and the reader never inspects file types.

Asset kinds (C3): 0 theme, 1 font, 2 icon, 3 license, 4 i18n, 5 shader, 6 image,
7 script, 8 plugin, 9 uninstaller (only valid in the uninstaller section; rejected
elsewhere). Unknown kinds are rejected.

Codec parameters (C4): LZMA2 dictionary size is recorded per payload in the manifest
(`lzma2_dict_size: u32`, field added to `PackManifest`), bounded by `LZMA2_MAX_DICT`
(below); Brotli large windows are disabled at encode time and rejected at decode time
(`lgwin <= 24`); `store` has no parameters. A decoder must be able to reconstruct every
block from the manifest alone.

Filter allowlist (C4): v1.0 has exactly two pre-filters, `bcj-x86` (id 1) and `bcj-arm64`
(id 2), both length-preserving bijections over arbitrary bytes. Filter id 0 means no
pre-filter. A filter id outside the allowlist, or a filtered stream whose output length
differs from the block's `raw_len`, is ORKI-1001. Which blocks are filtered is packer policy
(see "Normative content vs packer policy"): the packer filters only blocks whose bytes
belong exclusively to PE input, selected by the PE `Machine` field (0x8664 -> bcj-x86,
0xAA64 -> bcj-arm64), and groups PE files into their own blocks; the reader never derives
this from the payload (D15). The filter position base is 0 for every block (see "Hash and
filter semantics"). The
remaining `lzma-rust2` filters (ARM, ARM-Thumb, PPC, SPARC, IA64, RISC-V, BCJ2, delta) are
deliberately not part of the format: each is attack surface with no payload benefit here.

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
| LZMA2 dict | LZMA2_MAX_DICT = 32 MiB | reader limit derived from the 200 MiB extraction budget: the worst legal combination (32 MiB dictionary with a 16 MiB block) costs 64.2 MiB per worker, so two workers plus the fixed overhead stay inside the budget and the derived worker count is 3 (see "Decoder memory budget"); packer default dict = block size rounded up to a supported value |
| Brotli window | lgwin <= 22 (BROTLI_MAX_LGWIN) | reader limit checked from the stream header before decoding (2^22 = 4 MiB window per worker); the RFC allows 24 and the large-window bitstream beyond it, and the Rust decoder accepts both, so the check is the reader's (D16) |
| block slack | comp_len <= raw_len + 1 KiB | incompressible input stored, not expanded |
| decompression output | raw_len + 1 | bomb rejection |
| signature block | 128 B fixed | |

Block `comp_len` sums must not exceed the blocks region length; the partition check (A1)
enforces it structurally.

### Decoder memory budget (P0-3, D8)

Extraction is a streaming pipeline with a hard **decoder** budget of 200 MiB, independent of
payload size. The budget bounds the memory the extraction workers allocate (dictionaries,
codec state, block buffers), not the whole process; the whole-process peak of a headless run
is reported next to it as an informational number (decoder formula plus the stub's own
baseline, measured by the same T13) and carries no normative limit. The budget is enforced
**by construction from the format's own limits**, not
from a measurement on one machine: every per-worker buffer is either a constant or bounded by
`raw_len`, and `raw_len` is bounded by the maximum raw block size.

```text
window      = (dict_size + 15) & ~15                   LZMA2 dictionary window
              or 1 << lgwin (<= 4 MiB)                 Brotli ring buffer, lgwin <= BROTLI_MAX_LGWIN
codec_state = 64 KiB range-decoder buffer + 40 KiB     lzma-rust2 get_memory_usage()
              or Huffman tables + bookkeeping          brotli-decompressor, bounded constant
filter      = 4 KiB                                    bcj filter buffer, when filter != 0
comp_buffer = max_raw_block + COMPRESSION_SLACK        stored bytes of one block
out_buffer  = max_raw_block + 1                        decompression cap (bomb rejection)
thread      = 64 KiB stack + bookkeeping

per_worker  = window + codec_state + filter + comp_buffer + out_buffer + thread
workers     = max(1, min(logical_cores, floor((budget - fixed_overhead) / per_worker)))
```

With the default policy (`dict_size` = 8 MiB, `max_raw_block` = 8 MiB) the arithmetic is
8 + 8 + 8 MiB + 172 KiB = 24.2 MiB per worker, so the 200 MiB budget supports eight workers
and the core count alone decides the parallelism. At the worst combination the format allows
(32 MiB dictionary with a 16 MiB block) it is 64.2 MiB per worker and the derived worker count
is 3, i.e. 192.6 MiB - inside the budget with no special-casing. A payload that forces
`workers = 1` still decodes correctly and only loses parallelism; a reader that cannot fit
even one worker refuses the payload instead of exceeding the budget. A streaming reader may
hold less than `comp_buffer` + `out_buffer` (it can hash the compressed bytes while feeding
the decoder, and write the output as it is produced), which is why those two terms are the
conservative assumption rather than an optimization target.

The probability tables of the range decoder and the literal coder do not break this bound:
lzma-rust2 keeps them inside its fixed 40 KiB term (next to the 64 KiB compressed-byte
buffer), and its LZMA2 property decoding rejects any stream with `lc + lp > 4` or
`props > (4 * 5 + 4) * 9 + 8` (`lzma2_reader.rs::decode_lzma2_props`, `get_memory_usage`;
`filter/bcj.rs::FILTER_BUF_SIZE = 4096`), so no payload can enlarge them.

A Brotli worker allocates its ring buffer as `1 << window_bits` (`brotli-decompressor`
`decode.rs`: `ringbuffer_size = 1 << s.window_bits`) plus fixed Huffman tables, so with
`BROTLI_MAX_LGWIN = 22` a Brotli block costs at most about 4 MiB of window, well below the
LZMA2 worst case, and the budget arithmetic does not change with the codec. The same crate
enables the large-window mode by default (`BrotliState::new` sets `large_window = true`;
only `new_strict` disables it), which is why the reader's own `WBITS` check is the control
rather than the decoder configuration.

The constants above come from this arithmetic, not from a measurement (D8). A measurement of
peak working set has exactly one job: verifying that the formula really is an upper bound
(T13: peak RSS with four workers and an 8 MiB dictionary <= 200 MiB **and** <= the formula).
Measured values below the bound may tighten the constants in a later revision; they can never
widen the normative budget. The block-size study in `docs/reports/m1-compression.md` fixes the
default block size this arithmetic assumes.

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

The payload fingerprint is the verified `sig_digest` itself (the 32-byte BLAKE3 digest over
the covered region), not a stored field. Carrying a hash *of* the digest inside the region
that digest covers would be circular: the digest would depend on a field that depends on the
digest, and any placeholder scheme would have to zero bytes to break the loop. Tooling prints
the fingerprint (`orki inspect`), and receipts, repair, and the remote manifest (14a)
reference an installed payload by this value. It changes whenever any covered byte changes,
which includes the whole plan, every path, and every block digest.

## Decision log

| # | decision | date | status |
|---|---|---|---|
| D1 | ORKI-1002 exits 1625; 1621/1622 stay MSI semantics; ORKI-1003 added for versions | 2026-10-05 | accepted |
| D2 | unsigned dev payloads gated by the `dev-unsigned` Cargo feature, not debug assertions | 2026-10-05 | accepted |
| D3 | ed25519-dalek 3.0.0 stays (canonical release cost +65.5 KiB, BSD-3-Clause, `verify_strict`); `ed25519-compact` dropped | 2026-10-06 | accepted |
| D4 | two-level data model: block = compression unit, CDC chunk = raw-hashing unit | 2026-10-05 | accepted; margin re-checked on corpora 2 and 3 |
| D5 | LZMA2_MAX_DICT = 32 MiB as a reader limit; packer dictionary = block size | 2026-10-06 | accepted |
| D6 | format filter allowlist: bcj-x86 and bcj-arm64 only, PE-only, length-preserving | 2026-10-06 | accepted |
| D7 | packer policy: bcj-x86 on by default for PE x64, ARM64 off until corpus 2 | 2026-10-06 | provisional, pending corpus 2 |
| D8 | extraction memory budget 200 MiB, worker count derived from it | 2026-10-06 | accepted; constants derive from the buffer arithmetic in "Decoder memory budget", verified (not defined) by the T13 measurement |
| D9 | key table without validity windows; rotation by stub rebuild; revocation on the update channel | 2026-10-06 | accepted |
| D10 | stub size budgets: full release 6.94 MiB, full ci 7.97 MiB; lite and headless measured separately | 2026-10-06 | accepted; extended by D14 |
| D11 | compression numbers stay out of the normative text; reports carry them | 2026-10-06 | accepted |
| D12 | the payload fingerprint is the verified `sig_digest`, not a stored manifest field (circular dependency found by the worked example) | 2026-10-06 | accepted |
| D13 | v1.0 carries no per-chunk (CDC) records: no chunk table in the manifest, block-level `comp_blake3`/`raw_blake3` as the integrity unit inside a block, delta readiness moved to the chunk-addressed phase-3 artifact | 2026-10-06 | accepted |
| D14 | stub size budgets: full release 6.94 MiB, full ci 7.97 MiB; lite and headless from their own measurement plus 15%; alarm when a variant grows more than 5% against `main`; the size gate builds with a trusted key present, and `xtask/budgets.toml` is updated in the implementation PRs | 2026-10-06 | accepted |
| D15 | a block with `filter != 0` must be covered exclusively by PE file records and must inverse-filter to a stream starting with `MZ` | 2026-10-06 | rejected by the owner: a BCJ filter is a length-preserving bijection over arbitrary bytes, so filtering non-PE content is safe; a large PE spans blocks whose continuation blocks do not start with `MZ`, so the rule would have removed the filter from 72% of the corpus; filtering is packer policy (PE-only blocks), never a reader check |
| D16 | Brotli window limit `BROTLI_MAX_LGWIN` = 22, validated from the stream header before decoding; the decoder is created without relying on large-window mode | 2026-10-06 | accepted |

## Traceability matrix

Test IDs (the full criteria follow under "Acceptance criteria for the v1 implementation PRs"):

| id | test |
|---|---|
| T1 | golden files (unsigned, signed, signed with certificate table) read back byte-exact |
| T2 | coverage: a single-byte flip anywhere in the payload is detected |
| T3 | downgrade: a cleared signed flag does not open a release installation |
| T4 | the stub reads a key-table value patched after compilation |
| T5 | malformed inputs: oversized counts, overlapping ranges, reserved bytes, bad magics |
| T6 | decompression bomb: cap `raw_len + 1`, `comp_blake3` checked before decompression |
| T7 | PE locator fuzz plus edge cases (unsorted sections, zero raw pointers, short files) |
| T8 | packaging order: editing resources after the payload is refused |
| T9 | real Authenticode certificate in CI; re-sign and dual-sign cycles |
| T10 | determinism: identical payload bytes on Windows and Linux with threaded compression |
| T11 | cross-domain replay: payload signatures and remote-manifest signatures do not interchange |
| T12 | nightly fuzz targets: header, string table, manifest, block decoding, PE locator |
| T13 | decoder memory bound: peak working set at four workers with an 8 MiB dictionary is <= 200 MiB and <= the calculated formula |
| T14 | decoder vectors V1-V8 (`docs/examples/vectors/`) decode to the declared length and `raw_blake3` with external tooling |
| T15 | negative vectors: every mutation in `docs/examples/v1-negative.py` is rejected with the documented ORKI code |

| requirement | section | tests |
|---|---|---|
| A1 full byte coverage | Signed region and coverage | T2, T5 |
| A2 signed flag informational | Signed flag | T3 |
| A3 verify before decompression | Validation pipeline (step 7), Block metadata | T6 |
| A4 domain-separated signature | Signed region and coverage | T11 |
| A5 allocation discipline | Validation pipeline (step 4), Manifest budget | T5 |
| A6 key table in PE section | Key table, Packaging order | T4, T8 |
| B1 8-byte magics | Container layout, Signature block, String table | T5, T12 |
| B2 flat file-to-block references | Manifest | T5 |
| B3 string table in v1.0 | String table | T5 |
| B4 single version counter | Manifest | T5 |
| B5 error catalog | Exit codes and error catalog | T5, T15 |
| B6 mutually derived limits | Hard limits, Manifest budget | T5, T6 |
| B7 section order | Container layout, Packaging order | T8 |
| B8 no per-chunk CRC32; canonical reserved bytes | Manifest, Payload header | T5 |
| C1 file metadata | Manifest (FileEntry) | T2 |
| C2 component/target/arch_mask | Manifest (FileEntry) | T5 |
| C3 asset kinds | Asset kinds | T5 |
| C4 codec parameters and filter allowlist | Codec parameters, Block metadata | T5, T6, T14 |
| C5 payload fingerprint | Payload fingerprint | T11 |
| C6 encryption reservation | Encryption | T3 |
| C7 unsigned diagnostic policy | Signed-flag downgrade and unsigned diagnostic policy | T3 |
| C8 repair/modify payload source | Uninstaller | implementation PRs |
| Q1 exit codes for the three Orki errors | Exit codes and error catalog | T5 |
| Q2 dev builds and release CI assertions | Unsigned dev builds and release CI, `.orkikey` | T3, T4 |
| Q3 Ed25519 conditions and key custody | Ed25519, CLI surface, Key table | T9 |
| Q4 block/chunk model and decoder budget | Block metadata, Decoder memory budget | T6, T13, T14 |
| Q5 uninstaller wire shape | Uninstaller | T5 |

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

## Worked example (byte map)

The reference material for independent implementations lives in `docs/examples/` (see
`docs/examples/README.md`). It is not product code: the scripts exist so that a second
implementation can be written from this document alone and compared byte for byte, and the
golden files (T1) must reproduce it exactly.

### Example 1: minimal payload (byte map)

Two files (`a.txt` = `hello`, `b.bin` = `world!`), one block with codec `store`, one
signature block, empty config, asset, and uninstaller regions. `v1-example-1.bin`, 454 bytes:

| region | offset | size |
|---|---|---|
| header | 0 | 64 |
| string table (`ORKISTR\0`, count 3, blob 10 B: `""`, `a.txt`, `b.bin`) | 64 | 38 |
| manifest (postcard) | 102 | 213 |
| config | 315 | 0 |
| assets | 315 | 0 |
| uninstaller | 315 | 0 |
| blocks (one block, codec `store`, raw 11 B) | 315 | 11 |
| signature block | 326 | 128 |
| `payload_len` | | 454 |

Header values: `format_version` 1, `header_size` 64, `file_count` 2, `block_count` 1,
`flags` 1 (signature present), `manifest_offset` 102, `manifest_len` 213, `blocks_offset`
315, `header_crc32` `0x330a711e`, reserved 0. Because `store` is used, `raw_blake3` and
`comp_blake3` of the block are equal:
`9bc016b22c6e916e738e5d16dbd373bbb4185776b7c8c6742dcaf5f5628c922c`. The covered region is
`[56, 326)`, its digest — and therefore the payload fingerprint (C5) — is
`bbf60e4a35fb491e7be719f7db70cb28c13e5e3060fcd2d375a87dad684404a8`, and the file's SHA-256 is
`01b810338574d3427746520c90806b6dd982c2fc27a99efe7bb76c746a4816f2`. The signature message is
`DOMAIN_TAG_ORKI_PAYLOAD_V1 || header[0..56] || sig_digest`. The 128 signature bytes in the
committed file are zero-filled — their content depends on the test key and is pinned by the
signed golden file (T1), not by this example.

Note that `config_offset` and `uninstaller_offset` are payload-relative offsets (315), not
zero: an earlier revision of this example wrote `0` and produced a manifest two bytes
shorter, which the section-coverage rule rejects. The values here are the ones a reader
accepts.

### Example 2: structural payload (byte map)

Exercises the paths where the risk sits: a non-empty config with its hash inside the signed
region, two asset records (`icon`, `license`), an uninstaller, two blocks, a file crossing a
block boundary, a `bcj-x86` filtered block, non-zero component and target string indices, and
manifest offsets whose two-byte varints make the layout a fixed point rather than a one-pass
estimate. Block size 256 is artificial, chosen to force a multi-block layout at a small file
size. `v1-example-2.bin`, 1252 bytes:

| region | offset | size |
|---|---|---|
| header | 0 | 64 |
| string table (`ORKISTR\0`, count 5, blob 32 B: `""`, `app`, `app/a.bin`, `app/data/app.exe`, `core`) | 64 | 68 |
| manifest | 132 | 374 |
| config (`{"install":{"scope":"user","shortcuts":["demo2"]}}`) | 506 | 50 |
| assets (`icon-bytes`, `license text`) | 556 | 22 |
| uninstaller (34 B, `MZ` + 32 bytes) | 578 | 34 |
| blocks | 612 | 512 |
| signature block | 1124 | 128 |
| `payload_len` | | 1252 |

Header values: `format_version` 1, `file_count` 2, `block_count` 2, `flags` 1,
`manifest_offset` 132, `manifest_len` 374, `blocks_offset` 612, `header_crc32` `0x03d849c9`,
reserved 0. The covered region is `[56, 1124)`, `sig_digest` (payload fingerprint) is
`31d72217d028a1edbed1e63a9c9ad16f08088c76fa400d988de54d4e4d0738a3`, and the file's SHA-256 is
`b90f795d8f1981c8b33456f34288c828f95f0d363bfe38fa4fcd5f928156f124`. Config hash, asset
hashes, and uninstaller hash are in the file's manifest and checked by the reader script.

Files: `app/data/app.exe` (340 B, `pe_version` 1.2.3.0, component index of `core`, target
index of `app`, raw range `[0, 340)`) and `app/a.bin` (172 B, component and target index 0,
raw range `[340, 512)`). The PE-like file therefore crosses the block boundary at 256.

| block | codec | filter | raw_len | comp_len | comp_blake3 | raw_blake3 |
|---|---|---|---|---|---|---|
| 0 | store | bcj-x86 | 256 | 256 | `d40c5d2844d54faaa304ac031dfb166dc0f2f6efc137974cdd1b39cadd3d2106` | `d10ae3425b9de06e1088dbf4116c9a042641e8cbf84519a179d52e722129f959` |
| 1 | store | none | 256 | 256 | `9f31569691a3b2bc6985f116355712c952236d46752cfbcdf3c85ccfd6dff2cc` | `9f31569691a3b2bc6985f116355712c952236d46752cfbcdf3c85ccfd6dff2cc` |

Block 0 is covered exclusively by the PE file record, so the packer filters it; block 1 mixes
the PE tail with `app/a.bin`, so packer policy keeps it unfiltered. The two hashes
differ exactly where the filter acted: `comp_blake3` covers the filtered bytes the codec sees,
`raw_blake3` covers the unfiltered bytes the file records address. Regenerate with
`python v1-example.py` and re-validate with `python v1-example-check.py`.

### Decoder vectors

`docs/examples/vectors/` holds ten block payloads with metadata in
`docs/examples/v1-vectors.json`; `docs/examples/v1-vectors-check.py` decodes every positive
vector with independent tooling (xz 5.8.3, brotli 1.2.0, and Python's liblzma binding) and
asserts the declared length, `raw_blake3`, and the stream's own parameters. The format defines
decoding, not encoding: these streams come from external encoders and are deliberately not
pinned to a library version, whereas a packer's own compressed bytes are a regression test
that is re-recorded consciously when a codec version changes.

| id | codec | filter | dictionary or window | declared raw_len | expectation |
|---|---|---|---|---|---|
| V1 | lzma2 | none | dictionary 8 MiB | 32768 | decodes, `raw_blake3` matches |
| V2 | lzma2 | bcj-x86 | dictionary 8 MiB | 16384 | decodes, `raw_blake3` matches |
| V3 | brotli | none | window bits 22 | 32768 | decodes |
| V4 | lzma2 | none | dictionary 32 MiB (`LZMA2_MAX_DICT`) | 32768 | decodes |
| V5 | lzma2 | none | dictionary 1 MiB | 4096 | ORKI-1001: first chunk declares 1048575 bytes |
| V6 | brotli | none | large window 25 | 32768 | ORKI-1001: window above 22 |
| V7 | lzma2 | none | dictionary 64 MiB | 32768 | ORKI-1001: dictionary above `LZMA2_MAX_DICT` |
| V8 | brotli | none | window bits 16 | 32768 | decodes |
| V9 | lzma2 | bcj-x86 | dictionary 8 MiB | 32768 | decodes: the filter acts on non-PE bytes (D15) |
| V10 | brotli | none | window bits 23 | 32768 | ORKI-1001: window above `BROTLI_MAX_LGWIN` |

Two of these vectors pin reader rules:

- The `raw_len + 1` output cap is the primary bomb defence: a reader stops and reports
  ORKI-1001 the moment the decompressed size would exceed it. Because an LZMA2 chunk header
  declares the uncompressed size that chunk will produce, the reader applies the same rule
  earlier and cheaper: a chunk whose declared size exceeds the remaining block budget is
  rejected before its bytes reach the decoder (V5). The cap stays in force for streams whose
  chunks are each small enough but whose total is not.
- The Brotli window bits are validated from the stream header **before** decoding, against
  `BROTLI_MAX_LGWIN` = 22: `1` -> 16; `0` then a non-zero 3-bit `n` -> `17 + n`; `0 000` then
  `000` -> 17; `0 000` then `001` -> the incompatible large-window bitstream. Anything above
  22, and every large-window bitstream, is ORKI-1001 (V6, V10). The decoder cannot be trusted
  with this: brotli 1.2.0 accepts `--large_window=25`, and brotli-decompressor 6.0.1 — the
  crate `orki-pack` links — enables the large-window mode by default, so a v1.0 reader checks
  the header itself and never relies on decoder configuration.

### Negative vectors

`docs/examples/v1-negative.py` mutates example 2 one field at a time and runs the independent
reader over the result; each mutation must be rejected with the documented code (29
mutations, all reproduced). The last three rows are signature-level and are covered by T3/T11
in the implementation PRs rather than by a structural reader.

| mutation | expected |
|---|---|
| `format_version` 1 -> 2 | ORKI-1003 |
| header CRC32 field changed | ORKI-1001 |
| reserved header word non-zero | ORKI-1001 |
| `flags` bit 1 (encryption) set | ORKI-1001 |
| `payload_len` off by one | ORKI-1001 |
| `file_count` 100001 or `block_count` 200001 | ORKI-1001 |
| `manifest_len` shortened (gap) or extended (overlap) | ORKI-1001 |
| `blocks_offset` moved by one (gap) | ORKI-1001 |
| string table magic broken | ORKI-1001 |
| manifest `schema` 1 -> 2 | ORKI-1001 |
| `config_offset` shifted (section gap) | ORKI-1001 |
| config, asset, or uninstaller hash byte flipped | ORKI-1001 |
| config, asset, or uninstaller byte flipped | ORKI-1001 |
| asset kind 11 | ORKI-1001 |
| file `size` raised by one | ORKI-1001 |
| block reference length past the end of its block | ORKI-1001 |
| block codec 3 (reserved zstd) | ORKI-1001 |
| block filter 3 (outside the allowlist) | ORKI-1001 |
| `comp_len` beyond `raw_len + 1 KiB` | ORKI-1001 |
| stored block byte flipped | ORKI-1001 |
| signature block zeroed on a keyed stub | ORKI-1002 |
| signature domain tag replaced | ORKI-1002 |
| signed flag cleared on a keyed stub | ORKI-1002 |

Invariants the reader script checks for both examples (all pass): magic, version,
`header_size` and reserved bytes, header CRC32 over `0..56`, the flag bits, the hard limits,
the string table ending exactly at `manifest_offset`, sections covering
`[64, payload_len - 128)` with no gaps or overlaps, manifest counts matching the header,
every block reference staying inside its block, per-file raw accounting equal to `size`, file
sizes summing to the block's raw length, every block covered by file records, block slack,
the filter length rule, and the manifest consumed byte-exactly. A malformed payload never
propagates an exception out of the reader: structural faults become ORKI-1001.

Writing these examples changed the format four times. The manifest had no `blocks` field to
carry the `Block` records that `block_count` announces (added). `payload_fingerprint` was
defined as a hash of the digest inside the region that digest covers (circular; the field is
removed and C5 now defines the fingerprint as the verified digest itself). The filter record
raised the question of what constrains a filtered block; the proposed reader rule was
rejected (D15) because a BCJ filter is a length-preserving bijection over arbitrary bytes,
so the only reader obligations are the allowlist, the length rule, and `raw_blake3` after
the inverse filter. And the two block hashes needed a stated subject: `comp_blake3` covers
the stored (filtered) bytes, `raw_blake3` covers the raw byte range after the inverse filter.

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
- Decoder vectors: the ten vectors in `docs/examples/v1-vectors.json` decode to their
  declared length and `raw_blake3`, and the negative cases (V5 output cap, V6 and V10 window
  limits, V7 dictionary limit) are rejected before allocation or decoding.
- Negative vectors: every structural mutation in `docs/examples/v1-negative.py` is rejected
  with the documented ORKI code, and no malformed payload escapes the reader as an unhandled
  error.
- Golden examples: the implementation reproduces `v1-example-1.bin` and `v1-example-2.bin`
  byte for byte (T1), including the filtered and the boundary-crossing block of example 2.
- Memory bound: peak working set at four workers with an 8 MiB dictionary is at most 200 MiB
  and at most the calculated formula (T13). T13 also reports the whole-process peak, which is
  informational and carries no normative limit. The measurement may tighten the constants; it
  must never widen the normative budget.

Readiness for ratification (owner review 2026-10-05) adds three gates beyond the sections
themselves: the CDC question is resolved rather than deferred (D13), the decoder memory budget
is derived from buffer arithmetic instead of from one desktop measurement (D8), and both the
decoder vectors and the negative-vector table are present and reproducible from external
tooling. All three were open in the withdrawn draft and are closed in this revision.

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
