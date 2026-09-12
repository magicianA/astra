#!/usr/bin/env python3
"""Build pinned lunar surface and HIP constellation assets for Astra."""

import json
from pathlib import Path

import numpy as np
from bootstrap import ROOT, digest, download
from PIL import Image

COMMIT = "014fbb5e59233d133c22f9811af96b67d05a95c9"
SVS = "https://svs.gsfc.nasa.gov/vis/a000000/a004700/a004720/"
NAIF = "https://naif.jpl.nasa.gov/pub/naif/generic_kernels/"


def main():
    moon = ROOT / "data/moon"
    moon.mkdir(parents=True, exist_ok=True)
    previous = moon / "manifest.json"
    expected = (
        json.loads(previous.read_text()).get("source_sha256", {}) if previous.exists() else {}
    )
    sources = {}
    for name in ("lroc_color_poles_4k.tif", "ldem_16_uint.tif"):
        path = download(f"artifacts/expansion/{name}", SVS + name, expected.get(name))
        sources[name] = {"url": SVS + name, "sha256": digest(path)}
    color = Image.open(ROOT / "artifacts/expansion/lroc_color_poles_4k.tif").convert("RGBA")
    color.save(moon / "albedo.png")
    height = Image.open(ROOT / "artifacts/expansion/ldem_16_uint.tif")
    # Heights are integer half-metres relative to 1727.4 km, north at top.
    # Preserve the two bytes separately; the shader decodes before interpolation.
    samples = np.asarray(height.resize((4096, 2048), Image.Resampling.BILINEAR), dtype=np.uint16)
    rgba = np.zeros((*samples.shape, 4), dtype=np.uint8)
    rgba[..., 0] = samples >> 8
    rgba[..., 1] = samples & 255
    rgba[..., 3] = 255
    Image.fromarray(rgba).save(moon / "height.png")
    for relative in (
        "pck/pck00011.tpc",
        "fk/satellites/moon_de440_250416.tf",
        "pck/moon_pa_de440_200625.bpc",
        "pck/moon_pa_de440_200625.cmt",
    ):
        name = Path(relative).name
        path = download(f"data/moon/{name}", NAIF + relative, expected.get(name))
        sources[name] = {"url": NAIF + relative, "sha256": digest(path)}
    manifest = {
        "id": "lro-lunar-v1",
        "source": "NASA SVS / LROC WAC / LOLA; NAIF DE440 PCK",
        "credits": "NASA's Scientific Visualization Studio; Ernie Wright; LRO LROC/LOLA teams; NASA/JPL/NAIF",
        "sources": sources,
        "source_sha256": {k: v["sha256"] for k, v in sources.items()},
        "orientation": "MOON_ME_DE440_ME421 within PCK coverage; IAU_MOON approximate elsewhere",
        "limits": "SVS colour map is adjusted for visualization, not absolute albedo; terrain shadows sampled at 4K; no historical terrain evolution",
        "albedo_normalization": "Divide linear RGB by 0.21012569, the near-side full-phase luminance-weighted map mean",
        "height_decode_km": "(256*R_byte + G_byte)*0.0005 - 10",
        "runtime_sha256": {
            "moon/" + p.name: digest(p) for p in sorted(moon.iterdir()) if p.name != "manifest.json"
        },
    }
    previous.write_text(json.dumps(manifest, indent=2) + "\n")
    out = ROOT / "data/skycultures"
    out.mkdir(parents=True, exist_ok=True)
    base = f"https://raw.githubusercontent.com/Stellarium/stellarium-skycultures/{COMMIT}/western/"
    culture_manifest = out / "manifest.json"
    prior_culture = json.loads(culture_manifest.read_text()) if culture_manifest.exists() else {}
    source = download(
        "artifacts/expansion/western-index.json",
        base + "index.json",
        prior_culture.get("source_sha256"),
    )
    description = download("artifacts/expansion/western-description.md", base + "description.md")
    data = json.loads(source.read_text())
    figures = [
        {
            "id": c["iau"],
            "name": c["common_name"]["native"],
            "lines": [[v for v in line if isinstance(v, int)] for line in c["lines"]],
        }
        for c in data["constellations"]
        if "iau" in c
    ]
    (out / "western.json").write_text(
        json.dumps(
            {
                "id": "stellarium-western-" + COMMIT,
                "source": base + "index.json",
                "license": "CC BY-SA",
                "figures": figures,
            },
            indent=2,
        )
        + "\n"
    )
    (out / "LICENSE.md").write_text(
        "# Attribution\n\nConstellation line data adapted from Stellarium's Western sky culture,\ncommit "
        + COMMIT
        + ". Data and text: CC BY-SA (as stated upstream).\nOnly HIP line figures and names are included; no illustrations.\n\n"
        + description.read_text()
    )
    (out / "manifest.json").write_text(
        json.dumps(
            {
                "id": "stellarium-western-" + COMMIT,
                "source_sha256": digest(source),
                "runtime_sha256": {
                    "skycultures/" + p.name: digest(p)
                    for p in sorted(out.iterdir())
                    if p.name != "manifest.json"
                },
            },
            indent=2,
        )
        + "\n"
    )
    print(f"Prepared lunar maps and {len(figures)} constellation figures", flush=True)


if __name__ == "__main__":
    main()
