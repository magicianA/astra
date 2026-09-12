#version 450
#extension GL_GOOGLE_include_directive : require
#include "atmosphere_render.glsl"
#include "features.glsl"
layout(location = 0) in vec2 local;
layout(location = 1) flat in vec4 color;
layout(location = 2) flat in vec4 shape;
layout(location = 3) flat in vec3 centre;
layout(location = 4) flat in vec3 tangentRight;
layout(location = 5) flat in vec3 tangentUp;
layout(location = 0) out vec4 outputColor;

void main() {
    vec2 footprint = fwidth(local);
    vec2 uv = (gl_FragCoord.xy - features.viewport.xy) / p.viewport.zw;
    if (outsideFisheye(uv) || terrainOccludes(skyDirection(uv))) {
        discard;
    }
    if (shape.x > 3.5) {
        float edge = clamp((1. - abs(local.y)) / max(fwidth(local.y), .001), 0., 1.);
        outputColor = vec4(color.rgb * color.a * edge, .45 * edge);
        return;
    }
    float rr = dot(local, local);
    if (rr > 1. && shape.x < .5) {
        discard;
    }
    vec3 transmittance;
    vec3 foreground = physicalSky(skyDirection(uv), transmittance);
    float exposure = adaptation();
    if (shape.x < .5) {
        // A fixed 0.5 logical-pixel sigma inside a six-sigma quad keeps bright
        // stars compact. Include pixel coverage while preserving total energy
        // during subpixel motion; the quad edge is far into the negligible tail.
        const float baseVariance = 1. / 36.;
        float variance = baseVariance + dot(footprint, footprint) / 24.;
        float alpha = exp(-rr / (2. * variance)) * baseVariance / variance;
        // Point sources add light without replacing the diffuse sky behind them.
        vec3 light = color.rgb * color.a * transmittance * exposure;
        outputColor = vec4(light * alpha, 0.);
    } else {
        float width = max(fwidth(sqrt(rr)), 1e-5);
        float edge = 1. - smoothstep(1. - .5 * width, 1. + .5 * width, sqrt(rr));
        if (edge == 0.) {
            discard;
        }
        float z = sqrt(max(0., 1. - rr));
        float ca = 2. * shape.y - 1.;
        float sa = sqrt(max(0., 1. - ca * ca));
        vec3 light = vec3(cos(shape.z) * sa, sin(shape.z) * sa, ca);
        float illumination = shape.x < 1.5 ? 1. : max(0., dot(vec3(local.x, -local.y, z), light));
        // Opaque unlit hemisphere masks stars; brightness variation is illumination.
        float phaseAngle = acos(clamp(ca, -1., 1.));
        float lambertPhase = (sin(phaseAngle) + (PI - phaseAngle) * ca) / PI;
        // Normalize each surface profile so its disc integral equals the source
        // illuminance, including phase and distance. No separate Moon exposure.
        float shade = shape.x < 1.5 ? (.75 + .25 * z) / (11. / 12.)
                                    : illumination / max(1e-7, (2. / 3.) * lambertPhase);
        vec3 surface = color.rgb * color.a * shade;
        if (shape.x > 2.5) {
            vec3 normal = normalize(local.x * tangentRight - local.y * tangentUp - z * centre);
            surface = lunarSurface(moonFixed(normal), color.a, shape.y);
        }
        surface *= transmittance * exposure;
        outputColor = vec4(safeHdr(surface + foreground * exposure) * edge, edge);
    }
}
