import json
import os
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "vectors")
MIB = 1 << 20


def tool(name):
    path = shutil.which(name)
    if not path:
        sys.exit(f"v1-vectors: {name} is required")
    return path


XZ = tool("xz")
BROTLI = tool("brotli")
BLAKE3 = tool("b3sum") if shutil.which("b3sum") else os.environ.get("BLAKE3_BIN")
if not BLAKE3 or not shutil.which(BLAKE3):
    sys.exit("v1-vectors: need b3sum (or BLAKE3_BIN) for digests")


def blake3(path):
    out = subprocess.run([BLAKE3, path], capture_output=True, check=True, text=True).stdout
    return out.split()[0]


def xz(data, extra):
    return subprocess.run(
        [XZ, "--format=raw", "-6"] + extra + ["-c", "-"], input=data, capture_output=True, check=True
    ).stdout


def brotli(data, extra):
    return subprocess.run(
        [BROTLI, "-q", "9"] + extra + ["-c", "-"], input=data, capture_output=True, check=True
    ).stdout


def app_input():
    blob = bytearray()
    for i in range(32768):
        blob.append(((i * 31 + (i >> 8) * 17 + (i >> 4)) % 253) + 1)
    text = b"orki-installer block vector sample payload for the v1 format specification\n"
    for i in range(0, 32768, 4096):
        blob[i : i + len(text)] = text
    return bytes(blob)


def pe_input():
    blob = bytearray(16384)
    blob[0:2] = b"MZ"
    blob[0x3C] = 0x80
    blob[0x80:0x84] = b"PE\x00\x00"
    blob[0x84:0x86] = (0x8664).to_bytes(2, "little")
    cursor = 0x200
    while cursor + 6 < len(blob):
        blob[cursor] = 0xE8 if (cursor & 2) else 0xE9
        blob[cursor + 1 : cursor + 5] = (cursor - 0x400).to_bytes(4, "little", signed=True)
        cursor += 0x20
    for i in range(0x1000, len(blob)):
        blob[i] = (i * 7 + 3) % 256
    return bytes(blob)


def first_lzma2_chunk(blob):
    control = blob[0]
    reset = (control >> 5) & 3
    u_size = ((control & 0x1F) << 16) | (blob[1] << 8) | blob[2]
    c_size = ((blob[3] << 8) | blob[4]) + 1
    props = blob[5] if reset >= 2 else None
    return reset, u_size, c_size, props


def write(name, data):
    path = os.path.join(OUT, name)
    with open(path, "wb") as fh:
        fh.write(data)
    return name


def main():
    os.makedirs(OUT, exist_ok=True)
    app = app_input()
    pe = pe_input()
    app_name = write("input.app.bin", app)
    pe_name = write("input.pe.bin", pe)

    vectors = []

    store = "lzma2-8m.bin"
    write(store, xz(app, [f"--lzma2=dict={8 * MIB}"]))
    vectors.append(
        dict(
            id="V1",
            codec="lzma2",
            codec_id=1,
            filter="none",
            filter_id=0,
            lzma2_dict_size=8 * MIB,
            comp_file=store,
            raw_file=app_name,
            raw_len=len(app),
            expect="ok",
            note="plain LZMA2, dictionary 8 MiB, preset 6 parameters",
        )
    )

    store = "lzma2-x86-8m.bin"
    write(store, xz(pe, ["--x86", f"--lzma2=dict={8 * MIB}"]))
    vectors.append(
        dict(
            id="V2",
            codec="lzma2",
            codec_id=1,
            filter="bcj-x86",
            filter_id=1,
            lzma2_dict_size=8 * MIB,
            comp_file=store,
            raw_file=pe_name,
            raw_len=len(pe),
            expect="ok",
            note="bcj-x86 pre-filter, then LZMA2 dictionary 8 MiB",
        )
    )

    store = "brotli-q9-w22.bin"
    write(store, brotli(app, ["-w", "22"]))
    vectors.append(
        dict(
            id="V3",
            codec="brotli",
            codec_id=2,
            filter="none",
            filter_id=0,
            lzma2_dict_size=0,
            comp_file=store,
            raw_file=app_name,
            raw_len=len(app),
            expect="ok",
            note="Brotli quality 9, window bits 22",
            brotli_wbits=22,
        )
    )

    store = "lzma2-dict32m.bin"
    write(store, xz(app, [f"--lzma2=dict={32 * MIB}"]))
    vectors.append(
        dict(
            id="V4",
            codec="lzma2",
            codec_id=1,
            filter="none",
            filter_id=0,
            lzma2_dict_size=32 * MIB,
            comp_file=store,
            raw_file=app_name,
            raw_len=len(app),
            expect="ok",
            note="dictionary exactly at LZMA2_MAX_DICT",
        )
    )

    store = "lzma2-bomb.bin"
    write(store, xz(bytes(MIB), [f"--lzma2=dict={1 * MIB}"]))
    vectors.append(
        dict(
            id="V5",
            codec="lzma2",
            codec_id=1,
            filter="none",
            filter_id=0,
            lzma2_dict_size=1 * MIB,
            comp_file=store,
            raw_file=None,
            raw_len=4096,
            expect="ORKI-1001",
            note="declares raw_len 4096 but decompresses to 1 MiB; the reader stops at raw_len + 1",
        )
    )

    store = "brotli-lgwin25.bin"
    write(store, brotli(app, ["--large_window=25"]))
    vectors.append(
        dict(
            id="V6",
            codec="brotli",
            codec_id=2,
            filter="none",
            filter_id=0,
            lzma2_dict_size=0,
            comp_file=store,
            raw_file=None,
            raw_len=len(app),
            expect="ORKI-1001",
            note="large-window Brotli bitstream (window 25 > 24); rejected before decoding",
        )
    )

    store = "lzma2-dict64m.bin"
    write(store, xz(app, [f"--lzma2=dict={64 * MIB}"]))
    vectors.append(
        dict(
            id="V7",
            codec="lzma2",
            codec_id=1,
            filter="none",
            filter_id=0,
            lzma2_dict_size=64 * MIB,
            comp_file=store,
            raw_file=None,
            raw_len=len(app),
            expect="ORKI-1001",
            note="dictionary above LZMA2_MAX_DICT (32 MiB); rejected from the manifest value",
        )
    )

    store = "brotli-q9-w16.bin"
    write(store, brotli(app, ["-w", "16"]))
    vectors.append(
        dict(
            id="V8",
            codec="brotli",
            codec_id=2,
            filter="none",
            filter_id=0,
            lzma2_dict_size=0,
            comp_file=store,
            raw_file=app_name,
            raw_len=len(app),
            expect="ok",
            note="Brotli quality 9, the smallest window bits and the shortest WBITS prefix",
            brotli_wbits=16,
        )
    )

    for vector in vectors:
        path = os.path.join(OUT, vector["comp_file"])
        vector["comp_len"] = os.path.getsize(path)
        vector["comp_blake3"] = blake3(path)
        vector["raw_blake3"] = blake3(os.path.join(OUT, vector["raw_file"])) if vector["raw_file"] else None
        vector["lzma2_lclppb"] = first_lzma2_chunk(open(path, "rb").read())[3] if vector["codec"] == "lzma2" else None

    tools = {}
    for name, binary in (("xz", XZ), ("brotli", BROTLI), ("digest", BLAKE3)):
        out = subprocess.run([binary, "--version"], capture_output=True, text=True).stdout
        tools[name] = out.strip().splitlines()[0] if out.strip() else os.path.basename(binary)

    document = {
        "schema": 1,
        "generated_by": "docs/examples/v1-vectors.py",
        "verified_by": "docs/examples/v1-vectors-check.py",
        "tools": tools,
        "vectors": vectors,
    }
    with open(os.path.join(HERE, "v1-vectors.json"), "w", newline="\n") as fh:
        json.dump(document, fh, indent=2, sort_keys=True)
        fh.write("\n")

    for vector in vectors:
        print(
            f"{vector['id']}: codec={vector['codec']} filter={vector['filter']} "
            f"comp_len={vector['comp_len']} raw_len={vector['raw_len']} expect={vector['expect']}"
        )


if __name__ == "__main__":
    main()
