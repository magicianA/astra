# Physical dawn and dusk

Implemented on 2026-09-08. The earlier [research](TWILIGHT_DESIGN.md) records the
tradeoffs and longer-term validation goals; this file describes the delivered model.

## Transport and composition

The Vulkan compute generator evaluates the pinned Bruneton reference equations
with 45 wavelengths over 360–830 nm, CIE colour matching, ozone absorption, separate
single-Mie storage and eight scattering orders. Clear and hazy presets share a
6360 km spherical Earth and 60 km atmosphere. The hazy preset multiplies aerosol
scattering and extinction by four. The solar zenith lookup extends to 120 degrees,
covering the whole civil, nautical and astronomical twilight interval.

Each preset contains transmittance (256×64), reduced scattering (256×128×32),
single Mie (256×128×32), and indirect irradiance (64×16), all RGBA32F. Both persistent
presets occupy about 64.53 MiB of image payload in total. The offline generator uses
five additional working images. Runtime uploads completed tables once; time, camera
and preset changes never regenerate them. Devices without RGBA32F linear filtering
use explicit bilinear/trilinear sampling specialized into the shaders. A floating-point
HDR attachment is required; there is no silent 8-bit rendering fallback.

The atmosphere uses the ephemeris Sun's **geometric** local direction and inverse-square
distance factor. The visible discs retain the existing apparent/refraction calculation.
Lunar sky illumination uses the Moon's magnitude, including phase and distance, to
scale a neutral solar-spectrum transport approximation. A full Moon corresponds to an
approximate 0.25 lux source normalization. This is not a lunar spectral reflectance model.

Sky radiance and transmitted Milky Way/stellar light share one HDR composition.
The old global daylight magnitude penalty, separate Milky Way twilight fade, and scalar
extinction multiplication have been removed. Stellar point widths and all camera mapping
remain unchanged. Opaque resolved discs preserve foreground atmospheric radiance while
occluding the background. The geometric ground takes a separate shader path so rays
through the opaque surface do not evaluate the space-background transport.

The Milky Way image, stellar point gain, airglow/light-pollution floor and lunar disc
display compression remain appearance approximations. In particular, resolved lunar
brightness is compressed to retain its terminator at night; its contribution to sky
illumination still uses the independent flux model. This is not a calibrated photometer.
Clouds, geographic terrain, historical weather and a wavelength-dependent refraction
solver are not included. Below-sea-level observers use the sea-level atmospheric column.
Pressure and temperature affect apparent refraction; the transport uses the selected
fixed density profile. Turning off the ground overlay does not remove atmospheric
extinction through Earth; turn off the atmosphere for an unobstructed geometric view.

## Exposure, controls and saved scenes

Automatic exposure reads the preset's hemispherical indirect irradiance at the observer
and both source elevations once per frame. Metering is independent of camera direction,
field of view, previous frame and playback direction. Its gain is
`0.12 / (0.0004 + mean_sky_luminance)`, followed by the user's exposure multiplier.
The manual setting fixes the metering gain at 300. This provides deterministic exposure;
it does not simulate delayed physiological dark adaptation.

The Time panel shows geometric solar altitude and twilight phase. Previous/next dawn
and dusk find **solar-centre crossings at −6 degrees**, then face the appropriate horizon.
These are civil twilight events, not upper-limb sunrise/sunset times. Searches bracket
at half-hour intervals, refine nearby extrema for grazing polar events, and bisect to
0.1 seconds. They stop after 370 days, cancellation, or the supported calendar/time-scale
boundary; the numerical solver tolerance is not a claim of historical timing accuracy.

The View panel contains automatic exposure and clear/hazy presets. Both languages include
the new controls. Scene files store model, preset and exposure mode; exports also record
the effective gain. Legacy scenes select clear atmosphere with automatic exposure.
Their old `extinction` value is retained for file compatibility but no longer affects
physical transport. All time, observer and camera settings survive migration.

## Rebuilding and verification

Cloning with Git LFS supplies the ready-to-use tables. To regenerate after changing the
model or spectral coefficients, with the usual Vulkan environment configured:

```sh
python3 scripts/prepare_atmosphere.py
cmake --build build -j
build/astra_atmosphere data/atmosphere 8 --validation
python3 scripts/prepare_atmosphere.py --manifest data/atmosphere
```

The cache header binds the precompute shader/reference/coefficient source hashes,
preset, order count and wavelength count. Runtime checks its exact length, CRC32 and
finite values. SHA-256 manifests are verified before packaging. The binary format is
little-endian with an explicit 92-byte header; model mismatches fail with a regeneration
instruction instead of silently displaying incompatible tables. Upstream notices are
bundled with the app.

Numerical validation requires NumPy:

```sh
build/astra_atmosphere artifacts/atmosphere-reference 12 --validation
python3 scripts/verify_atmosphere.py data/atmosphere \
  --reference artifacts/atmosphere-reference
ctest --test-dir build --output-on-failure
```

Measured on Apple M4 Pro using MoltenVK:

| Measurement | Clear | Hazy |
| --- | ---: | ---: |
| Transmission vs independent 16,384-interval FP64 density integration, maximum absolute residual | 0.00278 | 0.00516 |
| 8 vs 12 scattering orders, 95th-percentile relative luminance difference | 0.044% | 0.071% |
| 8 vs 12 scattering orders, maximum relative luminance difference | 0.131% | 0.225% |
| 15 vs 45 wavelengths, maximum relative luminance difference | 4.33% | 4.37% |

The sky comparison samples 720 rays per preset: two observer heights, twelve solar
elevations from +10 to −30 degrees, six view elevations including 0.1 degrees, and five
relative azimuths. Relative comparisons use a `1e-4` luminance denominator floor near
black. The measured residuals are numerical comparisons for this model and sampling;
they are not accuracy bounds against the real sky. Full upstream CPU spectral-renderer
comparison and increased LUT-resolution convergence remain unmeasured.

Night and dusk screenshot runs at 2880×1800 reported approximately 16.6–16.7 ms median
CPU-plus-presentation frame intervals on this Mac. These are presentation-capped frame
measurements, not GPU-only timings. Windows and Linux have not been tested.

Mac regression runs covered dawn, dusk, daylight, atmosphere disabled, lunar phase,
forward/reverse playback, repeated preset/exposure changes, resize, historical dates
and polar views with zero Vulkan synchronization-validation errors. Core tests also
exercise grazing polar dawn/dusk within one half-hour bracket, cancellation, scene
migration and both date limits. A deliberately corrupted cache was rejected before
rendering. Three repeated explicit-filter screenshots differed from native filtering
by at most one 8-bit channel level. An explicit HDR visibility barrier prevents stale
tiles on this MoltenVK path. Existing camera and label-motion regression tests pass.
