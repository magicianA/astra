#!/usr/bin/env python3
"""Encode NASA's native 16K Gaia star-background data for the Vulkan renderer."""

import json
import os

import numpy as np
from bootstrap import ROOT, digest, download
from PIL import Image

SOURCE_PAGE = "https://svs.gsfc.nasa.gov/4851/"
SOURCE_URL = "https://svs.gsfc.nasa.gov/vis/a000000/a004800/a004851/milkyway_2020_16k_gal.exr"
SOURCE_SHA256 = "ee023f03e99466b3829117c15a001269ba32a3fecb6012d0b2d41f201c955351"
BACKGROUND_ID = "nasa-gaia-dr2-diffuse-16k-v1"
DISPLAY_GAIN = 16.0


def prepare():
    # Only offline asset preparation needs an EXR decoder, not the application.
    os.environ["OPENCV_IO_ENABLE_OPENEXR"] = "1"
    import cv2

    source = download("data/background/nasa-milkyway-2020-16k-gal.exr", SOURCE_URL, SOURCE_SHA256)
    image = cv2.imread(str(source), cv2.IMREAD_UNCHANGED)
    if image is None or image.shape != (8192, 16384, 3) or image.dtype != np.float32:
        raise RuntimeError("Expected NASA's 16384x8192 linear RGB EXR background")

    # NASA separates the bright Hipparcos/Tycho foreground. Keep native texels:
    # no median, blur, sharpening or upscaling. Strips bound temporary memory.
    pixels = np.empty(image.shape, dtype=np.uint8)
    for y in range(0, image.shape[0], 64):
        rgb = image[y : y + 64, :, ::-1].copy()  # OpenCV decodes BGR.
        if not np.isfinite(rgb).all() or np.any(rgb < 0):
            raise RuntimeError("Non-finite or negative intensity in the source map")
        rgb *= DISPLAY_GAIN
        rgb /= 1 + rgb  # Smooth highlight compression for the display texture.
        rgb = np.where(rgb <= 0.0031308, rgb * 12.92, 1.055 * rgb ** (1 / 2.4) - 0.055)
        pixels[y : y + 64] = np.rint(np.clip(rgb, 0, 1) * 255).astype(np.uint8)
    del image

    # All longitudes at each pole converge to one direction. Average in linear
    # light; no smoothing is applied to any other row or across the map.
    for row in (0, pixels.shape[0] - 1):
        color = pixels[row].astype(np.float32) / 255
        color = np.where(color <= 0.04045, color / 12.92, ((color + 0.055) / 1.055) ** 2.4)
        color = color.mean(axis=0)
        color = np.where(color <= 0.0031308, color * 12.92, 1.055 * color ** (1 / 2.4) - 0.055)
        pixels[row] = np.rint(color * 255).astype(np.uint8)

    target = ROOT / "data/background/milky-way.png"
    Image.fromarray(pixels).save(target, compress_level=6)
    manifest = {
        "background_id": BACKGROUND_ID,
        "source_page": SOURCE_PAGE,
        "source_url": SOURCE_URL,
        "source_sha256": SOURCE_SHA256,
        "credit": (
            "NASA/Goddard Space Flight Center Scientific Visualization Studio; Ernie Wright. "
            "Gaia DR2: ESA/Gaia/DPAC."
        ),
        "license": "NASA media usage guidelines; underlying Gaia data acknowledgements apply",
        "license_url": "https://www.nasa.gov/nasa-brand-center/images-and-media/",
        "projection": "Galactic equirectangular; u = 0.5 - l/(2*pi), v = 0.5 - b/pi",
        "dimensions": [pixels.shape[1], pixels.shape[0]],
        "processing": (
            "Native 16384x8192, no spatial filtering except linear-light polar-row averages; "
            "linear EXR decoded, per-channel gain 16 and x/(1+x), sRGB8 PNG encoding; "
            "runtime linear-light mipmaps and optional 8x anisotropic filtering"
        ),
        "limitations": (
            "Gaia DR2 faint-star visualization with Hipparcos/Tycho foreground omitted; "
            "fixed distant background, not an evolving star catalogue or calibrated radiance; "
            "Gaia survey artefacts and source pixel resolution remain"
        ),
        "runtime_sha256": {"background/milky-way.png": digest(target)},
    }
    (target.parent / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print(f"Prepared {target}: {pixels.shape[1]}x{pixels.shape[0]}, SHA-256 {digest(target)}")


if __name__ == "__main__":
    prepare()
