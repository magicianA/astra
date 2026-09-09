#version 450
#extension GL_GOOGLE_include_directive : require
#include "scene.glsl"
layout(location = 0) in vec4 positionRadiusKind;
layout(location = 1) in vec4 colorBrightness;
layout(location = 2) in vec4 phaseLimb;
layout(location = 0) out vec2 local;
layout(location = 1) flat out vec4 color;
layout(location = 2) flat out vec4 shape;

void main() {
    const vec2 corners[6] =
        vec2[6](vec2(-1, -1), vec2(1, -1), vec2(1, 1), vec2(-1, -1), vec2(1, 1), vec2(-1, 1));
    local = corners[gl_VertexIndex];
    if (positionRadiusKind.w > .5) {
        local *= 1. + 2. / max(positionRadiusKind.z, 1.);
    }
    vec2 offset = local * positionRadiusKind.z;
    if (positionRadiusKind.w > .5) {
        offset = projectDiscOffset(positionRadiusKind.xy, offset);
    }
    vec2 pos = positionRadiusKind.xy + offset;
    gl_Position = vec4(pos / p.viewport.xy * 2. - 1., 0, 1);
    color = colorBrightness;
    shape = vec4(positionRadiusKind.w, phaseLimb.xyz);
}
