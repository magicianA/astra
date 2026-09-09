#pragma once

#include "i18n.hpp"
#include "sky.hpp"
#include <imgui.h>

namespace astro {
enum class UiPanel { None, Location, Time, Display, Search };

struct UiState {
    Language language = Language::English;
    UiPanel panel = UiPanel::None;
    bool visible = true, playing = false, track = false, selected_body = false;
    bool shot_done = false;
    uint64_t selected = 0;
    double speed = 60;
    int rate = 1;
    char search[96]{};
    std::string notice;
    std::filesystem::path pending_shot;
};

struct UiFrame {
    Scenario& scene;
    const SkySnapshot* sky;
    const SkyEngine* engine;
    const std::filesystem::path& user;
    int width, height;
    bool busy;
    ImFont* title;
    const SkySnapshot* stars;
};

struct UiActions {
    bool recompute = false, seek_moon = false;
    bool language_changed = false;
};

void initialize_ui_style();
UiActions draw_ui(UiState&, const UiFrame&);
} // namespace astro
