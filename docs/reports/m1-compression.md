# M1 report: solid blocks vs independent chunks

Owner requirement (2026-10-05 review, Q4): the measurement precedes rc2 ratification and
decides the data model — one level (a chunk is both the compression and the hashing unit) or
two levels (a block is the compression unit, CDC chunks inside it are the raw-hashing unit).
This report is the input for the "Compression study" section of `docs/format.md` and for the
compression decisions that follow.

Status: **first run complete (corpus 1 only)**. It settles the model direction; it does not
settle default parameters (block size, dictionary rule, grouping, BCJ adoption, Brotli
policy, CI level, decode-side cost) — those wait for the extensions listed at the end.

## Method

- Tool: a scratch Rust binary outside the repository (release build) that uses the production codec
  path: `lzma-rust2` 0.21 `Lzma2Options::with_preset(6)` with a per-variant dictionary,
  `brotli` 9 (`BrotliEncoderParams { quality: 9 }`), the packer's auto policy (LZMA2 first,
  then Brotli, then store; store below 512 B raw), and
  `lzma_rust2::filter::bcj::BcjWriter::new_x86` for the BCJ variants.
- Independent chunking: fixed-size chunks over the concatenated payload stream (64 KiB in
  A/B, 256 KiB in C) — a conservative proxy for FastCDC, which only shifts boundaries and
  cannot remove the per-unit dictionary reset this study measures.
- Solid blocking: the concatenated payload stream split into 8 MiB blocks (D/E/F); G
  compresses each input file as a single unit instead. File order inside the stream is
  ascending path order (deterministic).
- Units are compressed in parallel over 4 threads, one variant per process run; the reported
  size is the sum of codec payload lengths and excludes the container (header, string table,
  manifest, per-block metadata, signature block).
- Correctness: every variant re-compresses its largest unit and decodes it with the matching
  reader (`Lzma2Reader`, `brotli::Decompressor`, `BcjReader`) — byte-exact round trip
  (`verify=ok`) for all seven variants. This proves the numbers come from valid streams; it
  is not a full-corpus decode check.

## Reference payload (corpus 1)

- Installed Electron-style desktop application of a developer machine
  (`%LOCALAPPDATA%\Programs`), used as a stand-in for a Tauri app payload. This report
  records class-level aggregates and the listing fingerprint only: no file names, no
  application identity.
- Selection: regular files >= 64 KiB — 131 files, 540 106 180 B (515.1 MiB) raw.
- Composition by class (PE detected by the `MZ`/`PE\0\0` header and the `Machine` field):

  | class | files | bytes | share |
  |---|---|---|---|
  | PE x64 | 11 | 388 620 032 | 71.9% |
  | PE x86 | 2 | 328 136 | 0.1% |
  | web (JS/CSS/HTML/JSON/SVG/map/XML/WASM) | 43 | 44 841 870 | 8.3% |
  | media / already compressed | 71 | 66 104 296 | 12.2% |
  | rest | 4 | 40 211 846 | 7.4% |

  PE total: 13 files, 388 948 168 B (72.0% of the corpus). Corpus 1 is therefore
  native-code dominated, which matters for the BCJ reading below.
- Listing fingerprint: BLAKE3 over the sorted `size<TAB>relative-path` listing of the corpus
  = `d0ee3a74d52110e0877cfb30847fccaf30c6a5f805bba1bc5a061da5f17fb82a`.
- Known bias: the >= 64 KiB filter drops small files, which is where solid blocking gains the
  most and where per-file metadata cost is highest. Corpora 2 and 3 in the extensions below
  address this.

## Environment

- Windows 11 Pro; Intel Core i5-7500 (4 cores / 4 threads); 7.4 GiB RAM; `C:` on a 1 TB NVMe
  SSD; all inputs local.
- rustc/cargo 1.99.0 (2026-09-28); rayon 1.12.0 (4 threads); lzma-rust2 0.21.0; brotli 9.0.0.
- Release profile (`opt-level = 3`), one run per variant, 2026-10-06.

## Results (one tool build, one payload)

| variant | configuration | compressed | % of raw | encode time |
|---|---|---|---|---|
| A | chunks 64 KiB, lzma2 p6 dict 8 MiB | 207 820 097 B | 38.48% | 110.2 s |
| B | chunks 64 KiB, auto (lzma2/brotli9/store) | 207 778 670 B | 38.47% | 106.8 s |
| C | chunks 256 KiB, auto | 195 301 969 B | 36.16% | 93.7 s |
| D | solid 8 MiB blocks, lzma2 p6 dict 64 MiB | 178 747 172 B | 33.09% | 147.8 s |
| E | solid 8 MiB blocks, lzma2 p6 dict 8 MiB | 178 747 837 B | 33.09% | 117.9 s |
| F | solid 8 MiB blocks, BCJ x86 + lzma2 p6 dict 8 MiB | 175 828 111 B | 32.55% | 114.4 s |
| G | whole file per input, BCJ x86 + lzma2 p6 dict 64 MiB | 169 801 341 B | 31.44% | 303.2 s |

Relative size reduction against the independent baselines:

| comparison | reduction |
|---|---|
| production packer vs fresh A (fixed 64 KiB, lzma2) | 1.6% |
| C vs A (256 KiB chunks vs 64 KiB chunks) | 6.0% |
| D / E vs A (solid 8 MiB, dict 64/8 MiB) | 14.0% |
| F vs A (solid 8 MiB + BCJ) | 15.4% |
| G vs A (per file + BCJ) | 18.3% |
| D / E vs C (best independent configuration) | 8.48% |
| F vs C | 9.97% |
| G vs C | 13.1% |
| **D / E vs production packer** | **12.62%** |
| **F vs production packer** | **14.05%** |
| **G vs production packer** | **16.99%** |

Raw tool output (one line per variant; `comp` is the summed codec payload):

```text
variant=A units=8242 unit=64KiB raw=540106180 comp=207820097 pct=38.48 time_s=110.2 files=131 mix=lzma2:8242,brotli:0,store:0 verify=ok
variant=B units=8242 unit=64KiB raw=540106180 comp=207778670 pct=38.47 time_s=106.8 files=131 mix=lzma2:7882,brotli:181,store:179 verify=ok
variant=C units=2061 unit=256KiB raw=540106180 comp=195301969 pct=36.16 time_s=93.7 files=131 mix=lzma2:1987,brotli:5,store:69 verify=ok
variant=D units=65 unit=8MiB raw=540106180 comp=178747172 pct=33.09 time_s=147.8 files=131 mix=lzma2:65,brotli:0,store:0 verify=ok
variant=E units=65 unit=8MiB raw=540106180 comp=178747837 pct=33.09 time_s=117.9 files=131 mix=lzma2:65,brotli:0,store:0 verify=ok
variant=F units=65 unit=8MiB raw=540106180 comp=175828111 pct=32.55 time_s=114.4 files=131 mix=lzma2:65,brotli:0,store:0 verify=ok
variant=G units=131 unit=per-file raw=540106180 comp=169801341 pct=31.44 time_s=303.2 files=131 mix=lzma2:131,brotli:0,store:0 verify=ok
```

## Production baseline (real v0 packer)

Owner requirement (2026-10-06 review): variant A/B must reflect the shipped packer, not a
reconstruction. The baseline was produced by the actual production code: commit `ca90b02`
(PR #10, the last commit carrying the v0 writer) built with
`cargo build --release -p orki-cli`, then
`orki pack app -o app-v0.orkipack` over the same staged corpus (a temporary worktree, no
repository change).

That code chunked with FastCDC v2020 (min 16 KiB, average 64 KiB, max 256 KiB), hashed each
chunk with BLAKE3 and CRC32, encoded with the same auto policy the tool mirrors (LZMA2
preset 6 with an 8 MiB dictionary, then Brotli q9 if smaller, then store if smaller, store
for anything below 512 B), and concatenated the compressed chunk payloads without any
per-chunk framing; chunk lengths, hashes, and order live in the postcard manifest.

| metric | value |
|---|---|
| wall time | 333 s (single process) |
| chunks | 6 636 |
| realized chunk raw sizes | min 854 B, p50 76 271 B, mean 81 390 B, max 262 144 B |
| codec mix | lzma2 6 329, brotli 131, store 176 |
| sum of compressed chunk payloads | 204 565 331 B (37.88% of raw) |
| manifest + 64 B footer | 295 480 B (0.14% of the payload) |
| packed file | 204 860 811 B |

The realized mean chunk (81.4 KiB) sits above the nominal 64 KiB because FastCDC cuts when
the mask fires, and the mask targets the average over the distribution; the minimum of
854 B shows how far individual cuts can fall below it.

### What the old run's numbers were

The chunked numbers from the lost 2026-10-05 run (A 224 327 819 B, B 224 263 182 B,
C 209 473 827 B) are **not reproducible** in any configuration measured now: fixed 64 KiB
chunks give 207.8 MB and the production packer gives 204.6 MB, both far below 224 MB. The
solid variants, by contrast, reproduced byte-identically (D 178 747 172 B, E 178 747 837 B),
so codec settings, solid blocking, and the summation rule were the same code. The
difference is therefore confined to the chunked path, and the most plausible cause is a
chunker configuration with a smaller effective unit (or a per-chunk dictionary/state cost)
— consistent with that run also being about seven times slower per variant. The old
chunked figures are withdrawn and are not used in any margin.

Checked one by one, the owner's hypotheses: LZMA2 options — identical (preset 6, dict
8 MiB), and the identical solid results prove it; chunker — production FastCDC beats fixed
64 KiB by 1.6%, so a *worse* chunked result requires smaller units, not bounds like
production's; auto policy — the same policy on both sides, and it moves the total by 0.02%
(B vs A), so it cannot explain 16 MB; per-chunk framing — production adds only the
295 KB manifest/footer (0.14%) and nothing per chunk in the payload; "compressed bytes" —
both the tool and the report mean the summed codec payload, with the container reported
separately.

## BCJ on PE files only (corpus 1, PE subset)

The corpus-wide F-vs-E reading (1.63%) dilutes the filter over the 28% of bytes that are
not native code, because the first run applied BCJ to the concatenated stream. To measure
the owner's criterion (>= 2% on the PE group, <= 10% slowdown) directly, the same tool
build ran the E and F configurations over a staged subset of the 13 PE files
(388 948 168 B, 72.0% of the corpus):

| variant | compressed | % of PE raw | encode time |
|---|---|---|---|
| E, 8 MiB blocks, dict 8 MiB | 132 493 115 B | 34.06% | 118.6 s |
| F, 8 MiB blocks, BCJ x86 + dict 8 MiB | 129 689 906 B | 33.34% | 107.7 s |

Result: **2.12% smaller on the PE bytes with no slowdown** (the BCJ pass is a
length-preserving preprocessing step; F measured faster within run noise). Cross-check: the
same delta applied to the full corpus predicts 1.57% overall, and the whole-corpus
measurement showed 1.63%, which is consistent. The adoption criterion for a default BCJ on
PE input is therefore met on this corpus, and the packer policy adopts bcj-x86 for PE x64
conditionally; ARM64 stays off until corpus 2 provides data. Note which measurement is which:
variant F in the results table applies BCJ to **every byte of the concatenated stream**,
including the 28% that are not PE, so its 1.63% is a diluted lower bound; the PE-only run in
this section is the one that matches the policy (the filter is selected per PE file by its
`Machine` field), and the policy effect on the whole payload is therefore ~1.5%

## Reading of the results

- Model: solid blocking is the smaller representation, but the margin depends on the
  baseline. Against the shipped packer (measured, not reconstructed) the gain is 12.62%
  (D/E) to 14.05% (F), so the owner's >= 10% bar is met against production. Against the best
  independent configuration measured (C, 256 KiB auto, a hypothetical improvement over
  production) it is 8.48% (D/E) and 9.97% (F) — formally 0.03 points below the bar in that
  best case, which is why the spec records the justification for keeping the block model
  instead of silently rounding toward the threshold.
- Dictionary rule: D (dict 64 MiB) and E (dict 8 MiB) differ by 665 B out of 178.7 MB on
  8 MiB blocks. An 8 MiB block cannot use more than 8 MiB of window, so the dictionary can
  be sized to the block and decoder memory stays at dict x threads.
- BCJ x86: -1.63% (E -> F) across the whole corpus and **-2.12% on the PE subset**, at no
  measurable time cost (see the dedicated section above).
- Brotli q9 rarely beats LZMA2 p6 here: B selects it for 181 of 8242 chunks, C for 5 of 2061.
  Store fallback fires for 179 (B) and 69 (C) chunks, so incompressible data exists in the
  corpus. Both numbers are corpus-specific, not a policy.
- Encode cost: the 8 MiB solid variants run in 114-148 s versus 94-110 s for chunks; D
  (dict 64 MiB) is the slowest solid variant, G (per-file, dict 64 MiB) the slowest overall.
- Headroom in G: units larger than 8 MiB, or per-file units for the largest binaries, gain
  another 3.4% over F. The default block size is therefore not settled by this run.

## End-to-end estimate (v1 container vs the shipped v0 file)

The comparisons above are payload against payload. For an end-to-end number, the v1
container overhead was estimated from the record layout in `docs/format.md` (postcard
varint widths per field, one block record per 8 MiB block, one reference per file extent),
not from a rule of thumb: the script and its inputs live in the measurement artifact
directory. For this corpus (131 files, 65 blocks, 180 file-to-block references):

| component | bytes |
|---|---|
| string table (134 strings, 4 846 B blob) | 4 846 |
| manifest: 65 block records | 4 809 |
| manifest: 131 file records (incl. 180 refs) | 3 648 |
| manifest: header fields, app strings, hashes | 132 |
| payload header | 64 |
| signature block | 128 |
| **container overhead total** | **13 627** |

The estimate excludes config, assets, and uninstaller sections (this corpus has none) and the
Authenticode table (the v0 baseline file is unsigned as well).

| end-to-end variant | bytes | vs v0 file 204 860 811 B |
|---|---|---|
| v1 with dictionary-sized blocks (E payload) | 178 761 464 | **-12.74%** |
| v1 with the adopted BCJ policy (PE-only payload) | 175 958 255 | **-14.11%** |
| v1 with whole-stream BCJ (F payload, upper bound) | 175 841 738 | -14.17% |

Writing the estimate exposed one gap in the spec: the manifest structure listed `files` and
`assets` but no place to carry the `Block` records, even though the header carries
`block_count`. The spec now has `blocks: Vec<Block>` in `PackManifest`; without it the block
table had nowhere to live. This is exactly what the byte-level example is for.

## Verdict

The two-level model (block = compression unit, CDC chunk = raw-hashing unit) is the v1.0
data model: the measured gain is 14.0-15.4% over the 64 KiB configuration and at least 8.5%
against every independent configuration measured, and the chunk-level hashing unit is
required by the phase-3 delta artifact regardless. Default parameters are not ratified by
this run; the extensions below settle them.

## Packer policy (current defaults)

Not normative: the reader accepts any payload that satisfies the format rules, and these
values can change without re-ratifying the specification (decision D11 in `docs/format.md`).

| policy | current default | source |
|---|---|---|
| block size | 8 MiB raw | first run; the block-size sweep (extension b) may raise it |
| dictionary | block size, capped by LZMA2_MAX_DICT (32 MiB) | D vs E measured equal on 8 MiB blocks |
| pre-filter | bcj-x86 for PE x64 (2.12% on the PE subset, no slowdown); ARM64 off | this report; ARM64 re-checked on corpus 2 |
| codec | LZMA2 preset 6, Brotli q9 if smaller, store if smaller | production parity |
| grouping / order | deterministic ascending path order | B7 in the spec keeps offsets normative, order stays policy |
| compression profile | release: LZMA2 preset 6; CI: lower preset or store | extension i, unmeasured |

## Planned extensions (owner review 2026-10-05, section 4.3)

Decision thresholds in parentheses; each item blocks a default parameter, not the rc2
review round.

| id | measurement | criterion |
|---|---|---|
| a | Corpus 2 and 3 without the 64 KiB filter: a Tauri v2 template app built in release, and an application with a large dist plus thousands of small files | representativeness |
| b | Block size 1 / 2 / 4 / 8 / 16 / 32 MiB with dict = block | smallest block where doubling the size gains < 2% |
| c | Grouping: filesystem order vs type-sorted vs separate streams (PE code, web, rest) | ratio gain vs complexity |
| d | BCJ: x86 for PE x64, ARM64 for PE ARM64 (chosen by the PE `Machine` field), no filter; applied only to PE files | adopt at >= 2% gain on a native-code corpus and <= 10% slowdown |
| e | Brotli vs LZMA2 on the web group (JS/CSS/HTML/JSON/SVG), quality 9 and 11 | Brotli only at >= 3% better, or >= 2x faster decode |
| f | Incompressible data: detection (magic/entropy) then store | effect on time and ratio |
| g | Decompression: throughput (MiB/s) at 1 and 4 threads for LZMA2 (with and without BCJ) and Brotli, peak RSS, time to first extracted file, versus the chunk baseline | soft target set by the owner after seeing the numbers |
| h | Metadata overhead: chunk and block counts, manifest size per corpus | consistency with the B6 limits |
| i | CI profile: lower preset or store for pull-request builds | time vs size |

Decisions that come out of these runs land in the "Compression decisions" section of
`docs/format.md`: default block size and dictionary rule (decoder memory = dict x threads),
grouping and ordering, the filter allowlist as part of the codec parameters (C4), when
Brotli applies, when store applies, and the per-profile preset (release vs CI) with the
semantics of `package.level`.

## Caveats

- The 2026-10-05 run (files lost with the OS temp directory) is not quoted here: numbers
  from different tool builds are not comparable. Its chunked variants measured worse
  (A 41.5%, C 38.8% of raw), so margins derived from it (for example "14.7% over C") are not
  valid against this run.
- Encode times are wall-clock on one machine with 4 threads and no isolation from the rest
  of the desktop session; treat them as orders of magnitude, not as benchmarks.
- Only the largest unit per variant is round-tripped; the container overhead is excluded
  from all sizes (measured separately under item h).
- Corpus 1 is a developer tool installation, not a Tauri app; declared as a proxy, replaced
  by corpus 2 for the parameter decisions.

## Whole-process memory target (informational)

The specification's 200 MiB budget bounds the decoder: worker buffers, codec state, and
per-block buffers, as derived in `docs/format.md` ("Decoder memory budget"). It is not a
process limit. The informational target for a whole run is the decoder formula plus the
stub's own baseline working set, and the T13 harness measures both in one pass: the decoder
peak must satisfy `peak <= formula` (and the 200 MiB budget), while the whole-process peak is
recorded per worker count and per medium (decode to memory, extract to the NVMe system
volume, extract to a spinning-disk secondary volume) together with the CPU model and the
physical/logical core count. No normative limit is set for the whole process; a harness
result may only tighten the formula's constants, never widen the budget.
