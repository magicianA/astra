# Astra macOS icon

`Astra.png` is the original RGBA artwork, generated with the built-in imagegen tool.
`Astra.icns` contains the 16, 32, 128, 256 and 512 pt representations at 1× and 2×,
including the 1024 px Retina image. The transparent exterior is preserved.

Regenerate on macOS with `python3 scripts/prepare_app_icon.py`. The generated ICNS
is checked in so normal builds do not require image processing. CMake includes it
in `Contents/Resources` and sets `CFBundleIconFile`; the standalone packaging
script preserves both the resource and that property.

`en.lproj/InfoPlist.strings` and `zh-Hans.lproj/InfoPlist.strings` localize the bundle's
display name using the system language. The application window title follows the
language selected inside Astra.

## Generation prompt

Use case: logo-brand. Asset type: production macOS application icon for Astra, a precision astronomy / night-sky simulator. Create ONE finished icon, not a mockup, in a square 1024x1024 image. Dark midnight navy macOS rounded-square tile, occupying approximately 88% of the canvas, centered, with truly transparent exterior margins and rounded corners. On the tile, one bold elegant champagne-gold four-point North Star, surrounded by a thin gold circular astrolabe ring with a single graceful diagonal orbital arc. A tiny warm star on that orbit and only two very restrained faint background stars. Deep navy and subtle desaturated teal depth in the background, refined restrained metallic gold highlights, crisp silhouette and strong legibility at 32px. A sophisticated scientific instrument feeling, matching a dark astronomy app with gold controls. Face-on, symmetric main star, gentle dimensional shading, no dramatic perspective. No text, no letters, no numbers, no gradients outside the tile, no scenery, no galaxies/photo background, no planets, no decorative frame, no presentation grid, no UI or device. Deliver only the icon artwork with a real alpha channel, not a checkerboard illustration.
