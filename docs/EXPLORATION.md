# Exploring, planning and recording

Implemented September 2026. These tools share Astra's DE441 astrometry, observer,
time standards and scene files. Windows and Linux remain untested.

![Measured lunar terrain rendered in Astra](images/moon.png)

*Unedited Vulkan export using `examples/moon-detail.json`; UI hidden, fixed lunar-detail exposure.*

## Moon surface

**Explore → Constellations & epochs → Expose lunar detail** centres a 1.2° view and
sets a fixed exposure of −18 EV relative to the normal night-sky exposure. Use the
View panel to restore automatic exposure and EV 0 for a wide night view. A Moon
exposed for surface detail naturally leaves most surrounding stars invisible.

The renderer maps the [NASA CGI Moon Kit](https://svs.gsfc.nasa.gov/4720/) onto a
sphere: 4096 × 2048 LROC colour, resampled LOLA heights, terrain normals and sampled
shadows toward the Sun. The packed height map retains 0.5-metre quantization; this
is encoding precision, not 0.5-metre terrain resolution. The sphere's limb is not
geometrically displaced. Near-terminator shadows sample the height field at 16
increasing distances; they are a real-time approximation, not a resolved mesh.

The SVS colour map was adjusted for visualization. It supplies spatial variation,
not an absolute calibrated albedo measurement. Linear RGB is divided by its
near-side, full-phase, Lambert-weighted luminance mean (0.21012569). Existing
phase/distance photometry still sets the overall illumination. Terrain and changing
visible surface introduce residual flux differences; this is not precision lunar
radiometry. Earthshine uses a Lambertian Earth with albedo 0.3, its phase and distance.

The [NAIF lunar frame](https://naif.jpl.nasa.gov/pub/naif/generic_kernels/fk/satellites/moon_de440_250416.tf)
uses the DE440 numerical lunar orientation kernel from **1549-12-31 through
2650-01-25**. Outside its actual kernel coverage, the app explicitly labels the
IAU orientation approximation. Optical libration follows the changing viewpoint;
physical libration within coverage follows the kernel. Astrometric positions still
use DE441. Neither ancient terrain evolution nor remote-epoch lunar orientation
accuracy is established. Source hashes and credits are in `data/moon/manifest.json`.

Example: `examples/moon-detail.json`.

## Constellations and two epochs

Turn on **Constellation lines** and **Constellation names**, or select a figure and
choose **Focus constellation**. The 88 figures use HIP-linked stars from
[Stellarium's Western sky culture](https://github.com/Stellarium/stellarium-skycultures/tree/014fbb5e59233d133c22f9811af96b67d05a95c9/western).
Names are available in English and Chinese. Lines follow the computed proper motion
of their endpoints. These are modern guide figures, not IAU boundary polygons or a
historical reconstruction of ancient sky cultures. Their CC BY-SA attribution is
preserved in `data/skycultures/LICENSE.md`.

**Compare two epochs** divides the same window into two sky views. Both share the
observer, month, day, clock reading, projection, horizon and camera. The right view
uses **Comparison year**. Dragging either side moves both cameras; selection belongs
to the left side. The right side uses UT1 when its year cannot use the modern UTC
data, and February 29 becomes February 28 in a non-leap comparison year. Each view
shows its date and time standard. This compares the same clock reading, rather than
the same sidereal time. Atmospheric exposure is calculated independently for each
sky; immutable lunar and Milky Way textures are shared.

Example: `examples/epoch-comparison.json`.

Search now indexes complete HIP and Gaia DR3 identifiers and caches named-star
results. Examples: `HIP 32349`, `Gaia DR3 2947050466531873024`, `Sirius`. Named searches
are case insensitive for ASCII. A selected faint star raises the display magnitude
limit when needed; search is independent of the previously visible catalogue subset.

## Observing planner

**Explore → Observing planner** calculates the next 24 hours for a solar-system body
or the selected catalogue star. The chart shows target, Sun and Moon altitude. Hover
for values; click the curve or **Visit recommended time** to visit that time. The
results retain the site and starting date used for their calculation.

The plan lists rise/set crossings and the time of highest altitude, including cases
with no crossing in the interval. A five-minute grid brackets crossings and the
maximum, then numerical refinement uses a 0.1-second interval. That numerical step
is not an accuracy claim: refraction, terrain resolution and source models dominate.
Rise/set uses centre altitude plus angular radius against the local horizon profile.
Sharp terrain features can produce events between samples that this grid misses.

Recommended windows require altitude ≥20°, a clear horizon, and geometric Sun altitude
≤−18° (≤−6° for the Moon). The Sun itself has no darkness requirement. For other
night targets a visible Moon must be at least 20° away. The ranking also includes
an approximate lunar sky-brightness estimate and artificial skyglow. Atmospheric
transparency is simplified, and weather forecasts are not included. The displayed
sky-brightness estimate represents night-sky background, not a full daytime LUT
measurement. Green intervals have five-minute resolution.

**Export observing table** writes `observing-plan.csv` and its starting scene to the
app's user directory. Time values use the scene's time standard, not the computer's
civil time zone. The headless equivalent is:

```sh
build/astra_cli --scenario examples/milky-way.json --plan --target 301 \
  --output moon-plan.json
build/astra_cli --scenario examples/milky-way.json --plan --hip 32349 \
  --output sirius-plan.json
```

## Local horizon

**Explore → Landscape → Choose horizon file → Apply horizon** imports CSV azimuth /
altitude columns, or JSON containing a `points` array. Angles are degrees; north is
0°, east is 90°. Azimuths must be unique, within 0..360, and altitudes within −20..89.
A closing 360° sample must agree with 0°. Intermediate samples are interpolated
cyclically onto 720 half-degree bins. The full profile is embedded in saved scenes.

```json
{
  "name": "My observing site",
  "points": [[0, 4], [90, 12], [180, 3], [270, 6], [360, 4]]
}
```

The silhouette masks the sky and bodies and is used for object selection and plans.
Its surface has the existing simple ground shading; it is not a textured landscape
or a 3D terrain mesh. The profile remains a physical observing constraint for plans
even if the Ground toggle is off for viewing below the horizon.

`examples/mountain-horizon.json` is an explicitly synthetic profile for testing;
`examples/mountain-sky.json` demonstrates the same shape against the Milky Way.
It is not measured terrain for the named observing site.

## Timelapse, camera paths and trails

In **Explore → Record & export**, choose a parent directory, frame count, simulation
seconds per frame and video frame rate. Each export creates a new subdirectory.
A fixed sequence waits for the full astrometry and, when enabled, both epoch views
before capturing each frame. It saves `frame-000000.png` and the corresponding scene,
with no UI overlays. It does not sample real-time playback or drop frames.

For a camera path, first move to the desired endpoint and click **Set current view
as endpoint**. Return to the starting view, enable **Camera movement**, then start.
Azimuth and roll take their shortest angular paths, elevation interpolates linearly,
and FOV interpolates logarithmically, all with a smooth start and stop. Both endpoints
must use the same projection. Camera roll remains explicit; no roll is introduced by
a normal drag.

**Lock exposure** converts the current effective exposure to a fixed exposure for the
sequence. Disable it to recalculate atmospheric exposure for each simulated time.
For epoch comparisons, lock exposure applies the primary view's exposure to both sides.
The window cannot be resized while capturing. Changing pixel density aborts capture
rather than silently writing mixed image dimensions. Escape or **Cancel export**
keeps completed frames and marks the sequence manifest as cancelled. Closing the
window also preserves completed frames.

MP4 encoding uses a separately installed [FFmpeg](https://ffmpeg.org/), with H.264,
CRF 18 and yuv420p. Install it with `brew install ffmpeg` on this Mac. The application
checks PATH and standard Homebrew locations; `ASTRA_FFMPEG` may name an executable.
FFmpeg is not bundled. Encoder failure preserves the PNG sequence and reports the
failure. Arguments are passed directly through SDL's process API, without a shell.

**Star-trail composite** selects the whole RGB pixel with greatest displayed luminance
across all frames. This preserves colour but is a photographic lighten composite,
not a radiometric long exposure. Use a fixed camera and small enough simulation steps
to avoid visible gaps; larger steps intentionally produce dotted trails.

```sh
build/astra.app/Contents/MacOS/astra --scenario examples/milky-way.json \
  --sequence ./new-empty-sequence --steps 120 --step 10 --fps 30 --trails
```

The CLI requires an empty directory. Use `--no-video` for PNG-only output or
`--camera-end end-scene.json` for a camera path. `sequence.json` records requested and
completed frame counts, frame rate and time step; `start.json` and `camera-end.json`
preserve the settings. UTC stepping includes leap seconds. Export is bounded by the
same astrometric data interval as the rest of the application.

## Eclipses, occultations and close approaches

**Explore → Event search** searches up to 370 days from the current time. Solar and
lunar eclipses select their bodies automatically. For close approaches and
occultations choose the pair, optionally using the selected star as the second
object. Occultations require the first body to be the foreground object. Use
**View maximum** or **Play event** to visit the result. The playing view tracks the
first body. Adjust FOV and exposure as needed; the lunar detail exposure suits the
ordinary Moon, while an eclipsed Moon needs a longer exposure.

```sh
build/astra_cli --scenario examples/solar-eclipse.json --events solar --days 1 \
  --output solar-events.json
build/astra_cli --date 2025-03-14T00:00:00 --scale UTC --site -122.4194,37.7749,10 \
  --events lunar --days 1 --output lunar-events.json
build/astra_cli --scenario examples/milky-way.json --events occultation \
  --target 301 --hip 80763 --days 35 --output occultations.json
```

NAIF body identifiers: Sun 10, Moon 301, Mercury 199, Venus 299, Mars 4, Jupiter 5,
Saturn 6, Uranus 7, Neptune 8. The last five are planetary-system barycentres in this
application. Rings, satellites, atmospheric refraction within occulting planets,
lunar limb topography and Bailey's beads are not modelled.

The search brackets angular-separation minima on a 30-minute grid and refines each
candidate, including minima in the first or last grid interval. Peaks exactly at a
search boundary may require extending the interval. This handles short grazing
contacts around a bracketed minimum, but it is not an exhaustive certified event
catalogue. Returned contacts are clipped to the requested search interval and a
24-hour contact-search span; missing contacts are null. Results are limited to 256.

Solar eclipses use apparent angular disc overlap at the observer; overlap reduces
solar irradiance and atmospheric illumination. The opaque Moon masks the Sun in the
renderer. Nearby illuminated atmosphere is not ray-traced through the moving umbra,
and no solar corona is included: totality sky appearance is approximate.

Lunar eclipses use Sun/Earth geometry at the lunar surface emission epoch, including
light time to the illuminating Sun and occulting Earth. A spherical Earth enlarged
by 1% approximates its atmospheric shadow. Every lunar surface fragment samples the
fraction of visible Sun, producing a moving curved shadow. Residual umbral red light
is an empirical approximation; its actual colour depends on Earth's atmosphere.
For lunar events `start/end` denote penumbral contact, and `inner_start/inner_end`
denote entry to/exit from the umbra (U1/U4), not totality contacts (U2/U3).

Remote epochs retain DE441 coverage but inherit uncertain Earth rotation (ΔT),
stellar propagation and lunar orientation. A numerically precise timestamp does
not establish historical eclipse locality or observational accuracy.

## Verification

The added numerical suite checks indexed search, faint-object selection, all 88
figures, off-axis illumination, radius-aware culling, horizon seams, scene version
rejection/migration, lunar orientation, independent eclipse references, boundary
interval searches, polar rise/set and terrain effects. Export tests cover fixed
clock steps, UTC leap seconds, camera endpoints, whole-pixel composites, cancellation,
changing dimensions and missing encoders. macOS Vulkan validation covers lunar
rendering, both epoch viewports, resize and frame export.

Independent eclipse references are the
[NASA 2024 solar path table](https://eclipse.gsfc.nasa.gov/SEpath/SEpath2001/SE2024Apr08Tpath.html)
and [NASA lunar catalogue](https://eclipse.gsfc.nasa.gov/LEcat5/LE2001-2100.html).
The latter lists 2025-03-14 greatest eclipse at 06:59:56 TD with ΔT 75 s, or
06:58:41 UT. Astra's spherical calculation gives approximately 06:58:47 UTC for
San Francisco. This single agreement is a regression reference, not a universal
accuracy bound.
