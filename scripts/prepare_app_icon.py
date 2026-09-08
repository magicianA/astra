#!/usr/bin/env python3
"""Build the macOS icon from the checked-in artwork using Apple's icon tools."""

import platform
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main():
    if platform.system() != "Darwin":
        raise SystemExit("Icon generation requires macOS sips and iconutil")
    assets = ROOT / "assets/macos"
    with tempfile.TemporaryDirectory(prefix="astra-icon-") as temporary:
        iconset = Path(temporary) / "Astra.iconset"
        iconset.mkdir()
        for size in (16, 32, 128, 256, 512):
            for scale in (1, 2):
                suffix = "@2x" if scale == 2 else ""
                output = iconset / f"icon_{size}x{size}{suffix}.png"
                pixels = str(size * scale)
                subprocess.run(
                    [
                        "sips",
                        "--resampleHeightWidth",
                        pixels,
                        pixels,
                        str(assets / "Astra.png"),
                        "--out",
                        str(output),
                    ],
                    check=True,
                    stdout=subprocess.DEVNULL,
                )
        destination = assets / "Astra.icns"
        subprocess.run(
            ["iconutil", "--convert", "icns", "--output", str(destination), str(iconset)],
            check=True,
        )
    print(f"Created {destination}")


if __name__ == "__main__":
    main()
