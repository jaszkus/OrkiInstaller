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

- Tool: a scratch Rust binary (`orki-ratio`, release build) that uses the production codec
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

- Installed Electron-style desktop application (`@codebufffreebuff-desktop` under
  `%LOCALAPPDATA%\Programs`) as a stand-in for a Tauri app payload.
- Selection: regular files >= 64 KiB — 131 files, 540 106 180 B (515.1 MiB) raw.
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
| C vs A (256 KiB chunks vs 64 KiB chunks) | 6.0% |
| D / E vs A (solid 8 MiB, dict 64/8 MiB) | 14.0% |
| F vs A (solid 8 MiB + BCJ) | 15.4% |
| G vs A (per file + BCJ) | 18.3% |
| D / E vs C (best independent configuration) | 8.5% |
| F vs C | 10.0% |
| G vs C | 13.1% |

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

## Reading of the results

- Model: solid blocking is the smaller representation, but the margin depends on the
  baseline. Against the 64 KiB chunking that matches the shipped v0 behaviour (A/B) the
  gain is 14.0-15.4%; against the best independent configuration measured (C, 256 KiB auto)
  it is 8.5% (D/E) to 10.0% (F) — at the owner's >= 10% bar, not comfortably above it.
- Dictionary rule: D (dict 64 MiB) and E (dict 8 MiB) differ by 665 B out of 178.7 MB on
  8 MiB blocks. An 8 MiB block cannot use more than 8 MiB of window, so the dictionary can
  be sized to the block and decoder memory stays at dict x threads.
- BCJ x86: -1.63% (E -> F) at no measurable time cost (114.4 s vs 117.9 s). Corpus 1 mixes
  JavaScript/HTML/JSON with native code, so it under-represents the owner's >= 2% adoption
  threshold for native-code payloads; a PE-dominated corpus is in the extensions.
- Brotli q9 rarely beats LZMA2 p6 here: B selects it for 181 of 8242 chunks, C for 5 of 2061.
  Store fallback fires for 179 (B) and 69 (C) chunks, so incompressible data exists in the
  corpus. Both numbers are corpus-specific, not a policy.
- Encode cost: the 8 MiB solid variants run in 114-148 s versus 94-110 s for chunks; D
  (dict 64 MiB) is the slowest solid variant, G (per-file, dict 64 MiB) the slowest overall.
- Headroom in G: units larger than 8 MiB, or per-file units for the largest binaries, gain
  another 3.4% over F. The default block size is therefore not settled by this run.

## Verdict

The two-level model (block = compression unit, CDC chunk = raw-hashing unit) is the v1.0
data model: the measured gain is 14.0-15.4% over the 64 KiB configuration and at least 8.5%
against every independent configuration measured, and the chunk-level hashing unit is
required by the phase-3 delta artifact regardless. Default parameters are not ratified by
this run; the extensions below settle them.

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
