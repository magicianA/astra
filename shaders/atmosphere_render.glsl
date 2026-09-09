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
    vec4 photometry; // twilight gain, natural floor, pollution, diffuse-map radiance scale
    vec4 lunarIrradiance; // hemisphere integral of the calibrated lunar sky, lux
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
    return a.sun.w * a.photometry.x * diffuseIrradiance(a.sun.xyz) + a.lunarIrradiance.xyz;
}

float adaptation() {
    return a.settings.w;
}

vec3 nightEmission(vec3 direction) {
    float horizon = exp(-max(0., asin(clamp(direction.z, -1., 1.))) * 2.);
    // Neutral unresolved airglow floor; pollution is an extra zenith luminance.
    // The warm skyglow spectrum has unit photopic luminance.
    return vec3(a.photometry.y) +
           a.photometry.z * (1. + 2. * horizon) / (1. + 2. * exp(-PI)) * vec3(1.25, .97, .56102493);
}

vec3 moonSky(vec3 direction, vec3 viewTransmission) {
    vec3 unused;
    vec3 raw = a.moon.w * sourceSky(direction, a.moon.xyz, unused);
    float visible = clamp((a.moon.z + .0047) / .0094, 0., 1.);
    if (visible <= 0.) {
        return raw;
    }
    AtmosphereParameters model = earthAtmosphere(SPECTRAL_GROUPS, int(a.settings.y));
    float r = min(length(atmosphereCamera()), model.top_radius);
    vec3 moonTransmission = a.settings.y < .5 ? GetTransmittanceToTopAtmosphereBoundary(
                                                    model, clearTransmittance, r, max(0., a.moon.z))
                                              : GetTransmittanceToTopAtmosphereBoundary(
                                                    model, hazyTransmittance, r, max(0., a.moon.z));
    const vec3 y = vec3(.2126, .7152, .0722);
    float solarLux = dot(SOLAR_ILLUMINANCE, y);
    float moonT = dot(moonTransmission * SOLAR_ILLUMINANCE, y) / solarLux;
    float viewT = dot(viewTransmission * SOLAR_ILLUMINANCE, y) / solarLux;
    float cosine = clamp(dot(direction, a.moon.xyz), -1., 1.);
    float rho = degrees(acos(cosine));
    float scattering = pow(10., 5.36) * (1.06 + cosine * cosine) + pow(10., 6.15 - rho / 40.);
    // Empirical lunar angular normalization in cd/m²; see ING TN 127.
    float luminance = scattering * a.moon.w * solarLux / 10.76391 * moonT *
                      (1. - clamp(viewT, 0., 1.)) * 1e-5 / PI;
    vec3 colour = raw / max(dot(raw, y), 1e-20);
    return mix(raw, colour * luminance, visible);
}

vec3 physicalSky(vec3 direction, out vec3 transmittance) {
    transmittance = vec3(1.);
    if (p.sunAtmosphere.w < .5) {
        return vec3(0.);
    }
    vec3 light = a.sun.w * a.photometry.x * sourceSky(direction, a.sun.xyz, transmittance);
    if (a.moon.w > 1e-10) {
        light += moonSky(direction, transmittance);
    }
    return light + nightEmission(direction);
}

vec3 safeHdr(vec3 value) {
    // RGBA32F retains solar and lunar radiance; tone mapping happens after blending.
    return max(value, vec3(0.));
}
