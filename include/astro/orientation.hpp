#pragma once
#include "math.hpp"
#include "time.hpp"
#include <filesystem>
#include <vector>

namespace astro {
// Versioned CIO integral cache. Angles are radians, time is days TT from J2000.
class Orientation {
    double start_{}, step_{};
    std::vector<double> s_;

public:
    explicit Orientation(const std::filesystem::path&);
    static Vec3 pole(double days);
    static void generate(const std::filesystem::path&, double step_days = 4);
    double cio(double days) const;
    Mat3 celestial(const TimeContext&) const;
    Mat3 terrestrial(const TimeContext&) const;
};
} // namespace astro
