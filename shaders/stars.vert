#version 450
#extension GL_GOOGLE_include_directive : require
#include "scene.glsl"
layout(location = 0) in vec4 positionRadiusKind;
layout(location = 1) in vec4 colorBrightness;
layout(location = 2) in vec4 phaseLimb;
layout(location = 0) out vec2 local;
layout(location = 1) flat out vec4 color;
layout(location = 2) flat out vec4 shape;
layout(location = 3) flat out vec3 centre;
layout(location = 4) flat out vec3 tangentRight;
layout(location = 5) flat out vec3 tangentUp;

void main() {
    const vec2 corners[6] =
        vec2[6](vec2(-1, -1), vec2(1, -1), vec2(1, 1), vec2(-1, -1), vec2(1, 1), vec2(-1, 1));
    local = corners[gl_VertexIndex];
    if (positionRadiusKind.w > .5 && positionRadiusKind.w < 3.5) {
        local *= 1. + 2. / max(positionRadiusKind.z, 1.);
    }
    vec2 offset = local * positionRadiusKind.z;
    if (positionRadiusKind.w > .5 && positionRadiusKind.w < 3.5) {
        offset = projectDiscOffset(positionRadiusKind.xy, offset);
    }
    vec2 pos = positionRadiusKind.xy + offset;
    if (positionRadiusKind.w > 3.5) {
        vec2 end = vec2(positionRadiusKind.z, phaseLimb.x);
        vec2 delta = end - positionRadiusKind.xy;
        vec2 normal = vec2(-delta.y, delta.x) / max(length(delta), .0001);
        pos = mix(positionRadiusKind.xy, end, (local.x + 1.) * .5) + normal * local.y;
    }
    centre = skyDirection(positionRadiusKind.xy / p.viewport.xy);
    float denominator = max(1e-6, 1. + dot(centre, p.forwardProjection.xyz));
    tangentRight = p.rightScale.xyz - (centre + p.forwardProjection.xyz) *
                                          (dot(p.rightScale.xyz, centre) / denominator);
    tangentUp = p.upAspect.xyz -
                (centre + p.forwardProjection.xyz) * (dot(p.upAspect.xyz, centre) / denominator);
    gl_Position = vec4(pos / p.viewport.xy * 2. - 1., 0, 1);
    color = colorBrightness;
    shape = vec4(positionRadiusKind.w, phaseLimb.xyz);
}
