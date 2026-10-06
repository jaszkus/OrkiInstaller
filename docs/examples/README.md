# v1 format reference material

This directory is **not product code**. It exists so that a second implementation can be
written from `docs/format.md` alone and compared byte for byte, which is the only way the
specification's ambiguity can be found before Rust code depends on it.

Everything here is generated and checked by scripts in this directory. Requirements:

- Python 3.11 or newer; the checkers prefer the `blake3` and `brotli` wheels
  (`pip install blake3 brotli`) and fall back to the CLI tools below.
- BLAKE3: the `blake3` Python module, `b3sum` on `PATH`, or `BLAKE3_BIN` pointing at a
  BLAKE3-capable binary that prints the hash of a file followed by the file name
  (`b3sum <file>`); for example `BLAKE3_BIN=C:/tools/orki-digest.exe`.
- `xz` (tested with XZ Utils 5.8.3), required only by the generators: it produces the LZMA2
  vectors and the `bcj-x86` pre-filtered block of example 2. The checkers decode with
  Python's liblzma binding, including the x86 filter, and fall back to the `xz` CLI.
- `brotli` (tested with Brotli 1.2.0), required only by the vector generator; the checker
  uses the `brotli` Python module or the CLI.

CI runs the three checkers against the committed files (job `docs examples`), so the
artifacts cannot drift from the tables in `docs/format.md` without failing the build.
Waivers for these test tools live in `docs/dependency-waivers.md`.

The scripts are deliberately small and free of comments: the document is the specification,
these files are the executable form of the byte map and the vector tables in it.

## Files

| file | role |
|---|---|
| `v1-example.py` | builds `v1-example-1.bin` and `v1-example-2.bin` and prints every field value (offsets, lengths, hashes, header values) |
| `v1-example-check.py` | independent reader: parses both examples from the format rules and validates every invariant, reporting the Orki error code of the first failure |
| `v1-negative.py` | mutates example 2 field by field (29 cases) and asserts each mutation is rejected with the documented code |
| `v1-vectors.py` | produces the decoder vectors in `vectors/` with `xz` and `brotli` and writes `v1-vectors.json` |
| `v1-vectors-check.py` | decodes every vector with external tooling and validates length, `raw_blake3`, the LZMA2 dictionary, and the Brotli window bits |
| `v1-example-1.bin`, `v1-example-2.bin` | the byte maps in `docs/format.md`, committed so a reviewer can diff them |
| `vectors/` | block payloads for V1-V10 plus their inputs |

The examples are unsigned: their 128-byte signature blocks are zero-filled. The signature
bytes depend on the test key and belong to the golden files of the implementation PR, which
must reproduce the payload bytes here exactly.

## Regenerating

```sh
cd docs/examples
export BLAKE3_BIN=/path/to/b3sum-compatible/binary
python v1-example.py
python v1-example-check.py
python v1-negative.py
python v1-vectors.py
python v1-vectors-check.py
```

`v1-example.py` calls `xz` to produce the `bcj-x86` pre-filtered block of example 2: the
filtered bytes are recovered from `xz --format=raw -6 --x86 --lzma2=...` by decoding the LZMA2
layer alone, which leaves the filter's output. The same output was cross-checked against
`lzma_rust2::filter::bcj::BcjWriter` (identical bytes for the sample and for an adversarial
input of 44 624 bytes with dense `E8`/`E9` patterns), so the example pins the behaviour both
implementations share. No comments and no absolute paths are stored in the scripts.

## Findings these artifacts produced

- The first `payload_fingerprint` definition was circular and was removed (D12).
- The manifest had no `blocks` field to carry the records `block_count` announces.
- "A filter applies only to PE input" was unenforceable while a block could mix PE and
  non-PE content; the rule became D15.
- `raw_len` and `raw_blake3` needed an explicit subject for filtered blocks: they describe
  the block's raw byte range, not the codec's filtered output.
- A large-window Brotli bitstream (`lgwin > 24`) is accepted by brotli 1.2.0 and by the Rust
  `brotli` crate used by `orki-pack`, so the reader must check `WBITS` itself.
- An LZMA2 chunk header declares its uncompressed size, which lets a reader reject a
  decompression bomb before allocating anything.
