#include "astro/i18n.hpp"
#include "astro/sky.hpp"
#include <chrono>
#include <fstream>
#include <iostream>
#include <regex>
#include <set>
#include <stdexcept>
#include <vector>

using namespace astro;

namespace {
int checks = 0;

void require(bool condition, const char* message) {
    ++checks;
    if (!condition) {
        throw std::runtime_error(message);
    }
}

std::vector<std::string> formats(const std::string& text) {
    static const std::regex format("%[-+ #0]*[0-9]*(\\.[0-9]+)?[a-zA-Z%]");
    std::vector<std::string> result;
    for (auto it = std::sregex_iterator(text.begin(), text.end(), format);
         it != std::sregex_iterator();
         ++it) {
        // Width and precision may differ, but the argument type and order must match.
        result.push_back(it->str().substr(it->str().size() - 1));
    }
    return result;
}

void check_translations() {
    const Translator en{Language::English}, zh{Language::Chinese};
    std::set<std::string> chinese, english;
    for (const auto& entry : translations()) {
        require(*entry.chinese && *entry.english, "both languages have text");
        require(chinese.insert(entry.chinese).second, "Chinese translation keys are unique");
        require(english.insert(entry.english).second, "English translation keys are unique");
        require(std::string(en(entry.chinese)) == entry.english, "Chinese to English lookup");
        require(std::string(zh(entry.english)) == entry.chinese, "English to Chinese lookup");
        require(formats(entry.chinese) == formats(entry.english), "format argument types match");
        const std::string a = entry.chinese, b = entry.english;
        if (const auto id = a.find("###"); id != std::string::npos) {
            require(b.substr(b.find("###")) == a.substr(id), "widget IDs survive language changes");
        }
    }
    for (int body : {10, 301, 199, 299, 4, 5, 6, 7, 8}) {
        require(std::string(en(body_name(body).c_str())).find(' ') == std::string::npos,
                "English body names have no Chinese prefix");
    }
    require(std::string(en("天狼星 Sirius")) == "Sirius", "bright-star labels translate");
    require(std::string(en("北极 North Pole")) == "North Pole", "preset names retain spaces");
    require(std::string(en("My 自定义 observatory")) == "My 自定义 observatory",
            "user-defined location names are preserved");
    require(en.message("截图已保存：/tmp/我的星空.png") == "Screenshot saved: /tmp/我的星空.png",
            "translated notices preserve paths verbatim");
    require(zh.message("Could not save language preference: /tmp/prefs.json") ==
                "语言设置保存失败：/tmp/prefs.json",
            "preference errors translate without changing the path");
    Object star;
    star.flags = Gaia | RVUnknown | DistanceUnknown | Multiple;
    require(en.message(quality_text(star)) ==
                "Gaia DR3 · unknown radial velocity · unknown distance · multiple-star / "
                "photocentre approximation",
            "all combined quality flags translate");
    require(std::string(en("Cannot open scenario")) == "Cannot open scenario",
            "English technical errors remain readable");
    require(std::string(zh("Cannot open scenario")) == "无法打开场景，请先保存",
            "common user-facing errors translate to Chinese");
}

void check_preferences() {
    for (const char* locale : {"en", "en-US", "en_GB.UTF-8", "EN"}) {
        require(parse_language(locale) == Language::English, "English locale variants");
    }
    for (const char* locale : {"zh", "zh-CN", "zh_Hans_CN", "zh-TW", "ZH"}) {
        require(parse_language(locale) == Language::Chinese, "Chinese locale variants");
    }
    require(!parse_language("fr-FR") && !parse_language("") && !parse_language("english"),
            "unsupported locales fall back explicitly");
    const auto root = std::filesystem::temp_directory_path() /
                      ("astra-i18n-" +
                       std::to_string(std::chrono::steady_clock::now().time_since_epoch().count()));
    const auto file = root / "ui-preferences.json";
    require(load_language(file, Language::Chinese) == Language::Chinese,
            "first launch follows the supplied system language");
    for (auto language : {Language::English, Language::Chinese}) {
        save_language(file, language);
        require(load_language(file,
                              language == Language::English ? Language::Chinese
                                                            : Language::English) == language,
                "saved preference overrides the next launch's system language");
    }
    for (const char* contents :
         {"{broken", "{\"language\":42}", "[]", "{\"language\":\"unknown\"}"}) {
        std::ofstream(file) << contents;
        require(load_language(file, Language::English) == Language::English,
                "invalid preferences do not prevent startup");
    }
    bool failed = false;
    try {
        save_language(root, Language::English);
    } catch (const std::exception&) {
        failed = true;
    }
    require(failed, "preference save failures are reported");
    std::filesystem::remove_all(root);
}
} // namespace

int main() {
    try {
        check_translations();
        check_preferences();
        std::cout << checks << " localization checks passed\n";
    } catch (const std::exception& error) {
        std::cerr << "Localization test failed after " << checks << " checks: " << error.what()
                  << '\n';
        return 1;
    }
}
