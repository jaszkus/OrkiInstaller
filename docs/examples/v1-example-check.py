import hashlib
import lzma
import os
import shutil
import struct
import subprocess
import sys
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))
HEADER_SIZE = 64
SIG_SIZE = 128
FILTER_NAMES = {0: "none", 1: "bcj-x86", 2: "bcj-arm64"}
CODEC_NAMES = {0: "store", 1: "lzma2", 2: "brotli"}


class Fault(Exception):
    def __init__(self, code, stage):
        super().__init__(f"{code} {stage}")
        self.code = code
        self.stage = stage


def require(condition, stage, code="ORKI-1001"):
    if not condition:
        raise Fault(code, stage)


def blake3_tool():
    for candidate in (os.environ.get("BLAKE3_BIN"), "b3sum", "blake3"):
        if not candidate:
            continue
        path = shutil.which(candidate)
        if path:
            return path
    return None


BLAKE3 = blake3_tool()
try:
    import blake3 as blake3_module
except ImportError:
    blake3_module = None
if blake3_module is None and BLAKE3 is None:
    sys.exit("v1-example-check: need the blake3 Python module, b3sum, or BLAKE3_BIN")

BCJ_DICT = 1 << 20


def blake3(data, tag="digest"):
    if blake3_module is not None:
        return blake3_module.blake3(data).digest()
    path = os.path.join(HERE, f".{tag}.tmp")
    with open(path, "wb") as fh:
        fh.write(data)
    out = subprocess.run([BLAKE3, path], capture_output=True, check=True, text=True).stdout
    os.remove(path)
    return bytes.fromhex(out.split()[0])


def inverse_bcj_x86(data):
    chain = lzma.compress(
        data,
        format=lzma.FORMAT_RAW,
        filters=[{"id": lzma.FILTER_LZMA2, "preset": 6, "dict_size": BCJ_DICT}],
    )
    return lzma.decompress(
        chain,
        format=lzma.FORMAT_RAW,
        filters=[{"id": lzma.FILTER_X86}, {"id": lzma.FILTER_LZMA2, "dict_size": BCJ_DICT}],
    )


def varint_at(buf, pos):
    value = 0
    shift = 0
    while True:
        byte = buf[pos]
        pos += 1
        value |= (byte & 0x7F) << shift
        if not byte & 0x80:
            return value, pos
        shift += 7


def pstring_at(buf, pos):
    length, pos = varint_at(buf, pos)
    return buf[pos : pos + length], pos + length


def validate(payload):
    try:
        return parse_payload(payload)
    except Fault:
        raise
    except (IndexError, struct.error, ValueError) as error:
        raise Fault("ORKI-1001", f"malformed payload structure ({type(error).__name__})") from error


def parse_payload(payload):
    positions = {}
    require(payload[0:8] == b"ORKIHDR\x00", "payload magic")
    positions["version"] = 8
    positions["file_count"] = 24
    positions["block_count"] = 28
    positions["manifest_offset"] = 32
    positions["manifest_len"] = 40
    positions["flags"] = 44
    positions["blocks_offset"] = 48
    positions["crc"] = 56
    positions["reserved"] = 60
    positions["string_magic"] = 64
    version, header_size = struct.unpack_from("<II", payload, 8)
    payload_len = struct.unpack_from("<Q", payload, 16)[0]
    file_count, block_count = struct.unpack_from("<II", payload, 24)
    manifest_offset = struct.unpack_from("<Q", payload, 32)[0]
    manifest_len = struct.unpack_from("<I", payload, 40)[0]
    flags = struct.unpack_from("<I", payload, 44)[0]
    blocks_offset = struct.unpack_from("<Q", payload, 48)[0]
    crc = struct.unpack_from("<I", payload, 56)[0]
    reserved = struct.unpack_from("<I", payload, 60)[0]
    require(version == 1, "format_version", "ORKI-1003")
    require(header_size == HEADER_SIZE, "header_size")
    require(file_count <= 100000, "max files")
    require(block_count <= 200000, "max blocks")
    require(manifest_len <= 64 << 20, "max manifest_len")
    require(payload_len == len(payload), "payload_len")
    require(reserved == 0, "reserved bytes")
    require(flags & 0x1 and flags & ~0x1 == 0, "flag bits")
    require(crc == zlib.crc32(payload[0:56]) & 0xFFFFFFFF, "header_crc32")

    require(payload[HEADER_SIZE:HEADER_SIZE + 8] == b"ORKISTR\x00", "string table magic")
    str_count, blob_len = struct.unpack_from("<II", payload, HEADER_SIZE + 8)
    string_end = HEADER_SIZE + 16 + 4 * str_count + blob_len
    require(string_end == manifest_offset, "string table end")
    require(str_count <= 65536, "string table count")

    m = payload[manifest_offset : manifest_offset + manifest_len]
    pos = 0
    positions["schema"] = manifest_offset
    schema, pos = varint_at(m, pos)
    require(schema == 1, "manifest schema")
    app_id, pos = pstring_at(m, pos)
    app_name, pos = pstring_at(m, pos)
    app_version, pos = pstring_at(m, pos)
    require(app_id and app_name and app_version, "app identity strings")
    slot = m[pos]
    pos += 1
    require(slot <= 3, "signer_key_slot")
    positions["config_offset"] = manifest_offset + pos
    config_offset, pos = varint_at(m, pos)
    positions["config_len"] = manifest_offset + pos
    config_len, pos = varint_at(m, pos)
    positions["config_hash"] = manifest_offset + pos
    config_hash = m[pos : pos + 32]
    pos += 32
    uninstaller_offset, pos = varint_at(m, pos)
    uninstaller_len, pos = varint_at(m, pos)
    positions["uninstaller_hash"] = manifest_offset + pos
    uninstaller_hash = m[pos : pos + 32]
    pos += 32
    positions["asset_count"] = manifest_offset + pos
    asset_count, pos = varint_at(m, pos)
    require(asset_count <= 10000, "asset count")
    assets = []
    for _ in range(asset_count):
        positions.setdefault("first_asset_kind", manifest_offset + pos)
        kind = m[pos]
        pos += 1
        path_idx, pos = varint_at(m, pos)
        target_idx, pos = varint_at(m, pos)
        offset, pos = varint_at(m, pos)
        length, pos = varint_at(m, pos)
        positions.setdefault("first_asset_hash", manifest_offset + pos)
        digest = m[pos : pos + 32]
        pos += 32
        require(kind <= 9, "asset kind")
        require(path_idx < str_count and target_idx < str_count, "asset string index")
        assets.append((kind, offset, length, digest))
    positions["block_count_manifest"] = manifest_offset + pos
    blocks, pos = varint_at(m, pos)
    require(blocks == block_count, "block count")
    block_table = []
    for index in range(blocks):
        if index == 0:
            positions["first_block"] = manifest_offset + pos
            positions["first_block_codec"] = manifest_offset + pos
            positions["first_block_filter"] = manifest_offset + pos + 1
            positions["first_block_comp_len"] = manifest_offset + pos + 2
            positions["first_block_comp_hash"] = manifest_offset + pos + 10
        codec, filt = m[pos], m[pos + 1]
        comp_len, raw_len = struct.unpack_from("<II", m, pos + 2)
        comp_hash, raw_hash = m[pos + 10 : pos + 42], m[pos + 42 : pos + 74]
        pos += 74
        require(codec in (0, 1, 2), "codec id")
        require(filt in FILTER_NAMES, "filter allowlist")
        block_table.append((codec, filt, comp_len, raw_len, comp_hash, raw_hash))
    positions["file_count_manifest"] = manifest_offset + pos
    files, pos = varint_at(m, pos)
    require(files == file_count, "file count")
    extents = []
    total = 0
    for _ in range(files):
        path_idx, pos = varint_at(m, pos)
        component, pos = varint_at(m, pos)
        target, pos = varint_at(m, pos)
        arch_mask = struct.unpack_from("<H", m, pos)[0]
        pos += 2
        entry_kind = m[pos]
        attrs = m[pos + 1]
        pos += 2
        mtime, pos = varint_at(m, pos)
        pe_version = m[pos]
        pos += 1
        if pe_version:
            require(pe_version == 1, "pe_version tag")
            for _ in range(4):
                _, pos = varint_at(m, pos)
        require(
            path_idx < str_count and component < str_count and target < str_count,
            "file string indices",
        )
        require(entry_kind <= 1 and attrs <= 0x7 and mtime > 0, "file metadata")
        require(arch_mask <= 0x3, "arch_mask")
        positions.setdefault("first_file_size", manifest_offset + pos)
        size, pos = varint_at(m, pos)
        refs, pos = varint_at(m, pos)
        accounted = 0
        for _ in range(refs):
            index, pos = varint_at(m, pos)
            offset, pos = varint_at(m, pos)
            positions.setdefault("first_ref_len", manifest_offset + pos)
            length, pos = varint_at(m, pos)
            require(index < blocks, "reference block index")
            require(offset + length <= block_table[index][3], "reference inside block")
            extents.append((index, offset, offset + length))
            accounted += length
        require(accounted == size, "per-file raw accounting")
        total += size
    require(pos == len(m), "manifest fully consumed")
    require(total == sum(b[3] for b in block_table), "file bytes equal block raw bytes")

    sections = [
        ("string table", HEADER_SIZE, manifest_offset),
        ("manifest", manifest_offset, manifest_offset + manifest_len),
        ("config", config_offset, config_offset + config_len),
        ("assets", config_offset + config_len, uninstaller_offset),
        ("uninstaller", uninstaller_offset, uninstaller_offset + uninstaller_len),
        ("blocks", blocks_offset, payload_len - SIG_SIZE),
    ]
    cursor = HEADER_SIZE
    for name, start, end in sections:
        require(start == cursor, f"section coverage before {name}")
        cursor = end
    require(cursor == payload_len - SIG_SIZE, "section coverage end")

    require(blake3(payload[config_offset : config_offset + config_len], "cfg") == config_hash, "config hash")
    require(
        blake3(payload[uninstaller_offset : uninstaller_offset + uninstaller_len], "uni")
        == uninstaller_hash,
        "uninstaller hash",
    )
    for kind, offset, length, digest in assets:
        start = config_offset + config_len + offset
        require(blake3(payload[start : start + length], "asset") == digest, "asset hash")

    cursor = blocks_offset
    for codec, filt, comp_len, raw_len, comp_hash, raw_hash in block_table:
        blob = payload[cursor : cursor + comp_len]
        require(len(blob) == comp_len, "block region truncated")
        require(blake3(blob, "comp") == comp_hash, "comp_blake3")
        require(comp_len <= raw_len + 1024, "block slack")
        if filt:
            require(comp_len == raw_len, "filter is length preserving")
            require(codec == 0, "the examples filter store blocks only")
            unfiltered = inverse_bcj_x86(blob)
            require(len(unfiltered) == raw_len, "inverse filter length")
            require(
                blake3(unfiltered, "unfiltered") == raw_hash,
                "raw_blake3 after the inverse filter",
            )
        elif codec == 0:
            require(blake3(blob, "raw") == raw_hash, "raw_blake3")
        cursor += comp_len
    require(cursor == payload_len - SIG_SIZE, "blocks region end")

    for index in range(blocks):
        covered = [e for e in extents if e[0] == index]
        require(covered, f"block {index} referenced")
        require(max(e[2] for e in covered) == block_table[index][3], f"block {index} covered")

    report = {
        "files": files,
        "blocks": blocks,
        "assets": asset_count,
        "payload_len": payload_len,
        "manifest_offset": manifest_offset,
        "manifest_len": manifest_len,
        "blocks_offset": blocks_offset,
        "header_crc32": crc,
        "sig_digest": blake3(payload[56 : payload_len - SIG_SIZE], "covered").hex(),
        "sha256": hashlib.sha256(payload).hexdigest(),
        "sections": sections,
        "positions": positions,
    }
    return report


PINNED = {
    "v1-example-1.bin": {
        "payload_len": 454,
        "manifest_offset": 102,
        "manifest_len": 213,
        "blocks_offset": 315,
        "header_crc32": 0x330A711E,
        "sig_digest": "bbf60e4a35fb491e7be719f7db70cb28c13e5e3060fcd2d375a87dad684404a8",
        "sha256": "01b810338574d3427746520c90806b6dd982c2fc27a99efe7bb76c746a4816f2",
    },
    "v1-example-2.bin": {
        "payload_len": 1252,
        "manifest_offset": 132,
        "manifest_len": 374,
        "blocks_offset": 612,
        "header_crc32": 0x03D849C9,
        "sig_digest": "31d72217d028a1edbed1e63a9c9ad16f08088c76fa400d988de54d4e4d0738a3",
        "sha256": "b90f795d8f1981c8b33456f34288c828f95f0d363bfe38fa4fcd5f928156f124",
    },
}


def check(payload, label):
    report = validate(payload)
    mismatches = []
    for key, value in PINNED.get(label, {}).items():
        if report[key] != value:
            mismatches.append(f"{key}: got {report[key]}, spec pins {value}")
    print(f"{label}: OK")
    for name, start, end in report.pop("sections"):
        print(f"  section {name}: offset={start} len={end - start}")
    report.pop("positions")
    for key, value in report.items():
        print(f"  {key}={value}")
    for mismatch in mismatches:
        print(f"  PINNED FAIL {mismatch}")
    if mismatches:
        sys.exit(f"v1-example-check: {label} does not match the values pinned in docs/format.md")
    return report


def main():
    for name in sys.argv[1:] or ["v1-example-1.bin", "v1-example-2.bin"]:
        path = os.path.join(HERE, name)
        with open(path, "rb") as fh:
            check(fh.read(), name)


if __name__ == "__main__":
    main()
