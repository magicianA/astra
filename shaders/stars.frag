#version 450
#extension GL_GOOGLE_include_directive : require
#include "atmosphere_render.glsl"
layout(location = 0) in vec2 local;
layout(location = 1) flat in vec4 color;
layout(location = 2) flat in vec4 shape;
layout(location = 0) out vec4 outputColor;

void main() {
    vec2 footprint = fwidth(local);
    vec2 uv = gl_FragCoord.xy / p.viewport.zw;
    if (outsideFisheye(uv) || (p.moonGround.w > .5 && skyDirection(uv).z < 0.)) {
        discard;
    }
    float rr = dot(local, local);
    if (rr > 1.) {
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
        light = 4. * log(1. + light / 4.);
        outputColor = vec4(light * alpha, 0.);
    } else {
        float edge = 1. - smoothstep(.94, 1., sqrt(rr));
        float z = sqrt(max(0., 1. - rr));
        float ca = 2. * shape.y - 1.;
        float sa = sqrt(max(0., 1. - ca * ca));
        vec3 light = vec3(cos(shape.z) * sa, sin(shape.z) * sa, ca);
        float illumination = shape.x < 1.5 ? 1. : max(0., dot(vec3(local.x, -local.y, z), light));
        // Opaque unlit hemisphere masks stars; brightness variation is illumination.
        float shade = shape.x < 1.5 ? (.75 + .25 * z) : pow(illumination, .6);
        // Compress resolved lunar/planetary surface brightness for display so
        // night adaptation does not flatten the terminator into a white disc.
        // Atmospheric illumination still uses the independent physical flux.
        float discExposure = shape.x < 1.5 ? exposure : min(exposure, .0006);
        vec3 surface = color.rgb * color.a * shade * transmittance * discExposure;
        outputColor = vec4(safeHdr(surface + foreground * exposure) * edge, edge);
    }
}
