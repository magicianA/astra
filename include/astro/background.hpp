#pragma once
#include "math.hpp"

namespace astro {
inline constexpr const char* background_id = "nasa-gaia-dr2-diffuse-16k-v1";

// Unit quaternion (x, y, z, w), mapping the current local sky into the
// Galactic coordinates of the fixed, distant Gaia background.
std::array<float, 4> galactic_rotation(const Mat3& celestial_to_enu);
} // namespace astro
