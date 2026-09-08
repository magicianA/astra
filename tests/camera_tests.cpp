#include "astro/camera.hpp"
#include "astro/scenario.hpp"
#include "json.hpp"
#include <chrono>
#include <fstream>
#include <iostream>
#include <limits>
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

void same_direction(Vec3 a, Vec3 b, const char* message) {
    require(norm(a - b) < 1e-10, message);
}

void check_basis(const Camera& camera) {
    require(abs(norm(camera.forward()) - 1) < 1e-12 && abs(norm(camera.right()) - 1) < 1e-12 &&
                abs(norm(camera.up()) - 1) < 1e-12,
            "camera basis stays normalized");
    require(abs(dot(camera.forward(), camera.right())) < 1e-12 &&
                abs(dot(camera.forward(), camera.up())) < 1e-12 &&
                abs(dot(camera.up(), camera.right())) < 1e-12,
            "camera basis stays orthogonal");
}

void check_panning() {
    for (auto kind :
         {ProjectionKind::Perspective, ProjectionKind::Fisheye, ProjectionKind::Stereographic}) {
        for (auto size : {std::array{800., 800.}, {1440., 800.}, {800., 1440.}}) {
            for (double fov : {1., 90., maximum_fov(kind) / rad}) {
                for (double elevation : {-90., -89., -35., 0., 65., 89., 90.}) {
                    Camera start;
                    start.width = size[0];
                    start.height = size[1];
                    start.projection = kind;
                    start.fov = fov * rad;
                    start.elevation = elevation * rad;
                    start.azimuth = 359.9 * rad;
                    const double span = std::min(start.width, start.height);
                    const double x0 = start.width / 2 - .25 * span;
                    const double y0 = start.height / 2 - .2 * span;
                    for (auto offset : {std::array{0., 0.}, {.28, .3}, {-.3, .28}, {.4, -.2}}) {
                        const double x = start.width / 2 + offset[0] * span;
                        const double y = start.height / 2 + offset[1] * span;
                        Camera view = start;
                        view.pan(x0, y0, x, y);
                        check_basis(view);
                        require(view.roll == 0 && abs(view.right().z) < 1e-12,
                                "diagonal drags never tilt the horizon");
                        require(view.elevation >= -pi / 2 && view.elevation <= pi / 2,
                                "panning cannot turn the observer upside down");
                        Camera doubled = start;
                        doubled.width *= 2;
                        doubled.height *= 2;
                        doubled.pan(x0 * 2, y0 * 2, x * 2, y * 2);
                        same_direction(view.forward(),
                                       doubled.forward(),
                                       "angular motion is independent of window scale");
                        same_direction(view.right(),
                                       doubled.right(),
                                       "window scale cannot change the horizon orientation");
                        if (abs(view.elevation) < pi / 2) {
                            view.pan(x, y, x0, y0);
                            same_direction(view.forward(),
                                           start.forward(),
                                           "reversing an unclamped pan restores its direction");
                            same_direction(view.right(),
                                           start.right(),
                                           "reversing a pan restores its horizontal basis");
                        }
                    }
                    Camera horizontal = start;
                    horizontal.pan(x0, y0, x0 + 100, y0);
                    require(horizontal.elevation == start.elevation,
                            "horizontal drags only turn left or right");
                    Camera vertical = start;
                    vertical.pan(x0, y0, x0, y0 + 100);
                    same_direction(
                        vertical.right(), start.right(), "vertical drags cannot spin the horizon");
                    Camera unchanged = start;
                    unchanged.pan(x0, y0, x0, y0);
                    same_direction(unchanged.forward(),
                                   start.forward(),
                                   "a stationary pointer does not move the sky");
                }
            }
        }
    }
}

void check_centre_scale() {
    for (auto kind :
         {ProjectionKind::Perspective, ProjectionKind::Fisheye, ProjectionKind::Stereographic}) {
        for (double fov : {1., 90., 150.}) {
            Camera start;
            start.projection = kind;
            start.elevation = 0;
            start.fov = fov * rad;
            for (auto delta : {std::array{100., 0.}, {-100., 0.}, {0., 100.}, {0., -100.}}) {
                const double x = start.width / 2 + delta[0], y = start.height / 2 + delta[1];
                Camera view = start;
                view.pan(start.width / 2, start.height / 2, x, y);
                const auto pixel = view.project(start.forward());
                require(pixel.visible && hypot(pixel.x - x, pixel.y - y) < 1e-7,
                        "at the horizon, a central axis drag follows the pointer exactly");
            }
        }
    }
    Camera close_up, wide;
    close_up.elevation = wide.elevation = 0;
    close_up.fov = rad;
    wide.fov = 90 * rad;
    const auto initial = close_up.forward();
    close_up.pan(640, 400, 700, 400);
    wide.pan(640, 400, 700, 400);
    require(angle(close_up.forward(), initial) < angle(wide.forward(), initial) / 50,
            "high magnification slows the angular pan for precise positioning");
}

void check_zoom_round_trip() {
    for (auto kind :
         {ProjectionKind::Perspective, ProjectionKind::Fisheye, ProjectionKind::Stereographic}) {
        Camera start;
        start.projection = kind;
        start.fov = maximum_zoom_fov(kind);
        start.elevation = 15 * rad;
        start.width = 1440;
        start.height = 900;
        Camera view = start;
        for (int cycle = 0; cycle < 100; ++cycle) {
            for (double step : {0.125, 0.5, 1., 2., 4.}) {
                view.fov = zoom_fov(view.fov, kind, step);
            }
            for (double step : {-4., -2., -1., -0.5, -0.125}) {
                view.fov = zoom_fov(view.fov, kind, step);
            }
            require(abs(view.fov - start.fov) < 1e-12,
                    "repeated fractional wheel round trips restore the original field");
            for (auto pixel : {std::array{0., 0.}, {720., 450.}, {1440., 900.}}) {
                same_direction(view.unproject(pixel[0], pixel[1]),
                               start.unproject(pixel[0], pixel[1]),
                               "zoom round trips restore centre and edge sky directions");
            }
        }
        view.fov = zoom_fov(view.fov, kind, 10000);
        require(view.fov == rad, "a large zoom gesture stops at the minimum field");
        view.fov = zoom_fov(view.fov, kind, -10000);
        require(view.fov == start.fov,
                "zooming out after hitting the close-up limit restores the panorama");
        for (int i = 0; i < 100; ++i) {
            view.fov = zoom_fov(view.fov, kind, -0.25);
        }
        require(view.fov == start.fov, "outward wheel inertia cannot overshoot the panorama");
        view.fov = zoom_fov(view.fov, kind, 0.25);
        require(view.fov < start.fov, "zoom responds immediately after reaching the panorama");
    }
    require(zoom_fov(default_camera_fov, ProjectionKind::Perspective, -10) == default_camera_fov,
            "ordinary perspective zoom cannot enter the stretched ultra-wide field");
    require(maximum_zoom_fov(ProjectionKind::Stereographic) == pi &&
                maximum_zoom_fov(ProjectionKind::Fisheye) == pi,
            "explicit wide projection modes retain their full sky range");
}

void check_poles() {
    for (auto kind :
         {ProjectionKind::Perspective, ProjectionKind::Fisheye, ProjectionKind::Stereographic}) {
        for (double pole : {-1., 1.}) {
            Camera start;
            start.width = start.height = 800;
            start.elevation = 80 * rad * pole;
            start.fov = 90 * rad;
            start.projection = kind;
            Camera previous = start;
            for (int i = 1; i <= 300; ++i) {
                Camera view = start;
                view.pan(400, 400, 400, 400 + i * pole);
                check_basis(view);
                require(angle(view.forward(), previous.forward()) < .3 * rad &&
                            angle(view.up(), previous.up()) < .3 * rad,
                        "approaching a pole does not jump or flip the camera");
                same_direction(view.right(), start.right(), "pole panning never introduces roll");
                previous = view;
            }
            same_direction(previous.forward(), {0, 0, pole}, "vertical pan stops at the pole");
        }
    }
}

void check_wide_projection() {
    for (auto size : {std::array{800., 800.}, {1440., 900.}, {2560., 900.}, {800., 1440.}}) {
        for (double elevation : {-90., -15., 0., 5., 15., 35., 65., 90.}) {
            for (double fov : {1., 90., 110., 150., 180.}) {
                Camera camera;
                camera.projection = ProjectionKind::Stereographic;
                camera.width = size[0];
                camera.height = size[1];
                camera.elevation = elevation * rad;
                camera.fov = fov * rad;
                const auto projection = camera.prepare();
                const auto top = camera.unproject(camera.width / 2, 0);
                require(abs(angle(top, camera.forward()) - camera.fov / 2) < 1e-10,
                        "stereographic FOV measures the vertical field");
                for (double sx : {.001, .1, .5, .9, .999}) {
                    for (double sy : {.001, .1, .5, .9, .999}) {
                        const double x = sx * camera.width, y = sy * camera.height;
                        const auto ray = camera.unproject(x, y);
                        const auto point = projection.project(ray);
                        require(
                            point.visible && hypot(point.x - x, point.y - y) < 1e-8,
                            "wide corners remain projectable, including behind the front plane");
                        require(angle(ray, camera.forward()) <= projection.corner_angle(),
                                "the catalogue culling cone includes the entire wide viewport");

                        // Orthogonal unit tangents on the sky must stay orthogonal
                        // and equally sized on screen, even near the horizon/edges.
                        const auto a = unit(cross(ray, camera.up()));
                        const auto b = unit(cross(ray, a));
                        constexpr double step = 1e-6;
                        const auto ap = projection.project(ray * cos(step) + a * sin(step));
                        const auto am = projection.project(ray * cos(step) - a * sin(step));
                        const auto bp = projection.project(ray * cos(step) + b * sin(step));
                        const auto bm = projection.project(ray * cos(step) - b * sin(step));
                        const double ax = ap.x - am.x, ay = ap.y - am.y;
                        const double bx = bp.x - bm.x, by = bp.y - bm.y;
                        const double length_a = hypot(ax, ay), length_b = hypot(bx, by);
                        require(abs(length_a / length_b - 1) < 1e-7,
                                "wide sky features have no directional stretching");
                        require(abs((ax * bx + ay * by) / (length_a * length_b)) < 1e-7,
                                "wide sky features preserve their local angles");
                        require(abs(length_a / (2 * step * projection.maximum_scale(point)) - 1) <
                                    1e-7,
                                "resolved bodies use the same local scale as the sky");
                    }
                }
                require(!camera.project(camera.forward() * -1).visible,
                        "the antipode cannot produce a bogus point at the screen centre");
            }
        }
    }
}

void check_scene_storage() {
    const auto path =
        std::filesystem::temp_directory_path() /
        ("astra-camera-" +
         std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()) + ".json");
    Scenario scene;
    scene.roll = -137.25;
    save_scenario(scene, path);
    require(load_scenario(path).roll == scene.roll, "preserve explicit roll in existing scenes");
    auto json = nlohmann::json::parse(std::ifstream(path));
    json.erase("roll");
    std::ofstream(path) << json;
    require(load_scenario(path).roll == 0, "old scenes open with a level horizon");
    for (auto kind :
         {ProjectionKind::Stereographic, ProjectionKind::Perspective, ProjectionKind::Fisheye}) {
        scene.projection = kind;
        save_scenario(scene, path);
        require(load_scenario(path).projection == kind, "named projection survives scene export");
    }
    json.erase("projection");
    for (bool fish : {false, true}) {
        json["fisheye"] = fish;
        json["fov"] = 180;
        std::ofstream(path) << json;
        auto legacy = load_scenario(path);
        require(legacy.projection == (fish ? ProjectionKind::Fisheye : ProjectionKind::Perspective),
                "legacy scenes retain their original projection");
        require(legacy.fov == (fish ? 180. : 150.), "legacy effective FOV is preserved");
    }
    json["projection"] = "stereographic";
    json["fisheye"] = true;
    std::ofstream(path) << json;
    require(load_scenario(path).projection == ProjectionKind::Stereographic,
            "named projection takes precedence over the legacy flag");
    json["projection"] = "unknown";
    std::ofstream(path) << json;
    bool rejected_projection = false;
    try {
        load_scenario(path);
    } catch (const std::invalid_argument&) {
        rejected_projection = true;
    }
    require(rejected_projection, "invalid projection names are rejected");
    std::filesystem::remove(path);
    for (double invalid : {181., -181., std::numeric_limits<double>::quiet_NaN()}) {
        scene.roll = invalid;
        bool rejected = false;
        try {
            validate(scene);
        } catch (const std::invalid_argument&) {
            rejected = true;
        }
        require(rejected, "invalid camera roll is rejected");
    }
}
} // namespace

int main() {
    try {
        check_panning();
        check_centre_scale();
        check_zoom_round_trip();
        check_poles();
        check_wide_projection();
        check_scene_storage();
        Camera camera;
        require(camera.contains(1, 1), "rectangular sky accepts corner gestures");
        require(!camera.contains(-1, 400), "outside window does not start a gesture");
        camera.projection = ProjectionKind::Fisheye;
        require(!camera.contains(1, 1), "black corners of a fisheye do not start a gesture");
        require(camera.contains(camera.width / 2, camera.height / 2), "fisheye accepts its centre");
        std::cout << checks << " camera checks passed\n";
    } catch (const std::exception& error) {
        std::cerr << "Camera test failed after " << checks << " checks: " << error.what() << '\n';
        return 1;
    }
}
