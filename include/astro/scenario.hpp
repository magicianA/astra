#pragma once
#include "camera.hpp"
#include "horizon.hpp"
#include "time.hpp"
#include <filesystem>

namespace astro {
struct Scenario {
    CivilDate date;
    TimeScale scale = TimeScale::LocalMean;
    double longitude = 116.4074, latitude = 39.9042, height = 45;
    bool julian = false, override_delta_t = false;
    double custom_delta_t = 69;
    double azimuth = 180, elevation = 35, roll = 0, fov = default_camera_fov / rad, magnitude = 7;
    bool atmosphere = true, ground = true, grid = false, labels = true;
    ProjectionKind projection = ProjectionKind::Perspective;
    bool milky_way = true;
    bool moon_surface = true, earthshine = true, constellations = false,
         constellation_labels = true;
    bool compare = false;
    int comparison_year = -3000;
    Horizon horizon;
    int atmosphere_preset = 0;
    bool auto_exposure = true;
    bool adaptive_exposure = false;
    double pressure = 1013.25, temperature = 15, extinction = .2, exposure = 1,
           light_pollution = .08;
    std::string location_name = "北京 Beijing";
    // Loading legacy settings is an explicit migration, not exact reproduction.
    std::vector<std::string> migrations;
    bool operator==(const Scenario&) const = default;
};

void validate(const Scenario&);
void save_scenario(const Scenario&,
                   const std::filesystem::path&,
                   const std::string& data_id = "",
                   const TimeContext* observation = nullptr,
                   double effective_exposure = 0);
Scenario load_scenario(const std::filesystem::path&, const std::string& expected_data_id = "");
} // namespace astro
