// Vulkan does not guarantee linear filtering for RGBA32F on every device.
// Specialize away the fallback on devices with native full-float filtering.
layout(constant_id = 0) const bool manualAtmosphereFiltering = false;

vec4 filteredTexture(sampler2D source, vec2 uv) {
    if (!manualAtmosphereFiltering) {
        return textureLod(source, uv, 0.);
    }
    ivec2 size = textureSize(source, 0);
    vec2 position = clamp(uv * vec2(size) - .5, vec2(0.), vec2(size - 1));
    ivec2 low = ivec2(floor(position));
    ivec2 high = min(low + 1, size - 1);
    vec2 f = fract(position);
    return mix(mix(texelFetch(source, low, 0), texelFetch(source, ivec2(high.x, low.y), 0), f.x),
               mix(texelFetch(source, ivec2(low.x, high.y), 0), texelFetch(source, high, 0), f.x),
               f.y);
}

vec4 filteredTexture(sampler3D source, vec3 uv) {
    if (!manualAtmosphereFiltering) {
        return textureLod(source, uv, 0.);
    }
    ivec3 size = textureSize(source, 0);
    vec3 position = clamp(uv * vec3(size) - .5, vec3(0.), vec3(size - 1));
    ivec3 low = ivec3(floor(position));
    vec3 f = fract(position);
    vec4 result = vec4(0.);
    for (int z = 0; z < 2; ++z) {
        for (int y = 0; y < 2; ++y) {
            for (int x = 0; x < 2; ++x) {
                ivec3 offset = ivec3(x, y, z);
                vec3 weight = mix(1. - f, f, vec3(offset));
                result += texelFetch(source, min(low + offset, size - 1), 0) * weight.x * weight.y *
                          weight.z;
            }
        }
    }
    return result;
}
