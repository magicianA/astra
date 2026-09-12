#pragma once
#include "sky.hpp"

namespace astro {
struct StarInstance {
    float x, y, radius, kind;
    float r, g, b, brightness;
    float phase, limb, pad0 = 0, pad1 = 0;
};

struct RenderScene {
    Camera camera;
    Vec3 sun{0, 0, -1}, moon{0, 0, -1};
    Vec3 geometric_sun{0, 0, -1}, geometric_moon{0, 0, -1};
    float solar_flux = 1, lunar_flux = 0, height_km = 0;
    int atmosphere_preset = 0;
    bool auto_exposure = true;
    bool adaptive_exposure = false;
    bool atmosphere = true, ground = true;
    bool milky_way = false;
    float extinction = .2;
    std::array<float, 4> galactic_rotation{0, 0, 0, 1};
    float pollution = .05, moon_phase = 0, exposure = 1;
    std::vector<StarInstance> points;
    Horizon horizon;
    LunarSurface lunar;
    bool moon_surface = true;
};

RenderScene render_scene(const SkySnapshot&,
                         const SkySnapshot&,
                         const Scenario&,
                         const Camera&,
                         const Catalog* = nullptr);
Vec3 disc_tangent(Vec3 centre, Vec3 axis, Vec3 forward);
} // namespace astro
