#include "astro/app.hpp"
#include <SDL3/SDL.h>
#include <cmath>
#include <cstdlib>
#include <iostream>
#include <stdexcept>
using namespace astro;

int main(int argc, char** argv) {
    try {
        AppOptions options;
        options.shaders = ASTRA_SHADER_DIR;
        if (auto* p = std::getenv("ASTRA_DATA")) {
            options.data = p;
        }
        auto executable = std::filesystem::absolute(argv[0]).parent_path();
        auto resources = executable.parent_path() / "Resources";
        if (std::filesystem::exists(resources / "data/catalog/manifest.json")) {
            options.data = resources / "data";
        }
        if (std::filesystem::exists(resources / "shaders")) {
            options.shaders = resources / "shaders";
        }
#ifdef __APPLE__
        if (std::filesystem::exists(resources / "vulkan/icd.d/MoltenVK_icd.json")) {
            setenv("VK_DRIVER_FILES", (resources / "vulkan/icd.d/MoltenVK_icd.json").c_str(), 0);
        }
#endif
        for (int i = 1; i < argc; i++) {
            std::string arg = argv[i];
            auto value = [&]() {
                if (i + 1 >= argc) {
                    throw std::invalid_argument("Missing argument for " + arg);
                }
                return std::string(argv[++i]);
            };
            if (arg == "--data") {
                options.data = value();
            } else if (arg == "--shaders") {
                options.shaders = value();
            } else if (arg == "--validation") {
                options.validation = true;
            } else if (arg == "--language") {
                options.language = parse_language(value());
                if (!options.language) {
                    throw std::invalid_argument("Supported UI languages: en, zh-CN");
                }
            } else if (arg == "--frames") {
                options.frames = std::stoi(value());
            } else if (arg == "--play-speed") {
                options.playback_speed = std::stod(value());
                if (!std::isfinite(options.playback_speed) ||
                    abs(options.playback_speed) > 31557600) {
                    throw std::invalid_argument(
                        "Playback speed must be finite and at most one year per second");
                }
            } else if (arg == "--screenshot") {
                options.screenshot = value();
            } else if (arg == "--hide-ui") {
                options.hide_ui = true;
            } else if (arg == "--smoke") {
                options.smoke = true;
            } else if (arg == "--scenario") {
                options.scenario = load_scenario(value());
            } else if (arg == "--year") {
                options.scenario.date.year = std::stoi(value());
            } else if (arg == "--date") {
                auto v = value();
                auto& d = options.scenario.date;
                if (sscanf(v.c_str(),
                           "%d-%d-%dT%d:%d:%lf",
                           &d.year,
                           &d.month,
                           &d.day,
                           &d.hour,
                           &d.minute,
                           &d.second) < 3) {
                    throw std::invalid_argument("Invalid date");
                }
            } else if (arg == "--help") {
                std::cout << "Astra: --data DIR --scenario FILE --date YEAR-MM-DDTHH:MM:SS --year "
                             "YEAR --validation --frames N --play-speed RATE --screenshot FILE.png "
                             "--hide-ui --smoke --language en|zh-CN\n";
                return 0;
            } else {
                throw std::invalid_argument("Unknown argument: " + arg);
            }
        }
        return run_app(options);
    } catch (const std::exception& e) {
        std::cerr << "Astra: " << e.what() << '\n';
        if (argc == 1) {
            SDL_ShowSimpleMessageBox(SDL_MESSAGEBOX_ERROR, "Astra", e.what(), nullptr);
        }
        return 1;
    }
}
