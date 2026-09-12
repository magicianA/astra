#include "astro/events.hpp"
#include "astro/render_scene.hpp"
#include "json.hpp"
#include <chrono>
#include <fstream>
#include <iostream>
#include <stdexcept>

using namespace astro;

namespace {
int checks = 0;

void require(bool condition, const char* message) {
    ++checks;
    if (!condition) {
        throw std::runtime_error(message);
    }
}

template <class F> void rejects(F f, const char* message) {
    bool rejected = false;
    try {
        f();
    } catch (const std::exception&) {
        rejected = true;
    }
    require(rejected, message);
}

void geometry() {
    require(disc_overlap(1, 1, 3) == 0, "Disjoint discs");
    require(disc_overlap(1, 2, 0) == 1, "Total occultation");
    require(abs(disc_overlap(2, 1, 0) - .25) < 1e-12, "Annular obscuration");
    require(abs(disc_overlap(1, 1, 1) - (2 * pi / 3 - sqrt(3.) / 2) / pi) < 1e-12,
            "Analytic equal-circle overlap");
    require(lunar_sunlight({au_km, 0, 0}, {384400, 0, 0}) == 0, "Central umbra");
    require(lunar_sunlight({au_km, 0, 0}, {0, 384400, 0}) == 1, "Unobscured sunlight");
    for (auto kind :
         {ProjectionKind::Perspective, ProjectionKind::Fisheye, ProjectionKind::Stereographic}) {
        Camera camera;
        camera.projection = kind;
        camera.fov = rad;
        auto projection = camera.prepare();
        const double edge = kind == ProjectionKind::Fisheye
                                ? (camera.width + std::min(camera.width, camera.height)) / 2
                                : camera.width;
        const Vec3 centre = camera.unproject(edge + 100, camera.height / 2);
        require(!projection.project(centre).visible, "Offscreen point is culled");
        require(projection.project(centre, .26 * rad).visible, "Large disc limb remains visible");
        if (kind == ProjectionKind::Fisheye) {
            continue;
        }
        SkySnapshot sky;
        sky.scenario.ground = false;
        sky.scenario.atmosphere = false;
        Object moon;
        moon.id = 301;
        moon.body = 301;
        moon.observed = centre;
        moon.angular_radius = .26 * rad;
        moon.illumination = camera.up();
        moon.unocculted_lux = .1;
        sky.bodies.push_back(moon);
        require(render_scene(sky, sky, sky.scenario, camera).points.size() == 1,
                "Renderer retains overlapping Moon");
    }
    Camera camera;
    camera.fov = 120 * rad;
    camera.elevation = 0;
    camera.azimuth = 0;
    const auto p = camera.prepare();
    const auto centre = unit(p.forward + p.right * .6 + p.up * .4);
    const auto r = disc_tangent(centre, p.right, p.forward),
               u = disc_tangent(centre, p.up, p.forward);
    require(abs(dot(r, centre)) < 1e-12 && abs(dot(u, centre)) < 1e-12 && abs(dot(r, u)) < 1e-12,
            "Off-axis tangent frame is orthogonal");
    // A light at a known position in the transported disc basis must retain its limb angle.
    SkySnapshot sky;
    sky.scenario.ground = false;
    sky.scenario.atmosphere = false;
    Object body;
    body.id = 299;
    body.body = 299;
    body.observed = centre;
    body.angular_radius = .01;
    body.unocculted_lux = 1;
    body.illumination = unit(r + u * 2);
    sky.bodies.push_back(body);
    const auto scene = render_scene(sky, sky, sky.scenario, camera);
    require(scene.points.size() == 1 && abs(scene.points[0].limb - atan2(2., 1.)) < 1e-6,
            "Illumination is projected at the body, not the camera centre");
}

void scenes(const std::filesystem::path& directory) {
    auto horizon = make_horizon({{0, 5}, {90, 15}, {180, 5}, {270, -5}, {360, 5}}, "Test ridge");
    require(abs(horizon.altitude(45 * rad) / rad - 10) < 1e-6, "Horizon interpolation");
    require(abs(horizon.altitude(-45 * rad) - horizon.altitude(315 * rad)) < 1e-12,
            "Horizon wraps");
    require(!horizon.visible({0, 1, 0}) && horizon.visible({0, 1, 0}, 6 * rad),
            "Disc horizon clearance");
    rejects(
        [] {
            make_horizon({{0, 1}, {360, 2}}, "");
        },
        "Discontinuous seam rejected");
    rejects(
        [] {
            make_horizon({{0, 1}, {0, 2}}, "");
        },
        "Duplicate azimuth rejected");
    auto flat = make_horizon({{0, 5}, {360, 5}}, "");
    require(abs(flat.altitude(1) / rad - 5) < 1e-6, "Two seam samples make a flat profile");
    Scenario scene;
    scene.horizon = horizon;
    scene.compare = true;
    scene.constellations = true;
    const auto path = directory / "scene.json";
    save_scenario(scene, path);
    require(load_scenario(path) == scene, "All exploration settings survive roundtrip");
    auto original = nlohmann::json::parse(std::ifstream(path));
    for (const auto* key : {"photometry_model",
                            "exposure_model",
                            "display_model",
                            "surface_model",
                            "eclipse_model"}) {
        auto future = original;
        future[key] = "future-model";
        std::ofstream(path) << future;
        rejects(
            [&] {
                load_scenario(path);
            },
            "Unknown model rejected");
    }
    auto old = original;
    old["schema_version"] = 1;
    old["exposure_model"] = "hemisphere-moon-v2";
    old.erase("display_model");
    std::ofstream(path) << old;
    auto migrated = load_scenario(path);
    require(migrated.migrations.size() == 2, "Known legacy models report migration");
    save_scenario(migrated, path);
    require(load_scenario(path).migrations.empty(), "Migrated scene saves current model versions");
    std::ofstream(directory / "ridge.csv")
        << "azimuth,altitude\n0,5\n90,15\n180,5\n270,-5\n360,5\n";
    require(load_horizon(directory / "ridge.csv").degrees == horizon.degrees, "CSV horizon import");
}

void catalog(const SkyEngine& engine) {
    const auto& catalog = engine.catalog;
    require(catalog.constellations.size() == 88, "All constellation figures load");
    for (const auto& figure : catalog.constellations) {
        require(!figure.segments.empty(), "Figure has usable HIP vertices");
    }
    const auto sirius = catalog.search("  hip 32349 ");
    require(sirius.size() == 1 && catalog.stars[sirius[0]].hip == 32349,
            "Exact HIP indexed lookup");
    require(catalog.search("sIrIuS") == sirius, "Named-star search is case insensitive");
    auto gaia = std::find_if(catalog.stars.begin(), catalog.stars.end(), [](auto& s) {
        return s.flags & Gaia;
    });
    const auto result = catalog.search("Gaia DR3 " + std::to_string(gaia->id));
    require(result.size() == 1 && catalog.stars[result[0]].id == gaia->id, "64-bit Gaia ID lookup");
    const auto start = std::chrono::steady_clock::now();
    for (int i = 0; i < 100; ++i) {
        require(catalog.search("no-such-object-973").empty(), "No-match search");
    }
    const double ms =
        std::chrono::duration<double, std::milli>(std::chrono::steady_clock::now() - start)
            .count() /
        100;
    std::cout << "Indexed no-match search: " << ms << " ms/query\n";
    require(ms < 20, "Search does not scan millions of formatted star IDs");
    Scenario scene;
    scene.magnitude = -2;
    const auto star = engine.compute(scene, 0, nullptr, SkyEngine::Scope::FullSky, sirius[0]);
    require(star->stars.size() == 1, "Search can select stars below the visible magnitude limit");
}

void events(const SkyEngine& engine) {
    Scenario scene;
    scene.date = {2024, 4, 8, 0, 0, 0};
    scene.scale = TimeScale::UTC;
    // NASA path table central line, 18:42 UT, 32°17'N 96°42'W.
    // https://eclipse.gsfc.nasa.gov/SEpath/SEpath2001/SE2024Apr08Tpath.html
    scene.latitude = 32 + 17. / 60;
    scene.longitude = -(96 + 42. / 60);
    scene.height = 0;
    EventSearch query;
    query.days = 1;
    auto found = find_events(engine, scene, query);
    require(found.size() == 1 && found[0].type == "Total solar eclipse" && found[0].visible,
            "2024 path totality");
    const auto event = found[0];
    require(abs(event.peak - (18 * 3600 + 42 * 60)) < 60,
            "Solar maximum agrees with independent NASA path table");
    require(event.start && event.end && event.inner_start && event.inner_end &&
                *event.start < *event.inner_start && *event.inner_start < event.peak &&
                event.peak < *event.inner_end && *event.inner_end < *event.end,
            "Ordered solar contacts");
    auto peak =
        engine.compute(scene_at(scene, event.peak), 0, nullptr, SkyEngine::Scope::SolarSystem);
    require(peak->solar_visibility < .001 && peak->lunar.precise_orientation,
            "Totality reduces solar illumination");
    query.days = 1200. / 86400;
    found = find_events(engine, scene_at(scene, event.peak - 500), query);
    require(found.size() == 1 && abs(found[0].peak - 500) < 1,
            "Peak inside an interval shorter than one sample step");
    query.days = 1;
    found = find_events(engine, scene_at(scene, event.peak - 600), query);
    require(found.size() == 1 && abs(found[0].peak - 600) < 1, "Peak in first sampling interval");
    found = find_events(engine, scene_at(scene, event.peak - 86000), query);
    require(found.size() == 1 && abs(found[0].peak - 86000) < 1, "Peak in last sampling interval");
    query.kind = EventKind::Occultation;
    query.first = {301, {}};
    query.second = {10, {}};
    found = find_events(engine, scene, query);
    require(found.size() == 1 && found[0].obscuration > .99,
            "Occultation foreground covers background");
    std::swap(query.first, query.second);
    require(find_events(engine, scene, query).empty(),
            "Farther foreground cannot occult nearer body");
    query.kind = EventKind::LunarEclipse;
    scene.date = {2025, 3, 14, 0, 0, 0};
    scene.latitude = 37.7749;
    scene.longitude = -122.4194;
    found = find_events(engine, scene, query);
    require(found.size() == 1 && found[0].type == "Total lunar eclipse",
            "2025 March lunar totality");
    // NASA catalogue gives 06:59:56 TD and Delta T=75 s, hence 06:58:41 UT.
    // https://eclipse.gsfc.nasa.gov/LEcat5/LE2001-2100.html
    require(abs(found[0].peak - (6 * 3600 + 58 * 60 + 41)) < 120,
            "Lunar maximum within spherical-model tolerance");
    peak =
        engine.compute(scene_at(scene, found[0].peak), 0, nullptr, SkyEngine::Scope::SolarSystem);
    require(peak->lunar.solar_visibility < .001, "Earth shadow reaches Moon surface");
    const auto fixed = peak->lunar.fixed_from_enu;
    const auto product = fixed * fixed.transpose();
    for (int i = 0; i < 3; ++i) {
        for (int j = 0; j < 3; ++j) {
            require(abs(product.a[i][j] - double(i == j)) < 1e-12, "Lunar pose is orthonormal");
        }
    }
    scene.date = {-3000, 1, 2, 0, 0, 0};
    scene.scale = TimeScale::UT1;
    peak = engine.compute(scene, 0, nullptr, SkyEngine::Scope::SolarSystem);
    require(!peak->lunar.precise_orientation && std::isfinite(peak->lunar.sun_km.x),
            "Ancient lunar pose has explicit approximate quality");
    rejects(
        [&] {
            find_events(engine, scene, query, [] {
                return true;
            });
        },
        "Event search can cancel");
}

void planner(const SkyEngine& engine) {
    Scenario scene;
    scene.date = {2024, 3, 20, 0, 0, 0};
    scene.longitude = 0;
    scene.latitude = 0;
    scene.scale = TimeScale::UT1;
    scene.atmosphere = false;
    const auto plan = observing_plan(engine, scene, {10, {}});
    require(plan.samples.size() == 289 && plan.rises.size() == 1 && plan.sets.size() == 1,
            "Equatorial Sun rises and sets once");
    require(plan.rises[0] > 5 * 3600 && plan.rises[0] < 7 * 3600 && plan.sets[0] > 17 * 3600 &&
                plan.sets[0] < 19 * 3600,
            "Equinox solar rise/set near 06/18 UT at Greenwich meridian");
    scene.horizon.degrees.fill(30);
    const auto ridge = observing_plan(engine, scene, {10, {}});
    require(ridge.rises[0] > plan.rises[0] && ridge.sets[0] < plan.sets[0],
            "Terrain shortens visibility window");
    scene.horizon = {};
    scene.latitude = 90;
    scene.date = {2024, 6, 21, 0, 0, 0};
    const auto polar = observing_plan(engine, scene, {10, {}});
    require(polar.rises.empty() && polar.sets.empty(), "Circumpolar Sun has no rise/set");
    rejects(
        [&] {
            observing_plan(engine, scene, {}, [] {
                return true;
            });
        },
        "Planner can cancel");
    scene.date = {2016, 12, 31, 23, 59, 59};
    scene.scale = TimeScale::UTC;
    const auto leap = scene_at(scene, 1);
    require(leap.date.day == 31 && leap.date.second == 60,
            "Fixed-step clock includes UTC leap second");
    const auto next = scene_at(scene, 2);
    require(next.date.year == 2017 && next.date.day == 1 && next.date.second == 0,
            "Clock crosses UTC leap second");
}
} // namespace

int main(int argc, char** argv) {
    auto directory = std::filesystem::temp_directory_path() /
                     ("astra-exploration-" +
                      std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
    try {
        std::filesystem::create_directories(directory);
        geometry();
        scenes(directory);
        SkyEngine engine(argc > 1 ? argv[1] : ASTRA_SOURCE_DATA);
        catalog(engine);
        events(engine);
        planner(engine);
        std::filesystem::remove_all(directory);
        std::cout << checks << " exploration checks passed\n";
    } catch (const std::exception& e) {
        std::filesystem::remove_all(directory);
        std::cerr << "Exploration check " << checks << ": " << e.what() << '\n';
        return 1;
    }
}
