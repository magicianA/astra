#pragma once
#include "i18n.hpp"
#include "scenario.hpp"
#include <filesystem>

namespace astro {
struct AppOptions {
    std::optional<Language> language;
    std::filesystem::path data = ASTRA_SOURCE_DATA, shaders, screenshot;
    Scenario scenario;
    bool validation = false, smoke = false, hide_ui = false;
    int frames = 0;
    double playback_speed = 0;
};

int run_app(const AppOptions&);
} // namespace astro
