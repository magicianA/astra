#!/usr/bin/env python3
"""Check atmosphere caches, double-precision transmission, and optional convergence.

Requires NumPy. Lookup mappings follow the BSD-licensed Bruneton reference in
vendor/bruneton; the optical-depth check independently integrates density in FP64.
"""

import argparse
import itertools
import json
import math
import struct
import zlib
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[1]
BOTTOM, TOP = 6360.0, 6420.0
H = math.sqrt(TOP**2 - BOTTOM**2)
SHAPES = [(64, 256, 4), (32, 128, 256, 4), (32, 128, 256, 4), (16, 64, 4)]
Y = np.array([0.2126, 0.7152, 0.0722])


def read(path):
    raw = path.read_bytes()
    assert raw[:8] == b"ASTRAAT2", f"Unsupported cache: {path}"
    preset, orders, groups, crc = struct.unpack_from("<4I", raw, 76)
    assert zlib.crc32(raw[92:]) == crc, f"Checksum: {path}"
    data = np.frombuffer(raw, dtype="<f4", offset=92)
    assert np.isfinite(data).all(), f"Non-finite cache: {path}"
    result, offset = [], 0
    for shape in SHAPES:
        size = math.prod(shape)
        result.append(data[offset : offset + size].reshape(shape))
        offset += size
    assert offset == len(data), f"Cache length: {path}"
    assert ((result[0] >= 0) & (result[0] <= 1)).all(), "Transmission outside [0, 1]"
    return result, dict(preset=preset, orders=orders, wavelengths=groups * 3)


def sample(table, coordinates):
    # Normalized sampler coordinates, with clamp-to-edge and no mipmaps.
    shape = np.array(table.shape[:-1])
    position = np.clip(np.array(coordinates)[::-1] * shape - 0.5, 0, shape - 1)
    low = np.floor(position).astype(int)
    fraction = position - low
    result = np.zeros(3)
    for offset in itertools.product([0, 1], repeat=len(shape)):
        index = tuple(np.minimum(low + offset, shape - 1))
        weight = np.prod(np.where(offset, fraction, 1 - fraction))
        result += table[index][:3] * weight
    return result


def texcoord(value, size):
    return 0.5 / size + value * (1 - 1 / size)


def distance(r, mu):
    return -r * mu + math.sqrt(max(0, r * r * (mu * mu - 1) + TOP * TOP))


def sky(tables, height, sun_alt, view_alt, azimuth):
    r = BOTTOM + max(0.001, height)
    sun, view, azimuth = np.radians([sun_alt, view_alt, azimuth])
    mu, mu_s = math.sin(view), math.sin(sun)
    nu = mu * mu_s + math.cos(view) * math.cos(sun) * math.cos(azimuth)
    rho = math.sqrt(r * r - BOTTOM * BOTTOM)
    u_r = texcoord(rho / H, 32)
    u_mu = 0.5 + 0.5 * texcoord((distance(r, mu) - (TOP - r)) / (rho + H - (TOP - r)), 64)
    a = (distance(BOTTOM, mu_s) - (TOP - BOTTOM)) / (H - (TOP - BOTTOM))
    max_a = (distance(BOTTOM, -0.5) - (TOP - BOTTOM)) / (H - (TOP - BOTTOM))
    u_s = texcoord(max(1 - a / max_a, 0) / (1 + a), 32)
    x = (nu + 1) * 3.5
    ix, fraction = math.floor(x), x % 1

    def scattering(table):
        return (
            sample(table, [(ix + u_s) / 8, u_mu, u_r]) * (1 - fraction)
            + sample(table, [(ix + 1 + u_s) / 8, u_mu, u_r]) * fraction
        )

    rayleigh = 3 / (16 * math.pi) * (1 + nu * nu)
    g = 0.8
    mie = 3 / (8 * math.pi) * (1 - g * g) / (2 + g * g) * (1 + nu * nu)
    mie /= (1 + g * g - 2 * g * nu) ** 1.5
    return np.maximum(0, scattering(tables[1]) * rayleigh + scattering(tables[2]) * mie)


def transmission_error(table, preset):
    earth = json.loads((ROOT / "vendor/bruneton/earth.json").read_text())
    wavelengths = np.array([680, 550, 440])
    rayleigh = 1.24062e-3 * (wavelengths / 1000) ** -4
    ozone = (
        300
        * 2.687e20
        / 15
        * np.interp(wavelengths, np.arange(360, 831, 10), earth["kOzoneCrossSection"])
    )
    extinction = np.array([rayleigh, np.full(3, 0.00444 * (1 if preset == 0 else 4)), ozone])
    errors = []
    # Exact texel positions isolate the FP32 precompute from LUT interpolation.
    for iy, ix in itertools.product([0, 1, 4, 16, 40, 62], [0, 16, 64, 128, 220, 254]):
        rho = H * iy / 63
        r = math.sqrt(rho * rho + BOTTOM * BOTTOM)
        d = TOP - r + ix / 255 * (rho + H - (TOP - r))
        mu = (H * H - rho * rho - d * d) / (2 * r * d)
        steps = np.linspace(0, d, 16385)
        height = np.sqrt(r * r + steps * steps + 2 * r * mu * steps) - BOTTOM
        densities = np.array(
            [
                np.exp(-height / 8),
                np.exp(-height / 1.2),
                np.clip(np.minimum((height - 10) / 15, (40 - height) / 15), 0, 1),
            ]
        )
        depth = np.trapezoid(densities, steps, axis=1) @ extinction
        reference = np.exp(-depth)
        errors.extend(np.abs(table[iy, ix, :3] - reference))
    return float(max(errors))


def verify(folder, reference=None):
    report = {}
    for name in ["clear", "hazy"]:
        tables, info = read(folder / f"{name}.bin")
        error = transmission_error(tables[0], info["preset"])
        assert error < 0.01, f"FP64 transmission residual too large: {error}"
        info["transmission_max_absolute_error"] = error
        samples = [
            (height, sun, view, azimuth)
            for height, sun, view, azimuth in itertools.product(
                [0.001, 5],
                [10, 2, 0, -2, -4, -6, -9, -12, -15, -18, -24, -30],
                [0.1, 1, 5, 15, 45, 90],
                [0, 45, 90, 135, 180],
            )
        ]
        values = np.array([sky(tables, *point) for point in samples])
        assert np.isfinite(values).all(), "Non-finite sky lookup"
        info["sky_rays"] = len(samples)
        if reference:
            other, other_info = read(reference / f"{name}.bin")
            expected = np.array([sky(other, *point) for point in samples])
            luminance, target = values @ Y, expected @ Y
            relative = np.abs(luminance - target) / np.maximum(1e-4, target)
            index = int(relative.argmax())
            info["reference"] = other_info
            info["relative_luminance_p95"] = float(np.percentile(relative, 95))
            info["relative_luminance_max"] = float(relative[index])
            info["worst_ray_height_sun_view_azimuth"] = samples[index]
            info["absolute_luminance_max"] = float(np.max(np.abs(luminance - target)))
            info["deep_twilight"] = [
                dict(sun_altitude=sun, zenith_luminance=float(sky(tables, 0.001, sun, 90, 0) @ Y))
                for sun in [-6, -9, -12, -15, -18, -24, -30]
            ]
        report[name] = info
    return report


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("folder", type=Path)
    parser.add_argument("--reference", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    result = json.dumps(verify(args.folder, args.reference), indent=2) + "\n"
    if args.output:
        args.output.write_text(result)
    print(result)
