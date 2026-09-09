#!/usr/bin/env python3
"""Measure shipped photometric calibration; NumPy and Pillow are required."""

import argparse
import json
import math
import re

import numpy as np
from PIL import Image
from verify_atmosphere import ROOT, Y, read, sky


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=str, default="artifacts/photometry-validation.json")
    args = parser.parse_args()
    tables, _ = read(ROOT / "data/atmosphere/clear.bin")
    text = (ROOT / "include/astro/twilight_calibration.hpp").read_text()
    gains = np.array([float(n) for n in re.findall(r"-?\d+\.\d+", text.split("= {")[1])])
    assert len(gains) == 401
    zero = 2.54e-6 / (math.pi / 648000) ** 2
    residuals = []
    for depression in np.linspace(5.0125, 14.9875, 400):
        x = depression - 5
        target = zero * 10 ** (-0.4 * (11.84 + 1.518 * x - 0.057 * x * x))
        gain = math.exp(np.interp(depression, np.linspace(5, 15, 401), gains))
        actual = float(sky(tables, 2.635, -depression, 90, 0) @ Y * gain + 0.0002475)
        residuals.append(-2.5 * math.log10(actual / target))
    max_error = float(np.max(np.abs(residuals)))
    assert max_error < 0.02, f"Twilight interpolation error: {max_error} mag"
    manifest = json.loads((ROOT / "data/background/manifest.json").read_text())
    calibration = manifest["calibration"]
    ids = calibration["validation_pixels"]
    validation = np.array(calibration["relative_residual"])[ids]
    assert np.max(np.abs(validation)) < 0.3, "Regional Galactic normalization regressed"
    scale = calibration["radiance_scale_cd_m2"]
    # Keep analysis on native texels; stride bounds memory without filtering.
    Image.MAX_IMAGE_PIXELS = 16384 * 8192
    image = (
        np.asarray(Image.open(ROOT / "data/background/milky-way.png"))[::8, ::8].astype(float) / 255
    )
    image = np.where(image <= 0.04045, image / 12.92, ((image + 0.055) / 1.055) ** 2.4)
    radiance = image @ Y * scale
    latitude = (0.5 - (np.arange(radiance.shape[0]) * 8 + 0.5) / 8192) * 180
    plane = radiance[np.abs(latitude) < 5]
    # Independent photometric unit references: NPS natural sky and lunar flux.
    moon_lux = 2.54e-6 * 10 ** (-0.4 * -12.73)
    report = {
        "model": "photometric-v1",
        "twilight": {
            "reference": "https://arxiv.org/abs/astro-ph/0604128",
            "test": "400 midpoints between calibration knots, Paranal zenith",
            "maximum_interpolation_error_mag": max_error,
            "caution": "Agreement with the fitted curve, not independent atmospheric accuracy",
        },
        "milky_way": {
            "reference": "https://arxiv.org/abs/2101.01500",
            "training_pixels": calibration["training_pixels"],
            "held_out_pixels": ids,
            "held_out_relative_residual": validation.tolist(),
            "maximum_held_out_error_percent": float(np.max(np.abs(validation)) * 100),
            "radiance_scale_cd_m2": scale,
            "galactic_plane_luminance_p50_p90_p99": np.percentile(plane, [50, 90, 99]).tolist(),
            "caution": "Nine nearby HEALPix cells; this does not measure full-sky accuracy",
        },
        "unit_references": {
            "zero_magnitude_illuminance_lux": 2.54e-6,
            "full_moon_above_atmosphere_lux": moon_lux,
            "dark_sky_22mag_cd_m2": zero * 10 ** (-0.4 * 22),
            "natural_floor_cd_m2": 0.00015,
        },
    }
    path = ROOT / args.output
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
