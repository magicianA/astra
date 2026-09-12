# Brightness calibration

`photometric-v1` puts the Sun, Moon, planets, catalogue stars, scattered sky and
diffuse Galactic background into one linear light composition. Source illuminance
is measured in lux and diffuse/surface luminance in cd/m². This is a calibrated
approximation with explicit references, not a guarantee of naked-eye appearance
on every monitor or a radiometric survey of the entire sky.

## Stars and resolved bodies

For visual magnitude, `E = 2.54e-6 × 10^(-0.4m)` lux. Five magnitudes correspond to
exactly a factor of 100 in incident light. Surface magnitudes use
`L = E / (π/648000)²`; 22 mag/arcsec² is 171 µcd/m², consistent with the
[NPS natural-sky benchmarks](https://www.nps.gov/subjects/nightskies/NSQmetrics.html).
V-to-photopic conversion assumes a reference stellar spectrum and varies with colour.

Gaia G is approximately converted to V using the BP−RP polynomial in
[ESA's photometric relations, Table 5.9](https://gea.esac.esa.int/archive/documentation/GDR3/Data_processing/chap_cu5pho/cu5pho_sec_photSystem/cu5pho_ssec_photRelations.html).
Our pack stores a clipped colour proxy, so extreme/missing colours remain approximate.
Native G/Hp/V catalogue magnitudes are exported unchanged, with illuminance above
the atmosphere in a separate field. RGB is normalized to unit luminance so colour
does not accidentally change source brightness.

An unresolved source uses a Gaussian with 0.5 logical-pixel sigma and peak
`E / (2πσ²Ωpixel)`, including the projection's solid-angle Jacobian. Pixel coverage
broadens the footprint without adding energy. The user's magnitude cutoff and
its final 0.75-mag visibility fade remain catalogue selection controls; omitted
stars do not contribute unresolved light.

Resolved discs use mean luminance `E / [4π sin²(radius/2)]`. The Sun's limb profile
and the illuminated Lambertian lunar/planetary profiles preserve this integral.
Lunar magnitude includes phase, observer distance and distance from the Sun. A
reference full Moon at magnitude −12.73 supplies **0.314 lux above the atmosphere**.
The Sun uses the atmosphere's integrated spectrum, about 132,000 lux at 1 AU.
Neither disc has a separate display gain. Planetary phase laws remain approximate;
lunar terrain, libration, eclipses and Earthshine use the approximations documented
in [the exploration notes](EXPLORATION.md). Opposition surge is not modeled.

## Twilight, moonlight and natural darkness

The 45-wavelength Bruneton model supplies solar scattering, ozone absorption,
extinction and the clear/hazy density profiles. Its twilight normalization follows
the near-zenith V-band observations at Paranal in
[Patat et al. (2006), Table 1](https://arxiv.org/abs/astro-ph/0604128).
For solar depression 5°–15°, with `x = depression − 5`, the reference is
`μV = 11.84 + 1.518x − 0.057x²`. We subtract a 247.5 µcd/m² reference night floor
before fitting scattered sunlight at 2.635 km altitude. Generated logarithmic
factors have 0.025° spacing, blending smoothly to unity at depressions 4° and 18°.
The same factor affects sky radiance, diffuse ground lighting and exposure
metering, but not the direct solar beam. This regional constraint does not imply
that other sites have Paranal's weather or that horizon errors match zenith errors.

Lunar sky brightness uses the Krisciunas–Schaefer scattering law reproduced in
[Isaac Newton Group TN 127](https://www.ing.iac.es/astronomy/observing/manuals/ps/tech_notes/tn127.pdf).
Illuminance is converted to footcandles and predicted nanoLamberts to cd/m².
Moon/sightline transmission and approximate scattered-light colour come from the
atmosphere LUT. Phase and distance use the same illuminance as the visible disc.
A deterministic hemisphere integral supplies diffuse ground lighting and metering.
Below the horizon, the model smoothly returns to the transport solution.
Predictions within 10° of the Moon, unusual aerosols and the moonrise extension
are not validated. Ocular glare and a resolved lunar aureole are omitted.

The neutral natural-emission floor is **150 µcd/m²**, with Galactic light added
on top. Airglow variability and zodiacal-light geometry are absent. Light pollution
adds `150e-6 × (101^level − 1)` cd/m² at zenith and increases toward the horizon.
Zero means no artificial light. This is a chosen skyglow level, not a city map.

## Diffuse Galactic background

The [NASA 16K EXR](https://svs.gsfc.nasa.gov/4851/) is encoded without the old gain
or highlight compression. Native pixels, spherical mapping, linear-light mipmaps
and anisotropic filtering are retained. sRGB8 is only the storage transfer function.

One linear scale is fitted to five HEALPix cells from the published photopic
samples in [Masana et al. (2021), Table 3](https://arxiv.org/abs/2101.01500).
Catalogue foreground brighter than V=11.5 is subtracted before fitting because
NASA omits that component. Four adjacent cells are reserved for validation.
The scale is **0.017437294948 cd/m² per linear texture unit**. Held-out total-light
residuals are +14.45%, +2.27%, +22.39% and −4.51%. These nearby samples constrain
overall brightness, not full-sky accuracy. Estimated source colours, survey
artefacts, mixed catalogue bands and small bright/faint catalogue overlaps remain
limitations. The background does not evolve over 10,000 years. References,
foreground integrals, validation split and hashes are in the background manifest.

## Display and verification

All light accumulates in RGBA32F. One luminance-based tone curve follows blending.
Automatic gain is `0.12 / (0.003 + mean_sky_luminance + visible_direct_moon_lux/(2π))`,
followed by the user's exposure multiplier. Metering is independent of camera
direction, FOV and temporal history. Manual exposure fixes the base gain at 40.
The exposure slider is compensation from −40 to +4 EV; 0 EV is the default.
Bright discs can saturate at night; reduce exposure to inspect phases. Exposure
changes all sources together instead of selectively dimming the Moon.

The optional **Adaptive exposure** mode (`adaptive_exposure: true`,
`adaptive_exposure_model: view-trimmed-v1`) overrides the whole-sky/manual base gain.
It meters the actual HDR view before compensation, tone mapping and UI. A Vulkan
compute pass takes four fixed samples per cell on a 64×64 grid, with independent
meters for the two comparison views. The brightest 2% of samples are discarded to
reject isolated stars. The meter uses the greater of the remaining arithmetic mean
and 0.2 times the mean from the 90th–98th percentile, protecting broad lunar highlights.
Gain is `0.12 / (0.003 + metered_luminance)`, bounded between `40 × 2^-40` and 40.
That ceiling retains the dark-sky floor rather than raising every view to middle grey.

Two frame buffers provide asynchronous meter readback; the first valid reading
initializes exposure. Later changes follow a rate-limited exponential in EV:
bright adaptation uses 0.45 s / 12 EV per second, dark adaptation 1.2 s / 4 EV per
second. The integration is independent of frame rate, with elapsed time capped at
0.1 s after a stall. Sampling is deterministic and UI text cannot affect it.
The UI resets compensation to 0 EV when enabling this mode; saved compensation is
preserved on load. Old scenes default to adaptive exposure off.

Screenshots record the effective display gain. Sequence exports with exposure
locked retain the starting primary gain as a fixed manual exposure; unlocked
exports advance adaptation once per output frame using the requested frame rate,
after allowing the asynchronous meter to catch up. Rendering or encoding delays
do not become adaptation time. This is a display convenience, not a physiological
model of eye adaptation, and small bright discs can still saturate in a wide view.

The display transform preserves the computed RGB chromaticity. After exposure,
it maps luminance `y` to `y² / [(y + 0.01)(1 + y)]`: a smooth shadow toe keeps the
natural sky floor near black, while the shoulder compresses bright light. All RGB
channels share that luminance scale. Only out-of-gamut highlights are desaturated.
This does not subtract sky radiance or change the physical Moon/sky/star ratios.

The previous per-pixel grayscale blend has been removed: a pixel's luminance alone
is not an eye-adaptation model. The revised night exposure limit also avoids
lifting the natural background into a grey veil. `hemisphere-moon-v3` and
`colour-preserving-toe-v1` identify these display choices in exported scenes;
the physical `photometric-v1` calibration remains unchanged. Default screenshots
are colour-preserving visualizations, not a validated simulation of naked-eye
colour perception. Physiological adaptation delays, retinal contrast thresholds, individual
eyesight, telescope aperture and the monitor's absolute output are not simulated.

Reproduce calibration and checks:

```sh
python3 -m pip install numpy pillow opencv-python-headless astropy-healpix
python3 scripts/prepare_background.py
python3 scripts/calibrate_twilight.py
python3 scripts/format.py
cmake --build build -j
ctest --test-dir build --output-on-failure
python3 scripts/verify_photometry.py --output docs/PHOTOMETRY_VALIDATION.json
```

Regenerate twilight factors after changing the clear LUT. Maximum interpolation
residual at 400 midpoints is **0.0128 mag**; this measures reproduction of the
chosen curve, not independent atmospheric accuracy. C++ tests cover flux ratios,
disc phase integrals, distance invariance, projection solid angles, colour
normalization, point energy at multiple DPI/subpixel offsets and lunar units.
[Machine-readable results](PHOTOMETRY_VALIDATION.json) accompany this document.

On macOS/MoltenVK, day, civil dusk, astronomical dusk, moonless night and full-Moon
scenes rendered at 2880×1800 with zero Vulkan validation errors and median frame
intervals around 16.7 ms. Windows and Linux were not tested.
