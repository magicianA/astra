#pragma once
#include "math.hpp"

namespace astro {
// Fraction of the luminous disc covered by the nearer opaque disc.
double disc_overlap(double luminous_radius, double opaque_radius, double separation);
// Visibility of sunlight from a point on/near the Moon, including the geometric
// terrestrial umbra and penumbra. Vectors are point-to-Sun and point-to-Earth, km.
double lunar_sunlight(Vec3 sun, Vec3 earth);

struct LunarSurface {
    Mat3 fixed_from_enu;
    Vec3 sun_km, earth_km;
    double solar_visibility = 1, earthshine_lux = 0;
    bool precise_orientation = false;
};
} // namespace astro
