// Eight vec4s fit Vulkan's guaranteed 128-byte push-constant budget.
// clang-format off
layout(push_constant) uniform Scene {
    vec4 rightScale;
    vec4 upAspect;
    vec4 forwardProjection;
    vec4 sunAtmosphere;
    vec4 moonGround;
    vec4 options;
    vec4 viewport;
    vec4 galacticRotation;
} p;
// clang-format on

vec3 skyDirection(vec2 uv) {
    vec2 q = (uv - .5) * p.viewport.xy / p.rightScale.w;
    q.y = -q.y;
    if (p.forwardProjection.w > 1.5) {
        float r2 = dot(q, q) * .25;
        return ((1. - r2) * p.forwardProjection.xyz + q.x * p.rightScale.xyz +
                q.y * p.upAspect.xyz) /
               (1. + r2);
    }
    if (p.forwardProjection.w > .5) {
        float theta = length(q);
        return cos(theta) * p.forwardProjection.xyz +
               (theta > 1e-6 ? sin(theta) / theta : 1.) *
                   (q.x * p.rightScale.xyz + q.y * p.upAspect.xyz);
    }
    return normalize(p.forwardProjection.xyz + p.rightScale.xyz * q.x + p.upAspect.xyz * q.y);
}

bool outsideFisheye(vec2 uv) {
    return p.forwardProjection.w > .5 && p.forwardProjection.w < 1.5 &&
           length((uv - .5) * p.viewport.xy / p.rightScale.w) > p.upAspect.w;
}

// Apply the local projection Jacobian only to resolved discs. Unresolved
// stars keep their fixed pixel PSF, independent of location in the viewport.
vec2 projectDiscOffset(vec2 centre, vec2 offset) {
    vec2 q = (centre - .5 * p.viewport.xy) / p.rightScale.w;
    float r2 = dot(q, q);
    if (p.forwardProjection.w > 1.5) {
        return offset * (1. + r2 * .25);
    }
    if (r2 < 1e-8) {
        return offset;
    }
    float radius = sqrt(r2);
    bool fish = p.forwardProjection.w > .5;
    float radial = fish ? 1. : 1. + r2;
    float tangential = fish ? radius / sin(radius) : sqrt(radial);
    return tangential * offset + q * dot(q, offset) * ((radial - tangential) / r2);
}
