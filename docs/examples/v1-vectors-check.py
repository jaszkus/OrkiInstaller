import json
import lzma
import os
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
VECTORS = os.path.join(HERE, "vectors")
LZMA2_MAX_DICT = 32 << 20


XZ = shutil.which("xz")
BROTLI = shutil.which("brotli")
BLAKE3 = os.environ.get("BLAKE3_BIN") or shutil.which("b3sum")
try:
    import blake3 as blake3_module
except ImportError:
    blake3_module = None
try:
    import brotli as brotli_module
except ImportError:
    brotli_module = None
if blake3_module is None and BLAKE3 is None:
    sys.exit("v1-vectors-check: need the blake3 Python module, b3sum, or BLAKE3_BIN")
if brotli_module is None and BROTLI is None:
    sys.exit("v1-vectors-check: need the brotli Python module or the brotli CLI")

FAILURES = []


def blake3(data):
    if blake3_module is not None:
        return blake3_module.blake3(data).hexdigest()
    path = os.path.join(HERE, ".vector.tmp")
    with open(path, "wb") as fh:
        fh.write(data)
    out = subprocess.run([BLAKE3, path], capture_output=True, check=True, text=True).stdout
    os.remove(path)
    return out.split()[0]


def expect(condition, label):
    if not condition:
        FAILURES.append(label)
    print(f"  {'ok  ' if condition else 'FAIL'} {label}")
    return condition


def first_lzma2_chunk(blob):
    control = blob[0]
    reset = (control >> 5) & 3
    u_size = ((control & 0x1F) << 16) | (blob[1] << 8) | blob[2]
    c_size = ((blob[3] << 8) | blob[4]) + 1
    props = blob[5] if reset >= 2 else None
    return reset, u_size, c_size, props


def brotli_window_bits(head):
    if not head[0] & 1:
        return 16, False
    n = (head[0] >> 1) & 7
    if n:
        return 17 + n, False
    n = (head[0] >> 4) & 7
    if n == 1:
        return None, not (head[0] >> 7) & 1
    if n:
        return 8 + n, False
    return 17, False


def decode_lzma2(blob, dict_size):
    dec = lzma.LZMADecompressor(
        format=lzma.FORMAT_RAW, filters=[{"id": lzma.FILTER_LZMA2, "dict_size": dict_size}]
    )
    try:
        return dec.decompress(blob), None
    except lzma.LZMAError as error:
        return None, str(error)


def decode_lzma2_x86(blob, dict_size):
    try:
        return (
            lzma.decompress(
                blob,
                format=lzma.FORMAT_RAW,
                filters=[
                    {"id": lzma.FILTER_X86},
                    {"id": lzma.FILTER_LZMA2, "dict_size": dict_size},
                ],
            ),
            None,
        )
    except (lzma.LZMAError, ValueError) as error:
        if XZ is None:
            return None, str(error)
    out = subprocess.run(
        [XZ, "-d", "--format=raw", "--x86", f"--lzma2=dict={dict_size}", "-c", "-"],
        input=blob,
        capture_output=True,
    )
    return (out.stdout, None) if out.returncode == 0 else (None, out.stderr.decode().strip())


def decode_brotli(blob):
    if brotli_module is not None:
        try:
            return brotli_module.decompress(blob), None
        except Exception as error:
            return None, str(error)
    out = subprocess.run([BROTLI, "-d", "-c", "-"], input=blob, capture_output=True)
    return (out.stdout, None) if out.returncode == 0 else (None, out.stderr.decode().strip())


def main():
    with open(os.path.join(HERE, "v1-vectors.json"), "r", encoding="utf-8") as fh:
        document = json.load(fh)
    limit = document.get("brotli_max_lgwin", 22)
    print(f"tools: {document['tools']} brotli_max_lgwin={limit}")
    for vector in document["vectors"]:
        blob = open(os.path.join(VECTORS, vector["comp_file"]), "rb").read()
        print(f"{vector['id']}: {vector['comp_file']} ({vector['note']})")
        expect(len(blob) == vector["comp_len"], f"{vector['id']} comp_len {vector['comp_len']}")
        expect(
            blake3(blob) == vector["comp_blake3"], f"{vector['id']} comp_blake3"
        )
        chunk = None
        if vector["codec"] == "lzma2":
            reset, u_size, c_size, props = first_lzma2_chunk(blob)
            chunk = u_size
            print(
                f"  LZMA2 first chunk: reset={reset} u_size={u_size} c_size={c_size} lclppb={props}"
            )
            expect(
                props == vector["lzma2_lclppb"], f"{vector['id']} LZMA2 lclppb {vector['lzma2_lclppb']}"
            )
            if vector["expect"] == "ok":
                expect(
                    u_size <= vector["raw_len"],
                    f"{vector['id']} first chunk within raw_len {vector['raw_len']}",
                )
        if vector["codec"] == "brotli":
            wbits, large = brotli_window_bits(blob)
            if vector["expect"] == "ok":
                expect(
                    not large and wbits is not None and wbits <= limit,
                    f"{vector['id']} brotli window bits within {limit}",
                )

        declared = vector["raw_len"]
        if vector["codec"] == "lzma2" and vector["filter_id"] == 0:
            raw, error = decode_lzma2(blob, vector["lzma2_dict_size"])
        elif vector["codec"] == "lzma2":
            raw, error = decode_lzma2_x86(blob, vector["lzma2_dict_size"])
        else:
            raw, error = decode_brotli(blob)

        if vector["expect"] == "ok":
            expect(error is None, f"{vector['id']} decodes")
            expect(
                raw is not None
                and len(raw) == declared
                and blake3(raw) == vector["raw_blake3"],
                f"{vector['id']} raw_len and raw_blake3",
            )
            if vector["raw_file"]:
                expect(
                    blake3(open(os.path.join(VECTORS, vector["raw_file"]), "rb").read())
                    == vector["raw_blake3"],
                    f"{vector['id']} raw file matches the vector metadata",
                )
            expect(
                len(blob) <= declared + 1024, f"{vector['id']} comp_len within raw_len + 1 KiB"
            )
        else:
            reason = None
            if error is not None:
                reason = "the decoder rejects the stream"
            elif raw is not None and len(raw) > declared + 1:
                reason = f"output {len(raw)} exceeds the declared raw_len + 1 cap"
            if chunk is not None and chunk > declared:
                reason = (
                    f"first LZMA2 chunk declares {chunk} bytes for a block declaring raw_len {declared}"
                )
            if vector["codec"] == "lzma2" and vector["lzma2_dict_size"] > LZMA2_MAX_DICT:
                reason = f"dictionary {vector['lzma2_dict_size']} exceeds LZMA2_MAX_DICT"
            if vector["codec"] == "brotli":
                wbits, large = brotli_window_bits(blob)
                if large:
                    reason = "large-window brotli bitstream"
                elif (wbits or 0) > limit:
                    reason = f"window bits {wbits} above BROTLI_MAX_LGWIN ({limit})"
            expect(reason is not None, f"{vector['id']} rejected: {reason}")

    print(f"vectors={len(document['vectors'])} failures={len(FAILURES)}")
    if FAILURES:
        for failure in FAILURES:
            print(f"  FAIL {failure}")
        sys.exit(1)
    print("all vectors verified")


if __name__ == "__main__":
    main()
