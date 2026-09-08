#version 450
#extension GL_GOOGLE_include_directive : require
#include "scene.glsl"
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
    float sunAlt = asin(clamp(p.sunAtmosphere.z, -1., 1.));
    float daylight = smoothstep(-.13, .10, sunAlt);
    float twilight = exp(-pow((sunAlt + .07) / .11, 2.));
    float horizon = exp(-abs(altitude) * 5.);
    float toward = pow(max(0., dot(dir, p.sunAtmosphere.xyz)), 12.);
    vec3 night = vec3(.0003, .0007, .0016) +
                 p.options.x * vec3(.018, .015, .012) * exp(-max(0., altitude) * 2.);
    vec3 sky = night;
    if (p.options.z > .5) {
        vec4 rotation = p.galacticRotation;
        vec3 galactic =
            normalize(dir + 2. * cross(rotation.xyz, cross(rotation.xyz, dir) + rotation.w * dir));
        // NASA's Galactic equirectangular map: l=0 at the centre, l increases
        // toward the left, and north is at the top. Repeat only longitude.
        vec2 mapUV = vec2(.5 - atan(galactic.y, galactic.x) / 6.28318530718,
                          .5 - asin(clamp(galactic.z, -1., 1.)) / 3.14159265359);
        float visibility = 1.;
        if (p.sunAtmosphere.w > .5) {
            float darkness = 1. - smoothstep(-.3141593, -.1047198, sunAlt);
            float airMass = 1. / max(.08, dir.z);
            float extinction = pow(10., -.4 * p.options.w * airMass);
            float moonlight = max(0., p.moonGround.z) * p.options.y * p.options.y;
            visibility = darkness * extinction * exp(-6. * p.options.x) / (1. + 40. * moonlight);
        }
        // Longitude wraps at the map seam. Its derivative must wrap too, or
        // mip selection creates a blurred strip when the seam crosses the view.
        vec2 dx = dFdx(mapUV), dy = dFdy(mapUV);
        dx.x -= round(dx.x);
        dy.x -= round(dy.x);
        vec3 light = textureGrad(milkyWay, mapUV, dx, dy).rgb;
        float luminance = dot(light, vec3(.2126, .7152, .0722));
        // A restrained night-sky display, rather than the source image's
        // photographic colour. This remains a visual, uncalibrated intensity.
        light = mix(vec3(luminance), light, .35);
        sky += light * (.065 * visibility);
    }
    if (p.sunAtmosphere.w > .5) {
        sky += daylight * mix(vec3(.025, .10, .30), vec3(.22, .39, .61), horizon);
        sky += twilight * horizon * (.3 + .7 * toward) * vec3(.22, .057, .017);
        float lunar = max(0., p.moonGround.z) * p.options.y;
        sky += lunar * (.002 + pow(max(0., dot(dir, p.moonGround.xyz)), 16.) * .015) *
               vec3(.45, .60, .9);
    }
    if (p.moonGround.w > .5 && altitude < 0.) {
        sky = vec3(.003, .006, .008) * (1. + daylight * 20.) +
              vec3(.003, .012, .015) * exp(altitude * 30.);
    }
    // A narrow, unobtrusive geometric horizon; no invented geographic terrain.
    if (p.moonGround.w > .5) {
        sky += exp(-abs(altitude) * 1200.) * vec3(.009, .05, .06);
    }
    outputColor = vec4(sky, 1.);
}
