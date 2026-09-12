#include "astro/ui.hpp"
#include "astro/camera.hpp"
#include <cctype>
#include <ctime>
#include <iomanip>
#include <sstream>

namespace astro {
namespace {
constexpr ImGuiWindowFlags fixed = ImGuiWindowFlags_NoTitleBar | ImGuiWindowFlags_NoMove |
                                   ImGuiWindowFlags_NoResize | ImGuiWindowFlags_NoSavedSettings |
                                   ImGuiWindowFlags_NoFocusOnAppearing;
const ImVec4 accent{.84f, .78f, .63f, 1};
const ImVec4 muted{.51f, .57f, .64f, 1};
const ImVec4 ink{.91f, .92f, .94f, 1};

std::string number(double value, int precision = 1) {
    std::ostringstream stream;
    stream << std::fixed << std::setprecision(precision) << value;
    return stream.str();
}

void window(const char* id, float x, float y, float width, float height) {
    ImGui::SetNextWindowPos({x, y});
    ImGui::SetNextWindowSize({width, height});
    ImGui::Begin(id, nullptr, fixed);
}

void heading(const char* text, ImFont* font) {
    if (font) {
        ImGui::PushFont(font);
    }
    ImGui::TextUnformatted(text);
    if (font) {
        ImGui::PopFont();
    }
}

void toggle(UiState& state, UiPanel panel) {
    state.panel = state.panel == panel ? UiPanel::None : panel;
}

bool primary(const char* label, ImVec2 size) {
    ImGui::PushStyleColor(ImGuiCol_Button, accent);
    ImGui::PushStyleColor(ImGuiCol_ButtonHovered, ImVec4{.94f, .87f, .72f, 1});
    ImGui::PushStyleColor(ImGuiCol_ButtonActive, ImVec4{.72f, .65f, .51f, 1});
    ImGui::PushStyleColor(ImGuiCol_Text, ImVec4{.08f, .09f, .11f, 1});
    bool pressed = ImGui::Button(label, size);
    ImGui::PopStyleColor(4);
    return pressed;
}

const Object* find_object(const SkySnapshot* sky, uint64_t id, bool body) {
    if (!sky || id == 0) {
        return nullptr;
    }
    const auto& objects = body ? sky->bodies : sky->stars;
    auto found = std::find_if(objects.begin(), objects.end(), [&](const Object& object) {
        return object.id == id;
    });
    return found == objects.end() ? nullptr : &*found;
}

void focus(UiState& state, Scenario& scene, const Object& object, bool close = false) {
    state.selected = object.id;
    state.selected_body = object.body != 0;
    state.track = true;
    scene.azimuth = object.azimuth() / rad;
    scene.elevation = object.altitude() / rad;
    scene.roll = 0;
    if (close) {
        scene.fov = 6;
        scene.projection = ProjectionKind::Perspective;
    }
}

void step_time(UiState& state, Scenario& scene, UiActions& actions, double seconds) {
    try {
        auto candidate = scene;
        candidate.date =
            from_jd(to_jd(scene.date, scene.julian).add_seconds(seconds), scene.julian);
        validate(candidate);
        scene = candidate;
        state.playing = false;
        actions.recompute = true;
    } catch (const std::exception& error) {
        state.notice = error.what();
    }
}

void draw_header(UiState& state, const UiFrame& frame) {
    const Translator tr{state.language};
    auto* draw = ImGui::GetBackgroundDrawList();
    const ImU32 gold = ImGui::ColorConvertFloat4ToU32(accent);
    draw->AddCircle({43, 43}, 16, gold, 48, 1.2f);
    draw->AddLine({43, 20}, {43, 66}, gold, 1.2f);
    draw->AddLine({20, 43}, {66, 43}, gold, 1.2f);
    draw->AddCircleFilled({43, 43}, 3, gold);
    draw->AddText(frame.title ? frame.title : ImGui::GetFont(),
                  24,
                  {80, 21},
                  IM_COL32(237, 235, 228, 255),
                  "A S T R A");
    draw->AddText({82, 53}, IM_COL32(141, 151, 164, 255), tr("万年星空"));

    ImGui::PushStyleVar(ImGuiStyleVar_WindowPadding, ImVec2{14, 9});
    window("observer-pill", (frame.width - 400.f) / 2, 24, 400, 58);
    if (ImGui::InvisibleButton("open-observer", {150, 38})) {
        toggle(state, UiPanel::Location);
    }
    auto p = ImGui::GetItemRectMin();
    auto name = std::string(tr(frame.scene.location_name.c_str()));
    if (ImGui::IsItemHovered()) {
        ImGui::SetTooltip("%s", name.c_str());
    }
    auto* foreground = ImGui::GetWindowDrawList();
    foreground->PushClipRect(p, {p.x + 150, p.y + 38}, true);
    foreground->AddText(p, ImGui::GetColorU32(ImGuiCol_TextDisabled), tr("观察地点"));
    foreground->AddText({p.x, p.y + 20}, ImGui::GetColorU32(ImGuiCol_Text), name.c_str());
    foreground->PopClipRect();
    ImGui::SameLine();
    if (ImGui::InvisibleButton("open-time", {208, 38})) {
        toggle(state, UiPanel::Time);
    }
    p = ImGui::GetItemRectMin();
    auto date = format_date(frame.sky ? frame.sky->scenario.date : frame.scene.date);
    foreground->AddText(
        p, ImGui::GetColorU32(ImGuiCol_Text), date.substr(0, date.size() - 3).c_str());
    foreground->AddText({p.x, p.y + 20},
                        ImGui::GetColorU32(ImGuiCol_TextDisabled),
                        scale_name(frame.sky ? frame.sky->scenario.scale : frame.scene.scale));
    ImGui::End();
    ImGui::PopStyleVar();

    ImGui::PushStyleVar(ImGuiStyleVar_WindowPadding, ImVec2{12, 8});
    window("search-pill", frame.width - 274.f, 24, 250, 48);
    ImGui::SetNextItemWidth(-1);
    if (ImGui::InputTextWithHint(
            "##search", tr("搜索天体 / HIP 编号"), state.search, sizeof(state.search))) {
        state.panel = state.search[0] ? UiPanel::Search : UiPanel::None;
    }
    ImGui::End();
    ImGui::PopStyleVar();
}

void rail_button(UiState& state, UiPanel panel, const char* label, int icon) {
    ImGui::PushStyleColor(ImGuiCol_Button,
                          state.panel == panel ? ImVec4{.20f, .22f, .26f, 1} : ImVec4{0, 0, 0, 0});
    auto p = ImGui::GetCursorScreenPos();
    ImGui::PushID(icon);
    if (ImGui::Button("##tool", {48, 64})) {
        toggle(state, panel);
    }
    auto* draw = ImGui::GetWindowDrawList();
    ImU32 color = ImGui::ColorConvertFloat4ToU32(state.panel == panel ? accent : muted);
    ImVec2 center{p.x + 24, p.y + 22};
    if (icon == 0) {
        draw->AddCircle(center, 9, color, 32, 1.5f);
        draw->AddCircle(center, 3, color, 24, 1.5f);
        draw->AddLine({center.x, center.y + 10}, {center.x, center.y + 14}, color, 1.5f);
    } else if (icon == 1) {
        draw->AddCircle(center, 10, color, 32, 1.5f);
        draw->AddLine(center, {center.x, center.y - 6}, color, 1.5f);
        draw->AddLine(center, {center.x + 5, center.y + 3}, color, 1.5f);
    } else {
        for (int i = 0; i < 3; ++i) {
            float y = center.y - 7 + i * 7;
            draw->AddLine({center.x - 10, y}, {center.x + 10, y}, color, 1.3f);
            draw->AddCircleFilled({center.x + (i == 1 ? -4.f : 4.f), y}, 2.7f, color);
        }
    }
    auto size = ImGui::CalcTextSize(label);
    draw->AddText({p.x + (48 - size.x) / 2, p.y + 42}, color, label);
    ImGui::PopID();
    ImGui::PopStyleColor();
}

void draw_rail(UiState& state) {
    const Translator tr{state.language};
    ImGui::PushStyleVar(ImGuiStyleVar_WindowPadding, ImVec2{8, 8});
    ImGui::PushStyleVar(ImGuiStyleVar_ItemSpacing, ImVec2{0, 4});
    window("tools", 24, 112, 64, 288);
    rail_button(state, UiPanel::Location, tr("地点"), 0);
    rail_button(state, UiPanel::Time, tr("时间"), 1);
    rail_button(state, UiPanel::Display, tr("显示"), 2);
    rail_button(state, UiPanel::Explore, tr("探索"), 3);
    ImGui::End();
    ImGui::PopStyleVar(2);
}

void draw_location(UiState& state, const UiFrame& frame, UiActions& actions) {
    const Translator tr{state.language};
    auto& scene = frame.scene;
    ImGui::TextDisabled("%s", tr("选择一处地点，或输入地理坐标"));
    ImGui::Spacing();
    const char* places[] = {"北京 Beijing",
                            "上海 Shanghai",
                            "旧金山 San Francisco",
                            "阿塔卡马 Atacama",
                            "赤道 Equator",
                            "北极 North Pole",
                            "南极 South Pole"};
    const double coords[][3] = {{116.4074, 39.9042, 45},
                                {121.4737, 31.2304, 5},
                                {-122.4194, 37.7749, 16},
                                {-70.403, -24.625, 2635},
                                {0, 0, 0},
                                {0, 90, 0},
                                {0, -90, 0}};
    ImGui::SetNextItemWidth(-1);
    if (ImGui::BeginCombo("##place", tr(scene.location_name.c_str()))) {
        for (int i = 0; i < 7; ++i) {
            if (ImGui::Selectable(tr(places[i]))) {
                scene.longitude = coords[i][0];
                scene.latitude = coords[i][1];
                scene.height = coords[i][2];
                scene.location_name = places[i];
                actions.recompute = true;
            }
        }
        ImGui::EndCombo();
    }
    ImGui::TextDisabled("%s", tr("经度 / 东经为正"));
    ImGui::SetNextItemWidth(-1);
    if (ImGui::InputDouble("##longitude", &scene.longitude, 0, 0, "%.4f°")) {
        scene.location_name = "自定义坐标";
        actions.recompute = true;
    }
    ImGui::TextDisabled("%s", tr("纬度 / 北纬为正"));
    ImGui::SetNextItemWidth(-1);
    if (ImGui::InputDouble("##latitude", &scene.latitude, 0, 0, "%.4f°")) {
        scene.location_name = "自定义坐标";
        actions.recompute = true;
    }
    ImGui::TextDisabled("%s", tr("海拔 / 米"));
    ImGui::SetNextItemWidth(-1);
    actions.recompute |= ImGui::InputDouble("##height", &scene.height, 0, 0, "%.0f m");
    ImGui::Spacing();
    if (ImGui::Button(tr("收藏地点"), {126, 36})) {
        save_scenario(scene, frame.user / "favorite.json");
        state.notice = "地点已收藏";
    }
    ImGui::SameLine();
    if (ImGui::Button(tr("恢复收藏"), {126, 36})) {
        auto saved = load_scenario(frame.user / "favorite.json");
        scene.longitude = saved.longitude;
        scene.latitude = saved.latitude;
        scene.height = saved.height;
        scene.location_name = saved.location_name;
        actions.recompute = true;
    }
}

void draw_time(UiState& state, const UiFrame& frame, UiActions& actions) {
    const Translator tr{state.language};
    auto& scene = frame.scene;
    ImGui::TextDisabled("%s", tr("公元 2000 年 · 前后各 5000 年"));
    ImGui::Spacing();
    ImGui::TextDisabled("%s", tr("天文年 / 含公元 0 年"));
    ImGui::SetNextItemWidth(-1);
    ImGui::InputInt("##year", &scene.date.year, 0, 0);
    if (scene.date.year <= 0) {
        ImGui::TextColored(accent, tr("公元前 %d 年"), 1 - scene.date.year);
    }
    ImGui::TextDisabled("%s", tr("月 / 日          时 / 分"));
    int md[] = {scene.date.month, scene.date.day};
    int hm[] = {scene.date.hour, scene.date.minute};
    ImGui::SetNextItemWidth(124);
    if (ImGui::InputInt2("##month-day", md)) {
        scene.date.month = md[0];
        scene.date.day = md[1];
    }
    ImGui::SameLine();
    ImGui::SetNextItemWidth(128);
    if (ImGui::InputInt2("##hour-minute", hm)) {
        scene.date.hour = hm[0];
        scene.date.minute = hm[1];
    }
    ImGui::TextDisabled("%s", tr("时间标准"));
    int scale = int(scene.scale);
    const char* scales[] = {tr("UT1 · 地球自转时"),
                            tr("TT · 地球时"),
                            tr("TDB · 历表时"),
                            tr("UTC · 现代民用时"),
                            tr("LMT · 当地平太阳时")};
    ImGui::SetNextItemWidth(-1);
    if (ImGui::Combo("##scale", &scale, scales, 5)) {
        scene.scale = TimeScale(scale);
    }
    ImGui::Checkbox(tr("使用儒略历"), &scene.julian);
    if (primary(tr("前往这个时刻"), {164, 38})) {
        scene.date.second = 0;
        state.playing = false;
        actions.recompute = true;
    }
    ImGui::SameLine();
    if (ImGui::Button(tr("现在"), {88, 38})) {
        std::time_t now = std::time(nullptr);
        auto* time = std::gmtime(&now);
        scene.date = {time->tm_year + 1900,
                      time->tm_mon + 1,
                      time->tm_mday,
                      time->tm_hour,
                      time->tm_min,
                      double(time->tm_sec)};
        scene.scale = TimeScale::UTC;
        scene.julian = false;
        state.playing = false;
        actions.recompute = true;
    }
    ImGui::Spacing();
    ImGui::Separator();
    ImGui::Spacing();
    ImGui::TextDisabled("%s", tr("穿越一万年"));
    ImGui::SetNextItemWidth(-1);
    int year = scene.date.year;
    if (ImGui::SliderInt("##millennia", &year, -3000, 6999, tr("%d 年"))) {
        scene.date.year = year;
        if (scene.scale == TimeScale::UTC && (year < 1973 || year >= 2027)) {
            scene.scale = TimeScale::UT1;
            state.notice = "该年代使用 UT1 地球自转时";
        }
        if (!valid_date(scene.date, scene.julian)) {
            scene.date.day = 28;
        }
        actions.recompute = true;
        state.playing = false;
    }
    ImGui::Spacing();
    if (frame.sky) {
        for (const auto& body : frame.sky->bodies) {
            if (body.body != 10) {
                continue;
            }
            const double altitude = asin(std::clamp(body.geometric.z, -1., 1.)) / rad;
            const char* phase = altitude >= 0     ? "白昼"
                                : altitude >= -6  ? "民用晨昏"
                                : altitude >= -12 ? "航海晨昏"
                                : altitude >= -18 ? "天文晨昏"
                                                  : "黑夜";
            ImGui::TextColored(accent, "%s  /  %.1f°", tr(phase), altitude);
        }
    }
    ImGui::TextDisabled("%s", tr("太阳中心高度 −6°"));
    const char* events[] = {"上次黎明", "下次黎明", "上次黄昏", "下次黄昏"};
    const int codes[] = {-1, 1, -2, 2};
    ImGui::BeginDisabled(frame.busy);
    for (int i = 0; i < 4; ++i) {
        if (i % 2) {
            ImGui::SameLine();
        }
        if (ImGui::Button(tr(events[i]), {126, 32})) {
            state.playing = state.track = false;
            actions.seek_twilight = codes[i];
        }
    }
    ImGui::EndDisabled();
}

void draw_display(UiState& state, const UiFrame& frame, UiActions& actions) {
    const Translator tr{state.language};
    auto& scene = frame.scene;
    ImGui::TextDisabled("Language / 语言");
    ImGui::SetNextItemWidth(-1);
    int language = int(state.language);
    const char* languages[] = {"English", "简体中文"};
    if (ImGui::Combo("##language", &language, languages, 2)) {
        state.language = Language(language);
        actions.language_changed = true;
    }
    ImGui::Spacing();
    actions.recompute |= ImGui::Checkbox(tr("大气与晨昏"), &scene.atmosphere);
    ImGui::SameLine(162);
    actions.recompute |= ImGui::Checkbox(tr("地面"), &scene.ground);
    ImGui::Checkbox(tr("天体名称"), &scene.labels);
    ImGui::SameLine(162);
    ImGui::Checkbox(tr("坐标网"), &scene.grid);
    ImGui::Checkbox(tr("银河光带"), &scene.milky_way);
    ImGui::TextDisabled("%s", tr("天空投影"));
    constexpr std::array modes{
        ProjectionKind::Perspective, ProjectionKind::Stereographic, ProjectionKind::Fisheye};
    int mode = int(std::find(modes.begin(), modes.end(), scene.projection) - modes.begin());
    ImGui::SetNextItemWidth(-1);
    const char* projections[] = {tr("直线透视"), tr("球面广角（保角）"), tr("全天鱼眼")};
    if (ImGui::Combo("##projection", &mode, projections, 3)) {
        scene.projection = modes[mode];
        scene.fov = scene.projection == ProjectionKind::Fisheye
                        ? 180
                        : std::min(scene.fov, maximum_zoom_fov(scene.projection) / rad);
        state.track = false;
    }
    if (ImGui::Button(tr("恢复水平  R"), {-1, 32})) {
        scene.roll = 0;
    }
    ImGui::Spacing();
    ImGui::TextDisabled("%s", tr("可见恒星 / 星等上限"));
    float magnitude = float(scene.magnitude);
    ImGui::SetNextItemWidth(-1);
    if (ImGui::SliderFloat("##magnitude", &magnitude, 2, 12, "%.1f")) {
        scene.magnitude = magnitude;
        actions.recompute = true;
    }
    ImGui::TextDisabled("%s", tr("曝光"));
    float exposure = float(log2(scene.exposure));
    ImGui::SetNextItemWidth(-1);
    if (ImGui::SliderFloat(
            "##exposure", &exposure, -40, 4, "%+.1f EV", ImGuiSliderFlags_AlwaysClamp)) {
        scene.exposure = exp2(exposure);
    }
    if (ImGui::Checkbox(tr("自适应曝光"), &scene.adaptive_exposure) && scene.adaptive_exposure) {
        scene.exposure = 1;
    }
    if (ImGui::IsItemHovered()) {
        ImGui::SetTooltip("%s", tr("按当前画面测光，平滑适应明暗。启用时曝光补偿归零。"));
    }
    ImGui::BeginDisabled(scene.adaptive_exposure);
    ImGui::Checkbox(tr("自动曝光"), &scene.auto_exposure);
    if (ImGui::IsItemHovered()) {
        ImGui::SetTooltip("%s",
                          tr("按全天亮度测光，转动和缩放不会改变曝光。关闭后使用固定夜空曝光。"));
    }
    ImGui::EndDisabled();
    if (ImGui::CollapsingHeader(tr("大气与时间模型###models"))) {
        ImGui::TextDisabled("%s", tr("大气预设"));
        const char* presets[] = {tr("清澈"), tr("薄霾")};
        ImGui::SetNextItemWidth(-1);
        ImGui::Combo("##atmosphere-preset", &scene.atmosphere_preset, presets, 2);
        ImGui::TextDisabled("%s", tr("气压 hPa"));
        ImGui::SetNextItemWidth(-1);
        actions.recompute |= ImGui::InputDouble("##pressure", &scene.pressure, 0, 0, "%.1f");
        ImGui::TextDisabled("%s", tr("温度 °C"));
        ImGui::SetNextItemWidth(-1);
        actions.recompute |= ImGui::InputDouble("##temperature", &scene.temperature, 0, 0, "%.1f");
        float pollution = float(scene.light_pollution);
        ImGui::TextDisabled("%s", tr("光污染"));
        ImGui::SetNextItemWidth(-1);
        if (ImGui::SliderFloat("##pollution", &pollution, 0, 1)) {
            scene.light_pollution = pollution;
            actions.recompute = true;
        }
        actions.recompute |= ImGui::Checkbox(tr("自定义 ΔT"), &scene.override_delta_t);
        if (scene.override_delta_t) {
            ImGui::TextDisabled("%s", tr("ΔT 秒"));
            ImGui::SetNextItemWidth(-1);
            actions.recompute |=
                ImGui::InputDouble("##delta-t", &scene.custom_delta_t, 0, 0, "%.3f");
        }
    }
    ImGui::Spacing();
    ImGui::Separator();
    ImGui::Spacing();
    if (ImGui::Button(tr("保存场景"), {126, 36})) {
        save_scenario(scene, frame.user / "scene.json", frame.sky ? frame.sky->data_id : "");
        state.notice = "场景已保存";
    }
    ImGui::SameLine();
    if (ImGui::Button(tr("载入场景"), {126, 36})) {
        scene = load_scenario(frame.user / "scene.json", frame.sky ? frame.sky->data_id : "");
        if (!scene.migrations.empty()) {
            state.notice = "旧场景已迁移到当前模型，画面可能与原版本不同。";
        }
        state.track = false;
        state.playing = false;
        actions.recompute = true;
    }
    if (ImGui::Button(tr("导出 PNG + 场景"), {-1, 36})) {
        state.pending_shot = frame.user / ("astra-" + std::to_string(std::time(nullptr)) + ".png");
        state.shot_done = false;
        state.playing = false;
        actions.recompute = true;
    }
}

void draw_drawer(UiState& state, const UiFrame& frame, UiActions& actions) {
    const Translator tr{state.language};
    if (state.panel == UiPanel::None || state.panel == UiPanel::Search ||
        state.panel == UiPanel::Explore) {
        return;
    }
    const float drawer_height = state.panel == UiPanel::Location ? 550.f : 680.f;
    window("settings", 102, 112, 300, std::min(drawer_height, frame.height - 240.f));
    const char* title = state.panel == UiPanel::Location ? tr("观察地点")
                        : state.panel == UiPanel::Time   ? tr("日期与时间")
                                                         : tr("天空与显示");
    heading(title, frame.title);
    ImGui::SameLine(252);
    if (ImGui::SmallButton("×")) {
        state.panel = UiPanel::None;
    }
    ImGui::Spacing();
    ImGui::Separator();
    ImGui::Spacing();
    try {
        switch (state.panel) {
        case UiPanel::Location:
            draw_location(state, frame, actions);
            break;
        case UiPanel::Time:
            draw_time(state, frame, actions);
            break;
        case UiPanel::Display:
            draw_display(state, frame, actions);
            break;
        default:
            break;
        }
    } catch (const std::exception& error) {
        state.notice = error.what();
    }
    ImGui::End();
}

void moon_preview(ImDrawList* draw, ImVec2 center, float radius, double phase, double limb) {
    draw->AddCircleFilled(center, radius, IM_COL32(39, 43, 49, 255), 96);
    const auto rotate = [&](float x, float y) {
        return ImVec2{center.x + float(x * cos(limb) - y * sin(limb)),
                      center.y + float(x * sin(limb) + y * cos(limb))};
    };
    // Schematic illumination, oriented toward the Sun in the local Moon view.
    for (float y = -radius + .5f; y < radius; y += .5f) {
        float edge = std::sqrt(std::max(0.f, radius * radius - y * y));
        float terminator = float(1 - 2 * phase) * edge;
        draw->AddLine(rotate(terminator, y), rotate(edge, y), IM_COL32(221, 219, 205, 255), .65f);
    }
    draw->AddCircle(center, radius, IM_COL32(142, 148, 155, 70), 96, 1);
}

void draw_moon(UiState& state, const UiFrame& frame, UiActions& actions) {
    const Translator tr{state.language};
    const auto* moon = find_object(frame.sky, 301, true);
    window("moon", frame.width - 274.f, 94, 250, 322);
    ImGui::TextColored(accent, "%s", tr("月球"));
    if (!moon) {
        ImGui::Spacing();
        ImGui::TextDisabled("%s", tr("正在计算月球位置…"));
        ImGui::End();
        return;
    }
    auto origin = ImGui::GetWindowPos();
    Camera moon_camera;
    moon_camera.azimuth = moon->azimuth();
    moon_camera.elevation = moon->altitude();
    const auto* sun = find_object(frame.sky, 10, true);
    const double limb =
        sun ? atan2(-dot(sun->observed, moon_camera.up()), dot(sun->observed, moon_camera.right()))
            : 0;
    moon_preview(ImGui::GetWindowDrawList(), {origin.x + 53, origin.y + 93}, 32, moon->phase, limb);
    ImGui::SetCursorPos({105, 64});
    heading((number(moon->phase * 100) + "%").c_str(), frame.title);
    ImGui::SetCursorPos({105, 98});
    ImGui::TextDisabled("%s", tr("月面照明"));
    ImGui::SetCursorPos({18, 140});
    if (!frame.scene.horizon.visible(moon->observed, moon->angular_radius)) {
        ImGui::TextColored(accent, "%s", tr("地平线以下"));
    } else {
        ImGui::TextColored(ImVec4{.63f, .79f, .71f, 1}, "%s", tr("已在地平线上方"));
    }
    ImGui::TextDisabled(tr("高度 %+6.1f°"), moon->altitude() / rad);
    ImGui::SameLine(128);
    ImGui::TextDisabled(tr("方位 %5.1f°"), moon->azimuth() / rad);
    ImGui::TextDisabled(tr("距离 %s km"), number(moon->distance_au * au_km, 0).c_str());
    ImGui::Spacing();
    ImGui::BeginDisabled(frame.busy);
    if (frame.scene.horizon.visible(moon->observed, moon->angular_radius)) {
        if (primary(tr("拉近看月亮"), {-1, 36})) {
            focus(state, frame.scene, *moon, true);
        }
    } else if (primary(tr("跳到可观月时刻"), {-1, 36})) {
        state.playing = false;
        actions.seek_moon = true;
    }
    if (!frame.scene.horizon.visible(moon->observed, moon->angular_radius) &&
        ImGui::IsItemHovered(ImGuiHoveredFlags_AllowWhenDisabled)) {
        ImGui::SetTooltip("%s",
                          tr("在未来 35 天内寻找：月球高度 ≥ 8°，太阳高度 ≤ "
                             "−6°\n每半小时采样，保留当前地点和时间标准"));
    }
    if (ImGui::Button(tr("定位月球"), {102, 30})) {
        focus(state, frame.scene, *moon);
    }
    ImGui::SameLine();
    if (ImGui::Button(tr("星空全景"), {102, 30})) {
        state.track = false;
        frame.scene.fov = default_camera_fov / rad;
        frame.scene.elevation = 35;
        frame.scene.roll = 0;
        frame.scene.projection = ProjectionKind::Perspective;
    }
    ImGui::EndDisabled();
    ImGui::End();
}

std::string lower(std::string text) {
    std::transform(text.begin(), text.end(), text.begin(), [](unsigned char character) {
        return character < 128 ? char(std::tolower(character)) : char(character);
    });
    return text;
}

void draw_search(UiState& state, const UiFrame& frame, UiActions& actions) {
    const Translator tr{state.language};
    if (state.panel != UiPanel::Search || !frame.sky || !frame.engine) {
        return;
    }

    struct Match {
        const Object* object;
        std::string name;
        std::optional<uint32_t> index;
    };

    std::vector<Match> matches;
    const auto needle = lower(state.search);
    auto search_objects = [&](const std::vector<Object>& objects) {
        for (const auto& object : objects) {
            if (matches.size() >= 6) {
                break;
            }
            auto name =
                object.body
                    ? body_name(object.body)
                    : frame.engine->catalog.name(frame.engine->catalog.stars[object.catalog_index]);
            auto searchable = name + (object.hip ? " HIP " + std::to_string(object.hip) : "");
            if (lower(searchable).find(needle) != std::string::npos) {
                matches.push_back({&object, tr(name.c_str()), {}});
            }
        }
    };
    search_objects(frame.sky->bodies);
    if (state.search_query != needle || state.search_catalog != &frame.engine->catalog) {
        state.search_query = needle;
        state.search_catalog = &frame.engine->catalog;
        state.search_matches = frame.engine->catalog.search(needle);
    }
    for (const auto index : state.search_matches) {
        if (matches.size() == 6) {
            break;
        }
        const auto name = frame.engine->catalog.name(frame.engine->catalog.stars[index]);
        matches.push_back({nullptr, tr(name.c_str()), index});
    }
    window("search-results",
           frame.width - 274.f,
           80,
           250,
           std::max(96.f, 32.f + float(matches.size()) * 38));
    for (size_t i = 0; i < matches.size(); ++i) {
        ImGui::PushID(int(i));
        if (ImGui::Selectable(matches[i].name.c_str(), false, 0, {0, 30})) {
            try {
                if (matches[i].index) {
                    auto selected = frame.engine->compute(
                        frame.scene, 0, nullptr, SkyEngine::Scope::FullSky, matches[i].index);
                    if (!selected->stars.empty()) {
                        const auto& source = selected->stars.front();
                        frame.scene.magnitude =
                            std::min(16., std::max(frame.scene.magnitude, source.magnitude + 1));
                        focus(state, frame.scene, source);
                        actions.recompute = true;
                    }
                } else {
                    focus(state, frame.scene, *matches[i].object);
                }
                state.panel = UiPanel::None;
                state.search[0] = '\0';
            } catch (const std::exception& error) {
                state.notice = error.what();
            }
        }
        ImGui::PopID();
    }
    if (matches.empty()) {
        ImGui::TextWrapped("%s", tr("没有匹配的天体。支持恒星名称、完整 HIP 或 Gaia DR3 编号。"));
    }
    ImGui::End();
}

void draw_selection(UiState& state, const UiFrame& frame) {
    const Translator tr{state.language};
    auto* source = find_object(
        state.selected_body ? frame.sky : frame.stars, state.selected, state.selected_body);
    std::optional<Object> current_object;
    if (source && frame.sky) {
        current_object = source->body ? *source : observe_star(*source, *frame.sky);
    }
    const auto* object = current_object ? &*current_object : nullptr;
    if (!object || !frame.engine) {
        return;
    }
    if (object->body == 301) {
        ImGui::SetNextWindowBgAlpha(.75f);
        ImGui::PushStyleVar(ImGuiStyleVar_WindowPadding, ImVec2{12, 6});
        window("tracking", (frame.width - 250.f) / 2, frame.height - 155.f, 250, 44);
        ImGui::Checkbox(tr("跟踪月球"), &state.track);
        ImGui::SameLine();
        ImGui::TextDisabled("%s", tr("Esc 取消"));
        ImGui::End();
        ImGui::PopStyleVar();
        return;
    }
    window("selection", frame.width - 274.f, 428, 250, std::min(248.f, frame.height - 540.f));
    auto name =
        object->body
            ? body_name(object->body)
            : frame.engine->catalog.name(frame.engine->catalog.stars[object->catalog_index]);
    ImGui::TextWrapped("%s", tr(name.c_str()));
    ImGui::TextDisabled(tr("高度 %+6.1f°"), object->altitude() / rad);
    ImGui::SameLine(128);
    ImGui::TextDisabled(tr("方位 %5.1f°"), object->azimuth() / rad);
    ImGui::TextDisabled(tr("星等 %.2f"), object->magnitude);
    if (object->distance_au > 0) {
        ImGui::TextDisabled(tr("距离 %s %s"),
                            number(object->distance_au / (object->body ? 1 : 63241.077), 3).c_str(),
                            object->body ? "AU" : "ly");
    }
    if (object->altitude() < 0) {
        ImGui::TextColored(accent, "%s", tr("当前在地平线以下"));
    }
    ImGui::Checkbox(tr("跟踪此天体"), &state.track);
    if (ImGui::CollapsingHeader(tr("观测详情###details"))) {
        ImGui::TextWrapped("%s", tr.message(quality_text(*object)).c_str());
        ImGui::TextDisabled(tr("赤经 %.4f°"), wrap(atan2(object->icrs.y, object->icrs.x)) / rad);
        ImGui::TextDisabled(tr("赤纬 %.4f°"), asin(object->icrs.z) / rad);
        ImGui::TextDisabled("%s", tr("视方向 / ICRS 轴"));
        if (!object->body) {
            ImGui::TextWrapped(tr("形式误差约 %.2f″（不含模型）"), object->formal_error_arcsec);
        }
    }
    ImGui::End();
}

void draw_transport(UiState& state, const UiFrame& frame, UiActions& actions) {
    const Translator tr{state.language};
    ImGui::PushStyleVar(ImGuiStyleVar_WindowPadding, ImVec2{16, 12});
    window("transport", (frame.width - 644.f) / 2, frame.height - 96.f, 644, 64);
    if (ImGui::Button(tr("−1 日"), {70, 38})) {
        step_time(state, frame.scene, actions, -86400);
    }
    ImGui::SameLine();
    if (ImGui::Button(tr("−1 时"), {70, 38})) {
        step_time(state, frame.scene, actions, -3600);
    }
    ImGui::SameLine();
    if (primary(state.playing ? tr("暂停") : tr("播放"), {74, 38})) {
        state.playing = !state.playing;
    }
    ImGui::SameLine();
    if (ImGui::Button(tr("+1 时"), {70, 38})) {
        step_time(state, frame.scene, actions, 3600);
    }
    ImGui::SameLine();
    if (ImGui::Button(tr("+1 日"), {70, 38})) {
        step_time(state, frame.scene, actions, 86400);
    }
    ImGui::SameLine();
    ImGui::SetNextItemWidth(114);
    const char* rates[] = {
        tr("1 倍"), tr("60 倍"), tr("3600 倍"), tr("1 日 / 秒"), tr("1 年 / 秒")};
    if (ImGui::Combo("##speed", &state.rate, rates, 5)) {
        const double speeds[] = {1, 60, 3600, 86400, 31557600};
        state.speed = std::copysign(speeds[state.rate], state.speed);
    }
    ImGui::SameLine();
    if (ImGui::Button(state.speed < 0 ? tr("逆行") : tr("顺行"), {82, 38})) {
        state.speed = -state.speed;
    }
    ImGui::End();
    ImGui::PopStyleVar();
    auto* draw = ImGui::GetBackgroundDrawList();
    draw->AddText({24, float(frame.height - 25)},
                  IM_COL32(133, 144, 158, 200),
                  tr("拖动环顾 · 滚轮缩放 · R 恢复水平 · H 沉浸模式"));
    auto view = "FOV " + number(frame.scene.fov) + "°";
    if (frame.busy) {
        view = tr("正在计算…  /  ") + view;
    }
    auto width = ImGui::CalcTextSize(view.c_str()).x;
    draw->AddText({frame.width - width - 24, float(frame.height - 25)},
                  IM_COL32(133, 144, 158, 200),
                  view.c_str());
}
} // namespace

void initialize_ui_style() {
    auto& style = ImGui::GetStyle();
    style.WindowRounding = 16;
    style.FrameRounding = 8;
    style.GrabRounding = 6;
    style.PopupRounding = 10;
    style.WindowBorderSize = 1;
    style.WindowPadding = {18, 16};
    style.FramePadding = {10, 8};
    style.ItemSpacing = {8, 8};
    style.ScrollbarSize = 4;
    style.Colors[ImGuiCol_WindowBg] = {.045f, .058f, .079f, .94f};
    style.Colors[ImGuiCol_PopupBg] = {.07f, .085f, .11f, .99f};
    style.Colors[ImGuiCol_Border] = {.34f, .38f, .44f, .32f};
    style.Colors[ImGuiCol_Text] = ink;
    style.Colors[ImGuiCol_TextDisabled] = muted;
    style.Colors[ImGuiCol_FrameBg] = {.11f, .13f, .16f, 1};
    style.Colors[ImGuiCol_FrameBgHovered] = {.16f, .19f, .23f, 1};
    style.Colors[ImGuiCol_FrameBgActive] = {.21f, .24f, .28f, 1};
    style.Colors[ImGuiCol_Button] = {.12f, .145f, .18f, 1};
    style.Colors[ImGuiCol_ButtonHovered] = {.21f, .24f, .29f, 1};
    style.Colors[ImGuiCol_ButtonActive] = {.27f, .30f, .35f, 1};
    style.Colors[ImGuiCol_CheckMark] = accent;
    style.Colors[ImGuiCol_SliderGrab] = accent;
    style.Colors[ImGuiCol_SliderGrabActive] = {.95f, .88f, .71f, 1};
    style.Colors[ImGuiCol_Header] = {.15f, .17f, .21f, 1};
    style.Colors[ImGuiCol_HeaderHovered] = {.23f, .26f, .30f, 1};
    style.Colors[ImGuiCol_HeaderActive] = {.27f, .30f, .34f, 1};
    style.Colors[ImGuiCol_Separator] = {.34f, .38f, .44f, .32f};
}

UiActions draw_ui(UiState& state, const UiFrame& frame) {
    UiActions actions;
    draw_header(state, frame);
    draw_rail(state);
    if (state.panel != UiPanel::Search) {
        draw_moon(state, frame, actions);
        draw_selection(state, frame);
    }
    draw_transport(state, frame, actions);
    draw_drawer(state, frame, actions);
    draw_search(state, frame, actions);
    draw_exploration(state, frame, actions);
    return actions;
}
} // namespace astro
