# Bruneton atmospheric scattering reference

Upstream: https://github.com/ebruneton/precomputed_atmospheric_scattering

Pinned commit: `34f14e745cff948f4ca3157d1b62a445ffa7286f`.

`definitions.glsl`, `functions.glsl`, `constants.h` and `LICENSE` are unmodified
upstream files. `earth.json` extracts the 48 solar irradiance and ozone absorption
samples from `atmosphere/demo/demo.cc` at that commit. The wavelength grid is
360–830 nm in 10 nm steps. Upstream documents their original scientific sources
in that demo file.

Astra's wrappers, compute orchestration, spectral coefficient generation and
render integration are outside this directory. They use kilometre units, a
120-degree solar zenith limit, separate Mie tables, 45-wavelength integration,
eight scattering orders, and full-precision storage. The haze preset increases
the Mie scattering/extinction coefficients by four. The data is a fixed model
atmosphere, not a historical weather record.

BSD-3-Clause attribution is retained in `LICENSE`, in each upstream source file,
and in the packaged application's `licenses/Bruneton-BSD-3-Clause.txt`.
