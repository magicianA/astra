#include "astro/background.hpp"
#include "astro/camera.hpp"
#include "astro/sky.hpp"
#include "json.hpp"
#include <chrono>
#include <erfa.h>
#include <fstream>
#include <iostream>
#include <random>
#include <stdexcept>
using namespace astro;
static int checks = 0;

static void require(bool b, const char* message) {
    ++checks;
    if (!b) {
        throw std::runtime_error(message);
    }
}

static void close(double a, double b, double tol, const char* m) {
    require(std::abs(a - b) < tol, m);
}

template <class F> void rejects(F fn, const char* m) {
    bool threw = false;
    try {
        fn();
    } catch (const std::exception&) {
        threw = true;
    }
    require(threw, m);
}

int main(int argc, char** argv) {
    try {
        std::filesystem::path data = argc > 1 ? argv[1] : ASTRA_SOURCE_DATA;
        close(to_jd({2000, 1, 1, 12, 0, 0}).value(), 2451545., 1e-10, "J2000 calendar");
        for (int y : {-3000, -2000, -100, -1, 0, 1, 4, 1582, 1900, 2000, 2026, 4000, 6999}) {
            for (bool julian : {false, true}) {
                for (int m = 1; m <= 12; m++) {
                    CivilDate c{y, m, 17, 22, 15, 36.25};
                    auto back = from_jd(to_jd(c, julian), julian);
                    require(c.year == back.year && c.month == back.month && c.day == back.day &&
                                c.hour == back.hour && c.minute == back.minute,
                            "calendar round trip");
                    close(c.second, back.second, 1e-5, "calendar seconds");
                }
            }
        }
        require(valid_date({0, 2, 29}), "astronomical year zero leap year");
        require(!valid_date({1900, 2, 29}), "Gregorian 1900 non-leap");
        require(valid_date({1900, 2, 29}, true), "Julian 1900 leap");
        rejects(
            [] {
                to_jd({2026, 2, 30});
            },
            "invalid civil date");
        rejects(
            [] {
                check_time_range(to_jd({7000, 1, 1, 0, 0, 0}));
            },
            "upper bound exclusive");
        check_time_range(to_jd({-3000, 1, 1, 0, 0, 0}));
        close(delta_t(2000), 63.86, 1e-9, "NASA Delta T 2000");
        close(delta_t(-1000), 25427.68, 1e-6, "NASA long Delta T");
        EarthOrientationData eop(data / "time/finals2000A.all");
        auto now = make_time(CivilDate{2026, 9, 4, 12, 0, 0}, TimeScale::UTC, 0, eop);
        require(now.eop.available, "modern EOP present");
        close(now.delta_t_seconds, 69.184 - now.eop.dut1, 1e-5, "TT-UT1");
        const auto leap_second =
            make_time(CivilDate{2016, 12, 31, 23, 59, 60}, TimeScale::UTC, 0, eop);
        const auto next_second = make_time(CivilDate{2017, 1, 1, 0, 0, 0}, TimeScale::UTC, 0, eop);
        close(((next_second.tt.day - leap_second.tt.day) +
               (next_second.tt.fraction - leap_second.tt.fraction)) *
                  86400,
              1,
              1e-8,
              "UTC leap second is one SI second before midnight");
        rejects(
            [&] {
                make_time(CivilDate{2016, 12, 30, 23, 59, 60}, TimeScale::UTC, 0, eop);
            },
            "reject spurious leap second");
        rejects(
            [&] {
                make_time(CivilDate{6000, 1, 1, 0, 0, 0}, TimeScale::UTC, 0, eop);
            },
            "far future is not real UTC");
        auto lmt = make_time(CivilDate{2000, 1, 2, 22, 0, 0}, TimeScale::LocalMean, 120, eop);
        close(lmt.ut1.value(), to_jd({2000, 1, 2, 14, 0, 0}).value(), 1e-9, "east longitude LMT");
        auto t1 = make_time(CivilDate{1000, 1, 1, 0, 0, 0}, TimeScale::TT, 0, eop, true, 100);
        auto t2 = make_time(CivilDate{1000, 1, 1, 0, 0, 0}, TimeScale::TT, 0, eop, true, 200);
        close(t1.tt.value(), t2.tt.value(), 1e-9, "fixed TT");
        close(
            (t1.ut1.value() - t2.ut1.value()) * 86400, 100, 1e-4, "DeltaT changes UT1 at fixed TT");
        Orientation orientation(data / "time/cio.bin");
        Mat3 modern = orientation.celestial(now);
        double x, y, s;
        eraXys06a(now.tt.day, now.tt.fraction, &x, &y, &s);
        Mat3 ref;
        eraC2ixys(x + now.eop.dx, y + now.eop.dy, s, ref.a);
        for (int i = 0; i < 3; i++) {
            for (int j = 0; j < 3; j++) {
                close(modern.a[i][j], ref.a[i][j], 1e-12, "modern orientation reference");
            }
        }
        for (int year = -3000; year < 7000; year += 100) {
            auto t = make_time(CivilDate{year, 7, 1, 0, 0, 0}, TimeScale::UT1, 0, eop);
            auto r = orientation.terrestrial(t);
            auto identity = r * r.transpose();
            for (int i = 0; i < 3; i++) {
                for (int j = 0; j < 3; j++) {
                    close(identity.a[i][j],
                          i == j ? 1. : 0.,
                          1e-12,
                          "long-term orientation orthogonality");
                }
            }
        }
        std::mt19937 gen(42);
        std::uniform_real_distribution<double> dist(0, 1);
        // Compare the float quaternion used by the shader with ERFA's direct
        // spherical conversion, across epochs and both geographic poles.
        for (int year : {-3000, 0, 2000, 6999}) {
            const auto time = make_time({year, 7, 1, 0, 0, 0}, TimeScale::UT1, 0, eop);
            for (double latitude : {-90., 0., 39.9, 90.}) {
                for (double longitude : {-70., 0., 116.4}) {
                    const auto local =
                        enu_basis(longitude * rad, latitude * rad) * orientation.terrestrial(time);
                    const auto q = galactic_rotation(local);
                    const Vec3 axis{q[0], q[1], q[2]};
                    for (int i = 0; i < 8; ++i) {
                        const double ra = 2 * pi * dist(gen), dec = asin(2 * dist(gen) - 1);
                        const auto direction = local * sphere(ra, dec);
                        const auto rotated =
                            direction + cross(axis, cross(axis, direction) + direction * q[3]) * 2;
                        double longitude_galactic, latitude_galactic;
                        eraIcrs2g(ra, dec, &longitude_galactic, &latitude_galactic);
                        require(angle(rotated, sphere(longitude_galactic, latitude_galactic)) <
                                    3e-7,
                                "background orientation matches independent ERFA coordinates");
                        close(norm(rotated),
                              1,
                              3e-7,
                              "background quaternion preserves sky directions");
                    }
                }
            }
        }
        gen.seed(42);
        {
            const auto path =
                std::filesystem::temp_directory_path() /
                ("astra-background-" +
                 std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()) +
                 ".json");
            Scenario scene;
            scene.milky_way = false;
            save_scenario(scene, path);
            require(!load_scenario(path).milky_way,
                    "preserve the Milky Way toggle in saved scenes");
            auto json = nlohmann::json::parse(std::ifstream(path));
            require(json.at("background_id") == background_id,
                    "export identifies its background map");
            json["background_id"] = "gaia-edr3-diffuse-v1";
            std::ofstream(path) << json;
            require(!load_scenario(path).milky_way,
                    "v1 scenes migrate to the detailed map while retaining their settings");
            json["background_id"] = "gaia-edr3-diffuse-v2";
            std::ofstream(path) << json;
            require(load_scenario(path).fov == scene.fov,
                    "the 4K to 16K background upgrade does not change the view");
            json.erase("milky_way");
            json.erase("background_id");
            std::ofstream(path) << json;
            require(load_scenario(path).milky_way,
                    "legacy scenes enable the background by default");
            json["background_id"] = "unsupported-background-version";
            std::ofstream(path) << json;
            rejects(
                [&] {
                    load_scenario(path);
                },
                "reject a mismatched background version");
            std::filesystem::remove(path);
        }
        for (auto kind : {ProjectionKind::Perspective,
                          ProjectionKind::Fisheye,
                          ProjectionKind::Stereographic}) {
            for (double fov : {1., 60., 120., 180.}) {
                Camera c;
                c.projection = kind;
                c.fov = fov * rad;
                for (int i = 0; i < 500; i++) {
                    double sx = c.width * (.1 + .8 * dist(gen)),
                           sy = c.height * (.1 + .8 * dist(gen));
                    auto dir = c.unproject(sx, sy);
                    auto p = c.project(dir);
                    if (p.visible) {
                        close(p.x, sx, 1e-7, "camera x round trip");
                        close(p.y, sy, 1e-7, "camera y round trip");
                    }
                }
            }
        }
        require(refraction(0, 1013, 15) > 0, "horizon refraction positive");
        close(refraction(45 * rad, 0, 15), 0, 1e-15, "vacuum no refraction");
        for (double temperature : {-90., 15., 60.}) {
            for (int degrees = -90; degrees <= 90; degrees += 3) {
                const double altitude = degrees * rad;
                const double azimuth = 2 * pi * dist(gen);
                const Vec3 original{
                    sin(azimuth) * cos(altitude), cos(azimuth) * cos(altitude), sin(altitude)};
                const double corrected_altitude =
                    std::min(pi / 2, altitude + refraction(altitude, 1200, temperature));
                const Vec3 reference{sin(azimuth) * cos(corrected_altitude),
                                     cos(azimuth) * cos(corrected_altitude),
                                     sin(corrected_altitude)};
                const auto rotated = refract(original, 1200, temperature);
                require(angle(rotated, reference) < 1e-12,
                        "vertical-plane refraction matches angular reference");
                close(norm(rotated), 1, 1e-12, "refraction preserves unit direction");
            }
        }
        SkyEngine engine(data);
        const auto markab_count = std::count_if(
            engine.catalog.stars.begin(), engine.catalog.stars.end(), [](const Star& star) {
                return star.hip == 113963;
            });
        require(markab_count == 1, "Markab has one entry after missing Gaia cross-match fallback");
        // Independent Horizons observer table; different implementation and EOP
        // conventions permit a small residual, without asserting physical truth.
        auto fixture_path =
            std::filesystem::path(__FILE__).parent_path() / "fixtures/horizons-greenwich-2026.json";
        std::ifstream fixture_stream(fixture_path);
        require(bool(fixture_stream), "Horizons fixture available");
        nlohmann::json fixture;
        fixture_stream >> fixture;
        Scenario reference_scene;
        reference_scene.date = {2026, 9, 4, 12, 0, 0};
        reference_scene.scale = TimeScale::UTC;
        reference_scene.longitude = 0;
        reference_scene.latitude = 51.4779;
        reference_scene.height = 46;
        reference_scene.atmosphere = false;
        auto computed = engine.compute(reference_scene);
        for (const auto& reference : fixture["bodies"]) {
            auto object = std::find_if(
                computed->bodies.begin(), computed->bodies.end(), [&](const Object& body) {
                    return body.body == reference["body"].get<int>();
                });
            require(object != computed->bodies.end(), "Horizons target found");
            const double az = reference["azimuth_deg"].get<double>() * rad;
            const double alt = reference["altitude_deg"].get<double>() * rad;
            const Vec3 expected{sin(az) * cos(alt), cos(az) * cos(alt), sin(alt)};
            const double error = 2 * asin(norm(expected - object->observed) / 2) / arcsec;
            require(error < fixture["tolerance_arcsec"].get<double>(), "Horizons agreement");
        }
        Scenario beijing;
        auto before_moonrise = engine.compute(beijing, 0, nullptr, SkyEngine::Scope::SolarSystem);
        require(before_moonrise->stars.empty() && before_moonrise->bodies.size() == 9,
                "solar-system search skips the star catalog");
        require(before_moonrise->moon_altitude < 0, "default Beijing Moon is below horizon");
        auto moon_view = engine.next_moon_view(beijing);
        require(moon_view.has_value(), "find a night-time Moon view from default scene");
        require(moon_view->longitude == beijing.longitude &&
                    moon_view->latitude == beijing.latitude && moon_view->scale == beijing.scale &&
                    moon_view->julian == beijing.julian,
                "Moon suggestion preserves observer and clock conventions");
        const double elapsed_days = to_jd(moon_view->date).value() - to_jd(beijing.date).value();
        require(elapsed_days > 0 && elapsed_days < 1, "Beijing Moon becomes visible tonight");
        auto moon_sky = engine.compute(*moon_view);
        require(moon_sky->moon_altitude >= 8 * rad && moon_sky->sun_altitude <= -6 * rad,
                "Moon suggestion has a raised Moon and dark sky");
        close(moon_view->elevation * rad,
              moon_sky->moon_altitude,
              1e-10,
              "Moon suggestion centers the camera on the Moon");
        std::atomic<uint64_t> cancelled{2};
        require(!engine.next_moon_view(beijing, 1, &cancelled),
                "Moon search respects cancellation");
        Scenario polar_day = beijing;
        polar_day.latitude = 90;
        polar_day.date = {2026, 6, 1, 12, 0, 0};
        require(!engine.next_moon_view(polar_day), "no fake night-time Moon view during polar day");

        const auto star_epoch = engine.compute(beijing);
        auto playback_end = beijing;
        playback_end.date = from_jd(to_jd(beijing.date).add_seconds(540));
        const auto exact_end = engine.compute(playback_end);
        double largest_playback_error = 0;
        for (const auto& star : star_epoch->stars) {
            if (star.magnitude > 3 || star.altitude() < 10 * rad) {
                continue;
            }
            auto exact = std::find_if(
                exact_end->stars.begin(), exact_end->stars.end(), [&](const Object& value) {
                    return value.id == star.id;
                });
            require(exact != exact_end->stars.end(), "bright playback reference is present");
            const auto displayed = observe_star(star, *exact_end);
            const double error = angle(displayed.observed, exact->observed) / arcsec;
            largest_playback_error = std::max(largest_playback_error, error);
            require(error < .2,
                    "cached stellar astrometry stays subpixel over a 3600x refresh interval");
        }
        for (int direction : {-1, 1}) {
            auto clock = beijing;
            Object reference_star;
            reference_star.icrs = {1, 0, 0};
            Vec3 previous_star, previous_moon;
            double minimum_step = 1, maximum_step = 0;
            for (int frame = 0; frame <= 60; ++frame) {
                clock.date = from_jd(to_jd(beijing.date).add_seconds(direction * frame * 9.));
                clock.atmosphere = false;
                auto instant = engine.compute(clock, 0, nullptr, SkyEngine::Scope::SolarSystem);
                auto displayed = observe_star(reference_star, *instant);
                auto moon = std::find_if(
                    instant->bodies.begin(), instant->bodies.end(), [](const Object& value) {
                        return value.body == 301;
                    });
                if (frame > 0) {
                    const double step = angle(previous_star, displayed.observed);
                    minimum_step = std::min(minimum_step, step);
                    maximum_step = std::max(maximum_step, step);
                    require(step > 1e-5, "Earth rotation advances every presentation frame");
                    require(angle(previous_moon, moon->observed) > 1e-6,
                            "Moon position advances every presentation frame");
                }
                previous_star = displayed.observed;
                previous_moon = moon->observed;
            }
            require(maximum_step / minimum_step < 1.001,
                    "uniform playback has no stepped stellar motion");
        }
        std::cout << "Playback cached-star maximum residual: " << largest_playback_error
                  << " arcsec\n";

        Scenario scene;
        scene.scale = TimeScale::UT1;
        scene.atmosphere = false;
        scene.magnitude = 6.5;
        for (int year : {-3000, 0, 1969, 2000, 2026, 4000, 6999}) {
            scene.date = {year, 1, 2, 12, 0, 0};
            auto sky = engine.compute(scene);
            require(sky->bodies.size() == 9, "all solar system bodies");
            require(sky->stars.size() > 3000, "real star catalogue");
            for (auto& o : sky->bodies) {
                require(std::isfinite(o.altitude()) && std::isfinite(o.magnitude) &&
                            o.distance_au > 0,
                        "finite planetary state");
                close(norm(o.observed), 1, 1e-12, "unit direction");
            }
            std::cout << year << ": " << sky->stars.size() << " stars, " << sky->compute_ms
                      << " ms\n";
        }
        scene.date = {2026, 9, 4, 22, 0, 0};
        scene.latitude = 90;
        auto north = engine.compute(scene);
        scene.latitude = -90;
        auto south = engine.compute(scene);
        require(std::isfinite(north->bodies[0].azimuth()) &&
                    std::isfinite(south->bodies[0].azimuth()),
                "polar observer");
        {
            Ephemeris temporary(data);
            require(norm(temporary.state(399, now.tdb).position) > 1e7, "second kernel user");
        }
        require(engine.compute(scene)->bodies.size() == 9,
                "kernel survives another user's release");
        std::cout << "Passed " << checks << " checks\n";
        return 0;
    } catch (const std::exception& e) {
        std::cerr << "FAIL: " << e.what() << '\n';
        return 1;
    }
}
