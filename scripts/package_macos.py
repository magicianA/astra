#!/usr/bin/env python3
"""Create a local, ad-hoc-signed macOS app containing runtime libraries and data."""

import argparse
import json
import platform
import plistlib
import shutil
import subprocess
from pathlib import Path

from bootstrap import digest

ROOT = Path(__file__).resolve().parents[1]
XCODE_TOOLS = Path(
    "/Applications/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/bin"
)


def tool(name):
    bundled = XCODE_TOOLS / name
    return str(bundled) if bundled.exists() else name


def run(*args):
    return subprocess.check_output([str(arg) for arg in args], text=True)


def copy_file(source, destination):
    destination.parent.mkdir(parents=True, exist_ok=True)
    # APFS clone avoids duplicating multi-gigabyte, immutable DE441 kernels.
    clone = subprocess.run(["cp", "-c", str(source), str(destination)], capture_output=True)
    if clone.returncode:
        shutil.copy2(source, destination)


def dependencies(binary):
    return [
        line.strip().split(" (compatibility")[0]
        for line in run(tool("otool"), "-L", binary).splitlines()[1:]
    ]


def package(build, destination):
    if platform.system() != "Darwin":
        raise RuntimeError("This packaging script targets macOS only")
    source = build / "astra.app"
    if not source.exists():
        raise RuntimeError(f"Build the application first: {source}")
    manifest = json.loads((ROOT / "data/catalog/manifest.json").read_text())
    for name, expected in manifest["runtime_sha256"].items():
        if digest(ROOT / "data" / name) != expected:
            raise RuntimeError(f"Runtime data checksum differs from the manifest: {name}")
    background = json.loads((ROOT / "data/background/manifest.json").read_text())
    for name, expected in background["runtime_sha256"].items():
        if digest(ROOT / "data" / name) != expected:
            raise RuntimeError(f"Background checksum differs from the manifest: {name}")
    if destination.exists():
        if destination.name != "Astra.app" or destination.parent != ROOT / "dist":
            raise RuntimeError(
                "Automatic replacement is restricted to this project's dist/Astra.app"
            )
        shutil.rmtree(destination)
    shutil.copytree(source, destination)
    contents = destination / "Contents"
    resources = contents / "Resources"
    frameworks = contents / "Frameworks"
    frameworks.mkdir(parents=True, exist_ok=True)
    executable = contents / "MacOS/astra"
    brew = Path(run("brew", "--prefix").strip())
    bundled = {}

    def bundle_library(source, name):
        if name in bundled:
            if bundled[name] != source.resolve():
                raise RuntimeError(f"Conflicting runtime libraries named {name}")
            return
        bundled[name] = source.resolve()
        target = frameworks / name
        copy_file(source, target)
        target.chmod(target.stat().st_mode | 0o200)
        rewrite(target, source)
        subprocess.run(
            [tool("install_name_tool"), "-id", f"@rpath/{name}", str(target)], check=True
        )

    def rewrite(target, original):
        for dependency in dependencies(original):
            if dependency.startswith(("/usr/lib/", "/System/Library/")):
                continue
            if dependency.startswith("@loader_path/"):
                resolved = original.parent / dependency.removeprefix("@loader_path/")
            elif dependency.startswith("@rpath/"):
                name = Path(dependency).name
                candidates = [original.parent / name, brew / "lib" / name]
                resolved = next((path for path in candidates if path.exists()), None)
                if resolved is None:
                    raise RuntimeError(f"Unresolved runtime dependency: {dependency}")
            else:
                resolved = Path(dependency)
            if resolved.resolve() == original.resolve():
                continue  # dylib's own LC_ID_DYLIB entry
            name = Path(dependency).name
            bundle_library(resolved, name)
            new = (
                f"@executable_path/../Frameworks/{name}"
                if target == executable
                else f"@loader_path/{name}"
            )
            subprocess.run(
                [tool("install_name_tool"), "-change", dependency, new, str(target)], check=True
            )

    rewrite(executable, source / "Contents/MacOS/astra")
    molten = brew / "opt/molten-vk/lib/libMoltenVK.dylib"
    bundle_library(molten, "libMoltenVK.dylib")
    icd_source = brew / "opt/molten-vk/etc/vulkan/icd.d/MoltenVK_icd.json"
    icd = json.loads(icd_source.read_text())
    icd["ICD"]["library_path"] = "../../../Frameworks/libMoltenVK.dylib"
    icd_target = resources / "vulkan/icd.d/MoltenVK_icd.json"
    icd_target.parent.mkdir(parents=True, exist_ok=True)
    icd_target.write_text(json.dumps(icd, indent=2) + "\n")

    for relative in [
        "catalog/stars.bin",
        "background/milky-way.png",
        "background/manifest.json",
        "catalog/manifest.json",
        "catalog/names.json",
        "catalog/positional-matches.json",
        "time/cio.bin",
        "time/finals2000A.all",
        "time/naif0012.tls",
        "time/bulletin-c72.txt",
        "fonts/NotoSansCJKsc-Regular.otf",
        "fonts/LICENSE.txt",
        "fonts/ui-glyphs.txt",
        "kernels/de441_part-1.bsp",
        "kernels/de441_part-2.bsp",
    ]:
        copy_file(ROOT / "data" / relative, resources / "data" / relative)
    shutil.copytree(build / "shaders", resources / "shaders", dirs_exist_ok=True)
    copy_file(ROOT / "THIRD_PARTY_NOTICES.md", resources / "THIRD_PARTY_NOTICES.md")
    shutil.copytree(ROOT / "data/licenses", resources / "licenses", dirs_exist_ok=True)
    plist_path = contents / "Info.plist"
    with plist_path.open("rb") as stream:
        plist = plistlib.load(stream)
    plist.update(
        CFBundleDisplayName="Astra · 万年星空",
        CFBundleName="Astra",
        CFBundleShortVersionString="0.1.0",
        CFBundleVersion="1",
        NSHighResolutionCapable=True,
        LSMinimumSystemVersion="26.0",
    )
    with plist_path.open("wb") as stream:
        plistlib.dump(plist, stream)
    for library in frameworks.iterdir():
        subprocess.run(["codesign", "--force", "--sign", "-", str(library)], check=True)
    subprocess.run(["codesign", "--force", "--sign", "-", str(destination)], check=True)
    subprocess.run(["codesign", "--verify", "--deep", "--strict", str(destination)], check=True)
    for binary in [executable, *frameworks.iterdir()]:
        if any(dep.startswith(("/opt/homebrew/", "/usr/local/")) for dep in dependencies(binary)):
            raise RuntimeError(f"Unbundled package-manager dependency: {binary}")
    print(f"Packaged: {destination}")
    print("Local ad-hoc signature; this is not a notarized public distribution.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--build", type=Path, default=ROOT / "build")
    args = parser.parse_args()
    package(args.build.resolve(), ROOT / "dist/Astra.app")


if __name__ == "__main__":
    main()
