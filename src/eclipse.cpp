#include "astro/eclipse.hpp"

namespace astro {
double disc_overlap(double a, double b, double d) {
    if (a <= 0 || b <= 0 || d >= a + b) {
        return 0;
    }
    if (d <= abs(a - b)) {
        return std::min(1., b * b / (a * a));
    }
    const double aa = a * a, bb = b * b, dd = d * d;
    const double x = acos(std::clamp((dd + aa - bb) / (2 * d * a), -1., 1.));
    const double y = acos(std::clamp((dd + bb - aa) / (2 * d * b), -1., 1.));
    const double triangle =
        sqrt(std::max(0., (-d + a + b) * (d + a - b) * (d - a + b) * (d + a + b)));
    return std::clamp((aa * x + bb * y - .5 * triangle) / (pi * aa), 0., 1.);
}

double lunar_sunlight(Vec3 sun, Vec3 earth) {
    // Spherical Earth with a 1% shadow-radius enlargement for the atmosphere.
    const double solar = asin(std::clamp(695700. / norm(sun), 0., 1.));
    const double terrestrial = asin(std::clamp(6378.137 * 1.01 / norm(earth), 0., 1.));
    return 1 - disc_overlap(solar, terrestrial, angle(sun, earth));
}
} // namespace astro
