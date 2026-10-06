import os
import struct
import sys
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)

from importlib import import_module

checker = import_module("v1-example-check")
Fault = checker.Fault


def crc_fix(buf):
    struct.pack_into("<I", buf, 56, zlib.crc32(bytes(buf[0:56])) & 0xFFFFFFFF)


def u32_at(offset, value=None, delta=0, fix=True):
    def mutate(buf, report):
        current = struct.unpack_from("<I", buf, offset)[0]
        struct.pack_into("<I", buf, offset, value if value is not None else current + delta)
        if fix:
            crc_fix(buf)

    return mutate


def u64_at(offset, delta):
    def mutate(buf, report):
        current = struct.unpack_from("<Q", buf, offset)[0]
        struct.pack_into("<Q", buf, offset, current + delta)

    return mutate


def header_xor(offset, mask):
    def mutate(buf, report):
        buf[offset] ^= mask

    return mutate


def pos_xor(key, mask):
    def mutate(buf, report):
        buf[report["positions"][key]] ^= mask

    return mutate


def section_xor(name, offset, mask):
    def mutate(buf, report):
        start = next(start for section, start, _ in report["sections"] if section == name)
        buf[start + offset] ^= mask

    return mutate


MUTATIONS = [
    ("format_version set to 2", "ORKI-1003", u32_at(8, 2), "header"),
    ("header CRC32 field changed", "ORKI-1001", header_xor(56, 0x01), "header"),
    ("reserved header word set to 1", "ORKI-1001", u32_at(60, 1), "header"),
    ("flags bit 1 (encryption) set", "ORKI-1001", u32_at(44, 3), "header"),
    ("payload_len raised by one", "ORKI-1001", u64_at(16, 1), "header"),
    ("file_count raised to 100001", "ORKI-1001", u32_at(24, 100001), "header"),
    ("block_count raised to 200001", "ORKI-1001", u32_at(28, 200001), "header"),
    ("manifest_len shortened (gap)", "ORKI-1001", u32_at(40, delta=-1), "header"),
    ("manifest_len extended (overlap)", "ORKI-1001", u32_at(40, delta=1), "header"),
    ("blocks_offset moved by one (gap)", "ORKI-1001", u64_at(48, 1), "header"),
    ("string table magic broken", "ORKI-1001", section_xor("string table", 0, 0x20), "string table"),
    ("manifest schema set to 2", "ORKI-1001", pos_xor("schema", 0x03), "manifest"),
    ("config_offset shifted by one (section gap)", "ORKI-1001", pos_xor("config_offset", 0x01), "manifest"),
    ("config hash byte flipped", "ORKI-1001", pos_xor("config_hash", 0x01), "manifest"),
    ("uninstaller hash byte flipped", "ORKI-1001", pos_xor("uninstaller_hash", 0x01), "manifest"),
    ("asset kind set to 11", "ORKI-1001", pos_xor("first_asset_kind", 0x09), "manifest"),
    ("asset hash byte flipped", "ORKI-1001", pos_xor("first_asset_hash", 0x01), "manifest"),
    ("file size raised by one", "ORKI-1001", pos_xor("first_file_size", 0x01), "manifest"),
    ("first block reference length raised", "ORKI-1001", pos_xor("first_ref_len", 0x01), "manifest"),
    ("first block codec set to 3 (zstd)", "ORKI-1001", pos_xor("first_block_codec", 0x03), "manifest"),
    ("first block filter set to 3 (outside the allowlist)", "ORKI-1001", pos_xor("first_block_filter", 0x02), "manifest"),
    ("first block comp_len beyond slack", "ORKI-1001", pos_xor("first_block_comp_len", 0x08), "manifest"),
    ("config byte flipped", "ORKI-1001", section_xor("config", 0, 0x01), "section"),
    ("assets byte flipped", "ORKI-1001", section_xor("assets", 0, 0x01), "section"),
    ("uninstaller byte flipped", "ORKI-1001", section_xor("uninstaller", 0, 0x01), "section"),
    ("stored block byte flipped", "ORKI-1001", section_xor("blocks", 0, 0x01), "section"),
    ("signature block zeroed on a keyed stub", "ORKI-1002", None, "stub"),
    ("signature domain tag replaced", "ORKI-1002", None, "stub"),
    ("signed flag cleared", "ORKI-1002", None, "stub"),
]


def main():
    with open(os.path.join(HERE, "v1-example-2.bin"), "rb") as fh:
        payload = fh.read()
    base = checker.validate(payload)
    rows = []
    failures = []
    for name, expected, mutate, scope in MUTATIONS:
        if mutate is None:
            rows.append((name, expected, "stub-level", scope))
            continue
        buf = bytearray(payload)
        mutate(buf, base)
        try:
            checker.validate(bytes(buf))
            observed = "accepted"
        except Fault as fault:
            observed = fault.code
        rows.append((name, expected, observed, scope))
        if observed != expected:
            failures.append((name, expected, observed))
    print(f"{'mutation':52} {'expected':10} {'observed':10} scope")
    for name, expected, observed, scope in rows:
        marker = "ok" if observed in (expected, "stub-level") else "FAIL"
        print(f"{name:52} {expected:10} {observed:10} {scope:6} {marker}")
    print(f"mutations={len(rows)} failures={len(failures)}")
    for name, expected, observed in failures:
        print(f"  FAIL {name}: expected {expected}, observed {observed}")
    if failures:
        sys.exit(1)
    print("all mutations rejected as specified")


if __name__ == "__main__":
    main()
