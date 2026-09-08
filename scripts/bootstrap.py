#!/usr/bin/env python3
"""Fetch the project's public dependencies and data; no application code generation."""

import argparse
import hashlib
import json
import platform
import shutil
import subprocess
import tarfile
import urllib.request
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
NAIF = "https://naif.jpl.nasa.gov/pub/naif"


def digest(path):
    sha = hashlib.sha256()
    with path.open("rb") as source:
        for block in iter(lambda: source.read(4 * 1024 * 1024), b""):
            sha.update(block)
    return sha.hexdigest()


def download(relative, url, sha256=None):
    target = ROOT / relative
    target.parent.mkdir(parents=True, exist_ok=True)
    if target.exists() and (not sha256 or digest(target) == sha256):
        print(f"Verified/present: {relative}", flush=True)
        return target
    partial = target.with_name(target.name + ".partial")
    offset = partial.stat().st_size if partial.exists() else 0
    headers = {"Range": f"bytes={offset}-"} if offset else {}
    print(f"Downloading: {url}", flush=True)
    with urllib.request.urlopen(
        urllib.request.Request(url, headers=headers), timeout=120
    ) as response:
        resume = offset and response.status == 206
        if resume and not response.headers.get("Content-Range", "").startswith(f"bytes {offset}-"):
            raise RuntimeError("Server returned an incorrect resume offset")
        with partial.open("ab" if resume else "wb") as output:
            shutil.copyfileobj(response, output, 1024 * 1024)
    if sha256 and digest(partial) != sha256:
        raise RuntimeError(f"SHA-256 mismatch for {relative}; retained .partial for inspection")
    partial.replace(target)
    return target


def safe_member(name, destination):
    path = (destination / name).resolve()
    if path != destination.resolve() and destination.resolve() not in path.parents:
        raise RuntimeError(f"Archive member escapes destination: {name}")


def extract(archive, destination):
    destination.mkdir(parents=True, exist_ok=True)
    if archive.suffix == ".zip":
        with zipfile.ZipFile(archive) as package:
            for member in package.infolist():
                safe_member(member.filename, destination)
                if (member.external_attr >> 16) & 0o170000 == 0o120000:
                    raise RuntimeError("Symbolic links are not expected in the dependency archive")
            package.extractall(destination)
        return
    unpack = archive
    if archive.suffix == ".Z":
        unpack = archive.with_suffix("")
        with unpack.open("wb") as output:
            subprocess.run(["gzip", "-dc", str(archive)], stdout=output, check=True)
    try:
        with tarfile.open(unpack) as package:
            for member in package.getmembers():
                safe_member(member.name, destination)
                if member.issym() or member.islnk() or not (member.isfile() or member.isdir()):
                    raise RuntimeError(f"Unexpected archive entry: {member.name}")
            package.extractall(destination)
    finally:
        if unpack != archive:
            unpack.unlink(missing_ok=True)


def dependencies():
    archive = download(
        "third_party/imgui.tar.gz",
        "https://codeload.github.com/ocornut/imgui/tar.gz/refs/tags/v1.91.9b",
        "8e1bbc76c71d74fef2fb85db7e7ca8eba13d6a86623c54992b60162db554ffdb",
    )
    if not (ROOT / "third_party/imgui-1.91.9b/imgui.cpp").exists():
        extract(archive, ROOT / "third_party")
    download(
        "third_party/json.hpp",
        "https://raw.githubusercontent.com/nlohmann/json/v3.12.0/single_include/nlohmann/json.hpp",
        "aaf127c04cb31c406e5b04a63f1ae89369fccde6d8fa7cdda1ed4f32dfc5de63",
    )
    platforms = {
        ("Darwin", "arm64"): "MacM1_OSX_clang_64bit",
        ("Darwin", "x86_64"): "MacIntel_OSX_AppleC_64bit",
        ("Linux", "x86_64"): "PC_Linux_GCC_64bit",
        ("Windows", "AMD64"): "PC_Windows_VisualC_64bit",
    }
    key = (platform.system(), platform.machine())
    if key not in platforms:
        raise RuntimeError("Install CSPICE manually for this architecture and set CSPICE_ROOT")
    extension = "zip" if key[0] == "Windows" else "tar.Z"
    expected = (
        "0deae048443e11ca4d093cac651d9785d4f2594631a183d85a3d58949f4d0aa9"
        if key == ("Darwin", "arm64")
        else None
    )
    archive = download(
        f"third_party/cspice.{extension}",
        f"{NAIF}/toolkit/C/{platforms[key]}/packages/cspice.{extension}",
        expected,
    )
    marker = ROOT / "third_party/cspice/.astra-platform"
    if marker.exists() and marker.read_text().strip() != platforms[key]:
        raise RuntimeError(
            "CSPICE was installed for another platform; use a separate build checkout"
        )
    if not (ROOT / "third_party/cspice/include/SpiceUsr.h").exists():
        extract(archive, ROOT / "third_party")
    marker.write_text(platforms[key] + "\n")
    (ROOT / "third_party/dependencies-lock.json").write_text(
        json.dumps(
            {
                "imgui": "1.91.9b",
                "json": "3.12.0",
                "cspice": "N0067",
                "cspice_platform": platforms[key],
                "cspice_sha256": digest(archive),
            },
            indent=2,
        )
        + "\n"
    )


def kernels():
    for part, sha in [
        (1, "13757827f5db41b835a24bbd637488636ce79a8ca754062fed17844f7d5b618e"),
        (2, "3abb17dae2d78dd34880377544aacb54892104a0d4462b322cb9f4454d4887f6"),
    ]:
        name = f"de441_part-{part}.bsp"
        download(f"data/kernels/{name}", f"{NAIF}/generic_kernels/spk/planets/{name}", sha)


def assets():
    glyphs = set()
    for folder in ("src", "include"):
        for source in (ROOT / folder).rglob("*"):
            if source.suffix in (".cpp", ".hpp"):
                glyphs.update(c for c in source.read_text() if 127 < ord(c) < 0x10000)
    (ROOT / "data/fonts").mkdir(parents=True, exist_ok=True)
    (ROOT / "data/fonts/ui-glyphs.txt").write_text("".join(sorted(glyphs)) + "\n")
    files = [
        (
            "docs/research/hip2.dat.gz",
            "https://cdsarc.cds.unistra.fr/ftp/I/311/hip2.dat.gz",
            "8e624f843d4254a9b7c2e8dda8e3158dbe825bf98f6bf3dbbae7f0d0b73d6858",
        ),
        (
            "docs/research/bsc5-catalog.gz",
            "https://cdsarc.cds.unistra.fr/ftp/V/50/catalog.gz",
            "3dc44b1e90be8fbe5bcc7656032560f51275f985c7e3f783c9028e1838ec7bed",
        ),
        (
            "data/time/finals2000A.all",
            "https://maia.usno.navy.mil/ser7/finals2000A.all",
            "80119694522717471744a78b28bc872e998523f8b1e2007424f9cd08af7ac4c6",
        ),
        (
            "data/time/naif0012.tls",
            f"{NAIF}/generic_kernels/lsk/naif0012.tls",
            "678e32bdb5a744117a467cd9601cd6b373f0e9bc9bbde1371d5eee39600a039b",
        ),
        (
            "data/time/bulletin-c72.txt",
            "https://datacenter.iers.org/data/latestVersion/bulletinC.txt",
            "310e172eadacacca3adf92cb9bd646fcc0d6320e35e996a106d2644ede3b52f0",
        ),
        (
            "data/fonts/NotoSansCJKsc-Regular.otf",
            "https://raw.githubusercontent.com/notofonts/noto-cjk/Sans2.004/Sans/OTF/SimplifiedChinese/NotoSansCJKsc-Regular.otf",
            "2c76254f6fc379fddfce0a7e84fb5385bb135d3e399294f6eeb6680d0365b74b",
        ),
    ]
    for relative, url, sha in files:
        download(relative, url, sha)
    download(
        "data/fonts/LICENSE.txt",
        "https://raw.githubusercontent.com/notofonts/noto-cjk/main/Sans/LICENSE",
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dependencies", action="store_true")
    parser.add_argument("--kernels", action="store_true")
    parser.add_argument("--assets", action="store_true")
    parser.add_argument("--all", action="store_true")
    args = parser.parse_args()
    if not any(vars(args).values()):
        parser.error("Select --dependencies, --kernels, --assets or --all")
    for flag, action in [
        (args.dependencies, dependencies),
        (args.kernels, kernels),
        (args.assets, assets),
    ]:
        if args.all or flag:
            action()


if __name__ == "__main__":
    main()
