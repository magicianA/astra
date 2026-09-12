#include "astro/catalog.hpp"
#include "json.hpp"
#include <bit>
#include <cctype>
#include <charconv>
#include <cstring>
#include <fstream>
#include <stdexcept>

namespace astro {
Catalog::Catalog(const std::filesystem::path& root) {
    if constexpr (std::endian::native != std::endian::little) {
        throw std::runtime_error("This data reader requires a little-endian host");
    }
    std::ifstream in(root / "catalog/stars.bin", std::ios::binary);
    char magic[8];
    uint64_t count = 0, size = 0;
    in.read(magic, 8);
    in.read((char*)&count, 8);
    in.read((char*)&size, 8);
    if (!in || memcmp(magic, "ASTARS01", 8) || size != sizeof(Star) || count < 1 ||
        count > 10000000) {
        throw std::runtime_error("Missing/invalid star pack; run scripts/build_catalog.py");
    }
    stars.resize(count);
    in.read((char*)stars.data(), std::streamsize(count * sizeof(Star)));
    if (!in) {
        throw std::runtime_error("Truncated star pack");
    }
    std::ifstream names(root / "catalog/names.json");
    if (names) {
        auto j = nlohmann::json::parse(names);
        for (auto it = j.begin(); it != j.end(); ++it) {
            names_[std::stoul(it.key())] = it.value().value("name", "");
        }
    }
    std::ifstream manifest(root / "catalog/manifest.json");
    if (!manifest) {
        throw std::runtime_error("Star pack manifest missing");
    }
    data_id = nlohmann::json::parse(manifest).at("data_id");
    id_order_.reserve(stars.size());
    for (uint32_t i = 0; i < stars.size(); ++i) {
        const auto& star = stars[i];
        if (star.flags & Gaia) {
            id_order_.push_back(i);
        }
        if (star.hip) {
            hip_order_.emplace_back(star.hip, i);
        }
        if (auto found = names_.find(star.hip); found != names_.end()) {
            auto text = found->second;
            std::transform(text.begin(), text.end(), text.begin(), [](unsigned char c) {
                return c < 128 ? char(std::tolower(c)) : char(c);
            });
            search_names_.emplace_back(std::move(text), i);
        }
    }
    std::sort(id_order_.begin(), id_order_.end(), [&](uint32_t a, uint32_t b) {
        return stars[a].id < stars[b].id;
    });
    std::sort(hip_order_.begin(), hip_order_.end());
    constellation_member.resize(stars.size());
    std::ifstream figures(root / "skycultures/western.json");
    if (!figures) {
        throw std::runtime_error("Missing constellation data; run scripts/prepare_exploration.py");
    }
    const auto figure_data = nlohmann::json::parse(figures);
    for (const auto& item : figure_data.at("figures")) {
        Constellation figure;
        figure.id = item.at("id");
        figure.name = item.at("name");
        for (const auto& line : item.at("lines")) {
            for (size_t i = 1; i < line.size(); ++i) {
                auto a = hip_index(line[i - 1].get<uint32_t>());
                auto b = hip_index(line[i].get<uint32_t>());
                if (a && b) {
                    constellation_member[*a] = constellation_member[*b] = true;
                    figure.segments.push_back({*a, *b});
                }
            }
        }
        constellations.push_back(std::move(figure));
    }
}

std::optional<uint32_t> Catalog::hip_index(uint32_t hip) const {
    auto it = std::lower_bound(hip_order_.begin(), hip_order_.end(), std::pair{hip, uint32_t(0)});
    return it != hip_order_.end() && it->first == hip ? std::optional(it->second) : std::nullopt;
}

std::vector<uint32_t> Catalog::search(std::string query, size_t limit) const {
    std::vector<uint32_t> result;
    if (!limit || query.empty()) {
        return result;
    }
    std::transform(query.begin(), query.end(), query.begin(), [](unsigned char c) {
        return c < 128 ? char(std::tolower(c)) : char(c);
    });
    const auto first = query.find_first_not_of(" \t");
    if (first == std::string::npos) {
        return result;
    }
    query = query.substr(first, query.find_last_not_of(" \t") - first + 1);
    auto digits = query;
    const bool gaia = digits.starts_with("gaia");
    for (const auto prefix : {"gaia dr3", "gaia", "hip"}) {
        if (digits.starts_with(prefix)) {
            digits.erase(0, std::string_view(prefix).size());
            break;
        }
    }
    digits.erase(0, std::min(digits.find_first_not_of(' '), digits.size()));
    uint64_t number{};
    const auto [end, error] = std::from_chars(digits.data(), digits.data() + digits.size(), number);
    if (error == std::errc{} && end == digits.data() + digits.size()) {
        if (!gaia && number <= UINT32_MAX) {
            if (auto index = hip_index(uint32_t(number))) {
                result.push_back(*index);
            }
        }
        if (result.empty()) {
            auto it = std::lower_bound(
                id_order_.begin(), id_order_.end(), number, [&](uint32_t i, uint64_t id) {
                    return stars[i].id < id;
                });
            if (it != id_order_.end() && stars[*it].id == number) {
                result.push_back(*it);
            }
        }
        return result;
    }
    for (const auto& [name, index] : search_names_) {
        if (name.find(query) != std::string::npos) {
            result.push_back(index);
            if (result.size() == limit) {
                break;
            }
        }
    }
    return result;
}

std::string Catalog::name(const Star& s) const {
    auto it = names_.find(s.hip);
    return it != names_.end() ? it->second : id(s);
}

std::string Catalog::id(const Star& s) const {
    return s.flags & Gaia ? "Gaia DR3 " + std::to_string(s.id) : "HIP " + std::to_string(s.hip);
}

std::array<float, 3> star_color(double bv) {
    double temp = 4600 * (1 / (.92 * bv + 1.7) + 1 / (.92 * bv + .62));
    temp = std::clamp(temp, 2000., 30000.) / 100.;
    double r, g, b;
    r = temp <= 66 ? 255 : 329.698727446 * pow(temp - 60, -.1332047592);
    g = temp <= 66 ? 99.4708025861 * log(temp) - 161.1195681661
                   : 288.1221695283 * pow(temp - 60, -.0755148492);
    b = temp >= 66 ? 255 : temp <= 19 ? 0 : 138.5177312231 * log(temp - 10) - 305.0447927307;
    return {float(std::clamp(r / 255., 0., 1.)),
            float(std::clamp(g / 255., 0., 1.)),
            float(std::clamp(b / 255., 0., 1.))};
}
} // namespace astro
