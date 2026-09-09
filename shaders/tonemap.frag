#version 450
layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 outputColor;
layout(set = 0, binding = 0) uniform sampler2D sky;

// clang-format off
layout(push_constant) uniform Settings {
    float exposure;
    float adaptation;
} p;
// clang-format on

void main() {
    vec3 hdr = max(texelFetch(sky, ivec2(gl_FragCoord.xy), 0).rgb, vec3(0.));
    float luminance = dot(hdr, vec3(.2126, .7152, .0722));
    float physical = luminance / max(p.adaptation, 1e-9);
    // Approximate loss of colour discrimination at low luminance. This is a
    // display transform, not a spectral CIE mesopic-observer calculation.
    float colour = smoothstep(log(.005), log(5.), log(max(physical, 1e-9)));
    hdr = mix(vec3(luminance), hdr, colour) * p.exposure;
    float y = luminance * p.exposure;
    vec3 mapped = hdr / (1. + y);
    // Desaturate only out-of-gamut highlights while retaining their luminance.
    float peak = max(mapped.r, max(mapped.g, mapped.b));
    float grey = y / (1. + y);
    mapped = mix(vec3(grey), mapped, min(1., (1. - grey) / max(peak - grey, 1e-9)));
    vec3 srgb = mix(
        12.92 * mapped, 1.055 * pow(mapped, vec3(1. / 2.4)) - .055, step(vec3(.0031308), mapped));
    outputColor = vec4(srgb, 1.);
}
