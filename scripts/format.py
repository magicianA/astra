#!/usr/bin/env python3
"""Format owned source files. Use --check in CI; vendor files are excluded."""

import argparse
import shutil
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def executable(name, fallbacks=()):
    candidate = shutil.which(name)
    if candidate:
        return candidate
    for candidate in fallbacks:
        if Path(candidate).is_file():
            return str(candidate)
    raise SystemExit(f"Missing {name}; see README.md for formatting dependencies")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    clang_format = executable(
        "clang-format",
        [
            "/Applications/Xcode.app/Contents/Developer/Toolchains/"
            "XcodeDefault.xctoolchain/usr/bin/clang-format"
        ],
    )
    sources = sorted(
        path
        for folder in ("src", "include", "tests", "shaders")
        for path in (ROOT / folder).rglob("*")
        if path.suffix in (".cpp", ".hpp", ".vert", ".frag", ".glsl", ".comp")
    )
    subprocess.run(
        [clang_format, *(["--dry-run", "--Werror"] if args.check else ["-i"]), *map(str, sources)],
        check=True,
        cwd=ROOT,
    )
    subprocess.run(
        [executable("ruff"), "check", *([] if args.check else ["--fix"]), "scripts"],
        check=True,
        cwd=ROOT,
    )
    subprocess.run(
        [executable("ruff"), "format", *(["--check"] if args.check else []), "scripts"],
        check=True,
        cwd=ROOT,
    )
    subprocess.run(
        [executable("cmake-format"), "--check" if args.check else "-i", "CMakeLists.txt"],
        check=True,
        cwd=ROOT,
    )


if __name__ == "__main__":
    main()
