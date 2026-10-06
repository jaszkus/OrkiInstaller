import hashlib
import lzma
import os
import shutil
import struct
import subprocess
import sys
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))
SIG_SIZE = 128
HEADER_SIZE = 64
DOMAIN = b"DOMAIN_TAG_ORKI_PAYLOAD_V1"
CODEC_STORE = 0
FILTER_NONE = 0
FILTER_BCJ_X86 = 1
BCJ_DICT = 1 << 20


def tool(name, env=None):
    for candidate in ([os.environ.get(env)] if env else []) + [name]:
        if not candidate:
            continue
        path = shutil.which(candidate)
        if path:
            return path
    sys.exit(f"v1-example: {name} is required (set {env} to override the lookup)")


BLAKE3 = tool("b3sum", "BLAKE3_BIN")
XZ = tool("xz")


def blake3(data, tag):
    path = os.path.join(HERE, f".{tag}.tmp")
    with open(path, "wb") as fh:
        fh.write(data)
    out = subprocess.run([BLAKE3, path], capture_output=True, check=True, text=True).stdout
    os.remove(path)
    return bytes.fromhex(out.split()[0])


def bcj_x86(data):
    chain = subprocess.run(
        [XZ, "--format=raw", "-6", "--x86", f"--lzma2=dict={BCJ_DICT}", "-c", "-"],
        input=data,
        capture_output=True,
        check=True,
    ).stdout
    dec = lzma.LZMADecompressor(
        format=lzma.FORMAT_RAW, filters=[{"id": lzma.FILTER_LZMA2, "dict_size": BCJ_DICT}]
    )
    filtered = dec.decompress(chain)
    if len(filtered) != len(data):
        sys.exit("v1-example: bcj-x86 changed the length")
    return filtered


def varint(value):
    out = bytearray()
    while True:
        byte = value & 0x7F
        value >>= 7
        if value:
            out.append(byte | 0x80)
        else:
            out.append(byte)
            return bytes(out)


def pstring(text):
    blob = text.encode("utf-8")
    return varint(len(blob)) + blob


def crc32(data):
    return zlib.crc32(data) & 0xFFFFFFFF


def string_table(strings):
    blob = b"".join(s.encode("utf-8") for s in strings)
    offsets = []
    cursor = 0
    for s in strings:
        offsets.append(cursor)
        cursor += len(s.encode("utf-8"))
    return (
        b"ORKISTR\x00"
        + struct.pack("<II", len(strings), len(blob))
        + b"".join(struct.pack("<I", o) for o in offsets)
        + blob
    )


class Entry:
    def __init__(self, path, data, component="", target="", pe_version=None, filterable=False):
        self.path = path
        self.data = data
        self.component = component
        self.target = target
        self.pe_version = pe_version
        self.filterable = filterable


def place(entries):
    placed = []
    cursor = 0
    for entry in entries:
        placed.append((entry, cursor, cursor + len(entry.data)))
        cursor += len(entry.data)
    return placed


def plan_blocks(placed, total, block_size):
    blocks = []
    for start in range(0, max(total, 1), block_size):
        end = min(start + block_size, total)
        owners = [e for e, a, b in placed if a < end and b > start]
        filtered = len(owners) == 1 and owners[0].filterable
        blocks.append((start, end, FILTER_BCJ_X86 if filtered else FILTER_NONE))
    return blocks


def build(app_id, app_name, app_version, entries, config, assets, uninstaller, block_size):
    placed = place(entries)
    raw = b"".join(e.data for e, _, _ in placed)
    blocks = plan_blocks(placed, len(raw), block_size)

    strings = [""] + sorted(
        {
            s
            for e in entries
            for s in (e.path, e.component, e.target)
            if s != ""
        }
    )
    index = {s: i for i, s in enumerate(strings)}
    table = string_table(strings)
    manifest_offset = HEADER_SIZE + len(table)

    stored = []
    for start, end, filter_id in blocks:
        chunk = raw[start:end]
        stored.append(bcj_x86(chunk) if filter_id == FILTER_BCJ_X86 else chunk)
    stored_blob = b"".join(stored)
    asset_blob = b"".join(asset for _, asset in assets)

    def block_records():
        out = b""
        for (start, end, filter_id), blob in zip(blocks, stored):
            raw_bytes = raw[start:end]
            out += bytes([CODEC_STORE, filter_id])
            out += struct.pack("<II", len(blob), len(raw_bytes))
            out += blake3(blob, "comp") + blake3(raw_bytes, "raw")
        return out

    def file_records():
        out = b""
        for e, start, _ in placed:
            entry = varint(index[e.path]) + varint(index[e.component]) + varint(index[e.target])
            entry += struct.pack("<HB", 0, 0) + bytes([0]) + varint(1767225600)
            if e.pe_version:
                entry += bytes([1]) + b"".join(varint(part) for part in e.pe_version)
            else:
                entry += bytes([0])
            refs = []
            position = start
            remaining = len(e.data)
            while remaining:
                block_index = position // block_size
                offset = position % block_size
                take = min(remaining, block_size - offset)
                refs.append((block_index, offset, take))
                position += take
                remaining -= take
            entry += varint(len(e.data)) + varint(len(refs))
            for block_index, offset, take in refs:
                entry += varint(block_index) + varint(offset) + varint(take)
            out += entry
        return out

    asset_records = b""
    asset_offset = 0
    for kind, asset in assets:
        asset_records += bytes([kind]) + varint(0) + varint(0)
        asset_records += varint(asset_offset) + varint(len(asset)) + blake3(asset, "asset")
        asset_offset += len(asset)

    def manifest(config_offset, uninstaller_offset):
        out = varint(1) + pstring(app_id) + pstring(app_name) + pstring(app_version)
        out += bytes([0])
        out += varint(config_offset) + varint(len(config)) + blake3(config, "config")
        out += varint(uninstaller_offset) + varint(len(uninstaller)) + blake3(
            uninstaller, "uninstaller"
        )
        out += varint(len(assets)) + asset_records
        out += varint(len(blocks)) + block_records()
        out += varint(len(entries)) + file_records()
        return out

    blob = manifest(0, 0)
    for _ in range(8):
        config_offset = manifest_offset + len(blob)
        assets_offset = config_offset + len(config)
        uninstaller_offset = assets_offset + len(asset_blob)
        nxt = manifest(config_offset, uninstaller_offset)
        settled = len(nxt) == len(blob)
        blob = nxt
        if settled:
            break

    config_offset = manifest_offset + len(blob)
    assets_offset = config_offset + len(config)
    uninstaller_offset = assets_offset + len(asset_blob)
    blocks_offset = uninstaller_offset + len(uninstaller)

    payload_len = blocks_offset + len(stored_blob) + SIG_SIZE
    header = bytearray(HEADER_SIZE)
    header[0:8] = b"ORKIHDR\x00"
    struct.pack_into("<I", header, 8, 1)
    struct.pack_into("<I", header, 12, HEADER_SIZE)
    struct.pack_into("<Q", header, 16, payload_len)
    struct.pack_into("<I", header, 24, len(entries))
    struct.pack_into("<I", header, 28, len(blocks))
    struct.pack_into("<Q", header, 32, manifest_offset)
    struct.pack_into("<I", header, 40, len(blob))
    struct.pack_into("<I", header, 44, 1)
    struct.pack_into("<Q", header, 48, blocks_offset)
    struct.pack_into("<I", header, 56, crc32(bytes(header[0:56])))
    struct.pack_into("<I", header, 60, 0)

    payload = (
        bytes(header)
        + table
        + blob
        + config
        + asset_blob
        + uninstaller
        + stored_blob
        + bytes(SIG_SIZE)
    )
    covered = payload[56 : payload_len - SIG_SIZE]
    report = {
        "payload_len": payload_len,
        "manifest_offset": manifest_offset,
        "manifest_len": len(blob),
        "config_offset": config_offset,
        "config_len": len(config),
        "assets_offset": assets_offset,
        "assets_len": len(asset_blob),
        "uninstaller_offset": uninstaller_offset,
        "uninstaller_len": len(uninstaller),
        "blocks_offset": blocks_offset,
        "blocks_len": len(stored_blob),
        "signature_offset": payload_len - SIG_SIZE,
        "file_count": len(entries),
        "block_count": len(blocks),
        "string_count": len(strings),
        "string_blob": sum(len(s.encode("utf-8")) for s in strings),
        "header_crc32": struct.unpack_from("<I", header, 56)[0],
        "sig_digest": blake3(covered, "digest").hex(),
        "sha256": hashlib.sha256(payload).hexdigest(),
        "blocks": [
            {
                "index": i,
                "codec": CODEC_STORE,
                "filter": f,
                "raw_len": end - start,
                "comp_len": len(blob),
                "comp_blake3": blake3(blob, "comp").hex(),
                "raw_blake3": blake3(raw[start:end], "raw").hex(),
            }
            for i, ((start, end, f), blob) in enumerate(zip(blocks, stored))
        ],
        "files": [
            {
                "path": e.path,
                "size": len(e.data),
                "pe_version": e.pe_version,
                "raw_start": start,
                "filterable": e.filterable,
            }
            for e, start, _ in placed
        ],
        "signature_message": DOMAIN.hex() + "|" + bytes(header[0:56]).hex(),
    }
    return payload, report


def example_one():
    entries = [Entry("a.txt", b"hello"), Entry("b.bin", b"world!")]
    return build("com.example.demo", "demo", "1.0.0", entries, b"", [], b"", 8 * 1024 * 1024)


def pe_blob():
    head = bytearray(300)
    head[0:2] = b"MZ"
    struct.pack_into("<I", head, 0x3C, 0x80)
    head[0x80:0x84] = b"PE\x00\x00"
    struct.pack_into("<H", head, 0x84, 0x8664)
    body = bytes([0x90]) + bytes([0xE8]) + struct.pack("<i", 0x40) + bytes([0x90]) + bytes([0xE9])
    body += struct.pack("<i", -0x20) + bytes((i * 5 + 11) % 256 for i in range(0x90 + 40))
    head[0x90 : 0x90 + len(body)] = body
    return bytes(head)


def example_two():
    entries = [
        Entry(
            "app/data/app.exe",
            pe_blob(),
            component="core",
            target="app",
            pe_version=(1, 2, 3, 0),
            filterable=True,
        ),
        Entry("app/a.bin", bytes((i * 3 + 1) % 251 for i in range(172))),
    ]
    config = b'{"install":{"scope":"user","shortcuts":["demo2"]}}'
    assets = [(2, b"icon-bytes"), (3, b"license text")]
    uninstaller = b"MZ" + bytes(range(32))
    return build("com.example.demo2", "demo2", "2.0.0", entries, config, assets, uninstaller, 256)


def main():
    for name, (payload, report) in (("1", example_one()), ("2", example_two())):
        path = os.path.join(HERE, f"v1-example-{name}.bin")
        with open(path, "wb") as fh:
            fh.write(payload)
        print(f"v1-example-{name}.bin bytes={len(payload)} sha256={report['sha256']}")
        for key, value in report.items():
            if key == "blocks":
                for block in value:
                    print(f"  block {block['index']}: {block}")
            elif key == "files":
                for entry in value:
                    print(f"  file {entry}")
            else:
                print(f"  {key}={value}")


if __name__ == "__main__":
    main()
