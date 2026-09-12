#include "astro/explore.hpp"
#include "astro/text.hpp"
#include "astro/ui.hpp"
#include "json.hpp"
#include <SDL3/SDL.h>
#include <chrono>
#include <fstream>
#include <iomanip>
#include <sstream>

namespace astro {
namespace {
constexpr int bodies[] = {10, 301, 199, 299, 4, 5, 6, 7, 8};

struct Dialog {
    std::shared_ptr<Exploration> state;
    bool folder;
};

void SDLCALL dialog_result(void* userdata, const char* const* files, int) {
    std::unique_ptr<Dialog> dialog(static_cast<Dialog*>(userdata));
    std::lock_guard lock(dialog->state->dialog_mutex);
    if (files && files[0]) {
        (dialog->folder ? dialog->state->dialog_output : dialog->state->dialog_horizon) = files[0];
    }
    dialog->state->dialog_open = false;
}

void browse(std::shared_ptr<Exploration> state, bool folder) {
    {
        std::lock_guard lock(state->dialog_mutex);
        if (state->dialog_open) {
            return;
        }
        state->dialog_open = true;
    }
    auto* dialog = new Dialog{state, folder};
    if (folder) {
        SDL_ShowOpenFolderDialog(dialog_result, dialog, nullptr, nullptr, false);
    } else {
        SDL_ShowOpenFileDialog(dialog_result, dialog, nullptr, nullptr, 0, nullptr, false);
    }
}

void body_combo(const char* label, int& index, const Translator& tr) {
    ImGui::SetNextItemWidth(-1);
    if (ImGui::BeginCombo(label, tr(body_name(bodies[index]).c_str()))) {
        for (int i = 0; i < 9; ++i) {
            if (ImGui::Selectable(tr(body_name(bodies[i]).c_str()), index == i)) {
                index = i;
            }
        }
        ImGui::EndCombo();
    }
}

Target selected_target(const UiState& ui, const UiFrame& frame, int fallback, bool use) {
    if (use && ui.selected) {
        if (ui.selected_body) {
            return {int(ui.selected), {}};
        }
        for (const auto& star : frame.stars->stars) {
            if (star.id == ui.selected) {
                return {0, star.catalog_index};
            }
        }
    }
    if (use) {
        throw std::invalid_argument("Select an object first");
    }
    return {bodies[fallback], {}};
}

void jump(UiState& ui,
          const UiFrame& frame,
          UiActions& actions,
          const Scenario& base,
          double seconds,
          Target target,
          bool play = false) {
    frame.scene = scene_at(base, seconds);
    const auto sky = frame.engine->compute(frame.scene,
                                           0,
                                           nullptr,
                                           target.star ? SkyEngine::Scope::FullSky
                                                       : SkyEngine::Scope::SolarSystem,
                                           target.star);
    const auto object = target_object(*sky, target);
    frame.scene.azimuth = object.azimuth() / rad;
    frame.scene.elevation = object.altitude() / rad;
    ui.selected = object.id;
    ui.selected_body = object.body != 0;
    ui.track = true;
    ui.playing = play;
    if (play) {
        ui.speed = 120;
    }
    actions.recompute = true;
}

std::string clock_text(const Scenario& initial, double seconds) {
    return format_date(scene_at(initial, seconds).date);
}

void plot_plan(UiState& ui, const UiFrame& frame, UiActions& actions, const ObservationPlan& plan) {
    const Translator tr{ui.language};
    const ImVec2 origin = ImGui::GetCursorScreenPos();
    const ImVec2 size{ImGui::GetContentRegionAvail().x, 190};
    ImGui::InvisibleButton("plan-chart", size);
    auto* draw = ImGui::GetWindowDrawList();
    draw->AddRectFilled(
        origin, {origin.x + size.x, origin.y + size.y}, IM_COL32(12, 19, 30, 255), 8);
    auto xy = [&](double t, double altitude) {
        return ImVec2{origin.x + float(t / 86400) * size.x,
                      origin.y + size.y -
                          float((std::clamp(altitude / rad, -30., 90.) + 30) / 120) * size.y};
    };
    for (const auto& window : plan.windows) {
        draw->AddRectFilled({xy(window.start, 0).x, origin.y},
                            {xy(window.end, 0).x, origin.y + size.y},
                            IM_COL32(43, 116, 83, 45));
    }
    for (double altitude : {-18., 0., 30., 60.}) {
        draw->AddLine(xy(0, altitude * rad), xy(86400, altitude * rad), IM_COL32(80, 90, 100, 70));
        draw->AddText(xy(0, altitude * rad),
                      IM_COL32(140, 150, 160, 180),
                      std::to_string(int(altitude)).c_str());
    }
    for (size_t i = 1; i < plan.samples.size(); ++i) {
        const auto& a = plan.samples[i - 1];
        const auto& b = plan.samples[i];
        draw->AddLine(
            xy(a.seconds, a.altitude), xy(b.seconds, b.altitude), IM_COL32(118, 215, 217, 255), 2);
        draw->AddLine(xy(a.seconds, a.sun_altitude),
                      xy(b.seconds, b.sun_altitude),
                      IM_COL32(226, 173, 103, 200));
        draw->AddLine(xy(a.seconds, a.moon_altitude),
                      xy(b.seconds, b.moon_altitude),
                      IM_COL32(162, 161, 213, 200));
    }
    for (int hour : {0, 6, 12, 18, 24}) {
        const auto text = clock_text(plan.initial, hour * 3600.);
        const auto clock = text.substr(text.size() - 8, 5);
        const auto text_size = ImGui::CalcTextSize(clock.c_str());
        draw->AddText({std::clamp(xy(hour * 3600., 0).x - text_size.x / 2,
                                  origin.x,
                                  origin.x + size.x - text_size.x),
                       origin.y + size.y - text_size.y},
                      IM_COL32(150, 163, 180, 210),
                      clock.c_str());
    }
    if (ImGui::IsItemHovered()) {
        const auto i =
            size_t(std::clamp((ImGui::GetIO().MousePos.x - origin.x) / size.x, 0.f, 1.f) *
                   (plan.samples.size() - 1));
        const auto& point = plan.samples[i];
        ImGui::SetTooltip("%s\n%s %.1f°\n%s %.1f°",
                          clock_text(plan.initial, point.seconds).c_str(),
                          tr("目标高度"),
                          point.altitude / rad,
                          tr("距月球"),
                          point.moon_separation / rad);
        if (ImGui::IsMouseClicked(ImGuiMouseButton_Left)) {
            jump(ui, frame, actions, plan.initial, point.seconds, plan.target);
        }
    }
    ImGui::TextWrapped("%s", tr("青色：目标  金色：太阳  紫色：月球；点击曲线前往"));
}
} // namespace

void draw_exploration(UiState& ui, const UiFrame& frame, UiActions& actions) {
    auto& state = *ui.exploration;
    const Translator tr{ui.language};
    if (state.job.valid() &&
        state.job.wait_for(std::chrono::seconds(0)) == std::future_status::ready) {
        try {
            state.result = state.job.get();
        } catch (const std::exception& e) {
            ui.notice = e.what();
        }
    }
    {
        std::lock_guard lock(state.dialog_mutex);
        if (!state.dialog_horizon.empty()) {
            snprintf(
                state.horizon_path, sizeof(state.horizon_path), "%s", state.dialog_horizon.c_str());
            state.dialog_horizon.clear();
        }
        if (!state.dialog_output.empty()) {
            snprintf(
                state.output_path, sizeof(state.output_path), "%s", state.dialog_output.c_str());
            state.dialog_output.clear();
        }
    }
    if (ui.panel != UiPanel::Explore || !frame.sky || !frame.engine) {
        return;
    }
    ImGui::SetNextWindowPos({102, 112});
    ImGui::SetNextWindowSize(
        {std::min(470.f, frame.width - 210.f), std::min(680.f, frame.height - 220.f)});
    constexpr auto flags =
        ImGuiWindowFlags_NoSavedSettings | ImGuiWindowFlags_NoMove | ImGuiWindowFlags_NoResize;
    ImGui::Begin("exploration", nullptr, flags | ImGuiWindowFlags_NoTitleBar);
    ImGui::TextColored({.84f, .78f, .63f, 1}, "%s", tr("探索天空"));
    ImGui::SameLine(ImGui::GetWindowWidth() - 70);
    if (ImGui::SmallButton(tr("关闭"))) {
        ui.panel = UiPanel::None;
    }
    ImGui::Spacing();
    const char* tabs[] = {
        tr("星座与年代"), tr("观测计划"), tr("地景"), tr("录制与导出"), tr("天象搜索")};
    ImGui::SetNextItemWidth(-1);
    ImGui::Combo("##explore-section", &state.tab, tabs, 5);
    ImGui::Spacing();
    try {
        if (state.tab == 0) {
            actions.recompute |= ImGui::Checkbox(tr("星座连线"), &frame.scene.constellations);
            ImGui::SameLine();
            ImGui::Checkbox(tr("星座名称"), &frame.scene.constellation_labels);
            const auto& figures = frame.engine->catalog.constellations;
            if (!figures.empty()) {
                ImGui::SetNextItemWidth(-1);
                if (ImGui::BeginCombo("##constellation", tr(figures[state.figure].name.c_str()))) {
                    for (size_t i = 0; i < figures.size(); ++i) {
                        if (ImGui::Selectable(tr(figures[i].name.c_str()),
                                              state.figure == int(i))) {
                            state.figure = int(i);
                        }
                    }
                    ImGui::EndCombo();
                }
                if (ImGui::Button(tr("定位星座"), {-1, 32}) &&
                    !figures[state.figure].segments.empty()) {
                    auto base = frame.scene;
                    base.constellations = true;
                    auto computed = frame.engine->compute(base);
                    Vec3 centre{};
                    for (const auto& segment : figures[state.figure].segments) {
                        for (auto index : segment) {
                            if (auto it = computed->guide_stars.find(index);
                                it != computed->guide_stars.end()) {
                                centre = centre + it->second.observed;
                            }
                        }
                    }
                    frame.scene = base;
                    if (norm(centre) > 1e-8) {
                        centre = unit(centre);
                        frame.scene.azimuth = wrap(atan2(centre.x, centre.y)) / rad;
                        frame.scene.elevation = asin(centre.z) / rad;
                    }
                    ui.track = false;
                    frame.scene.fov = 60;
                    actions.recompute = true;
                }
            }
            ImGui::TextWrapped("%s", tr("连线跟随恒星运动；现代星座图形不代表古代星官。"));
            ImGui::Separator();
            ImGui::Checkbox(tr("双年代并排对照"), &frame.scene.compare);
            ImGui::TextUnformatted(tr("对照年份"));
            ImGui::SetNextItemWidth(-1);
            ImGui::SliderInt("##comparison-year",
                             &frame.scene.comparison_year,
                             -3000,
                             6999,
                             "%d",
                             ImGuiSliderFlags_AlwaysClamp);
            ImGui::TextWrapped(
                "%s", tr("两侧共用地点、月日、时刻和镜头。拖动同步环顾，在左侧选取天体。"));
            ImGui::Separator();
            actions.recompute |= ImGui::Checkbox(tr("月面地形与阴影"), &frame.scene.moon_surface);
            actions.recompute |= ImGui::Checkbox(tr("地球照"), &frame.scene.earthshine);
            if (ImGui::Button(tr("月面细节曝光"), {-1, 30})) {
                frame.scene.auto_exposure = false;
                frame.scene.adaptive_exposure = false;
                frame.scene.exposure = exp2(-18.);
                jump(ui, frame, actions, frame.scene, 0, {301, {}});
                frame.scene.fov = 1.2;
            }
            ImGui::TextWrapped("%s",
                               tr(frame.sky->lunar.precise_orientation
                                      ? "月面姿态：DE440 数值天平动"
                                      : "月面姿态：IAU 近似，远年代未经精度验证"));
        } else if (state.tab == 1) {
            body_combo("##plan-target", state.target_body, tr);
            ImGui::Checkbox(tr("使用当前选中天体"), &state.use_selected);
            ImGui::BeginDisabled(state.job.valid());
            if (ImGui::Button(tr("计算未来 24 小时"), {-1, 36})) {
                auto target = selected_target(ui, frame, state.target_body, state.use_selected);
                const auto initial = frame.scene;
                const auto engine = frame.shared_engine;
                state.cancelled->store(false);
                auto cancel = state.cancelled;
                state.job = std::async(std::launch::async, [engine, initial, target, cancel] {
                    ExploreResult result;
                    result.scene = initial;
                    result.plan = observing_plan(*engine, initial, target, [cancel] {
                        return cancel->load();
                    });
                    return result;
                });
            }
            ImGui::EndDisabled();
            if (state.result && state.result->plan) {
                const auto& plan = *state.result->plan;
                ImGui::TextWrapped("%s · %s",
                                   tr(plan.initial.location_name.c_str()),
                                   format_date(plan.initial.date).c_str());
                plot_plan(ui, frame, actions, plan);
                ImGui::Text("%s: %s (%.1f°)",
                            tr("最高位置"),
                            clock_text(plan.initial, plan.transit).c_str(),
                            plan.transit_altitude / rad);
                for (const auto time : plan.rises) {
                    ImGui::Text("%s: %s", tr("升起"), clock_text(plan.initial, time).c_str());
                }
                for (const auto time : plan.sets) {
                    ImGui::Text("%s: %s", tr("落下"), clock_text(plan.initial, time).c_str());
                }
                if (plan.rises.empty() && plan.sets.empty()) {
                    ImGui::TextWrapped("%s", tr("本区间没有升落事件。"));
                }
                if (plan.has_recommendation) {
                    if (ImGui::Button(tr("前往推荐时刻"), {-1, 32})) {
                        jump(ui, frame, actions, plan.initial, plan.best_time, plan.target);
                    }
                    for (const auto& interval : plan.windows) {
                        ImGui::TextWrapped("%s — %s",
                                           clock_text(plan.initial, interval.start).c_str(),
                                           clock_text(plan.initial, interval.end).c_str());
                    }
                } else {
                    ImGui::TextWrapped("%s", tr("本区间没有满足条件的观测窗口。"));
                }
                if (ImGui::Button(tr("导出观测表"), {-1, 30})) {
                    std::ofstream out(frame.user / "observing-plan.csv");
                    out << "time,altitude_deg,sun_altitude_deg,moon_separation_deg,sky_cd_m2,"
                           "suitable\n";
                    for (const auto& p : plan.samples) {
                        out << clock_text(plan.initial, p.seconds) << ',' << p.altitude / rad << ','
                            << p.sun_altitude / rad << ',' << p.moon_separation / rad << ','
                            << p.sky_luminance << ',' << p.suitable << '\n';
                    }
                    if (!out) {
                        throw std::runtime_error("Cannot write observation plan");
                    }
                    save_scenario(plan.initial,
                                  frame.user / "observing-plan.json",
                                  frame.engine->catalog.data_id);
                    ui.notice = (frame.user / "observing-plan.csv").string();
                }
            }
            ImGui::TextWrapped(
                "%s",
                tr("按高度、夜色、月光和地平线筛选；不包含天气预报。绿色窗口按 5 分钟采样。"));
        } else if (state.tab == 2) {
            ImGui::TextWrapped(
                "%s",
                tr("导入 CSV 的方位角、高度角两列，或 JSON points 数组。北方为 0°，向东增加。"));
            ImGui::SetNextItemWidth(-1);
            ImGui::InputText("##horizon-file", state.horizon_path, sizeof(state.horizon_path));
            if (ImGui::Button(tr("选择轮廓文件"))) {
                browse(ui.exploration, false);
            }
            ImGui::SameLine();
            if (ImGui::Button(tr("应用地平线"))) {
                frame.scene.horizon = load_horizon(state.horizon_path);
                frame.scene.ground = true;
                actions.recompute = true;
            }
            ImGui::TextWrapped("%s: %s",
                               tr("当前地景"),
                               frame.scene.horizon.name.empty() ? tr("平坦地平线")
                                                                : frame.scene.horizon.name.c_str());
            if (ImGui::Button(tr("恢复平坦地平线"), {-1, 32})) {
                frame.scene.horizon = {};
                actions.recompute = true;
            }
            ImGui::TextWrapped(
                "%s", tr("轮廓会遮挡天体，并参与升落和观测计划计算；保存场景时一并保存。"));
        } else if (state.tab == 3) {
            ImGui::SetNextItemWidth(-1);
            ImGui::InputTextWithHint(
                "##export-folder", tr("导出目录"), state.output_path, sizeof(state.output_path));
            if (ImGui::Button(tr("选择导出位置"))) {
                browse(ui.exploration, true);
            }
            ImGui::TextUnformatted(tr("帧数"));
            ImGui::SetNextItemWidth(-1);
            ImGui::InputInt("##frame-count", &state.sequence.frames);
            ImGui::TextUnformatted(tr("每帧推进秒数"));
            ImGui::SetNextItemWidth(-1);
            ImGui::InputDouble("##frame-step", &state.sequence.step_seconds, 0, 0, "%.3f");
            ImGui::TextUnformatted(tr("视频帧率"));
            ImGui::SetNextItemWidth(-1);
            ImGui::InputInt("##video-fps", &state.sequence.fps);
            ImGui::Checkbox(tr("同时导出 MP4"), &state.sequence.video);
            ImGui::Checkbox(tr("星轨叠加图"), &state.sequence.trails);
            ImGui::Checkbox(tr("锁定曝光"), &state.sequence.lock_exposure);
            ImGui::Checkbox(tr("镜头运动"), &state.sequence.camera_path);
            if (ImGui::Button(tr("将当前视角设为终点"), {-1, 30})) {
                state.sequence.camera_end = frame.scene;
            }
            ImGui::TextWrapped(
                "%s", tr("开始时的视角作为起点；星轨使用逐帧变亮叠加，不等同于物理长曝光。"));
            ImGui::BeginDisabled(state.recording);
            if (ImGui::Button(tr("开始导出"), {-1, 36})) {
                state.sequence.directory = state.output_path[0]
                                               ? std::filesystem::path(state.output_path)
                                               : frame.user / "exports";
                state.begin_recording = true;
            }
            ImGui::EndDisabled();
            if (state.recording) {
                ImGui::Text(
                    "%s %d / %d", tr("正在导出"), state.completed_frames, state.sequence.frames);
                if (ImGui::Button(tr("取消导出"), {-1, 30})) {
                    state.cancel_recording = true;
                }
            }
            ImGui::TextWrapped(
                "%s",
                tr("每帧等待精确计算，保存 PNG 和场景。MP4 使用 FFmpeg；取消后保留已完成帧。"));
        } else {
            const char* kinds[] = {tr("日食"), tr("月食"), tr("天体接近"), tr("掩星")};
            ImGui::SetNextItemWidth(-1);
            ImGui::Combo("##event-kind", &state.event_kind, kinds, 4);
            if (state.event_kind >= 2) {
                body_combo("##event-first", state.first_body, tr);
                body_combo("##event-second", state.second_body, tr);
                ImGui::Checkbox(tr("第二目标使用选中天体"), &state.use_selected_event);
            }
            ImGui::TextUnformatted(tr("搜索天数"));
            ImGui::SetNextItemWidth(-1);
            ImGui::InputDouble("##event-days", &state.event_days, 0, 0, "%.1f");
            if (state.event_kind == 2) {
                ImGui::TextUnformatted(tr("最大角距（度）"));
                ImGui::SetNextItemWidth(-1);
                ImGui::InputDouble("##event-separation", &state.separation_degrees, 0, 0, "%.2f");
            }
            ImGui::BeginDisabled(state.job.valid());
            if (ImGui::Button(tr("搜索天象"), {-1, 36})) {
                EventSearch request;
                request.kind = EventKind(state.event_kind);
                request.days = state.event_days;
                request.maximum_separation = state.separation_degrees * rad;
                request.first = {bodies[state.first_body], {}};
                request.second = selected_target(ui,
                                                 frame,
                                                 state.second_body,
                                                 state.event_kind >= 2 && state.use_selected_event);
                const auto initial = frame.scene;
                const auto engine = frame.shared_engine;
                state.cancelled->store(false);
                auto cancel = state.cancelled;
                state.job = std::async(std::launch::async, [engine, initial, request, cancel] {
                    ExploreResult result;
                    result.scene = initial;
                    result.search = request;
                    result.events = find_events(*engine, initial, request, [cancel] {
                        return cancel->load();
                    });
                    return result;
                });
            }
            ImGui::EndDisabled();
            if (state.result && !state.result->plan) {
                ImGui::TextWrapped("%s · %s",
                                   tr(state.result->scene.location_name.c_str()),
                                   format_date(state.result->scene.date).c_str());
                if (state.result->events.empty()) {
                    ImGui::TextWrapped("%s", tr("此区间未找到符合条件的天象。"));
                }
                for (size_t i = 0; i < state.result->events.size(); ++i) {
                    const auto& event = state.result->events[i];
                    ImGui::PushID(int(i));
                    ImGui::Separator();
                    ImGui::TextWrapped("%s · %s",
                                       tr(event.type.c_str()),
                                       tr(event.visible ? "当地可见" : "地平线以下"));
                    ImGui::TextWrapped("%s", clock_text(state.result->scene, event.peak).c_str());
                    ImGui::Text("%s %.4f°", tr("最小角距"), event.separation / rad);
                    if (event.start) {
                        ImGui::TextWrapped("%s: %s",
                                           tr("开始接触"),
                                           clock_text(state.result->scene, *event.start).c_str());
                    }
                    if (event.end) {
                        ImGui::TextWrapped("%s: %s",
                                           tr("结束接触"),
                                           clock_text(state.result->scene, *event.end).c_str());
                    }
                    Target target{event.kind == EventKind::SolarEclipse ? 10 : 301, {}};
                    if (event.kind == EventKind::CloseApproach ||
                        event.kind == EventKind::Occultation) {
                        target = state.result->search.first;
                    }
                    if (ImGui::Button(tr("查看峰值"))) {
                        jump(ui, frame, actions, state.result->scene, event.peak, target);
                    }
                    ImGui::SameLine();
                    if (ImGui::Button(tr("播放过程"))) {
                        jump(ui,
                             frame,
                             actions,
                             state.result->scene,
                             event.start.value_or(event.peak - 3600) - 300,
                             target,
                             true);
                    }
                    ImGui::PopID();
                }
            }
            ImGui::TextWrapped(
                "%s",
                tr("接触时刻采用球形天体模型；远年代受 ΔT 影响。月食红光及日食天空变暗为近似。"));
        }
        if (state.job.valid()) {
            ImGui::Separator();
            ImGui::TextUnformatted(tr("后台计算中…"));
            if (ImGui::Button(tr("取消计算"), {-1, 30})) {
                state.cancelled->store(true);
            }
        }
    } catch (const std::exception& e) {
        ui.notice = e.what();
    }
    ImGui::End();
}
} // namespace astro
