# Rust migration and validation

Updated 2026-09-29. The supported application is the Rust implementation under
`rust/`, built with Cargo. It was ported from the original C++ behavior on the new
`codex/rust-rewrite` branch. Retained `src/`, `include/`, CMake files, and C++ tests
are references for numerical and behavioral checks; Cargo does not compile them.
Older implementation documents can describe the former UI or build workflow.

## Architecture and maintenance

| Area | Current implementation |
| --- | --- |
| Time and coordinates | `math.rs`, `time.rs`, `orientation.rs`; split Julian dates, UTC leap seconds, EOP interpolation, long-term orientation and CIO tables |
| Astronomy | `sky.rs`, `catalog.rs`, `spice.rs`, `ffi.rs`; Rust calculations with direct ERFA/CSPICE C bindings |
| Photometry | `photometry.rs`, `calibration.rs`; shared physical units, exposure and generated twilight/background calibration |
| Scene and exploration | `scenario.rs`, `events.rs`, `eclipse.rs`, `horizon.rs`, `sequence.rs`, `recording.rs`, `explore.rs` |
| Vulkan sky | `gpu.rs`, `renderer.rs`, `atmosphere.rs`, `render_scene.rs`; ash HDR offscreen rendering and retained GLSL physics pipelines |
| Desktop | `main.rs`, `i18n.rs`, `text.rs`; egui/eframe, winit window events, eframe's glow backend for image presentation and GUI overlays |
| Background jobs | `jobs.rs`; complete sky/comparison snapshots, Moon/twilight searches, cancellable sequence export |
| CLI and packaging | `cli.rs`, `atmosphere_cli.rs`, `build.rs`, `scripts/package_macos.py` |

Cargo's default `desktop` feature includes `gpu`. `--no-default-features` builds the
numerical library and CLI without the desktop or Vulkan renderer. GPU builds compile
GLSL to embedded SPIR-V; an external shader directory is unnecessary at runtime.
ERFA and CSPICE are native C dependencies. macOS packaging includes the Vulkan loader,
MoltenVK, ERFA, and the existing runtime data. The dependency bootstrap prepares
CSPICE; Cargo.lock fixes Rust dependency versions.

Complete stellar astrometry runs on a worker thread. During playback, each displayed
time gets current Earth orientation and solar-system positions, while stellar inertial
directions come from the latest complete snapshot. A pause requests a complete snapshot
for the stopped time. Changed requests invalidate older worker results. Comparison
snapshots use the chosen year; a February 29 date becomes February 28 in a non-leap year,
and a UTC comparison uses UT1.

Moon and dawn/dusk navigation run asynchronously and stop playback. Moon navigation
adopts the returned 6° perspective scene and tracks the Moon. Dawn/dusk navigation
clears tracking. Time or location changes invalidate a pending navigation result.

Sequence export uses complete calculations at every requested time. PNGs are saved
before a frame is counted in the manifest. Cancellation preserves completed PNG/JSON
files and records cancellation; it also stops an active FFmpeg process. Exposure locking
uses the renderer's actual effective exposure after initial metering settles. MP4 needs
an installed FFmpeg; PNG and trail outputs do not.

## Verification recorded during the migration

The current automated suite passes 43 tests: 40 library tests, two desktop overlay/picking
tests, and one CLI test. A
separate Vulkan sequence integration test is ignored by default because it needs a
working GPU and the complete runtime assets. Numerical comparisons cover 13 cases
against outputs from the retained original C++ application. This is a migration
regression check, not a new claim about astronomical accuracy.
The 34-test numerical/core configuration and four calibration-script tests also pass.
The cached JPL Horizons check reports 0.41–0.58 arcsecond differences for the nine
solar-system bodies in its reference case.

Two native offscreen Vulkan checks on the tested Apple M4 Pro completed with zero
validation errors, including twilight and adaptive lunar rendering. The explicit
sequence integration test also passed: two saved frames retained their exact requested
times 60 seconds apart, and cancellation after the first frame of a three-frame request
retained one PNG/JSON pair and a cancelled manifest without writing the next frame.
The Rust atmosphere generator also produced both clear and hazy two-order caches with
zero validation errors. A native Chinese desktop regression opened the two-epoch scene,
rendered six frames, exported its PNG/JSON, and closed normally with validation enabled.
Manual click, drag, and language-switch checks remain unverified because macOS was
locked during the attempted Computer Use session.
The final ad-hoc-signed macOS package rendered successfully with development Vulkan
and data environment variables removed, using its bundled libraries and assets.

Run the ordinary suite and targeted core tests from the repository root:

```sh
cargo test
cargo test --no-default-features --lib
cargo fmt --check
```

With complete LFS data and a native Vulkan device, run the sequence integration test:

```sh
VK_DRIVER_FILES=/opt/homebrew/opt/molten-vk/etc/vulkan/icd.d/MoltenVK_icd.json \
cargo test --lib --no-default-features --features gpu \
  jobs::tests::sequence_export_keeps_exact_times_and_saved_frames_on_cancel \
  -- --ignored --nocapture
```

The Homebrew ICD path above is specific to Apple Silicon macOS. Restricted execution
environments can hide Metal devices; an incompatible-driver error there does not test
the renderer. Use a native environment with access to the actual graphics device.

## Current behavior and limits

- The sky is rendered to a Vulkan image, synchronized, read back to CPU memory, then
  uploaded to egui for desktop presentation. There is no direct shared GPU image or
  Vulkan window swapchain yet. This incurs transfer and synchronization costs that grow
  with resolution; no performance parity with the original application is claimed.
- PNG and sequence exports include the Vulkan sky layer. egui panels, object labels,
  selection marks, grid lines, and other GUI overlays are not embedded in these files.
  Interactive viewing still displays the overlays. Export sidecars record the full
  calculation time, data identity, and actual exposure.
- The language switch is **中文 / English** in the top bar. Saved preferences and
  `--language` take precedence; the fallback uses `LANG`, then English. The application
  does not currently query macOS preferred languages. Moon and Explore controls are
  expandable sections in the left sidebar.
- Playback reuses cached stellar directions and is approximate at high speeds. Manual
  PNG and sequence exports compute complete snapshots. Historical README images were
  captured before migration and remain examples of the scenes, not pixel comparisons.
- macOS is the tested native platform. Windows and Linux have build paths but remain
  unverified. Their CSPICE binaries and native library installations must match their
  target platform. The existing astronomical, atmospheric, and photometric model limits
  described elsewhere still apply.
