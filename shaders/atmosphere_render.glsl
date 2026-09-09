#include "atmosphere_model.glsl"
#include "scene.glsl"
layout(set = 1, binding = 0) uniform sampler2D clearTransmittance;
layout(set = 1, binding = 1) uniform sampler3D clearScattering;
layout(set = 1, binding = 2) uniform sampler3D clearMie;
layout(set = 1, binding = 3) uniform sampler2D clearIrradiance;
layout(set = 1, binding = 4) uniform sampler2D hazyTransmittance;
layout(set = 1, binding = 5) uniform sampler3D hazyScattering;
layout(set = 1, binding = 6) uniform sampler3D hazyMie;
layout(set = 1, binding = 7) uniform sampler2D hazyIrradiance;

// Geometric source directions are independent of the refracted visible discs.
// clang-format off
layout(set = 1, binding = 8) uniform AtmosphereFrame {
    vec4 sun;
    vec4 moon;
    vec4 settings; // observer height (km), preset, automatic exposure, metered gain
} a;
// clang-format on

vec3 atmosphereCamera() {
    // Below-sea-level observers use the sea-level column. This clear-sky model
    // has a spherical surface; pressure/temperature only control refraction.
    return vec3(0., 0., 6360. + max(.001, a.settings.x));
}

vec3 sourceSky(vec3 direction, vec3 source, out vec3 transmittance) {
    AtmosphereParameters model = earthAtmosphere(SPECTRAL_GROUPS, int(a.settings.y));
    vec3 light;
    if (a.settings.y < .5) {
        light = GetSkyRadiance(model,
                               clearTransmittance,
                               clearScattering,
                               clearMie,
                               atmosphereCamera(),
                               direction,
                               0.,
                               source,
                               transmittance);
    } else {
        light = GetSkyRadiance(model,
                               hazyTransmittance,
                               hazyScattering,
                               hazyMie,
                               atmosphereCamera(),
                               direction,
                               0.,
                               source,
                               transmittance);
    }
    return max(light, vec3(0.));
}

vec3 diffuseIrradiance(vec3 source) {
    AtmosphereParameters model = earthAtmosphere(SPECTRAL_GROUPS, int(a.settings.y));
    float r = min(length(atmosphereCamera()), model.top_radius);
    vec3 light = a.settings.y < .5 ? GetIrradiance(model, clearIrradiance, r, source.z)
                                   : GetIrradiance(model, hazyIrradiance, r, source.z);
    return max(light, vec3(0.));
}

vec3 skyIrradiance() {
    return a.sun.w * diffuseIrradiance(a.sun.xyz) + a.moon.w * diffuseIrradiance(a.moon.xyz);
}

float adaptation() {
    return a.settings.w;
}

vec3 nightEmission(vec3 direction) {
    float horizon = exp(-max(0., asin(clamp(direction.z, -1., 1.))) * 2.);
    // Display approximations for airglow and local light pollution, not weather.
    return vec3(1e-6, 2.3e-6, 5.3e-6) + p.options.x * vec3(6e-5, 5e-5, 4e-5) * horizon;
}

vec3 physicalSky(vec3 direction, out vec3 transmittance) {
    transmittance = vec3(1.);
    if (p.sunAtmosphere.w < .5) {
        return vec3(0.);
    }
    vec3 light = a.sun.w * sourceSky(direction, a.sun.xyz, transmittance);
    if (a.moon.w > 1e-10) {
        vec3 unused;
        // Lunar transport uses a neutral, phase-scaled solar spectrum.
        light += a.moon.w * sourceSky(direction, a.moon.xyz, unused);
    }
    return light + nightEmission(direction);
}

vec3 safeHdr(vec3 value) {
    // Keep bright resolved sources finite in the half-float HDR attachment.
    return max(value, vec3(0.)) * min(1., 60000. / max(1., max(value.r, max(value.g, value.b))));
}
