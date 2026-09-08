#include "astro/sky.hpp"
#include "json.hpp"
#include <algorithm>
#include <fstream>
#include <iomanip>
#include <iostream>
using namespace astro;

int main(int argc, char** argv) {
    try {
        std::filesystem::path data = ASTRA_SOURCE_DATA;
        Scenario s;
        s.scale = TimeScale::UT1;
        std::string output;
        bool geometric = false;
        for (int i = 1; i < argc; i++) {
            std::string a = argv[i];
            auto value = [&]() {
                if (i + 1 >= argc) {
                    throw std::invalid_argument("Missing value for " + a);
                }
                return std::string(argv[++i]);
            };
            if (a == "--generate-cio") {
                auto path = value();
                double step = argc > i + 1 ? std::stod(argv[i + 1]) : 4.;
                Orientation::generate(path, step);
                std::cout << "CIO table written: " << path << " step=" << step << " days\n";
                return 0;
            }
            if (a == "--compare-cio") {
                auto first = value();
                auto second = value();
                Orientation x(first), y(second);
                double worst = 0;
                for (int k = -5000; k <= 5000; k++) {
                    double days = k * 365.25 + .3;
                    worst = std::max(worst, std::abs(x.cio(days) - y.cio(days)) / arcsec);
                }
                std::cout << std::setprecision(12) << "Maximum CIO difference: " << worst
                          << " arcsec\n";
                return worst < .05 ? 0 : 2;
            }
            if (a == "--data") {
                data = value();
            } else if (a == "--date") {
                auto str = value();
                if (sscanf(str.c_str(),
                           "%d-%d-%dT%d:%d:%lf",
                           &s.date.year,
                           &s.date.month,
                           &s.date.day,
                           &s.date.hour,
                           &s.date.minute,
                           &s.date.second) < 3) {
                    throw std::invalid_argument("Date format: -3000-01-01T22:00:00");
                }
            } else if (a == "--site") {
                auto str = value();
                if (sscanf(str.c_str(), "%lf,%lf,%lf", &s.longitude, &s.latitude, &s.height) < 2) {
                    throw std::invalid_argument("Site: longitude,latitude,height");
                }
            } else if (a == "--scale") {
                auto v = value();
                if (v == "TT") {
                    s.scale = TimeScale::TT;
                } else if (v == "TDB") {
                    s.scale = TimeScale::TDB;
                } else if (v == "UTC") {
                    s.scale = TimeScale::UTC;
                } else if (v == "LMT") {
                    s.scale = TimeScale::LocalMean;
                } else if (v == "UT1") {
                    s.scale = TimeScale::UT1;
                } else {
                    throw std::invalid_argument("Unknown time scale");
                }
            } else if (a == "--magnitude") {
                s.magnitude = std::stod(value());
            } else if (a == "--delta-t") {
                s.override_delta_t = true;
                s.custom_delta_t = std::stod(value());
            } else if (a == "--no-atmosphere") {
                s.atmosphere = false;
            } else if (a == "--output") {
                output = value();
            } else if (a == "--geometric") {
                geometric = true;
            } else if (a == "--scenario") {
                s = load_scenario(value());
            } else if (a == "--help") {
                std::cout
                    << "astra_cli --data DIR --date YEAR-MM-DDTHH:MM:SS --site LON,LAT,HEIGHT "
                       "--scale UT1|TT|TDB|UTC|LMT --no-atmosphere --output result.json\n";
                return 0;
            } else {
                throw std::invalid_argument("Unknown argument: " + a);
            }
        }
        SkyEngine engine(data);
        auto sky = engine.compute(s);
        nlohmann::json j = {{"data_id", sky->data_id},
                            {"date", format_date(s.date)},
                            {"input_scale", scale_name(s.scale)},
                            {"jd_ut1", sky->time.ut1.value()},
                            {"jd_tt", sky->time.tt.value()},
                            {"jd_tdb", sky->time.tdb.value()},
                            {"delta_t_seconds", sky->time.delta_t_seconds},
                            {"quality", sky->time.note},
                            {"catalog_count", sky->catalog_count},
                            {"computed_stars", sky->stars.size()},
                            {"compute_ms", sky->compute_ms},
                            {"bodies", nlohmann::json::array()},
                            {"bright_stars", nlohmann::json::array()}};
        auto item = [](const Object& o) {
            return nlohmann::json{{"id", std::to_string(o.id)},
                                  {"hip", o.hip},
                                  {"body", o.body},
                                  {"azimuth_deg", o.azimuth() / rad},
                                  {"altitude_deg", o.altitude() / rad},
                                  {"geometric_altitude_deg", asin(o.geometric.z) / rad},
                                  {"ra_icrs_axes_deg", wrap(atan2(o.icrs.y, o.icrs.x)) / rad},
                                  {"dec_icrs_axes_deg", asin(o.icrs.z) / rad},
                                  {"magnitude", o.magnitude},
                                  {"distance_au", o.distance_au},
                                  {"phase", o.phase},
                                  {"quality", quality_text(o)}};
        };
        std::unique_ptr<Ephemeris> geometric_ephemeris;
        if (geometric) {
            geometric_ephemeris = std::make_unique<Ephemeris>(data);
        }
        for (auto& o : sky->bodies) {
            auto v = item(o);
            v["name"] = body_name(o.body);
            if (geometric) {
                auto st = geometric_ephemeris->state(o.body, sky->time.tdb);
                v["barycentric_km"] = {st.position.x, st.position.y, st.position.z};
            }
            j["bodies"].push_back(v);
        }
        std::sort(sky->stars.begin(), sky->stars.end(), [](auto& a, auto& b) {
            return a.magnitude < b.magnitude;
        });
        for (size_t k = 0; k < std::min(size_t(25), sky->stars.size()); k++) {
            auto v = item(sky->stars[k]);
            v["name"] = engine.catalog.name(engine.catalog.stars[sky->stars[k].catalog_index]);
            j["bright_stars"].push_back(v);
        }
        if (output.empty()) {
            std::cout << j.dump(2) << '\n';
        } else {
            std::ofstream out(output);
            out << j.dump(2) << '\n';
            if (!out) {
                throw std::runtime_error("Cannot write output");
            }
        }
        return 0;
    } catch (const std::exception& e) {
        std::cerr << "Astra: " << e.what() << '\n';
        return 1;
    }
}
