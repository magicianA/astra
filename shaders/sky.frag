#version 450
#extension GL_GOOGLE_include_directive : require
#include "atmosphere_render.glsl"
#include "features.glsl"
layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 outputColor;
layout(set = 0, binding = 0) uniform sampler2D milkyWay;

void main() {
    if (outsideFisheye(uv)) {
        outputColor = vec4(.0015, .002, .004, 1);
        return;
    }
    vec3 dir = skyDirection(uv);
    float altitude = asin(clamp(dir.z, -1., 1.));
    if (terrainOccludes(dir)) {
        // A neutral Lambertian horizon, without invented geographic terrain.
        vec3 irradiance = p.sunAtmosphere.w > .5 ? skyIrradiance() : vec3(0.);
        if (p.sunAtmosphere.w > .5) {
            AtmosphereParameters model = earthAtmosphere(SPECTRAL_GROUPS, int(a.settings.y));
            float r = min(length(atmosphereCamera()), model.top_radius);
            vec3 sunlight = a.settings.y < .5
                                ? GetTransmittanceToSun(model, clearTransmittance, r, a.sun.z)
                                : GetTransmittanceToSun(model, hazyTransmittance, r, a.sun.z);
            irradiance += sunlight * SOLAR_ILLUMINANCE * max(0., a.sun.z) * a.sun.w;
            vec3 moonlight = a.settings.y < .5
                                 ? GetTransmittanceToSun(model, clearTransmittance, r, a.moon.z)
                                 : GetTransmittanceToSun(model, hazyTransmittance, r, a.moon.z);
            irradiance += moonlight * SOLAR_ILLUMINANCE * max(0., a.moon.z) * a.moon.w;
            float pollutionMean = (1.5 + .5 * exp(-PI)) / (1. + 2. * exp(-PI));
            irradiance += PI * vec3(a.photometry.y + a.photometry.z * pollutionMean);
        }
        vec3 ground = vec3(.055, .065, .07) * irradiance / PI + vec3(1e-6, 2e-6, 2.7e-6);
        outputColor = vec4(safeHdr(ground * adaptation()), 1.);
        return;
    }
    vec3 transmittance;
    vec3 sky = physicalSky(dir, transmittance);
    float exposure = adaptation();
    if (p.options.z > .5) {
        vec4 rotation = p.galacticRotation;
        vec3 galactic =
            normalize(dir + 2. * cross(rotation.xyz, cross(rotation.xyz, dir) + rotation.w * dir));
        // NASA's Galactic equirectangular map: l=0 at the centre, l increases
        // toward the left, and north is at the top. Repeat only longitude.
        vec2 mapUV = vec2(.5 - atan(galactic.y, galactic.x) / 6.28318530718,
                          .5 - asin(clamp(galactic.z, -1., 1.)) / 3.14159265359);
        // Longitude wraps at the map seam. Its derivative must wrap too, or
        // mip selection creates a blurred strip when the seam crosses the view.
        vec2 dx = dFdx(mapUV), dy = dFdy(mapUV);
        dx.x -= round(dx.x);
        dy.x -= round(dy.x);
        vec3 light = textureGrad(milkyWay, mapUV, dx, dy).rgb;
        sky += light * a.photometry.w * transmittance;
    }
    sky *= exposure;
    outputColor = vec4(safeHdr(sky), 1.);
}
