#define IN(x) const in x
#define OUT(x) out x
#define TEMPLATE(x)
#define TEMPLATE_ARGUMENT(x)
#define assert(x)
const int TRANSMITTANCE_TEXTURE_WIDTH = 256;
const int TRANSMITTANCE_TEXTURE_HEIGHT = 64;
const int SCATTERING_TEXTURE_R_SIZE = 32;
const int SCATTERING_TEXTURE_MU_SIZE = 128;
const int SCATTERING_TEXTURE_MU_S_SIZE = 32;
const int SCATTERING_TEXTURE_NU_SIZE = 8;
const int SCATTERING_TEXTURE_WIDTH = 256;
const int SCATTERING_TEXTURE_HEIGHT = 128;
const int SCATTERING_TEXTURE_DEPTH = 32;
const int IRRADIANCE_TEXTURE_WIDTH = 64;
const int IRRADIANCE_TEXTURE_HEIGHT = 16;
#include "../vendor/bruneton/definitions.glsl"
#include "atmosphere_spectrum.glsl"

AtmosphereParameters earthAtmosphere(int group, int preset) {
    float aerosol = preset == 0 ? 1. : 4.;
    DensityProfile molecular = DensityProfile(DensityProfileLayer[2](
        DensityProfileLayer(0., 0., 0., 0., 0.), DensityProfileLayer(0., 1., -1. / 8., 0., 0.)));
    DensityProfile mie = DensityProfile(DensityProfileLayer[2](
        DensityProfileLayer(0., 0., 0., 0., 0.), DensityProfileLayer(0., 1., -1. / 1.2, 0., 0.)));
    DensityProfile ozone =
        DensityProfile(DensityProfileLayer[2](DensityProfileLayer(25., 0., 0., 1. / 15., -2. / 3.),
                                              DensityProfileLayer(0., 0., 0., -1. / 15., 8. / 3.)));
    return AtmosphereParameters(SOLAR[group],
                                .004675,
                                6360.,
                                6420.,
                                molecular,
                                RAYLEIGH[group],
                                mie,
                                vec3(.003996 * aerosol),
                                vec3(.00444 * aerosol),
                                .8,
                                ozone,
                                OZONE[group],
                                vec3(.1),
                                -.5);
}

#include "atmosphere_filter.glsl"
#define texture filteredTexture
#include "../vendor/bruneton/functions.glsl"
#undef texture
