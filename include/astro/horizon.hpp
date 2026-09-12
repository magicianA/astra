#pragma once
#include "math.hpp"
#include <filesystem>
#include <string>
#include <vector>

namespace astro {
struct Horizon {
    static constexpr size_t sample_count = 720;
    std::array<float, sample_count> degrees{};
    std::string name;
    double altitude(double azimuth) const;
    bool visible(Vec3 direction, double radius = 0) const;
    bool operator==(const Horizon&) const = default;
};

Horizon make_horizon(std::vector<std::array<double, 2>> points, std::string name);
Horizon load_horizon(const std::filesystem::path&);
} // namespace astro
