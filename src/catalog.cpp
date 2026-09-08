#include "astro/catalog.hpp"
#include "json.hpp"
#include <bit>
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
