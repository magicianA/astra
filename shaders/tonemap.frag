#version 450
layout(location = 0) in vec2 uv;
layout(location = 0) out vec4 outputColor;
layout(set = 0, binding = 0) uniform sampler2D sky;

// clang-format off
layout(push_constant) uniform Settings {
    float exposure;
} p;
// clang-format on

void main() {
    vec3 hdr = texture(sky, uv).rgb * p.exposure;
    vec3 mapped = 1. - exp(-hdr);
    outputColor = vec4(pow(mapped, vec3(1. / 2.2)), 1.);
}
