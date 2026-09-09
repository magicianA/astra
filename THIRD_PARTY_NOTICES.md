# Third-party software and astronomical data

This local application contains independently licensed software and data. The
project does not assign a new license to those components. Copies of available
license notices are in `data/licenses/` and in the packaged app's resources.

| Component | Version/source | Terms and attribution |
| --- | --- | --- |
| Dear ImGui | 1.91.9b, Omar Cornut and contributors | MIT |
| nlohmann/json | 3.12.0, Niels Lohmann and contributors | MIT |
| SDL | 3.4.10, SDL contributors | zlib |
| Vulkan Loader | 1.4.350.1, Khronos Group | Apache-2.0 and included third-party notices |
| MoltenVK | 1.4.1, Khronos Group and contributors | Apache-2.0 and included third-party notices |
| ERFA | 2.0.1, NumFOCUS and contributors; derived from IAU SOFA | BSD-3-Clause, with ERFA/SOFA notices |
| CSPICE | N0067, NASA/JPL/NAIF | Included SPICE software notice; [NAIF rules](https://naif.jpl.nasa.gov/naif/rules.html) |
| libpng | Homebrew-linked version | libpng license |
| Precomputed Atmospheric Scattering | Eric Bruneton, commit `34f14e745cff948f4ca3157d1b62a445ffa7286f` | BSD-3-Clause; `vendor/bruneton/LICENSE` and `data/licenses/Bruneton-BSD-3-Clause.txt` |
| Noto Sans CJK SC | Sans 2.004, Adobe and Google contributors | SIL Open Font License 1.1 |
| Gaia DR3 | ESA/Gaia/DPAC | [Official credit and citation instructions](https://gea.esac.esa.int/archive/documentation/GDR3/Miscellaneous/sec_credit_and_citation_instructions/) |
| NASA Deep Star Maps 2020 Milky Way layer | NASA/Goddard SVS; Ernie Wright; Gaia DR2: ESA/Gaia/DPAC | [NASA media guidelines](https://www.nasa.gov/nasa-brand-center/images-and-media/); see `data/licenses/NASA-SVS-background.txt` |
| Archived Gaia EDR3 colour map | ESA/Gaia/DPAC; A. Moitinho | [CC BY-SA 3.0 IGO](https://creativecommons.org/licenses/by-sa/3.0/igo/); former background retained in source archives only |
| Hipparcos-2 | F. van Leeuwen, 2007, CDS I/311 | Retain source catalogue terms and Credit: ESA |
| Bright Star Catalogue | Hoffleit & Warren, fifth revised preliminary edition, CDS V/50 | Retain original CDS catalogue documentation |
| DE441 | Park et al., 2021; NASA/JPL | [DE440/441 paper](https://ssd.jpl.nasa.gov/doc/de440_de441.html), NAIF source and notices |
| Earth orientation | IERS / USNO finals2000A | Retain product source, snapshot date and prediction flags |

Gaia acknowledgement: This work has made use of data from the European Space
Agency (ESA) mission Gaia, processed by the Gaia Data Processing and Analysis
Consortium (DPAC). Funding for the DPAC has been provided by national institutions,
in particular the institutions participating in the Gaia Multilateral Agreement.

Hipparcos source material includes noncommercial terms. This package was assembled
for the user's local use. Commercial redistribution of the combined catalogue
requires checking the terms of each upstream catalogue; the software component
licenses do not grant additional rights to the scientific data.

The renderer uses Bruneton spectral multiple-scattering atmosphere tables and
procedural body shading, together with the
[NASA SVS Deep Star Maps 2020 Milky Way layer](https://svs.gsfc.nasa.gov/4851/).
The native 16384x8192 Galactic-coordinate EXR omits the bright Hipparcos/Tycho
foreground. Astra applies display intensity mapping, sRGB encoding and polar-row
averages, preserving the original spatial detail. Runtime rendering adds
linear-light mipmaps, optional anisotropic filtering and atmospheric attenuation.
Source credits, changes and checksums are recorded in data/background/manifest.json
and data/licenses/NASA-SVS-background.txt. No planet surface imagery is included.
