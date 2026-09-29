# Astra

English · [Simplified Chinese](README.zh-CN.md)

A native **Rust / egui / Vulkan** desktop planetarium covering 10,000 years.
Choose a location on Earth, an elevation, and a date and time to see the topocentric
apparent positions of stars, the Sun, the Moon, and the seven other planets.
On macOS, Vulkan runs on Metal through MoltenVK. The interface supports **English and Simplified Chinese**.

The astronomy, scene handling, exports, and application are implemented in `rust/`.
The renderer uses ash for Vulkan HDR rendering; egui and winit provide the desktop
interface. ERFA and CSPICE remain native C library dependencies. Cargo is the supported
build workflow. Retained C++ sources and older implementation documents are migration
references and are not compiled into the Rust application.
See [Rust migration, validation and current limits](docs/RUST_MIGRATION.md) for the
maintained architecture and verification record.

Physical dawn and dusk use spectral Rayleigh/Mie multiple scattering and ozone absorption.
The Time panel can jump to previous/next civil dawn or dusk; the View panel offers clear
and hazy atmospheres and camera-independent automatic exposure. Try `examples/dawn.json`
or `examples/dusk.json`. Brightness now uses a shared photometric scale for stars,
the Moon, twilight and the Milky Way. See [calibration and limits](docs/PHOTOMETRY.md)
and [atmospheric transport](docs/TWILIGHT_IMPLEMENTATION.md).

Enable **Adaptive exposure** in **View → Exposure** to meter the current image as
you pan or zoom. It adjusts smoothly, preserves a dark night sky, and can expose
resolved lunar detail. Enabling it resets compensation to 0 EV; the slider remains
available for adjustment. This optional mode overrides whole-sky automatic exposure
and is saved with the scene. Try `examples/adaptive-moon.json`.
This example observes the Moon at night with the atmosphere enabled. If a custom
scene leaves the sky black during daylight, choose **Time → Restore day & night**
to restore atmosphere and automatic exposure at 0 EV. Dawn/dusk shortcuts also
restore these settings.

![Stars and the Milky Way above the faint glow of twilight in Astra](docs/images/dusk.png)

*Stars, the Milky Way, and lingering twilight from Beijing, September 8, 2026,
at 19:28 local mean solar time under ideal clear skies without artificial skyglow.*

![The Milky Way and stars rendered above the Atacama Desert](docs/images/milky-way.png)

*The Milky Way from the Atacama Desert, June 15, 2026, at 00:00 local mean solar time.*

Both images are unedited 2880 × 1800 exports from the original Vulkan renderer on
macOS, captured before the Rust migration. [Scene files and current capture commands](docs/images/README.md)
are included to render the same scenes with Rust.

The supported interval is **−3000-01-01 through +7000-01-01, exclusive of the end date**,
using astronomical year numbering and the proleptic Gregorian calendar by default.
Year 0 is 1 BCE; year −3000 is 3001 BCE. Historical and future dates use UT1, TT, TDB,
or local mean solar time. UTC is available only within the modern interval supported
by the leap-second and Earth-orientation data.

## New exploration tools

The **Explore** section in the left sidebar opens lunar surface detail, 88 constellation figures, a
simultaneous two-epoch view, a 24-hour observing planner, imported horizon profiles,
fixed-step video / star-trail export, and eclipse / occultation / close-approach search.
Start with `examples/moon-detail.json`, `examples/epoch-comparison.json`,
`examples/mountain-sky.json`, or `examples/lunar-eclipse.json`.
[Usage, data sources and accuracy limits](docs/EXPLORATION.md) describe each tool.
MP4 export requires a separately installed FFmpeg; PNG and trail exports work without it.

## Platform status

- **macOS:** tested on Apple M4 Pro, arm64, macOS 26.4.1. The current local package's
  ERFA library requires macOS 26. Other macOS targets require rebuilding dependencies
  for the appropriate deployment target.
- **Windows and Linux:** platform-specific build paths are present but have not been tested.

## Clone with Git LFS

Install and enable Git LFS before cloning. The repository stores approximately **4.3 GB**
of ephemerides, star catalogues, background images, fonts, and other binary assets in LFS;
ordinary Git stores their pointers.

```sh
brew install git-lfs
git lfs install
git clone https://github.com/magicianA/astra.git
cd astra
git lfs pull
```

Allow disk space for both the LFS object cache and working copies, plus build outputs
and the packaged application. `target/`, `dist/`, `artifacts/`, downloaded third-party
dependencies, and the raw Gaia download cache are excluded from Git.

## Build and run on macOS

Requires a current Rust toolchain with Cargo, Xcode Command Line Tools, Python 3.9+,
and Homebrew. After fetching the LFS assets, install the native libraries and shader
compiler, prepare CSPICE, and build:

```sh
brew install erfa vulkan-loader molten-vk glslang vulkan-tools vulkan-validationlayers
python3 scripts/bootstrap.py --dependencies

cargo build --release
cargo test --release

python3 scripts/package_macos.py
open dist/Astra.app
```

The locally packaged arm64 app includes its runtime libraries, fonts, star catalogue,
and DE441 ephemerides. It runs offline without setting `VULKAN_SDK`. `dist/Astra.app`
is generated by the packaging step and is not included in the repository.

The macOS icon is included automatically in packaged apps.
Its artwork and ICNS file live in [`assets/macos/`](assets/macos/). After changing the
artwork, run `python3 scripts/prepare_app_icon.py` on a Mac, then rebuild.

Run the development application with `cargo run --release --bin astra`. To build only
the numerical CLI, use `cargo build --release --no-default-features --bin astra_cli`;
its tests run with `cargo test --no-default-features --lib`. The `gpu` feature enables
the Vulkan renderer and atmospheric precomputation, while the default `desktop` feature
also enables the native interface. Compiled SPIR-V shaders are embedded by Cargo.

Other platforms require Rust, ERFA, and a matching CSPICE distribution; the desktop
application also needs a Vulkan 1.2 graphics/compute device, a Vulkan loader, and
glslangValidator at build time. Set `CSPICE_ROOT`, `ERFA_LIB_DIR`, and
`GLSLANG_VALIDATOR` for custom installations. Set `ASTRA_DATA` to override the runtime
data directory. Mac CSPICE binaries cannot be reused on other platforms.

### Rebuild the data assets

These steps are optional when the complete LFS assets are already available. They
download the source data and regenerate the catalogue, CIO table, and Milky Way texture:

```sh
python3 scripts/bootstrap.py --all
python3 -m pip install Pillow==11.3.0 numpy==2.0.2 opencv-python-headless==4.13.0.92
python3 scripts/prepare_background.py
python3 scripts/prepare_exploration.py

target/release/astra_cli --generate-cio data/time/cio.bin 4
python3 scripts/fetch_gaia.py
python3 scripts/build_catalog.py
```

DE441 occupies about 3 GB; the compressed Gaia source catalogue adds several hundred
megabytes. Initial downloads and conversion take time. `bootstrap.py` verifies pinned
SHA-256 checksums, reuses valid local files, and supports resuming large downloads.
The Gaia script reuses its asynchronous query job, but restarts interrupted CSV downloads.
The Earth-orientation snapshot is pinned to 2026-09-04; preserve the checked-in snapshot
when upstream files change.

## Explore the sky

Use **中文 / English** at the top to switch between English and Simplified Chinese immediately.
Without a saved preference or launch override, the app reads `LANG` and falls back
to English. It does not query macOS preferred languages.
Your selection is saved in `ui-preferences.json` alongside the application's user settings.
Changing language preserves the observing time, location, camera, selection, and playback.
Search accepts both English and Chinese object names in either interface language.
For a one-time launch override, use `--language en` or `--language zh-CN`; this does not
overwrite your saved preference unless you change the language in the interface.

Drag to look around, scroll to zoom, and click a celestial object for details. The left
sidebar contains expandable location, time, display, Moon, and exploration sections. Search is at the top; time stepping
and forward/reverse playback are at the bottom. The time panel contains the 10,000-year
timeline. Scene saving and PNG export are in the display panel.
PNG exports contain the Vulkan sky layer; GUI panels, labels, grid lines, and other
egui overlays are omitted even when visible in the window. Each PNG has a scene JSON
file recording its complete calculation and effective exposure.

| Input | Action |
| --- | --- |
| Drag | Change azimuth and elevation while keeping the horizon level |
| Scroll | Zoom in or out |
| Arrow keys | Turn the view |
| Space | Play or pause |
| R | Reset camera roll and level the horizon |
| H | Toggle the interface for an unobstructed sky view |
| Esc | Clear the selection and stop tracking |

The default view uses **rectilinear perspective with a 60° vertical field of view**.
Ordinary perspective zoom is limited to 1°–60°, so scrolling back out stops at the
normal panorama. Switching back from another projection also limits the field to 60°.
The **Panorama** button restores this view.

Panning is scaled to the window dimensions and field of view, introduces no camera roll,
and stops at the zenith or nadir. Use R to level an older scene that contains an explicit
roll angle. The display panel also offers stereographic wide-angle and full-sky fisheye
projections; these can curve lines away from the image centre. Perspective and
stereographic FOV values describe the vertical field. Switching projections preserves
the viewing direction, and old saved scenes retain their original projection.
Disable the ground to see objects below the horizon.

The default scene is Beijing at **2026-09-04 22:00 local mean solar time**, which differs
from civil time-zone time. Choose UTC for supported modern civil-time input. The **Now**
button fills in the current UTC time. Scenes, favourites, and manual exports are saved to:

```text
~/Library/Application Support/Astra/Sky/
```

### Find the Moon

Expand **Moon** in the left sidebar to see its illuminated fraction, altitude, and distance. In the
default scene the Moon is about 3.4° below the horizon. Click **Find a viewing time** to search
the next 35 days at half-hour intervals for a time with:

- Moon altitude of at least 8°;
- Sun altitude below −6°;
- Lunar illumination of at least 2%.

The search runs in the background, preserves the location and time standard, and then
centres a close-up on the Moon. It respects ground occlusion and the available data
interval. It suggests a viewing time rather than computing an exact moonrise, and
reports when no suitable time is found, for example during polar daylight.

**Focus Moon** centres the Moon without changing the time. **Zoom to Moon** adjusts the view,
and **Panorama** restores the normal panorama.

### Milky Way

The background uses the Gaia Milky Way layer from
[NASA Deep Star Maps 2020](https://svs.gsfc.nasa.gov/4851/) at its native
**16384×8192** resolution. Bright foreground stars are separated from this layer;
the background retains dark dust lanes, star clouds, and faint starlight.
Linear-light mipmaps and optional 8× anisotropic filtering stabilize zooming and panning.

The background rotates with the observer's location and time. Its visibility responds
to twilight, moonlight, extinction, and light pollution. Toggle it with **Milky Way** in
the display panel. To open a moonless Atacama example after packaging:

```sh
open dist/Astra.app --args --scenario "$PWD/examples/milky-way.json"
```

The map is a distant background fixed in Galactic coordinates. It does not model the
evolution of the Milky Way, dust, or background faint stars over 10,000 years. The linear
EXR source is encoded as sRGB PNG without highlight compression, preserving its native pixels
except for averaging the polar rows. No blur, sharpening, or upscaling is applied.
A regional photopic normalization sets the overall brightness; this is not a calibrated
all-sky radiometric survey. Individual catalogue stars are calculated separately.
See [photometric references and measured residuals](docs/PHOTOMETRY.md).

The 16K texture and mipmaps use about **683 MiB of GPU memory**. Devices without 16K
texture support use a supported lower mip level. Offline preparation uses the approximately
357 MiB source EXR; the app bundles only the processed PNG and does not require OpenCV
or network access. Sources and processing hashes are recorded in
[`data/background/manifest.json`](data/background/manifest.json); scene exports include
a separate `background_id`.

## Data and astronomical calculations

- **Stars:** an all-sky Gaia DR3 query with `G ≤ 12`, merged with Hipparcos-2. BSC5
  supplies bright-star names, colours, and some radial velocities. Source IDs are
  deduplicated and checked against an independent count. The acquisition scripts retain
  the source data and full ADQL query locally; the raw Gaia cache is not committed.
  See [`data/catalog/manifest.json`](data/catalog/manifest.json) for exact counts.
- **Solar system:** both complete JPL DE441 SPK segments cover the target interval.
  The Sun, Earth, Moon, Mercury, and Venus use body centres. Mars through Neptune use
  planetary-system barycentres, identified in the information panel.
- **Astrometry:** double-precision CPU calculations with ERFA and CSPICE account for
  stellar space motion, parallax, light time, solar gravitational deflection, aberration,
  Earth rotation, precession, nutation, polar motion, the observer's location, and optional
  near-horizon refraction.
- **Earth orientation:** modern IAU models apply from 1900 to 2100. Distant epochs use
  Vondrák long-term precession and a numerical CIO table, with smooth transitions.
  Unknown Earth rotation uses an explicitly estimated ΔT model, with a manual override.
- **Rendering:** ash Vulkan HDR rendering with egui desktop presentation, sky and star
  points, resolved Sun/Moon/planet discs and phases, exposure mapping, and screenshots.

**Data coverage does not imply uniform accuracy over 10,000 years.** ΔT, long-term
nutation, unknown stellar radial velocities, binary photocentres, and stellar evolution
limit distant-epoch reliability. Reported uncertainties use diagonal formal estimates;
full covariance propagation is not implemented. Gaia G is approximately transformed
to V for brightness calculations; mixed catalogue bands, clipped colour proxies and
non-spectral colour estimates still limit photometric accuracy. The
`G ≤ 12` selection applies at the catalogue epoch and does not include every fainter
star that could brighten at another epoch. The atmosphere does not model actual weather,
and the ground plane does not include terrain.

## Numerical queries and validation

```sh
target/release/astra_cli --date -2500-01-01T22:00:00 --site 116.4074,39.9042,45 \
  --scale UT1 --no-atmosphere --output artifacts/ancient.json

python3 scripts/validate_horizons.py
```

The CLI exports azimuth, altitude, apparent direction, distance, magnitude, and time-model
information. Use `--scenario FILE` to reproduce a saved scene. The JSON accompanying a
screenshot also records the actual TT/UT1/TDB values, ΔT, quality notes, and data version.

To run Vulkan synchronization validation and the automated window smoke test on the
tested Apple Silicon Homebrew installation:

```sh
DYLD_LIBRARY_PATH=/opt/homebrew/opt/vulkan-validationlayers/lib \
VK_DRIVER_FILES=/opt/homebrew/opt/molten-vk/etc/vulkan/icd.d/MoltenVK_icd.json \
VK_LAYER_PATH=/opt/homebrew/opt/vulkan-validationlayers/share/vulkan/explicit_layer.d \
VK_LAYER_VALIDATE_SYNC=1 \
target/release/astra --validation --smoke --frames 300 \
  --screenshot artifacts/astra-smoke.png
```

Use `--play-speed 3600 --frames 600` for a playback run; negative speeds play backward.
Expensive stellar calculations run on a worker thread. During playback, the renderer
projects cached inertial stellar directions with the current Earth orientation, refreshing
those directions at intervals of at least 150 ms, or longer when a full catalogue update
takes more time. Earth rotation and solar-system apparent positions are calculated for
each displayed time. Pausing and exporting use a complete calculation for the final time.
The stellar approximation becomes less accurate at high playback speeds, so playback
frames should not be treated as precision observations.

The current desktop renderer reads each rendered Vulkan sky image back to CPU memory
and uploads it to egui for presentation. This adds synchronization and transfer costs,
especially at high resolutions; it does not yet share a GPU image directly with the
window. See the [migration record](docs/RUST_MIGRATION.md) for tested behavior and limits.

Moving labels retain fractional-pixel coordinates to avoid integer-pixel jumps. The
Rust text tests check fractional positions and shadow alignment. Stars use compact points with
a fixed width and compressed brightness; resolved objects such as the Moon use their
angular radii.

## Code formatting

Rust uses rustfmt; GLSL uses clang-format with four-space indentation and a 100-column
limit. Python uses Ruff. The formatter handles the current Rust, shader, and Python
sources and leaves third-party code and historical C++ references unchanged.

```sh
python3 -m venv .venv
.venv/bin/pip install ruff==0.16.6
rustup component add rustfmt
PATH="$PWD/.venv/bin:$PATH" python3 scripts/format.py
PATH="$PWD/.venv/bin:$PATH" python3 scripts/format.py --check
```

## Project layout and documentation

| Directory | Contents |
| --- | --- |
| `rust/` | Astronomy, ERFA/CSPICE bindings, ash Vulkan renderer, egui application, CLI, and unit tests |
| `Cargo.toml`, `Cargo.lock` | Rust features and pinned dependency versions |
| `shaders/` | GLSL shaders compiled to embedded SPIR-V by `rust/build.rs` |
| `scripts/` | Data acquisition, conversion, validation, formatting, and packaging |
| `tests/` | Independent numerical reference data and retained migration regression references |
| `examples/` | Reproducible observing scenes |
| `assets/macos/` | macOS application icon and generation notes |

The detailed design and implementation documents are currently in Chinese. Their C++
build and UI implementation descriptions document the original application; use the
Cargo commands above for the Rust application:

[Original design](docs/DESIGN.md) · [Implementation and validation](docs/IMPLEMENTATION.md) ·
[Data sources](docs/DATA_SOURCES.md) · [Local Vulkan environment](docs/ENVIRONMENT.md) ·
[Third-party notices and licences](THIRD_PARTY_NOTICES.md)

Atmosphere documentation in English: [Research](docs/TWILIGHT_DESIGN.md) ·
[Implementation and validation](docs/TWILIGHT_IMPLEMENTATION.md).
