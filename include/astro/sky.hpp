#pragma once
#include "catalog.hpp"
#include "orientation.hpp"
#include "scenario.hpp"
#include <atomic>
#include <memory>
#include <optional>

namespace astro {
struct Object {
    uint64_t id{};
    uint32_t catalog_index = UINT32_MAX, hip{}, flags{};
    int body{};
    Vec3 geometric, observed, icrs;
    double magnitude{}, distance_au{}, angular_radius{}, phase = 1, bright_limb_angle{},
                                                         formal_error_arcsec{};
    std::array<float, 3> color{1, 1, 1};
    double illuminance_lux{}; // Above the atmosphere; native catalogue magnitude stays separate.

    double azimuth() const {
        return wrap(atan2(observed.x, observed.y));
    }

    double altitude() const {
        return asin(std::clamp(observed.z, -1., 1.));
    }
};

struct SkySnapshot {
    Scenario scenario;
    TimeContext time;
    uint64_t generation{};
    Mat3 celestial_to_enu;
    std::vector<Object> stars, bodies;
    std::vector<size_t> label_stars;
    std::string data_id;
    double compute_ms{}, sun_altitude = -1, moon_altitude = -1, moon_phase{};
    size_t catalog_count{};
};

struct State {
    Vec3 position, velocity;
};

class Ephemeris {
    std::vector<std::string> kernels_;

public:
    explicit Ephemeris(const std::filesystem::path&);
    ~Ephemeris();
    Ephemeris(const Ephemeris&) = delete;
    State state(int body, JulianDate tdb) const;
};

class SkyEngine {
    EarthOrientationData eop_;
    Orientation orientation_;
    Ephemeris ephemeris_;

public:
    enum class Scope { FullSky, SolarSystem };

    Catalog catalog;
    explicit SkyEngine(const std::filesystem::path&);
    std::shared_ptr<SkySnapshot> compute(const Scenario&,
                                         uint64_t generation = 0,
                                         const std::atomic<uint64_t>* latest = nullptr,
                                         Scope scope = Scope::FullSky) const;
    std::optional<Scenario> next_moon_view(const Scenario&,
                                           uint64_t generation = 0,
                                           const std::atomic<uint64_t>* latest = nullptr) const;
    // Civil dawn/dusk: geometric solar centre crossing -6 degrees.
    // Direction is -1 (previous) or +1 (next); search up to 370 days.
    std::optional<Scenario> twilight_view(const Scenario&,
                                          bool dawn,
                                          int direction,
                                          uint64_t generation = 0,
                                          const std::atomic<uint64_t>* latest = nullptr) const;

    const EarthOrientationData& eop() const {
        return eop_;
    }
};

double refraction(double geometric_altitude, double pressure, double temperature);
Vec3 refract(Vec3, double pressure, double temperature);
std::string body_name(int body);
std::string quality_text(const Object&);
// Reuse slowly changing stellar astrometry with the current frame's Earth orientation.
Object observe_star(const Object&, const SkySnapshot& frame);
} // namespace astro
