#include "astro/scenario.hpp"
#include "astro/background.hpp"
#include "json.hpp"
#include <cmath>
#include <fstream>
#include <stdexcept>

namespace astro {
namespace {
const char* projection_name(ProjectionKind kind) {
    switch (kind) {
    case ProjectionKind::Perspective:
        return "perspective";
    case ProjectionKind::Fisheye:
        return "fisheye";
    case ProjectionKind::Stereographic:
        return "stereographic";
    }
    throw std::invalid_argument("Unknown sky projection");
}

ProjectionKind load_projection(const nlohmann::json& json) {
    if (!json.contains("projection")) {
        // Preserve the projection of scenes saved before named modes existed.
        return json.value("fisheye", false) ? ProjectionKind::Fisheye : ProjectionKind::Perspective;
    }
    const auto name = json.at("projection").get<std::string>();
    for (auto kind :
         {ProjectionKind::Perspective, ProjectionKind::Fisheye, ProjectionKind::Stereographic}) {
        if (name == projection_name(kind)) {
            return kind;
        }
    }
    throw std::invalid_argument("Unknown sky projection: " + name);
}
} // namespace

void validate(const Scenario& s) {
    projection_name(s.projection);
    auto range = [](double x, double a, double b) {
        return std::isfinite(x) && x >= a && x <= b;
    };
    if (!range(s.latitude, -90, 90) || !range(s.longitude, -180, 180) ||
        !range(s.height, -500, 100000) || !range(s.fov, 1, maximum_fov(s.projection) / rad) ||
        !range(s.elevation, -90, 90) || !range(s.azimuth, 0, 360) || !range(s.roll, -180, 180) ||
        !range(s.magnitude, -2, 16) || !range(s.pressure, 0, 1200) ||
        !range(s.temperature, -90, 60) || !range(s.exposure, .05, 10) ||
        !range(s.extinction, 0, 2) || !range(s.light_pollution, 0, 1) ||
        !range(s.custom_delta_t, -86400, 1e7)) {
        throw std::invalid_argument(
            "Scenario contains invalid location, camera or atmosphere values");
    }
}

void save_scenario(const Scenario& s,
                   const std::filesystem::path& p,
                   const std::string& id,
                   const TimeContext* observation) {
    validate(s);
    nlohmann::json j = {
        {"schema_version", 1},
        {"data_id", id},
        {"background_id", background_id},
        {"date",
         {s.date.year, s.date.month, s.date.day, s.date.hour, s.date.minute, s.date.second}},
        {"scale", int(s.scale)},
        {"julian", s.julian},
        {"longitude", s.longitude},
        {"latitude", s.latitude},
        {"height", s.height},
        {"location_name", s.location_name},
        {"override_delta_t", s.override_delta_t},
        {"delta_t", s.custom_delta_t},
        {"azimuth", s.azimuth},
        {"elevation", s.elevation},
        {"roll", s.roll},
        {"fov", s.fov},
        {"magnitude", s.magnitude},
        {"atmosphere", s.atmosphere},
        {"ground", s.ground},
        {"grid", s.grid},
        {"labels", s.labels},
        {"projection", projection_name(s.projection)},
        {"milky_way", s.milky_way},
        {"pressure", s.pressure},
        {"temperature", s.temperature},
        {"extinction", s.extinction},
        {"exposure", s.exposure},
        {"light_pollution", s.light_pollution}};
    if (observation) {
        const auto& time = *observation;
        j["observation"] = {
            {"jd_tt", {time.tt.day, time.tt.fraction}},
            {"jd_ut1", {time.ut1.day, time.ut1.fraction}},
            {"jd_tdb", {time.tdb.day, time.tdb.fraction}},
            {"delta_t_seconds", time.delta_t_seconds},
            {"extrapolated", time.extrapolated},
            {"time_quality", time.note},
            {"model_version", "astra-astrometry-v1"},
        };
    }
    if (!p.parent_path().empty()) {
        std::filesystem::create_directories(p.parent_path());
    }
    std::ofstream out(p.string() + ".partial");
    out << j.dump(2) << '\n';
    out.close();
    if (!out) {
        throw std::runtime_error("Cannot write scenario");
    }
    std::error_code ec;
    std::filesystem::rename(p.string() + ".partial", p, ec);
    if (ec) {
        std::filesystem::remove(p);
        std::filesystem::rename(p.string() + ".partial", p);
    }
}

Scenario load_scenario(const std::filesystem::path& p, const std::string& id) {
    std::ifstream in(p);
    if (!in) {
        throw std::runtime_error("Cannot open scenario");
    }
    auto j = nlohmann::json::parse(in);
    if (j.at("schema_version") != 1) {
        throw std::runtime_error("Unsupported scenario version");
    }
    if (!id.empty() && !j.value("data_id", "").empty() && j["data_id"] != id) {
        throw std::runtime_error("Scenario data version differs from the installed data pack");
    }
    const auto background = j.value("background_id", std::string(background_id));
    // Older scenes retain their camera/time settings and use the updated map.
    if (background != background_id && background != "gaia-edr3-diffuse-v1" &&
        background != "gaia-edr3-diffuse-v2") {
        throw std::runtime_error("Scenario background version differs from the installed map");
    }
    Scenario s;
    auto d = j.at("date");
    if (d.size() != 6) {
        throw std::runtime_error("Invalid date tuple");
    }
    s.date = {d[0], d[1], d[2], d[3], d[4], d[5]};
    int scale = j.at("scale");
    if (scale < 0 || scale > 4) {
        throw std::runtime_error("Invalid time scale");
    }
    s.scale = TimeScale(scale);
#define LOAD(name) s.name = j.value(#name, s.name)
    LOAD(longitude);
    LOAD(latitude);
    LOAD(height);
    LOAD(location_name);
    LOAD(override_delta_t);
    s.custom_delta_t = j.value("delta_t", 69.);
    LOAD(julian);
    LOAD(azimuth);
    LOAD(elevation);
    LOAD(roll);
    LOAD(fov);
    LOAD(magnitude);
    LOAD(atmosphere);
    LOAD(ground);
    LOAD(grid);
    LOAD(labels);
    s.projection = load_projection(j);
    if (!j.contains("projection") && s.fov <= 180) {
        // Legacy perspective silently capped its effective field at 150 degrees.
        s.fov = std::min(s.fov, maximum_fov(s.projection) / rad);
    }
    LOAD(milky_way);
    LOAD(pressure);
    LOAD(temperature);
    LOAD(extinction);
    LOAD(exposure);
    LOAD(light_pollution);
#undef LOAD
    validate(s);
    return s;
}
} // namespace astro
