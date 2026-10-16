#!/usr/bin/env python3
"""Writes tests/fixtures/icon.exe: the smallest PE file with an application icon.

An empty 32-bit program whose only content is a resource section with one 8x8 red PNG icon,
so the icon extraction can be tested without a real game."""
import struct, zlib, pathlib

def png(w, h, rgb):
    def chunk(tag, data):
        body = tag + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))
    raw = b"".join(b"\x00" + bytes(rgb + (255,)) * w for _ in range(h))
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw)) + chunk(b"IEND", b""))

SECTION_RVA = 0x1000
icon = png(8, 8, (200, 30, 30))  # RGBA, as the ICO decoder requires
icon_at = 0xA0
group_at = (icon_at + len(icon) + 3) & ~3
group = struct.pack("<HHH", 0, 1, 1) + struct.pack("<BBBBHHIH", 8, 8, 0, 0, 1, 32, len(icon), 1)

def directory(entries):
    out = struct.pack("<IIHHHH", 0, 0, 0, 0, 0, len(entries))
    for ident, target in entries:
        out += struct.pack("<II", ident, target)
    return out

SUB = 0x80000000
rsrc = bytearray(group_at + len(group))
rsrc[0x00:0x20] = directory([(3, SUB | 0x20), (14, SUB | 0x60)])
rsrc[0x20:0x38] = directory([(1, SUB | 0x38)])
rsrc[0x38:0x50] = directory([(0, 0x50)])
rsrc[0x50:0x60] = struct.pack("<IIII", SECTION_RVA + icon_at, len(icon), 0, 0)
rsrc[0x60:0x78] = directory([(1, SUB | 0x78)])
rsrc[0x78:0x90] = directory([(0, 0x90)])
rsrc[0x90:0xA0] = struct.pack("<IIII", SECTION_RVA + group_at, len(group), 0, 0)
rsrc[icon_at:icon_at + len(icon)] = icon
rsrc[group_at:group_at + len(group)] = group

raw_size = (len(rsrc) + 0x1FF) & ~0x1FF
image_size = SECTION_RVA + ((len(rsrc) + 0xFFF) & ~0xFFF)

dos = bytearray(0x40)
dos[0:2] = b"MZ"
dos[0x3C:0x40] = struct.pack("<I", 0x40)
coff = struct.pack("<HHIIIHH", 0x14C, 1, 0, 0, 0, 0xE0, 0x0102)
optional = struct.pack(
    "<HBB" "IIIIII" "III" "HHHHHH" "IIII" "HH" "IIII" "II",
    0x10B, 2, 0,
    0, len(rsrc), 0, 0, SECTION_RVA, SECTION_RVA,
    0x400000, 0x1000, 0x200,
    4, 0, 0, 0, 4, 0,
    0, image_size, 0x200, 0,
    2, 0,
    0x100000, 0x1000, 0x100000, 0x1000,
    0, 16,
)
dirs = b"".join(
    struct.pack("<II", SECTION_RVA, len(rsrc)) if i == 2 else struct.pack("<II", 0, 0)
    for i in range(16)
)
section = struct.pack("<8sIIIIIIHHI", b".rsrc", len(rsrc), SECTION_RVA, raw_size, 0x200, 0, 0, 0, 0, 0x40000040)
headers = bytes(dos) + b"PE\0\0" + coff + optional + dirs + section
exe = headers.ljust(0x200, b"\0") + bytes(rsrc).ljust(raw_size, b"\0")
out = pathlib.Path(__file__).resolve().parent.parent / "tests/fixtures/icon.exe"
out.write_bytes(exe)
print(f"wrote {out} ({len(exe)} bytes)")
