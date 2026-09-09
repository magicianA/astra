#pragma once

#include <filesystem>
#include <optional>
#include <span>
#include <string>
#include <string_view>

namespace astro {
enum class Language { English, Chinese };
enum class TranslationMode { Exact, Prefix, Fragment };

struct Translation {
    const char* chinese;
    const char* english;
    TranslationMode mode = TranslationMode::Exact;
};

std::span<const Translation> translations();
std::optional<Language> parse_language(std::string_view locale);
const char* language_code(Language);
Language load_language(const std::filesystem::path&, Language fallback);
void save_language(const std::filesystem::path&, Language);

// No global locale: worker results and exported scenarios retain their source
// strings; the presentation layer translates them in the selected UI language.
struct Translator {
    Language language;
    const char* operator()(const char* source) const;
    std::string message(std::string_view source) const;
};
} // namespace astro
