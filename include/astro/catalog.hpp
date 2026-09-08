#pragma once
#include "math.hpp"
#include <cstdint>
#include <filesystem>
#include <string>
#include <unordered_map>
#include <vector>

namespace astro {
enum Quality : uint32_t {
    RVUnknown = 1,
    DistanceUnknown = 2,
    PMUnknown = 4,
    Multiple = 8,
    Gaia = 16,
    LowQuality = 32,
    PropagationWarning = 64,
    PositionMatch = 128
};

struct Star {
    uint64_t id;
    uint32_t hip, flags;
    double ra, dec, pmra, pmdec, parallax, rv, epoch;
    float magnitude, bv, ra_error, dec_error, pmra_error, pmdec_error;
};

static_assert(sizeof(Star) == 96);

class Catalog {
    std::unordered_map<uint32_t, std::string> names_;

public:
    std::vector<Star> stars;
    std::string data_id;
    explicit Catalog(const std::filesystem::path&);
    std::string name(const Star&) const;
    std::string id(const Star&) const;
};

std::array<float, 3> star_color(double bv);
} // namespace astro
