"""Render the app icon at every size the installers need.

    python3 packaging/icons/generate.py

Needs `resvg` (cargo install resvg). Writes PNGs for macOS and Linux and an
.ico for Windows next to this file. The outputs are committed, so building
an installer does not need resvg.
"""

import pathlib
import struct
import subprocess

HERE = pathlib.Path(__file__).parent
# macOS draws app icons on a grid with a margin; app.svg has it. Windows icons
# fill their square, so the .ico uses the plain icon.
SVG = HERE / "app.svg"
FULL = HERE.parent.parent / "assets" / "icon.svg"
# Each macOS icon slot at 1x and 2x, named the way the .icns builder reads
# them: 64 px is "32x32@2x". Linux uses the plain sizes.
PNG = {f"{s}x{s}.png": s for s in (32, 128, 256, 512)} | {f"{s}x{s}@2x.png": 2 * s for s in (32, 128, 256, 512)}
ICO = [16, 24, 32, 48, 64, 128, 256]


def render(size: int, out: pathlib.Path, svg: pathlib.Path = SVG) -> bytes:
    subprocess.run(["resvg", "-w", str(size), "-h", str(size), str(svg), str(out)], check=True)
    return out.read_bytes()


for old in HERE.glob("*.png"):
    old.unlink()
for name, size in PNG.items():
    render(size, HERE / name)

# An .ico may hold PNG images directly (Windows Vista and later).
tmp = HERE / ".ico-part.png"
images = [render(size, tmp, FULL) for size in ICO]
tmp.unlink()
header = struct.pack("<HHH", 0, 1, len(images))
offset = 6 + 16 * len(images)
entries = b""
for size, data in zip(ICO, images):
    dim = 0 if size >= 256 else size  # 0 means 256
    entries += struct.pack("<BBBBHHII", dim, dim, 0, 0, 1, 32, len(data), offset)
    offset += len(data)
(HERE / "icon.ico").write_bytes(header + entries + b"".join(images))
print("wrote", ", ".join(PNG), "and icon.ico")
