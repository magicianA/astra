# Rendered scenes

These PNGs are direct, unedited exports from Astra's Vulkan renderer, captured on
an Apple M4 Pro through MoltenVK. Each image is 2880 × 1800 pixels. The original
pixels are preserved: no cropping, compositing, or color adjustments were applied
after export. PNG assets are stored with Git LFS.

| Image | Scene | Local mean solar time | View |
| --- | --- | --- | --- |
| [Twilight and stars](dusk.png) | [Beijing](dusk.json), 39.9042° N, 116.4074° E, 45 m | 2026-09-08 19:28:00 | Azimuth 235°, elevation 25°, vertical FOV 60° |
| [Milky Way](milky-way.png) | [Atacama](milky-way.json), 23.029° S, 67.755° W, 5050 m | 2026-06-15 00:00:00 | Azimuth 190°, elevation 65°, vertical FOV 60° |

Both use perspective projection, the clear atmosphere preset, automatic exposure,
and no light pollution. The Beijing scene represents clear, idealized conditions
at those coordinates. The flat ground has no terrain model. Scene JSON files
record the camera, observer, time scales, data versions, and effective exposure.

The first image looks southwest during late astronomical twilight, with the Sun
13.35° below the horizon. Stars and a subdued Milky Way remain visible away from the brighter western
horizon, all rendered together in one frame. The shared photometric calibration
and low-luminance colour treatment are described in [Brightness calibration](../PHOTOMETRY.md).

After building from the repository root, reproduce the images on macOS:

```sh
VK_DRIVER_FILES=/opt/homebrew/opt/molten-vk/etc/vulkan/icd.d/MoltenVK_icd.json \
build/astra.app/Contents/MacOS/astra --hide-ui --frames 80 \
  --scenario docs/images/dusk.json --screenshot artifacts/dusk.png

VK_DRIVER_FILES=/opt/homebrew/opt/molten-vk/etc/vulkan/icd.d/MoltenVK_icd.json \
build/astra.app/Contents/MacOS/astra --hide-ui --frames 80 \
  --scenario docs/images/milky-way.json --screenshot artifacts/milky-way.png
```

`--hide-ui` omits all ImGui draw data, including panels, labels, and overlay hints.
The sky uses the same rendering path as the interactive app. Export resolution
follows the window's drawable size and display scale; another display may produce
a different resolution. The tested Retina display renders the default 1440 × 900
window at 2× scale. Each PNG export also writes a JSON file beside it.
