layout(set = 2, binding = 0) uniform sampler2D moonAlbedo;
layout(set = 2, binding = 1) uniform sampler2D moonHeight;

layout(set = 2, binding = 2) uniform Features {
    vec4 fixedRows[3];
    vec4 moonSun;
    vec4 moonEarth;
    vec4 settings;
    vec4 viewport;
    vec4 horizon[180];
}

features;

vec3 moonFixed(vec3 enu) {
    return vec3(dot(features.fixedRows[0].xyz, enu),
                dot(features.fixedRows[1].xyz, enu),
                dot(features.fixedRows[2].xyz, enu));
}

float terrainAltitude(vec3 direction) {
    float index = mod(atan(direction.x, direction.y) / (2. * PI) * 720. + 720., 720.);
    int a = int(floor(index)), b = (a + 1) % 720;
    return mix(features.horizon[a / 4][a % 4], features.horizon[b / 4][b % 4], fract(index)) * PI /
           180.;
}

bool terrainOccludes(vec3 direction) {
    return p.moonGround.w > .5 && asin(clamp(direction.z, -1., 1.)) < terrainAltitude(direction);
}

vec2 lunarUV(vec3 normal) {
    return vec2(.5 + atan(normal.y, normal.x) / (2. * PI),
                .5 - asin(clamp(normal.z, -1., 1.)) / PI);
}

float heightTexel(ivec2 pixel) {
    ivec2 size = textureSize(moonHeight, 0);
    pixel.x = (pixel.x % size.x + size.x) % size.x;
    pixel.y = clamp(pixel.y, 0, size.y - 1);
    vec2 bytes = round(texelFetch(moonHeight, pixel, 0).rg * 255.);
    return dot(bytes, vec2(256., 1.)) * .0005 - 10.;
}

float lunarHeight(vec2 uv) {
    vec2 p = uv * vec2(textureSize(moonHeight, 0)) - .5;
    ivec2 i = ivec2(floor(p));
    vec2 f = fract(p);
    return mix(mix(heightTexel(i), heightTexel(i + ivec2(1, 0)), f.x),
               mix(heightTexel(i + ivec2(0, 1)), heightTexel(i + ivec2(1)), f.x),
               f.y);
}

float solarVisibility(vec3 sun, vec3 earth) {
    float a = asin(695700. / length(sun)), b = asin(6378.137 * 1.01 / length(earth));
    vec3 s = normalize(sun), e = normalize(earth);
    float d = atan(length(cross(s, e)), dot(s, e));
    if (d >= a + b) {
        return 1.;
    }
    if (d <= abs(a - b)) {
        return 1. - min(1., b * b / (a * a));
    }
    float aa = a * a, bb = b * b, dd = d * d;
    float x = acos(clamp((dd + aa - bb) / (2. * d * a), -1., 1.));
    float y = acos(clamp((dd + bb - aa) / (2. * d * b), -1., 1.));
    float triangle = sqrt(max(0., (-d + a + b) * (d + a - b) * (d - a + b) * (d + a + b)));
    return clamp(1. - (aa * x + bb * y - .5 * triangle) / (PI * aa), 0., 1.);
}

vec3 lunarSurface(vec3 n, float meanLuminance, float phase) {
    vec2 uv = lunarUV(n);
    vec3 light = normalize(features.moonSun.xyz);
    vec3 perturbed = n;
    float height = 0., shadow = 1.;
    vec3 albedo = vec3(1.);
    if (features.settings.x > .5) {
        albedo = texture(moonAlbedo, uv).rgb / .21012569;
        height = lunarHeight(uv);
        vec2 texel = 1. / vec2(textureSize(moonHeight, 0));
        float longitude = (uv.x - .5) * 2. * PI;
        vec3 east = vec3(-sin(longitude), cos(longitude), 0.);
        vec3 north = cross(n, east);
        float dx = (lunarHeight(uv + vec2(texel.x, 0.)) - lunarHeight(uv - vec2(texel.x, 0.))) /
                   (4. * PI * texel.x * 1737.4 * max(.02, length(n.xy)));
        float dy = (lunarHeight(uv - vec2(0., texel.y)) - lunarHeight(uv + vec2(0., texel.y))) /
                   (2. * PI * texel.y * 1737.4);
        perturbed = normalize(n - east * dx - north * dy);
        float elevation = dot(n, light);
        if (elevation > -.02 && elevation < .3) {
            vec3 origin = n * (1737.4 + height + .08);
            // March through the measured height field toward the Sun. A larger
            // footprint farther away keeps horizon shadows affordable at 4K.
            float distance = 1.5;
            for (int i = 0; i < 16; ++i) {
                vec3 point = origin + light * distance;
                float ground = 1737.4 + lunarHeight(lunarUV(normalize(point)));
                if (length(point) < ground) {
                    shadow = 0.;
                    break;
                }
                distance *= 1.4;
            }
        }
    }
    vec3 point = n * (1737.4 + height);
    float visibility =
        solarVisibility(features.moonSun.xyz - point, features.moonEarth.xyz - point);
    vec3 eclipse = vec3(visibility) + (1. - visibility) * vec3(.00255, .00062, .00021);
    float ca = 2. * phase - 1., angle = acos(clamp(ca, -1., 1.));
    float phaseIntegral = (sin(angle) + (PI - angle) * ca) / PI;
    float shade = max(0., dot(perturbed, light)) * shadow / max(1e-7, (2. / 3.) * phaseIntegral);
    float earthshine =
        features.settings.y * .12 / PI * max(0., dot(perturbed, normalize(features.moonEarth.xyz)));
    return albedo * (meanLuminance * shade * eclipse + vec3(earthshine));
}
