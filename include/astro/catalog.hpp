#pragma once
#include "math.hpp"
#include <cstdint>
#include <filesystem>
#include <optional>
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

struct Constellation {
    std::string id, name;
    std::vector<std::array<uint32_t, 2>> segments;
};

class Catalog {
    std::unordered_map<uint32_t, std::string> names_;
    std::vector<uint32_t> id_order_;
    std::vector<std::pair<uint32_t, uint32_t>> hip_order_;
    std::vector<std::pair<std::string, uint32_t>> search_names_;

public:
    std::vector<Star> stars;
    std::vector<Constellation> constellations;
    std::vector<bool> constellation_member;
    std::string data_id;
    explicit Catalog(const std::filesystem::path&);
    std::string name(const Star&) const;
    std::string id(const Star&) const;
    std::vector<uint32_t> search(std::string query, size_t limit = 6) const;
    std::optional<uint32_t> hip_index(uint32_t hip) const;
};

std::array<float, 3> star_color(double bv);
} // namespace astro
