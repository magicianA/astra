#include "astro/horizon.hpp"
#include "json.hpp"
#include <fstream>
#include <sstream>
#include <stdexcept>

namespace astro {
double Horizon::altitude(double azimuth) const {
    const double index = wrap(azimuth) * (sample_count / (2 * pi));
    const auto i = size_t(index) % sample_count;
    const double fraction = index - floor(index);
    return ((1 - fraction) * degrees[i] + fraction * degrees[(i + 1) % sample_count]) * rad;
}

bool Horizon::visible(Vec3 direction, double radius) const {
    return asin(std::clamp(direction.z, -1., 1.)) + radius >=
           altitude(atan2(direction.x, direction.y));
}

Horizon make_horizon(std::vector<std::array<double, 2>> points, std::string name) {
    if (points.size() < 2 || points.size() > 100000) {
        throw std::invalid_argument("A horizon needs 2 to 100000 azimuth/altitude samples");
    }
    for (auto& point : points) {
        if (!std::isfinite(point[0]) || !std::isfinite(point[1]) || point[0] < 0 ||
            point[0] > 360 || point[1] < -20 || point[1] > 89) {
            throw std::invalid_argument(
                "Horizon azimuth must be 0..360 and altitude -20..89 degrees");
        }
    }
    std::sort(points.begin(), points.end());
    if (points.front()[0] == 0 && points.back()[0] == 360) {
        if (abs(points.front()[1] - points.back()[1]) > 1e-5) {
            throw std::invalid_argument("Horizon seam samples must agree");
        }
        points.pop_back();
    }
    for (size_t i = 1; i < points.size(); ++i) {
        if (points[i][0] == points[i - 1][0]) {
            throw std::invalid_argument("Duplicate horizon azimuth");
        }
    }
    Horizon out;
    out.name = std::move(name);
    points.insert(points.begin(), {points.back()[0] - 360, points.back()[1]});
    points.push_back({points[1][0] + 360, points[1][1]});
    size_t next = 1;
    for (size_t i = 0; i < Horizon::sample_count; ++i) {
        const double az = double(i) * 360 / Horizon::sample_count;
        while (points[next][0] < az) {
            ++next;
        }
        const auto a = points[next - 1], b = points[next];
        out.degrees[i] = float(a[1] + (b[1] - a[1]) * (az - a[0]) / (b[0] - a[0]));
    }
    return out;
}

Horizon load_horizon(const std::filesystem::path& path) {
    std::ifstream input(path);
    if (!input) {
        throw std::runtime_error("Cannot open horizon file");
    }
    std::vector<std::array<double, 2>> points;
    std::string name = path.stem().string();
    if (path.extension() == ".json") {
        const auto j = nlohmann::json::parse(input);
        name = j.value("name", name);
        points = j.at("points").get<decltype(points)>();
    } else {
        std::string line;
        while (std::getline(input, line)) {
            if (line.empty() || line.front() == '#' || line.starts_with("azimuth")) {
                continue;
            }
            std::replace(line.begin(), line.end(), ',', ' ');
            std::istringstream row(line);
            std::array<double, 2> point{};
            std::string extra;
            if (!(row >> point[0] >> point[1]) || (row >> extra)) {
                throw std::invalid_argument("Expected azimuth,altitude in horizon CSV");
            }
            points.push_back(point);
        }
    }
    return make_horizon(std::move(points), std::move(name));
}
} // namespace astro
