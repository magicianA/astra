#include "astro/app.hpp"
#include "astro/background.hpp"
#include "astro/renderer.hpp"
#include "astro/text.hpp"
#include "astro/ui.hpp"
#include "json.hpp"
#include <algorithm>
#include <atomic>
#include <chrono>
#include <condition_variable>
#include <ctime>
#include <fstream>
#include <imgui.h>
#include <imgui_impl_sdl3.h>
#include <imgui_impl_vulkan.h>
#include <imgui_internal.h>
#include <iomanip>
#include <iostream>
#include <mutex>
#include <optional>
#include <sstream>
#include <thread>

namespace astro {
namespace {
struct Worker {
    std::mutex mutex;
    std::condition_variable wake;
    std::atomic<uint64_t> generation{0};
    std::optional<Scenario> pending;
    std::shared_ptr<SkySnapshot> snapshot;
    std::shared_ptr<SkyEngine> engine;
    std::string error, status = "正在读取星表与历表…";
    bool stop = false, busy = true;
    bool pending_moon = false, adopt_scene = false;
    std::thread thread;

    Worker(std::filesystem::path data) {
        thread = std::thread([this, data] {
            try {
                auto ready = std::make_shared<SkyEngine>(data);
                {
                    std::lock_guard lock(mutex);
                    engine = ready;
                    status = "";
                    busy = false;
                }
                for (;;) {
                    Scenario scene;
                    uint64_t id;
                    bool seek_moon;
                    {
                        std::unique_lock lock(mutex);
                        wake.wait(lock, [&] {
                            return stop || pending.has_value();
                        });
                        if (stop) {
                            break;
                        }
                        scene = *pending;
                        seek_moon = pending_moon;
                        pending_moon = false;
                        pending.reset();
                        id = generation.load();
                        busy = true;
                    }
                    try {
                        if (seek_moon) {
                            auto found = ready->next_moon_view(scene, id, &generation);
                            if (!found) {
                                std::lock_guard lock(mutex);
                                if (generation.load() == id) {
                                    error = "未来 35 "
                                            "天或数据有效期内没有合适的夜间观月时刻。可调整日期或地"
                                            "点。";
                                }
                                busy = false;
                                continue;
                            }
                            scene = *found;
                        }
                        auto result = ready->compute(scene, id, &generation);
                        std::lock_guard lock(mutex);
                        if (result && generation.load() == id) {
                            snapshot = result;
                            adopt_scene = seek_moon;
                            error.clear();
                        }
                        busy = false;
                    } catch (const std::exception& e) {
                        std::lock_guard lock(mutex);
                        if (generation.load() == id) {
                            error = e.what();
                        }
                        busy = false;
                    }
                }
            } catch (const std::exception& e) {
                std::lock_guard lock(mutex);
                error = e.what();
                status = "数据未就绪";
                busy = false;
            }
        });
    }

    ~Worker() {
        {
            std::lock_guard lock(mutex);
            stop = true;
            ++generation;
        }
        wake.notify_one();
        thread.join();
    }

    void request(const Scenario& s, bool seek_moon = false) {
        {
            std::lock_guard lock(mutex);
            pending = s;
            pending_moon = seek_moon;
            adopt_scene = false;
            ++generation;
        }
        wake.notify_one();
    }
};

ImVec4 rgba(int r, int g, int b, float a = 1) {
    return {r / 255.f, g / 255.f, b / 255.f, a};
}

ImU32 color(int r, int g, int b, int a = 255) {
    return IM_COL32(r, g, b, a);
}

constexpr ImGuiWindowFlags fixed = ImGuiWindowFlags_NoTitleBar | ImGuiWindowFlags_NoMove |
                                   ImGuiWindowFlags_NoResize | ImGuiWindowFlags_NoSavedSettings;

const Object* selected_object(const SkySnapshot& sky, uint64_t id, bool body) {
    if (id == 0) {
        return nullptr;
    }
    const auto& v = body ? sky.bodies : sky.stars;
    auto it = std::find_if(v.begin(), v.end(), [&](auto& o) {
        return o.id == id;
    });
    return it == v.end() ? nullptr : &*it;
}

Camera camera_for(const Scenario& s, int w, int h) {
    Camera c;
    c.width = w;
    c.height = h;
    c.azimuth = s.azimuth * rad;
    c.elevation = s.elevation * rad;
    c.roll = s.roll * rad;
    c.fov = s.fov * rad;
    c.projection = s.projection;
    return c;
}

bool over_ui(float x, float y) {
    // WantCaptureMouse belongs to the previous frame. Hit-test the event's
    // position so a fast click-and-drag on a slider never grabs the sky too.
    ImGuiWindow* window = nullptr;
    ImGui::FindHoveredWindowEx({x, y}, false, &window, nullptr);
    return window != nullptr;
}

void draw_grid(const Camera& c, bool ground) {
    auto* draw = ImGui::GetBackgroundDrawList();
    auto line = [&](auto point, int count) {
        ScreenPoint prev;
        for (int i = 0; i <= count; i++) {
            auto v = point(i);
            auto p = c.project(v);
            if (p.visible && prev.visible && hypot(p.x - prev.x, p.y - prev.y) < 80) {
                draw->AddLine({float(prev.x), float(prev.y)},
                              {float(p.x), float(p.y)},
                              color(57, 106, 123, 52),
                              .7f);
            }
            prev = p;
        }
    };
    for (int alt = ground ? 0 : -60; alt <= 75; alt += 15) {
        line(
            [&](int i) {
                double az = i * 2. * rad;
                return Vec3{sin(az) * cos(alt * rad), cos(az) * cos(alt * rad), sin(alt * rad)};
            },
            180);
    }
    for (int az = 0; az < 360; az += 30) {
        line(
            [&](int i) {
                double alt = (ground ? 0 : -90) + i * (ground ? 90. : 180.) / 90.;
                return Vec3{
                    sin(az * rad) * cos(alt * rad), cos(az * rad) * cos(alt * rad), sin(alt * rad)};
            },
            90);
    }
    const char* labels[] = {"北 N", "东 E", "南 S", "西 W"};
    for (int i = 0; i < 4; i++) {
        auto p = c.project({sin(i * pi / 2), cos(i * pi / 2), .005});
        if (p.visible) {
            draw_moving_text(
                *draw, {float(p.x - 16), float(p.y + 8)}, color(130, 197, 202, 200), labels[i]);
        }
    }
}

RenderScene render_scene(const SkySnapshot& sky,
                         const SkySnapshot& stars,
                         const Scenario& view,
                         const Camera& camera) {
    RenderScene result;
    result.camera = camera;
    result.milky_way = view.milky_way;
    result.extinction = float(view.extinction);
    result.galactic_rotation = galactic_rotation(sky.celestial_to_enu);
    auto& s = sky.scenario;
    result.atmosphere = s.atmosphere;
    result.ground = s.ground;
    result.pollution = float(s.light_pollution);
    result.exposure = float(view.exposure);
    result.moon_phase = float(sky.moon_phase);
    for (auto& o : sky.bodies) {
        if (o.body == 10) {
            result.sun = o.observed;
        }
        if (o.body == 301) {
            result.moon = o.observed;
        }
    }
    double day = s.atmosphere ? std::clamp((sky.sun_altitude / rad + 12) / 18, 0., 1.) : 0.;
    const auto projection = camera.prepare();
    const float limb =
        float(atan2(dot(result.sun, projection.up), dot(result.sun, projection.right)));
    auto add = [&](const Object& o) {
        auto p = projection.project(o.observed);
        if (!p.visible) {
            return;
        }
        double alt = o.altitude();
        if (s.ground && alt + o.angular_radius < 0) {
            return;
        }
        double mag = o.magnitude;
        if (s.atmosphere) {
            double air = 1 / std::max(.035, sin(std::max(0., alt)));
            mag += s.extinction * (air - 1);
        }
        bool disk = o.body && (o.body == 301 || o.body == 10 ||
                               o.angular_radius * projection.maximum_scale(p) > 1.3);
        if (!disk && mag > s.magnitude - day * 11 - s.light_pollution * 2) {
            return;
        }
        // Unresolved sources share a compact screen-space point-spread function.
        // Its six-sigma support is independent of magnitude and camera zoom.
        float radius = disk ? float(o.angular_radius * projection.scale) : 3.f;
        float strength = disk ? (o.body == 10 ? 9.f : 1.5f)
                              : float(4 * std::log1p(.42 * pow(10., .4 * (3 - mag)) / 4));
        if (!disk) {
            // Fade through the detection limit instead of toggling stars on/off.
            const double limit = s.magnitude - day * 11 - s.light_pollution * 2;
            const double visibility = std::clamp((limit - mag) / .75, 0., 1.);
            strength *= float(visibility * visibility * (3 - 2 * visibility));
        }
        result.points.push_back({float(p.x),
                                 float(p.y),
                                 radius,
                                 disk ? (o.body == 10 ? 1.f : 2.f) : 0.f,
                                 o.color[0],
                                 o.color[1],
                                 o.color[2],
                                 strength,
                                 float(o.phase),
                                 limb,
                                 0,
                                 0});
    };
    const double limiting_magnitude = s.magnitude - day * 11 - s.light_pollution * 2;
    const double refraction_margin = s.atmosphere ? refraction(-rad, s.pressure, s.temperature) : 0;
    const double corner_angle = projection.corner_angle();
    const double minimum_dot = cos(std::min(pi, corner_angle + refraction_margin));
    const double lowest_altitude = -sin(std::min(pi / 2, refraction_margin));
    for (const auto& source : stars.stars) {
        if (source.magnitude > limiting_magnitude) {
            continue;
        }
        const Vec3 geometric = sky.celestial_to_enu * source.icrs;
        // A cone around the full viewport, enlarged by the maximum refraction,
        // safely rejects off-screen stars before expensive trigonometry.
        if (dot(geometric, projection.forward) < minimum_dot ||
            (s.ground && geometric.z < lowest_altitude)) {
            continue;
        }
        Object star = source;
        star.geometric = geometric;
        star.observed = s.atmosphere ? refract(geometric, s.pressure, s.temperature) : geometric;
        add(star);
    }
    auto bodies = sky.bodies;
    std::sort(bodies.begin(), bodies.end(), [](auto& a, auto& b) {
        return a.distance_au > b.distance_au;
    });
    for (auto& o : bodies) {
        add(o);
    }
    return result;
}

void annotate(const SkySnapshot& sky,
              const SkySnapshot& stars,
              const SkyEngine& engine,
              const Scenario& s,
              const Camera& c,
              uint64_t selected,
              bool selected_body) {
    auto* draw = ImGui::GetBackgroundDrawList();
    if (s.grid) {
        draw_grid(c, sky.scenario.ground);
    }
    std::vector<ImVec2> occupied;
    auto label = [&](const Object& source, bool force) {
        if (!force && source.body == 0 && source.magnitude > 2.7) {
            return;
        }
        if (!force && source.body == 0 && (source.flags & Gaia) && !source.hip) {
            return;
        }
        const auto o = source.body ? source : observe_star(source, sky);
        if (sky.scenario.ground && o.altitude() < 0) {
            return;
        }
        auto p = c.project(o.observed);
        if (!p.visible) {
            return;
        }
        if (!force) {
            for (auto xy : occupied) {
                if (abs(xy.x - p.x) < 100 && abs(xy.y - p.y) < 22) {
                    return;
                }
            }
            if (occupied.size() > 65) {
                return;
            }
        }
        occupied.push_back({float(p.x), float(p.y)});
        std::string name =
            o.body ? body_name(o.body) : engine.catalog.name(engine.catalog.stars[o.catalog_index]);
        const float disk_radius = float(o.angular_radius * c.prepare().maximum_scale(p));
        ImVec2 pos{float(p.x) + std::max(9.f, disk_radius + 15), float(p.y - 8)};
        draw_moving_text(*draw, {pos.x + 1, pos.y + 1}, color(0, 0, 0, 180), name.c_str());
        draw_moving_text(*draw,
                         pos,
                         o.body ? color(231, 201, 133, 215) : color(160, 188, 203, 180),
                         name.c_str());
        if (force) {
            const float radius = std::max(12.f, disk_radius + 7);
            const auto selection_color = color(214, 199, 161, 170);
            draw->AddCircle({float(p.x), float(p.y)}, radius, selection_color, 96, .8f);
            draw->AddLine({float(p.x) - radius - 6, float(p.y)},
                          {float(p.x) - radius - 1, float(p.y)},
                          selection_color);
            draw->AddLine({float(p.x) + radius + 1, float(p.y)},
                          {float(p.x) + radius + 6, float(p.y)},
                          selection_color);
        }
    };
    if (s.labels) {
        for (auto& o : sky.bodies) {
            if (!selected_body || o.id != selected) {
                label(o, false);
            }
        }
        for (auto index : stars.label_stars) {
            const auto& o = stars.stars[index];
            if (selected_body || o.id != selected) {
                label(o, false);
            }
        }
    }
    if (auto* o = selected_object(selected_body ? sky : stars, selected, selected_body)) {
        label(*o, true);
    }
}

} // namespace

int run_app(const AppOptions& options) {
    if (!SDL_Init(SDL_INIT_VIDEO)) {
        throw std::runtime_error(SDL_GetError());
    }
    SDL_Window* window =
        SDL_CreateWindow("Astra · 万年星空",
                         1440,
                         900,
                         SDL_WINDOW_VULKAN | SDL_WINDOW_RESIZABLE | SDL_WINDOW_HIGH_PIXEL_DENSITY);
    if (!window) {
        SDL_Quit();
        throw std::runtime_error(SDL_GetError());
    }
    SDL_SetWindowMinimumSize(window, 980, 650);
    IMGUI_CHECKVERSION();
    ImGui::CreateContext();
    auto& io = ImGui::GetIO();
    io.IniFilename = nullptr;
    io.ConfigFlags |= ImGuiConfigFlags_NavEnableKeyboard;
    initialize_ui_style();
    auto font = options.data / "fonts/NotoSansCJKsc-Regular.otf";
    ImFont* title = nullptr;
    ImVector<ImWchar> glyphs;
    if (std::filesystem::exists(font)) {
        ImFontConfig config;
        config.OversampleH = config.OversampleV = 1;
        ImFontGlyphRangesBuilder ranges;
        ranges.AddRanges(io.Fonts->GetGlyphRangesChineseSimplifiedCommon());
        ranges.AddText("儒Δ−枢璇玑觜氐亢箕昴");
        std::ifstream glyph_file(options.data / "fonts/ui-glyphs.txt");
        std::string ui_glyphs((std::istreambuf_iterator<char>(glyph_file)), {});
        ranges.AddText(ui_glyphs.c_str());
        ranges.BuildRanges(&glyphs);
        const float density = std::max(1.f, SDL_GetWindowPixelDensity(window));
        io.FontGlobalScale = 1.f / density;
        io.Fonts->AddFontFromFileTTF(font.string().c_str(), 15 * density, &config, glyphs.Data);
        title =
            io.Fonts->AddFontFromFileTTF(font.string().c_str(), 24 * density, &config, glyphs.Data);
    } else {
        io.Fonts->AddFontDefault();
    }
    int exit_status = 0;
    try {
        Renderer renderer(
            window, options.shaders, options.data / "background/milky-way.png", options.validation);
        ImGui_ImplSDL3_InitForVulkan(window);
        renderer.init_ui();
        {
            Worker worker(options.data);
            Scenario scene = options.scenario;
            worker.request(scene);
            UiState ui;
            ui.pending_shot = options.screenshot;
            if (options.playback_speed != 0) {
                ui.playing = true;
                ui.speed = options.playback_speed;
                const double rates[] = {1, 60, 3600, 86400, 31557600};
                for (int i = 0; i < 5; ++i) {
                    if (abs(ui.speed) == rates[i]) {
                        ui.rate = i;
                    }
                }
            }
            bool quit = false;
            auto& playing = ui.playing;
            auto& panels = ui.visible;
            auto& track = ui.track;
            auto& speed = ui.speed;
            auto& selected = ui.selected;
            auto& selected_body = ui.selected_body;
            auto& notice = ui.notice;
            auto& pending_shot = ui.pending_shot;
            auto& shot_done = ui.shot_done;
            std::shared_ptr<SkySnapshot> sky, stars;
            std::shared_ptr<SkySnapshot> latest_snapshot;
            std::shared_ptr<SkySnapshot> rendered_stars;
            bool was_playing = false, settling_pause = false;
            double busy_since = -1;
            std::shared_ptr<SkyEngine> engine;
            std::shared_ptr<SkySnapshot> rendered_snapshot;
            RenderScene render;
            int frame = 0;
            auto last = std::chrono::steady_clock::now();
            auto first = last;
            std::vector<double> steady_frame_ms;
            double last_request = 0;
            ImVec2 drag_origin{};
            std::optional<Camera> drag_camera;
            bool dragged = false;
            auto drag_view = [&](float x, float y) {
                if (!drag_camera) {
                    return;
                }
                dragged |= hypot(x - drag_origin.x, y - drag_origin.y) > 4;
                if (!dragged) {
                    return;
                }
                // Always use the button-down pose: the result must not depend on
                // mouse event frequency or on the path taken between two pixels.
                Camera camera = *drag_camera;
                camera.pan(drag_origin.x, drag_origin.y, x, y);
                scene.azimuth = camera.azimuth / rad;
                scene.elevation = camera.elevation / rad;
                scene.roll = camera.roll / rad;
                track = false;
            };
            char* pref = SDL_GetPrefPath("Astra", "Sky");
            std::filesystem::path user = pref ? pref : ".";
            SDL_free(pref);
            while (!quit) {
                auto current = std::chrono::steady_clock::now();
                double dt = std::chrono::duration<double>(current - last).count(),
                       elapsed = std::chrono::duration<double>(current - first).count();
                last = current;
                bool request = false, seek_moon = false;
                std::optional<ImVec2> pick_position;
                SDL_Event event;
                while (SDL_PollEvent(&event)) {
                    if (event.type == SDL_EVENT_MOUSE_BUTTON_DOWN ||
                        event.type == SDL_EVENT_MOUSE_BUTTON_UP) {
                        // A click can arrive without a preceding motion event (for example
                        // accessibility input). Keep its position with the button event.
                        io.AddMousePosEvent(event.button.x, event.button.y);
                    }
                    ImGui_ImplSDL3_ProcessEvent(&event);
                    if (event.type == SDL_EVENT_QUIT) {
                        quit = true;
                    }
                    if (event.type == SDL_EVENT_WINDOW_PIXEL_SIZE_CHANGED) {
                        renderer.request_resize();
                    }
                    if (event.type == SDL_EVENT_WINDOW_FOCUS_LOST ||
                        event.type == SDL_EVENT_WINDOW_RESIZED) {
                        drag_camera.reset();
                    }
                    if (event.type == SDL_EVENT_KEY_DOWN && !io.WantTextInput) {
                        switch (event.key.key) {
                        case SDLK_ESCAPE:
                            ui.panel = UiPanel::None;
                            track = false;
                            selected = 0;
                            break;
                        case SDLK_SPACE:
                            playing = !playing;
                            break;
                        case SDLK_H:
                            panels = !panels;
                            break;
                        case SDLK_R:
                            scene.roll = 0;
                            drag_camera.reset();
                            break;
                        case SDLK_LEFT:
                            scene.azimuth = wrap((scene.azimuth - 5) * rad) / rad;
                            track = false;
                            break;
                        case SDLK_RIGHT:
                            scene.azimuth = wrap((scene.azimuth + 5) * rad) / rad;
                            track = false;
                            break;
                        case SDLK_UP:
                            scene.elevation = std::min(90., scene.elevation + 5);
                            track = false;
                            break;
                        case SDLK_DOWN:
                            scene.elevation = std::max(-90., scene.elevation - 5);
                            track = false;
                            break;
                        default:
                            break;
                        }
                    }
                    if (event.type == SDL_EVENT_MOUSE_WHEEL &&
                        !over_ui(event.wheel.mouse_x, event.wheel.mouse_y) && !drag_camera) {
                        scene.fov =
                            zoom_fov(scene.fov * rad, scene.projection, event.wheel.y) / rad;
                    }
                    if (event.type == SDL_EVENT_MOUSE_BUTTON_DOWN &&
                        event.button.button == SDL_BUTTON_LEFT) {
                        drag_camera.reset();
                        drag_origin = {event.button.x, event.button.y};
                        dragged = false;
                        int w, h;
                        SDL_GetWindowSize(window, &w, &h);
                        const auto camera = camera_for(scene, w, h);
                        if (!over_ui(event.button.x, event.button.y) &&
                            camera.contains(event.button.x, event.button.y)) {
                            drag_camera = camera;
                        }
                    }
                    if (event.type == SDL_EVENT_MOUSE_MOTION &&
                        (event.motion.state & SDL_BUTTON_LMASK)) {
                        // The view owns this gesture until release, even over a panel.
                        drag_view(event.motion.x, event.motion.y);
                    }
                    if (event.type == SDL_EVENT_MOUSE_BUTTON_UP &&
                        event.button.button == SDL_BUTTON_LEFT && drag_camera) {
                        drag_view(event.button.x, event.button.y);
                        if (!dragged) {
                            pick_position = ImVec2{event.button.x, event.button.y};
                        }
                        drag_camera.reset();
                    }
                }
                bool busy;
                std::string error, status;
                {
                    std::lock_guard lock(worker.mutex);
                    latest_snapshot = worker.snapshot;
                    engine = worker.engine;
                    busy = worker.busy || worker.pending.has_value();
                    error = worker.error;
                    status = worker.status;
                    if (worker.adopt_scene && latest_snapshot) {
                        scene = latest_snapshot->scenario;
                        sky = latest_snapshot;
                        selected = 301;
                        selected_body = true;
                        track = true;
                        worker.adopt_scene = false;
                        notice = "已前往可观月时刻：" + format_date(scene.date) + " " +
                                 scale_name(scene.scale);
                    }
                }
                if (was_playing && !playing) {
                    request = true;
                    settling_pause = true;
                }
                const bool snapshot_changed = latest_snapshot && latest_snapshot != stars;
                if (snapshot_changed) {
                    stars = latest_snapshot;
                    const bool matches_clock =
                        valid_date(scene.date, scene.julian) &&
                        scene.scale == latest_snapshot->scenario.scale &&
                        scene.julian == latest_snapshot->scenario.julian &&
                        abs(to_jd(latest_snapshot->scenario.date, latest_snapshot->scenario.julian)
                                .value() -
                            to_jd(scene.date, scene.julian).value()) < 1e-8;
                    if (!playing && (!settling_pause || matches_clock)) {
                        sky = latest_snapshot;
                        settling_pause = false;
                    }
                }
                if (playing) {
                    settling_pause = false;
                }
                was_playing = playing;
                if (busy) {
                    if (busy_since < 0) {
                        busy_since = elapsed;
                    }
                } else {
                    busy_since = -1;
                }
                // A quick astrometry refresh must not flash buttons or progress text.
                const bool show_busy = busy && !playing && elapsed - busy_since > .35;
                if (options.frames && frame > 20 && sky && (!busy || playing) && dt < 1) {
                    steady_frame_ms.push_back(dt * 1000);
                }
                if (playing && engine && stars) {
                    try {
                        scene.date = from_jd(
                            to_jd(scene.date, scene.julian).add_seconds(std::min(dt, .25) * speed),
                            scene.julian);
                        if (scene.date.year < -3000 || scene.date.year >= 7000) {
                            playing = false;
                            scene.date = scene.date.year < -3000
                                             ? CivilDate{-3000, 1, 1, 0, 0, 0}
                                             : CivilDate{6999, 12, 31, 23, 59, 59};
                        }
                        if (!busy && elapsed - last_request > .15) {
                            request = true;
                            last_request = elapsed;
                        }
                    } catch (const std::exception& e) {
                        playing = false;
                        notice = e.what();
                    }
                }
                if (playing && engine && stars) {
                    try {
                        sky = engine->compute(scene, 0, nullptr, SkyEngine::Scope::SolarSystem);
                    } catch (const std::exception& error) {
                        playing = false;
                        notice = error.what();
                    }
                }
                if (track && sky && stars) {
                    if (auto* o = selected_object(
                            selected_body ? *sky : *stars, selected, selected_body)) {
                        const auto current_object = selected_body ? *o : observe_star(*o, *sky);
                        scene.azimuth = current_object.azimuth() / rad;
                        scene.elevation = current_object.altitude() / rad;
                    }
                }
                if (options.smoke) {
                    if (frame == 45) {
                        SDL_SetWindowSize(window, 1280, 800);
                    }
                    if (frame == 80) {
                        scene.date.year = -2500;
                        request = true;
                    }
                    if (frame == 125) {
                        scene.date = options.scenario.date;
                        scene.latitude = 90;
                        request = true;
                    }
                    if (frame == 165) {
                        scene = options.scenario;
                        scene.projection = ProjectionKind::Fisheye;
                        scene.fov = 180;
                        request = true;
                    }
                    if (frame == 210) {
                        scene = options.scenario;
                        request = true;
                    }
                }
                int width, height;
                SDL_GetWindowSize(window, &width, &height);
                if (options.smoke && frame >= 220 && frame <= 240) {
                    auto pan = camera_for(options.scenario, width, height);
                    const double travel = sin(pi * (frame - 220) / 20.);
                    pan.pan(width * .45,
                            height * .45,
                            width * (.45 + .2 * travel),
                            height * (.45 + .1 * travel));
                    scene.azimuth = pan.azimuth / rad;
                    scene.elevation = pan.elevation / rad;
                    scene.roll = pan.roll / rad;
                }
                auto camera = camera_for(scene, width, height);
                ImGui_ImplVulkan_NewFrame();
                ImGui_ImplSDL3_NewFrame();
                ImGui::NewFrame();
                if (pick_position && !io.WantCaptureMouse && sky && stars) {
                    int w, h;
                    SDL_GetWindowSize(window, &w, &h);
                    auto c = camera_for(scene, w, h);
                    double best = 18;
                    std::optional<Object> found;
                    auto pick = [&](const Object& o) {
                        if (sky->scenario.ground && o.altitude() < 0) {
                            return;
                        }
                        auto p = c.project(o.observed);
                        if (p.visible) {
                            double distance = hypot(p.x - pick_position->x, p.y - pick_position->y);
                            if (distance < best) {
                                best = distance;
                                found = o;
                            }
                        }
                    };
                    for (auto& o : stars->stars) {
                        auto current_star = observe_star(o, *sky);
                        pick(current_star);
                    }
                    for (auto& o : sky->bodies) {
                        pick(o);
                    }
                    if (found) {
                        selected = found->id;
                        selected_body = found->body != 0;
                    }
                }

                if (sky && engine) {
                    annotate(*sky, *stars, *engine, scene, camera, selected, selected_body);
                }
                if (panels) {
                    auto actions = draw_ui(ui,
                                           {scene,
                                            sky.get(),
                                            engine.get(),
                                            user,
                                            width,
                                            height,
                                            show_busy,
                                            title,
                                            stars.get()});
                    request |= actions.recompute;
                    seek_moon = actions.seek_moon;
                } else {
                    auto* draw = ImGui::GetForegroundDrawList();
                    draw->AddText({24, 22}, color(214, 199, 161), "A S T R A   /   H 显示面板");
                }
                if (!error.empty() || !notice.empty() || !status.empty() ||
                    (sky && sky->time.extrapolated)) {
                    ImGui::SetNextWindowPos(
                        {float(width / 2), float(height - 170)}, ImGuiCond_Always, {.5f, 1});
                    ImGui::SetNextWindowBgAlpha(.9f);
                    ImGui::Begin("status",
                                 nullptr,
                                 fixed | ImGuiWindowFlags_AlwaysAutoResize |
                                     ImGuiWindowFlags_NoFocusOnAppearing);
                    if (!error.empty()) {
                        ImGui::TextColored(rgba(244, 161, 136), "%s", error.c_str());
                    } else if (!status.empty()) {
                        ImGui::TextUnformatted(status.c_str());
                    } else if (!notice.empty()) {
                        ImGui::TextUnformatted(notice.c_str());
                        if (ImGui::SmallButton("关闭")) {
                            notice.clear();
                        }
                    } else if (sky) {
                        ImGui::TextColored(rgba(235, 197, 128),
                                           "长期外推 · ΔT %.1f 秒 · 误差范围未知",
                                           sky->time.delta_t_seconds);
                    }
                    ImGui::End();
                }
                if (request || seek_moon) {
                    try {
                        validate(scene);
                        worker.request(scene, seek_moon);
                    } catch (const std::exception& e) {
                        notice = e.what();
                    }
                }
                ImGui::Render();
                const auto& prior = render.camera;
                const bool camera_changed =
                    prior.width != camera.width || prior.height != camera.height ||
                    prior.azimuth != camera.azimuth || prior.elevation != camera.elevation ||
                    prior.roll != camera.roll || prior.fov != camera.fov ||
                    prior.projection != camera.projection;
                if (sky && stars &&
                    (sky != rendered_snapshot || stars != rendered_stars || camera_changed)) {
                    render = render_scene(*sky, *stars, scene, camera);
                    rendered_stars = stars;
                    rendered_snapshot = sky;
                }
                render.camera = camera;
                render.exposure = float(scene.exposure);
                render.milky_way = scene.milky_way && bool(sky);
                bool capture = !pending_shot.empty() && !shot_done && sky &&
                               sky == latest_snapshot && !playing && !busy && !request &&
                               frame > 10 && (!options.smoke || frame > 240);
                try {
                    renderer.render(render,
                                    ImGui::GetDrawData(),
                                    capture ? pending_shot : std::filesystem::path{});
                    if (capture) {
                        auto saved = sky->scenario;
                        saved.azimuth = scene.azimuth;
                        saved.elevation = scene.elevation;
                        saved.roll = scene.roll;
                        saved.fov = scene.fov;
                        saved.projection = scene.projection;
                        saved.exposure = scene.exposure;
                        saved.milky_way = scene.milky_way;
                        auto meta = pending_shot;
                        meta.replace_extension("json");
                        save_scenario(saved, meta, sky->data_id, &sky->time);
                        notice = "截图已保存：" + pending_shot.string();
                        std::cout << "Screenshot: " << pending_shot << std::endl;
                        shot_done = true;
                    }
                } catch (const std::exception& e) {
                    // A failed Vulkan submit can leave a frame fence unsignalled.
                    // Exit rather than retrying a frame with invalid GPU state.
                    throw std::runtime_error(std::string("Rendering failed: ") + e.what());
                }
                ++frame;
                if (options.frames && frame >= options.frames && !pending_shot.empty() &&
                    !shot_done) {
                    playing = false;
                }
                if (options.frames && frame >= options.frames && sky && !busy &&
                    (pending_shot.empty() || shot_done)) {
                    quit = true;
                }
                if (options.frames && elapsed > 120) {
                    throw std::runtime_error("Smoke test timed out: " + error);
                }
                if (SDL_GetWindowFlags(window) & SDL_WINDOW_MINIMIZED) {
                    SDL_Delay(20);
                }
            }
            std::cout << "Frames: " << frame << "; validation errors: " << renderer.errors()
                      << std::endl;
            if (!steady_frame_ms.empty()) {
                std::sort(steady_frame_ms.begin(), steady_frame_ms.end());
                std::cout << "Steady frame interval (CPU + present, ms): median "
                          << steady_frame_ms[steady_frame_ms.size() / 2] << "; p95 "
                          << steady_frame_ms[size_t((steady_frame_ms.size() - 1) * .95)]
                          << std::endl;
            }
            if (renderer.errors()) {
                exit_status = 2;
            }
        }
        ImGui_ImplSDL3_Shutdown();
    } catch (...) {
        ImGui::DestroyContext();
        SDL_DestroyWindow(window);
        SDL_Quit();
        throw;
    }
    ImGui::DestroyContext();
    SDL_DestroyWindow(window);
    SDL_Quit();
    return exit_status;
}
} // namespace astro
